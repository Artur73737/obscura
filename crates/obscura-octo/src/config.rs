//! Optimal defaults, overridable per surface. Precedence:
//! explicit request field  >  env (OBSCURA_SEARCH_*)  >  default here.
//! One place defines the defaults so no surface diverges.

use crate::schema::{Depth, Engine, OutputFormat, ScrapeKind, SearchRequest};

#[derive(Clone, Debug)]
pub struct ResolvedConfig {
    pub engine: Engine,
    pub max_results: usize,
    pub lang: String,
    pub site: Vec<String>,
    pub exclude_site: Vec<String>,
    pub site_exact: bool,
    pub depth: Depth,
    pub scrape: ScrapeKind,
    pub format: OutputFormat,
    pub wait: u64,
    pub timeout: u64,
    pub concurrency: usize,
    pub fallback: Option<Engine>,
}

fn env_usize(key: &str) -> Option<usize> {
    std::env::var(key).ok().and_then(|v| v.trim().parse().ok())
}

fn env_u64(key: &str) -> Option<u64> {
    std::env::var(key).ok().and_then(|v| v.trim().parse().ok())
}

fn env_engine(key: &str) -> Option<Engine> {
    match std::env::var(key).ok()?.trim().to_ascii_lowercase().as_str() {
        "google" => Some(Engine::Google),
        "bing" => Some(Engine::Bing),
        "duckduckgo" | "ddg" => Some(Engine::Duckduckgo),
        _ => None,
    }
}

/// Merge a request with env overrides and the built-in defaults into a fully
/// concrete config. Defaults are chosen so `{"query":"x"}` alone works well.
pub fn resolve(req: &SearchRequest) -> ResolvedConfig {
    let engine = req
        .engine
        .or_else(|| env_engine("OBSCURA_SEARCH_ENGINE"))
        .unwrap_or(Engine::Duckduckgo);

    let depth = req.depth.unwrap_or(Depth::Serp);

    // Scrape default depends on depth: nothing on a pure SERP, readable text
    // once we are actually navigating result pages.
    let scrape = req.scrape.unwrap_or(match depth {
        Depth::Serp => ScrapeKind::None,
        _ => ScrapeKind::Text,
    });

    // Hard cap so a request can never ask us to visit an unbounded number of
    // pages. 100 is far above any interactive use.
    let max_results = req
        .max_results
        .or_else(|| env_usize("OBSCURA_SEARCH_MAX_RESULTS"))
        .unwrap_or(10)
        .clamp(1, 100);

    ResolvedConfig {
        engine,
        max_results,
        lang: req.lang.clone().unwrap_or_else(|| "en".to_string()),
        site: req.site.clone(),
        exclude_site: req.exclude_site.clone(),
        site_exact: req.site_exact.unwrap_or(false),
        depth,
        scrape,
        format: req.format.unwrap_or(OutputFormat::Json),
        wait: req.wait.or_else(|| env_u64("OBSCURA_SEARCH_WAIT")).unwrap_or(2),
        timeout: req
            .timeout
            .or_else(|| env_u64("OBSCURA_SEARCH_TIMEOUT"))
            .unwrap_or(20)
            .max(1),
        concurrency: req.concurrency.unwrap_or(5).clamp(1, 25),
        fallback: req.fallback,
    }
}
