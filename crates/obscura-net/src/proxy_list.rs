use std::collections::HashSet;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

const PROXY_SOURCES: &[&str] = &[
    "https://raw.githubusercontent.com/TheSpeedX/SOCKS-List/master/socks5.txt",
    "https://raw.githubusercontent.com/Thordata/awesome-free-proxy-list/main/proxies/socks5.txt",
    "https://cdn.jsdelivr.net/gh/proxifly/free-proxy-list@main/proxies/protocols/socks5/data.txt",
    "https://raw.githubusercontent.com/jetkai/proxy-list/main/online-proxies/txt/proxies-socks5.txt",
];

const TEST_IP: [u8; 4] = [1, 1, 1, 1];
const TEST_PORT: u16 = 80;
const PROXY_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const BATCH_SIZE: usize = 200;
const MAX_WORKING: usize = 15;
const MAX_CANDIDATES: usize = 1000;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub struct ProxyEntry {
    pub url: String,
    pub latency_ms: u64,
}

pub async fn select_best() -> Option<String> {
    let all = fetch_and_test_all().await;
    all.into_iter().next().map(|e| e.url)
}

pub async fn fetch_and_test_all() -> Vec<ProxyEntry> {
    let raw = download_lists().await;
    if raw.is_empty() {
        warn!("No proxies downloaded from any source");
        return Vec::new();
    }

    let candidates = parse_proxy_lines(&raw);
    let candidate_count = candidates.len();
    info!("Testing {} unique SOCKS5 proxies...", candidate_count);

    let results = test_proxies(candidates).await;
    if results.is_empty() {
        warn!("No working SOCKS5 proxy found among {} candidates", candidate_count);
    } else {
        info!(
            "Found {} working proxies; fastest: {} ({}ms)",
            results.len(),
            results[0].url,
            results[0].latency_ms,
        );
    }

    results
}

async fn download_lists() -> String {
    let client = reqwest::Client::builder()
        .timeout(DOWNLOAD_TIMEOUT)
        .build()
        .expect("Failed to build HTTP client for proxy list download");

    let mut all = String::new();
    for &source in PROXY_SOURCES {
        match client.get(source).send().await {
            Ok(resp) => match resp.text().await {
                Ok(text) if !text.trim().is_empty() => {
                    debug!("Downloaded {} bytes from {}", text.len(), source);
                    all.push_str(&text);
                    all.push('\n');
                }
                _ => debug!("Empty response from {}", source),
            },
            Err(e) => {
                debug!("Failed to fetch proxy list from {}: {}", source, e);
            }
        }
    }
    all
}

fn parse_proxy_lines(text: &str) -> HashSet<String> {
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("//"))
        .filter_map(|l| {
            let s = l.strip_prefix("socks5://").unwrap_or(l);
            let addr = if let Some(at_idx) = s.rfind('@') {
                &s[at_idx + 1..]
            } else {
                s
            };
            let (host, _port) = addr.split_once(':')?;
            if host.is_empty() || !_port.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            Some(addr.to_string())
        })
        .collect()
}

async fn test_proxies(candidates: HashSet<String>) -> Vec<ProxyEntry> {
    let all: Vec<String> = candidates.into_iter().take(MAX_CANDIDATES).collect();
    let sem = std::sync::Arc::new(Semaphore::new(BATCH_SIZE));
    let mut results: Vec<ProxyEntry> = Vec::new();

    for chunk in all.chunks(BATCH_SIZE) {
        let mut batch = Vec::new();
        for addr in chunk {
            let sem = sem.clone();
            let addr = addr.clone();
            batch.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                test_single_proxy(&addr)
                    .await
                    .map(|latency| ProxyEntry {
                        url: format!("socks5://{}", addr),
                        latency_ms: latency,
                    })
            }));
        }
        for task in batch {
            if let Ok(Some(entry)) = task.await {
                results.push(entry);
                if results.len() >= MAX_WORKING {
                    results.sort_by_key(|e| e.latency_ms);
                    return results;
                }
            }
        }
    }

    results.sort_by_key(|e| e.latency_ms);
    results
}

async fn test_single_proxy(addr: &str) -> Option<u64> {
    let (host_str, port_str) = addr.split_once(':')?;
    let port: u16 = port_str.parse().ok()?;

    let start = Instant::now();

    // Wrap the ENTIRE SOCKS5 handshake in a timeout, not just connect
    let result = tokio::time::timeout(PROXY_CONNECT_TIMEOUT, async move {
        let stream = TcpStream::connect((host_str, port)).await.ok()?;
        let mut stream = stream;
        let _ = stream.set_nodelay(true);

        let mut buf = [0u8; 2];
        stream.write_all(b"\x05\x01\x00").await.ok()?;
        stream.read_exact(&mut buf).await.ok()?;
        if buf[0] != 0x05 || buf[1] != 0x00 {
            return None;
        }

        let mut req = Vec::with_capacity(10);
        req.extend_from_slice(&[0x05, 0x01, 0x00, 0x01]);
        req.extend_from_slice(&TEST_IP);
        req.extend_from_slice(&TEST_PORT.to_be_bytes());
        stream.write_all(&req).await.ok()?;

        let mut header = [0u8; 4];
        stream.read_exact(&mut header).await.ok()?;
        if header[0] != 0x05 || header[1] != 0x00 {
            return None;
        }

        let addr_len = match header[3] {
            0x01 => 4 + 2,
            0x04 => 16 + 2,
            0x03 => {
                let mut dlen = [0u8; 1];
                stream.read_exact(&mut dlen).await.ok()?;
                dlen[0] as usize + 2
            }
            _ => return None,
        };
        let mut rest = vec![0u8; addr_len];
        stream.read_exact(&mut rest).await.ok()?;

        Some(())
    })
    .await;

    result.ok()??;
    let elapsed = start.elapsed().as_millis() as u64;
    Some(elapsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_ip_port() {
        let input = "192.168.1.1:1080\n10.0.0.1:3128\n";
        let result = parse_proxy_lines(input);
        assert!(result.contains("192.168.1.1:1080"));
        assert!(result.contains("10.0.0.1:3128"));
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn parses_socks5_prefix() {
        let input = "socks5://1.2.3.4:1080\nsocks5://5.6.7.8:1080\n";
        let result = parse_proxy_lines(input);
        assert!(result.contains("1.2.3.4:1080"));
        assert!(result.contains("5.6.7.8:1080"));
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn parses_auth_format() {
        let input = "socks5://user:pass@1.2.3.4:1080\n";
        let result = parse_proxy_lines(input);
        assert!(result.contains("1.2.3.4:1080"));
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn skips_empty_and_comments() {
        let input = "1.2.3.4:1080\n\n# comment\n// also comment\n   \n5.6.7.8:1080\n";
        let result = parse_proxy_lines(input);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn deduplicates() {
        let input = "1.2.3.4:1080\n1.2.3.4:1080\n1.2.3.4:1080\n";
        let result = parse_proxy_lines(input);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn skips_invalid_lines() {
        let input = "not-a-proxy\n:1080\n192.168.1.1\nsocks5://\n";
        let result = parse_proxy_lines(input);
        assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    #[ignore]
    async fn integration_test_download_and_test() {
        let raw = download_lists().await;
        assert!(!raw.is_empty(), "Should download at least some proxy entries");
        let candidates = parse_proxy_lines(&raw);
        assert!(candidates.len() > 10, "Should parse at least 10 unique proxies, got {}", candidates.len());
        eprintln!("Downloaded and parsed {} unique proxies from {} sources", candidates.len(), PROXY_SOURCES.len());
        let results = test_proxies(candidates).await;
        eprintln!("Found {} working proxies", results.len());
        if !results.is_empty() {
            eprintln!("Fastest: {} ({}ms)", results[0].url, results[0].latency_ms);
        }
    }
}
