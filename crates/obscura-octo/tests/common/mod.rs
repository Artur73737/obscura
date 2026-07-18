//! Shared offline fetcher for the octo tests. Maps URLs to canned HTML so the
//! search core, and the HTTP/WS surfaces built on it, are exercised end to end
//! without a browser or the network.
#![allow(dead_code)] // each test binary uses a different subset of these helpers

use std::collections::HashMap;

use async_trait::async_trait;
use obscura_octo::{FetchOpts, FetchedPage, Fetcher};

#[derive(Default, Clone)]
pub struct MapFetcher {
    pub pages: HashMap<String, String>,
}

impl MapFetcher {
    pub fn with(mut self, url: &str, html: &str) -> Self {
        self.pages.insert(url.to_string(), html.to_string());
        self
    }
}

#[async_trait(?Send)]
impl Fetcher for MapFetcher {
    async fn fetch(&self, url: &str, _opts: FetchOpts<'_>) -> Result<FetchedPage, String> {
        match self.pages.get(url) {
            Some(html) => Ok(FetchedPage {
                final_url: url.to_string(),
                html: html.clone(),
                title: format!("title:{url}"),
                eval: None,
            }),
            None => Err(format!("no fixture for {url}")),
        }
    }
}

/// A DuckDuckGo-style SERP with two results on distinct hosts.
pub const DDG_SERP: &str = r#"
<html><body>
  <div class="result">
    <a class="result__a" href="https://example.com/a">Result A</a>
    <a class="result__snippet">Snippet about A</a>
  </div>
  <div class="result">
    <a class="result__a" href="https://example.org/b">Result B</a>
    <a class="result__snippet">Snippet about B</a>
  </div>
  <div class="result">
    <a class="result__a" href="https://example.com/a">Result A dup</a>
    <a class="result__snippet">dup</a>
  </div>
</body></html>
"#;

pub const PAGE_A: &str =
    r#"<html><body><h1>Alpha</h1><p>Hello A world</p><a href="/a2">next</a></body></html>"#;
