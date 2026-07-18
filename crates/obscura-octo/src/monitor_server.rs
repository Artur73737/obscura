//! HTTP + WS streaming for `monitor`. Unlike the search server (request ->
//! response), monitor is long-running: the poll loop pushes each change into a
//! `MonitorHub`, and clients read the latest value over HTTP (`GET /last`) or
//! subscribe to a live stream over WS (`/events`). Everything runs on one thread
//! via a LocalSet because the Page fetcher is `!Send`.

use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;

use crate::fetcher::Fetcher;
use crate::monitor::run_monitor;
use crate::output::OutputSink;
use crate::schema::MonitorRequest;

/// Shared state between the poll loop and the HTTP/WS handlers: the last emitted
/// event plus a broadcast channel of NDJSON lines for live subscribers.
#[derive(Clone)]
pub struct MonitorHub {
    last: Rc<RefCell<Option<Value>>>,
    tx: broadcast::Sender<String>,
}

impl MonitorHub {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(256);
        MonitorHub { last: Rc::new(RefCell::new(None)), tx }
    }
    fn last_json(&self) -> String {
        self.last
            .borrow()
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "null".to_string())
    }
}

impl Default for MonitorHub {
    fn default() -> Self {
        Self::new()
    }
}

/// OutputSink that feeds the hub: store the latest event and broadcast it.
pub struct HubSink {
    hub: MonitorHub,
}

impl OutputSink for HubSink {
    fn emit(&mut self, record: &Value) {
        *self.hub.last.borrow_mut() = Some(record.clone());
        // A send error just means no subscribers right now; the value is still
        // stored for GET /last.
        let _ = self.hub.tx.send(record.to_string());
    }
}

fn is_loopback_host(host: &str) -> bool {
    host == "127.0.0.1" || host == "::1" || host.eq_ignore_ascii_case("localhost")
}

/// Run the monitor loop and the HTTP/WS servers together. Blocks until the loop
/// finishes (max_runs) or a fatal accept error; with the default max_runs=0 it
/// serves indefinitely.
pub async fn run(
    req: MonitorRequest,
    fetcher: Rc<dyn Fetcher>,
    host: String,
    http_port: u16,
    ws_port: u16,
    token: Option<String>,
) -> Result<()> {
    if !is_loopback_host(&host) && token.as_deref().unwrap_or("").is_empty() {
        anyhow::bail!("refusing to bind monitor server on non-loopback host {host} without --token");
    }
    let http = TcpListener::bind((host.as_str(), http_port)).await?;
    let ws = TcpListener::bind((host.as_str(), ws_port)).await?;
    tracing::info!(
        "monitor server: http://{} (GET /last, /health), ws://{} (/events)",
        http.local_addr()?,
        ws.local_addr()?
    );

    let hub = MonitorHub::new();
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async move {
            // poll loop
            {
                let hub = hub.clone();
                let fetcher = fetcher.clone();
                let req = req.clone();
                tokio::task::spawn_local(async move {
                    let mut sink = HubSink { hub };
                    run_monitor(&req, fetcher.as_ref(), &mut sink, None).await;
                });
            }
            // HTTP accept loop
            {
                let hub = hub.clone();
                let token = token.clone();
                tokio::task::spawn_local(async move {
                    loop {
                        if let Ok((stream, _)) = http.accept().await {
                            let hub = hub.clone();
                            let token = token.clone();
                            tokio::task::spawn_local(async move {
                                let _ = handle_http(stream, &hub, token.as_deref()).await;
                            });
                        }
                    }
                });
            }
            // WS accept loop
            {
                let hub = hub.clone();
                let token = token.clone();
                tokio::task::spawn_local(async move {
                    loop {
                        if let Ok((stream, _)) = ws.accept().await {
                            let hub = hub.clone();
                            let token = token.clone();
                            tokio::task::spawn_local(async move {
                                let _ = handle_ws(stream, hub, token).await;
                            });
                        }
                    }
                });
            }
            std::future::pending::<()>().await
        })
        .await;
    Ok(())
}

async fn handle_http(stream: TcpStream, hub: &MonitorHub, token: Option<&str>) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).await? == 0 {
        return Ok(());
    }
    let parts: Vec<&str> = request_line.trim().splitn(3, ' ').collect();
    let (method, path) = (parts.first().copied().unwrap_or(""), parts.get(1).copied().unwrap_or("/"));

    let mut authorized = token.is_none();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).await? == 0 {
            break;
        }
        let t = line.trim_end();
        if t.is_empty() {
            break;
        }
        if let Some(tok) = token {
            if t.to_ascii_lowercase().starts_with("authorization:")
                && t[t.find(':').map(|i| i + 1).unwrap_or(0)..].trim().eq_ignore_ascii_case(&format!("Bearer {tok}"))
            {
                authorized = true;
            }
        }
    }

    let path = path.split('?').next().unwrap_or("/");
    if method == "GET" && path == "/health" {
        return respond(&mut writer, 200, "application/json", b"{\"ok\":true}").await;
    }
    if !authorized {
        return respond(&mut writer, 401, "application/json", b"{\"error\":\"unauthorized\"}").await;
    }
    if method == "GET" && (path == "/last" || path == "/") {
        let body = hub.last_json();
        return respond(&mut writer, 200, "application/json", body.as_bytes()).await;
    }
    respond(&mut writer, 404, "application/json", b"{\"error\":\"not found\"}").await
}

async fn respond(
    writer: &mut (impl AsyncWriteExt + Unpin),
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    let reason = match status {
        200 => "OK",
        401 => "Unauthorized",
        404 => "Not Found",
        _ => "OK",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    writer.write_all(head.as_bytes()).await?;
    writer.write_all(body).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hub_stores_last_and_broadcasts() {
        let hub = MonitorHub::new();
        let mut rx = hub.tx.subscribe();
        let mut sink = HubSink { hub: hub.clone() };
        sink.emit(&serde_json::json!({ "value": "x", "run": 1 }));
        // GET /last would return this.
        assert!(hub.last_json().contains("\"value\":\"x\""));
        // A subscriber (WS /events) receives it live.
        let got = rx.recv().await.unwrap();
        assert!(got.contains("\"value\":\"x\""));
    }
}

async fn handle_ws(stream: TcpStream, hub: MonitorHub, token: Option<String>) -> Result<()> {
    use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
    use tokio_tungstenite::tungstenite::http;
    use tokio_tungstenite::tungstenite::Message;

    let want = token;
    let ws = tokio_tungstenite::accept_hdr_async(stream, |req: &Request, resp: Response| {
        if let Some(tok) = &want {
            let ok = req
                .uri()
                .query()
                .map(|q| q.split('&').any(|p| {
                    let mut it = p.splitn(2, '=');
                    it.next() == Some("token") && it.next() == Some(tok.as_str())
                }))
                .unwrap_or(false);
            if !ok {
                return Err(http::Response::builder().status(401).body(Some("unauthorized".to_string())).unwrap());
            }
        }
        Ok(resp)
    })
    .await?;

    let (mut tx, mut rx_ws) = ws.split();
    let mut sub = hub.tx.subscribe();

    // Send the current value immediately so a new subscriber has state.
    {
        let cur = hub.last_json();
        if cur != "null" {
            let _ = tx.send(Message::Text(cur.into())).await;
        }
    }

    loop {
        tokio::select! {
            msg = sub.recv() => match msg {
                Ok(line) => { if tx.send(Message::Text(line.into())).await.is_err() { break; } }
                Err(broadcast::error::RecvError::Lagged(_)) => { /* slow client: skip */ }
                Err(broadcast::error::RecvError::Closed) => break,
            },
            // Detect client close / ignore incoming.
            incoming = rx_ws.next() => match incoming {
                None | Some(Ok(Message::Close(_))) => break,
                Some(Err(_)) => break,
                _ => {}
            },
        }
    }
    let _ = tx.send(Message::Close(None)).await;
    Ok(())
}
