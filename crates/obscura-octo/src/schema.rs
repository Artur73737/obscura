//! Shared request/response types for the octo features. One `Request` per
//! feature is deserialized identically from an HTTP body, an MCP `arguments`
//! object, and a WS message; the CLI builds the same struct from its flags.
//! Every knob is optional so a caller can send `{"query":"x"}` and get a
//! sensible search; the defaults are applied once in `config::resolve`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Google,
    Bing,
    Duckduckgo,
    /// User supplied SERP URL template (`engine_url`, `{query}`/`{lang}`
    /// placeholders). Also the escape hatch for self-hosted meta engines.
    Custom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Depth {
    /// Only the SERP results (rank, title, url, snippet).
    Serp,
    /// SERP + navigate every result and scrape it.
    Page,
    /// Page + follow same-host internal links one level, within the budget.
    Deep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScrapeKind {
    None,
    Text,
    Html,
    Links,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Json,
    Ndjson,
    Text,
}

/// A search request. All fields except `query` are optional and resolved
/// against the defaults in `config::resolve`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub engine: Option<Engine>,
    pub max_results: Option<usize>,
    pub lang: Option<String>,
    #[serde(default)]
    pub site: Vec<String>,
    #[serde(default)]
    pub exclude_site: Vec<String>,
    pub site_exact: Option<bool>,
    pub depth: Option<Depth>,
    pub scrape: Option<ScrapeKind>,
    pub format: Option<OutputFormat>,
    /// JS expression evaluated on every scraped page (depth page/deep).
    pub eval: Option<String>,
    pub wait: Option<u64>,
    pub timeout: Option<u64>,
    pub concurrency: Option<usize>,
    /// Engine to retry with when the primary returns zero results.
    pub fallback: Option<Engine>,
    /// SERP URL template for `Engine::Custom`.
    pub engine_url: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScrapeData {
    pub kind: ScrapeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchResult {
    pub rank: usize,
    pub title: String,
    pub url: String,
    pub snippet: String,
    /// Present only for depth page/deep.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scrape: Option<ScrapeData>,
    /// Result of `eval` on the page, if requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval: Option<serde_json::Value>,
    /// True for results discovered by following internal links (depth deep).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub followed: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchResponse {
    pub query: String,
    pub engine: Engine,
    pub lang: String,
    pub sites: Vec<String>,
    pub results: Vec<SearchResult>,
    pub took_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ---- monitor ----

/// A continuous page watch. All fields except `url` are optional; defaults are
/// applied in `monitor::resolve`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct MonitorRequest {
    pub url: String,
    /// CSS selector to watch. When absent, the whole document is watched.
    pub selector: Option<String>,
    /// JS predicate; a truthy result marks a candidate change. Evaluated with
    /// the selected element in scope (bare `textContent` works). Default: true.
    pub condition: Option<String>,
    /// JS expression producing the value to capture and compare. Default: the
    /// element's trimmed text.
    pub on_change: Option<String>,
    /// Seconds between polls.
    pub interval: Option<u64>,
    /// Per-cycle navigation timeout (seconds).
    pub timeout: Option<u64>,
    /// Post-load settle (seconds).
    pub wait: Option<u64>,
    /// Stop after N polls. 0 (default) = run forever.
    pub max_runs: Option<u64>,
    /// Do not emit more than one change per this many seconds (debounce).
    pub min_change_interval: Option<u64>,
}

/// One emitted change (or error) from a monitor run.
#[derive(Clone, Debug, Serialize)]
pub struct MonitorEvent {
    /// 1-based poll number that produced this event.
    pub run: u64,
    /// Epoch milliseconds.
    pub ts: u64,
    /// The captured value (from `on_change`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    /// Stable hash of the value; consumers dedupe/ignore repeats.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// Set when the poll failed (navigation or eval error).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
