//! HTTP and WS surface tests. Both run the real server against the offline
//! MapFetcher on a current-thread runtime + LocalSet (the server is `!Send`).

mod common;

use std::rc::Rc;

use common::{MapFetcher, DDG_SERP};
use futures_util::{SinkExt, StreamExt};
use obscura_octo::engine::build_serp_url;
use obscura_octo::schema::Engine;
use obscura_octo::Fetcher;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

fn ddg_fetcher(query: &str) -> Rc<dyn Fetcher> {
    let url = build_serp_url(Engine::Duckduckgo, query, "en", &[], None, 0);
    Rc::new(MapFetcher::default().with(&url, DDG_SERP)) as Rc<dyn Fetcher>
}

/// Minimal raw HTTP POST; returns (status_line, body).
async fn http_post(addr: std::net::SocketAddr, path: &str, body: &str) -> (String, String) {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    stream.flush().await.unwrap();
    let mut raw = String::new();
    stream.read_to_string(&mut raw).await.unwrap();
    let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((raw.as_str(), ""));
    let status = head.lines().next().unwrap_or("").to_string();
    (status, body.to_string())
}

#[test]
fn http_search_returns_json() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async {
        let server = obscura_octo::server::bind("127.0.0.1", 0, 0, ddg_fetcher("rust"), None)
            .await
            .unwrap();
        let http_addr = server.http_addr();
        tokio::task::spawn_local(async move {
            let _ = server.serve().await;
        });

        // Health probe.
        let (status, body) = http_post(http_addr, "/health", "").await;
        // /health is GET-only; POST to it returns 404, so hit search directly instead.
        let _ = (status, body);

        let (status, body) =
            http_post(http_addr, "/search", r#"{"query":"rust","max_results":10}"#).await;
        assert!(status.contains("200"), "status was: {status}");
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["results"].as_array().unwrap().len(), 2);
        assert_eq!(json["results"][0]["url"], "https://example.com/a");
    });
}

#[test]
fn http_search_ndjson_streams_records() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async {
        let server = obscura_octo::server::bind("127.0.0.1", 0, 0, ddg_fetcher("rust"), None)
            .await
            .unwrap();
        let http_addr = server.http_addr();
        tokio::task::spawn_local(async move {
            let _ = server.serve().await;
        });

        let (status, body) =
            http_post(http_addr, "/search", r#"{"query":"rust","format":"ndjson"}"#).await;
        assert!(status.contains("200"), "status: {status}");
        let lines: Vec<&str> = body.lines().filter(|l| !l.trim().is_empty()).collect();
        // 2 result records + 1 summary line.
        assert_eq!(lines.len(), 3);
        // The summary line carries aggregate metadata only — a `count`, not a
        // second copy of the streamed results.
        let last: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
        assert!(last.get("results").is_none());
        assert_eq!(last["count"].as_u64().unwrap(), 2);
    });
}

#[test]
fn http_rejects_when_token_required() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async {
        let server = obscura_octo::server::bind(
            "127.0.0.1",
            0,
            0,
            ddg_fetcher("rust"),
            Some("s3cr3t".to_string()),
        )
        .await
        .unwrap();
        let http_addr = server.http_addr();
        tokio::task::spawn_local(async move {
            let _ = server.serve().await;
        });

        let (status, _) = http_post(http_addr, "/search", r#"{"query":"rust"}"#).await;
        assert!(status.contains("401"), "status: {status}");
    });
}

#[test]
fn ws_search_streams_result_frames() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async {
        let server = obscura_octo::server::bind("127.0.0.1", 0, 0, ddg_fetcher("rust"), None)
            .await
            .unwrap();
        let ws_addr = server.ws_addr();
        tokio::task::spawn_local(async move {
            let _ = server.serve().await;
        });

        let url = format!("ws://{}/search", ws_addr);
        let (mut ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
        ws.send(tokio_tungstenite::tungstenite::Message::Text(
            r#"{"query":"rust"}"#.into(),
        ))
        .await
        .unwrap();

        let mut results = 0;
        let mut got_summary = false;
        while let Some(Ok(msg)) = ws.next().await {
            match msg {
                tokio_tungstenite::tungstenite::Message::Text(t) => {
                    let v: serde_json::Value = serde_json::from_str(&t).unwrap();
                    match v["type"].as_str() {
                        Some("result") => results += 1,
                        Some("summary") => got_summary = true,
                        _ => {}
                    }
                }
                tokio_tungstenite::tungstenite::Message::Close(_) => break,
                _ => {}
            }
        }
        assert_eq!(results, 2);
        assert!(got_summary);
    });
}
