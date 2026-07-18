//! HTTP + WS surfaces for the octo features, on two ports of one server. Both
//! delegate to the same `run_search` core via an injected `Fetcher`, so tests
//! drive the real surfaces against an offline fetcher. Connections are handled
//! sequentially on the current thread: the Page fetcher owns a single-threaded
//! V8 isolate and is `!Send`, so nothing crosses threads.
//!
//! - HTTP  `POST /search`  body = SearchRequest JSON. Returns the full
//!   SearchResponse as JSON, or NDJSON (one record per line + summary) when the
//!   request asks for `format: "ndjson"` or `Accept: application/x-ndjson`.
//! - HTTP  `GET /health`   liveness probe.
//! - WS    send a SearchRequest JSON text message; receive one text frame per
//!   result (`{"type":"result",...}`) then a `{"type":"summary",...}` frame.
//!
//! Security: bind is loopback by default. A non-loopback bind requires a token
//! (checked in `run`). When a token is set, HTTP needs `Authorization: Bearer
//! <token>` and WS needs `?token=<token>`.

use std::net::SocketAddr;
use std::rc::Rc;

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use crate::fetcher::Fetcher;
use crate::output::CollectSink;
use crate::schema::SearchRequest;
use crate::search::run_search;

/// Reject a Content-Length larger than this before allocating (unauthenticated
/// OOM guard), mirroring the MCP HTTP server.
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

pub struct Server {
    http: TcpListener,
    ws: TcpListener,
    fetcher: Rc<dyn Fetcher>,
    token: Option<String>,
}

impl Server {
    pub fn http_addr(&self) -> SocketAddr {
        self.http.local_addr().expect("http listener addr")
    }

    pub fn ws_addr(&self) -> SocketAddr {
        self.ws.local_addr().expect("ws listener addr")
    }

    /// Serve forever, handling one connection at a time across both ports.
    pub async fn serve(self) -> Result<()> {
        loop {
            tokio::select! {
                accepted = self.http.accept() => {
                    if let Ok((stream, _)) = accepted {
                        if let Err(e) = handle_http(stream, self.fetcher.as_ref(), self.token.as_deref()).await {
                            tracing::debug!("octo http connection closed: {e}");
                        }
                    }
                }
                accepted = self.ws.accept() => {
                    if let Ok((stream, _)) = accepted {
                        if let Err(e) = handle_ws(stream, self.fetcher.as_ref(), self.token.as_deref()).await {
                            tracing::debug!("octo ws connection closed: {e}");
                        }
                    }
                }
            }
        }
    }
}

/// Bind both listeners. Ports may be 0 to let the OS assign (tests read the
/// resolved addr back).
pub async fn bind(
    host: &str,
    http_port: u16,
    ws_port: u16,
    fetcher: Rc<dyn Fetcher>,
    token: Option<String>,
) -> Result<Server> {
    let http = TcpListener::bind((host, http_port)).await?;
    let ws = TcpListener::bind((host, ws_port)).await?;
    Ok(Server { http, ws, fetcher, token })
}

fn is_loopback_host(host: &str) -> bool {
    host == "127.0.0.1" || host == "::1" || host.eq_ignore_ascii_case("localhost")
}

/// CLI entry point: enforce the token-for-public-bind rule, then serve.
pub async fn run(
    host: &str,
    http_port: u16,
    ws_port: u16,
    fetcher: Rc<dyn Fetcher>,
    token: Option<String>,
) -> Result<()> {
    if !is_loopback_host(host) && token.as_deref().unwrap_or("").is_empty() {
        anyhow::bail!(
            "refusing to bind octo server on non-loopback host {host} without --token"
        );
    }
    let server = bind(host, http_port, ws_port, fetcher, token).await?;
    tracing::info!(
        "octo server: http://{} (POST /search), ws://{}",
        server.http_addr(),
        server.ws_addr()
    );
    server.serve().await
}

async fn handle_http(stream: TcpStream, fetcher: &dyn Fetcher, token: Option<&str>) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).await? == 0 {
        return Ok(());
    }
    let parts: Vec<&str> = request_line.trim().splitn(3, ' ').collect();
    if parts.len() < 3 {
        respond(&mut writer, 400, "application/json", b"{\"error\":\"bad request\"}").await?;
        return Ok(());
    }
    let method = parts[0].to_string();
    let path = parts[1].to_string();

    let mut content_length: usize = 0;
    let mut authorized = token.is_none();
    let mut want_ndjson = false;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).await? == 0 {
            break;
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            break;
        }
        let lower = trimmed.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            content_length = v.trim().parse().unwrap_or(0);
        }
        if lower.contains("application/x-ndjson") {
            want_ndjson = true;
        }
        if let Some(tok) = token {
            if let Some(v) = lower.strip_prefix("authorization:") {
                let v = v.trim();
                if v == format!("bearer {}", tok.to_ascii_lowercase())
                    || trimmed[trimmed.find(':').map(|i| i + 1).unwrap_or(0)..]
                        .trim()
                        .eq_ignore_ascii_case(&format!("Bearer {tok}"))
                {
                    authorized = true;
                }
            }
        }
    }

    if method == "GET" && path == "/health" {
        respond(&mut writer, 200, "application/json", b"{\"ok\":true}").await?;
        return Ok(());
    }

    if !authorized {
        respond(&mut writer, 401, "application/json", b"{\"error\":\"unauthorized\"}").await?;
        return Ok(());
    }

    if method != "POST" || path.split('?').next() != Some("/search") {
        respond(&mut writer, 404, "application/json", b"{\"error\":\"not found\"}").await?;
        return Ok(());
    }

    if content_length > MAX_BODY_BYTES {
        respond(&mut writer, 413, "application/json", b"{\"error\":\"body too large\"}").await?;
        return Ok(());
    }

    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body).await?;
    }

    let req: SearchRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("{{\"error\":\"invalid json: {}\"}}", e);
            respond(&mut writer, 400, "application/json", msg.as_bytes()).await?;
            return Ok(());
        }
    };

    let mut sink = CollectSink::default();
    let resp = run_search(&req, fetcher, &mut sink).await;

    if want_ndjson || matches!(req.format, Some(crate::schema::OutputFormat::Ndjson)) {
        let mut out = String::new();
        for rec in &sink.records {
            out.push_str(&serde_json::to_string(rec).unwrap_or_default());
            out.push('\n');
        }
        if let Some(summary) = &sink.summary {
            out.push_str(&serde_json::to_string(summary).unwrap_or_default());
            out.push('\n');
        }
        respond(&mut writer, 200, "application/x-ndjson", out.as_bytes()).await?;
    } else {
        let body = serde_json::to_vec(&resp).unwrap_or_default();
        respond(&mut writer, 200, "application/json", &body).await?;
    }
    Ok(())
}

async fn respond(
    writer: &mut (impl AsyncWriteExt + Unpin),
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        413 => "Payload Too Large",
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

async fn handle_ws(stream: TcpStream, fetcher: &dyn Fetcher, token: Option<&str>) -> Result<()> {
    use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
    use tokio_tungstenite::tungstenite::http;
    use tokio_tungstenite::tungstenite::Message;

    let want = token.map(|t| t.to_string());
    let ws = tokio_tungstenite::accept_hdr_async(stream, |req: &Request, resp: Response| {
        if let Some(tok) = &want {
            let ok = req
                .uri()
                .query()
                .map(|q| {
                    q.split('&').any(|p| {
                        let mut it = p.splitn(2, '=');
                        it.next() == Some("token") && it.next() == Some(tok.as_str())
                    })
                })
                .unwrap_or(false);
            if !ok {
                let err = http::Response::builder()
                    .status(401)
                    .body(Some("unauthorized".to_string()))
                    .unwrap();
                return Err(err);
            }
        }
        Ok(resp)
    })
    .await?;

    let (mut tx, mut rx) = ws.split();

    // One request per connection: the first text message is the SearchRequest.
    let first = match rx.next().await {
        Some(Ok(Message::Text(t))) => t.to_string(),
        _ => return Ok(()),
    };

    let req: SearchRequest = match serde_json::from_str(&first) {
        Ok(r) => r,
        Err(e) => {
            let _ = tx
                .send(Message::Text(
                    format!("{{\"type\":\"error\",\"error\":\"invalid json: {e}\"}}").into(),
                ))
                .await;
            return Ok(());
        }
    };

    let mut sink = CollectSink::default();
    let _ = run_search(&req, fetcher, &mut sink).await;

    for rec in &sink.records {
        let mut obj = rec.clone();
        if let Some(map) = obj.as_object_mut() {
            map.insert("type".to_string(), serde_json::json!("result"));
        }
        if tx.send(Message::Text(obj.to_string().into())).await.is_err() {
            return Ok(());
        }
    }
    if let Some(summary) = &sink.summary {
        let mut obj = summary.clone();
        if let Some(map) = obj.as_object_mut() {
            map.insert("type".to_string(), serde_json::json!("summary"));
        }
        let _ = tx.send(Message::Text(obj.to_string().into())).await;
    }
    let _ = tx.send(Message::Close(None)).await;
    Ok(())
}
