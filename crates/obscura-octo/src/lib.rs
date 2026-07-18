//! obscura-octo: our feature crate, kept separate from upstream files so
//! merges from upstream stay clean (see octo.md). Ships `search` and `monitor`,
//! usable from CLI, MCP, HTTP, and WS on top of one shared core.

pub mod config;
pub mod engine;
pub mod fetcher;
pub mod monitor;
pub mod monitor_server;
pub mod output;
pub mod schema;
pub mod search;
pub mod security;
pub mod server;

pub use fetcher::{FetchOpts, FetchedPage, Fetcher, PageFetcher};
// Re-export so callers can flip a build to the stealth feature in one place.
pub const STEALTH_BUILD: bool = cfg!(feature = "stealth");
pub use monitor::run_monitor;
pub use output::{CollectSink, NullSink, OutputSink};
pub use schema::{
    Depth, Engine, MonitorEvent, MonitorRequest, OutputFormat, ScrapeData, ScrapeKind,
    SearchRequest, SearchResponse, SearchResult,
};
pub use search::run_search;
