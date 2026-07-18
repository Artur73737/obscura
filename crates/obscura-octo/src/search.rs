//! The search core. Navigate a SERP, extract results, optionally scrape each
//! result page, and (for depth deep) follow same-host links one level. Pure
//! over a `Fetcher` and an `OutputSink` so every surface shares this exact
//! logic and it is testable offline.

use std::collections::HashSet;
use std::time::Instant;

use obscura_dom::parse_html;
use url::Url;

use crate::config::{resolve, ResolvedConfig};
use crate::engine::{build_serp_url, parse_serp, resolve_href};
use crate::fetcher::{FetchOpts, Fetcher};
use crate::output::OutputSink;
use crate::schema::{
    Depth, ScrapeData, ScrapeKind, SearchRequest, SearchResponse, SearchResult,
};
use crate::security::{canonical, host_of, is_engine_host, is_http, passes_site_filters};

pub async fn run_search(
    req: &SearchRequest,
    fetcher: &dyn Fetcher,
    sink: &mut dyn OutputSink,
) -> SearchResponse {
    let cfg = resolve(req);
    let start = Instant::now();
    let mut results: Vec<SearchResult> = Vec::new();
    let mut error: Option<String> = None;
    let mut seen: HashSet<String> = HashSet::new();

    match fetch_serp(req, &cfg, fetcher).await {
        Ok((raws, base)) => {
            for r in raws {
                if results.len() >= cfg.max_results {
                    break;
                }
                if !is_http(&r.url) || is_engine_host(&r.url) {
                    continue;
                }
                if !passes_site_filters(&r.url, &cfg.site, &cfg.exclude_site, cfg.site_exact) {
                    continue;
                }
                if !seen.insert(canonical(&r.url)) {
                    continue;
                }
                let rank = results.len() + 1;
                let mut result = SearchResult {
                    rank,
                    title: r.title,
                    url: r.url,
                    snippet: r.snippet,
                    scrape: None,
                    eval: None,
                    followed: false,
                };
                if cfg.depth != Depth::Serp {
                    // Send the SERP as Referer: a human opens each result by
                    // clicking it from the results page, so a real Chrome sends
                    // the search page as the referrer.
                    scrape_into(&mut result, req, &cfg, fetcher, base.as_str()).await;
                }
                emit(sink, &result);
                results.push(result);
            }

            // Depth deep: one level of same-host internal links, within budget.
            if cfg.depth == Depth::Deep {
                follow_internal_links(&mut results, &mut seen, req, &cfg, fetcher, sink, &base).await;
            }
        }
        Err(e) => error = Some(e),
    }

    let resp = SearchResponse {
        query: req.query.clone(),
        engine: cfg.engine,
        lang: cfg.lang.clone(),
        sites: cfg.site.clone(),
        results,
        took_ms: start.elapsed().as_millis() as u64,
        error,
    };
    if let Ok(v) = serde_json::to_value(&resp) {
        sink.finish(&v);
    }
    resp
}

/// Fetch and parse the SERP across as many result pages as needed to reach the
/// requested budget. One page of an engine returns only ~10 results, so a
/// `max_results` of 20 needs paging. Stops when the budget is met, a page adds
/// no new URLs (engine ignored our paging param), a page is empty, or the page
/// cap is hit. Retries once with the fallback engine when the first page is
/// empty (a blocked/empty primary).
async fn fetch_serp(
    req: &SearchRequest,
    cfg: &ResolvedConfig,
    fetcher: &dyn Fetcher,
) -> Result<(Vec<crate::engine::RawResult>, Url), String> {
    // Cap total pages so a stuck/looping engine cannot fan out unbounded.
    const MAX_PAGES: usize = 6;

    let mut engine = cfg.engine;
    let mut collected: Vec<crate::engine::RawResult> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut base_out: Option<Url> = None;
    let mut offset = 0usize;
    let mut fallback_used = false;
    // When the previous page carried a "next page" form (DuckDuckGo-style),
    // page forward by POSTing it instead of a GET offset the engine ignores.
    let mut next_form: Option<crate::engine::NextForm> = None;
    // URL of the previously fetched page, sent as Referer on the paging request
    // (engines gate their "next page" response on it).
    let mut prev_url: Option<String> = None;

    let mut page_idx = 0;
    while page_idx < MAX_PAGES {
        let opts = FetchOpts {
            timeout_secs: cfg.timeout,
            wait_secs: cfg.wait,
            eval: None,
            referer: prev_url.as_deref(),
        };
        let fetched = match &next_form {
            Some(nf) => fetcher.fetch_post(&nf.action, &nf.body, opts).await,
            None => {
                let url = build_serp_url(
                    engine,
                    &req.query,
                    &cfg.lang,
                    &cfg.site,
                    req.engine_url.as_deref(),
                    offset,
                );
                fetcher.fetch(&url, opts).await
            }
        };
        let fetched = match fetched {
            Ok(f) => f,
            Err(e) => {
                tracing::debug!("octo search: page {} fetch error: {}", page_idx, e);
                if page_idx == 0 {
                    return Err(e);
                }
                break;
            }
        };
        let base = Url::parse(&fetched.final_url)
            .unwrap_or_else(|_| base_out.clone().unwrap_or_else(|| Url::parse("https://invalid.local/").unwrap()));
        if base_out.is_none() {
            base_out = Some(base.clone());
        }

        // Treat an anti-bot / captcha page as "no results" up front: its markup
        // can still contain a stray parseable link that would otherwise suppress
        // the fallback and leave the caller with a silent empty set.
        let blocked = looks_blocked(&fetched.html);
        let raws = if blocked {
            Vec::new()
        } else {
            parse_serp(engine, &fetched.html, &base)
        };

        if raws.is_empty() {
            // First page empty: try the fallback engine once, from offset 0.
            if page_idx == 0 && !fallback_used {
                if let Some(fb) = cfg.fallback {
                    engine = fb;
                    fallback_used = true;
                    offset = 0;
                    base_out = None;
                    next_form = None;
                    continue; // re-enter without advancing page_idx
                }
            }
            // No results on the first page and no fallback left. If the engine
            // served an anti-bot page, say so with the ways out instead of
            // returning a silent empty result set.
            if page_idx == 0 && collected.is_empty() {
                if blocked {
                    return Err(format!(
                        "engine {:?} returned an anti-bot / captcha page. DuckDuckGo works without \
                         stealth; Bing works with a stealth build (--features stealth); Google \
                         additionally IP-reputation blocks with reCAPTCHA, so it needs a clean \
                         residential --proxy. Add --fallback duckduckgo to auto-recover.",
                        engine
                    ));
                }
                // A real results page with zero parseable results: either the
                // query genuinely has none, or the engine soft-throttled this IP
                // (an empty page without a captcha). Say so rather than returning
                // a silent empty set.
                return Err(format!(
                    "engine {:?} returned no results for this query. It may have none, or the \
                     engine soft-rate-limited this IP. Retry shortly, add --fallback duckduckgo, \
                     or try another --engine.",
                    engine
                ));
            }
            break;
        }

        // Advance the GET paging offset by how many results the engine actually
        // showed on this page, not by how many were new after de-duplication.
        // Stepping by unique-added made the next page (Bing &first=, Google
        // &start=) overlap the previous one, so results repeated and paging
        // stalled early.
        let shown = raws.len();
        let mut added = 0usize;
        for r in raws {
            if seen.insert(canonical(&r.url)) {
                collected.push(r);
                added += 1;
            }
        }
        // Engine returned only results we already have (it ignored our paging
        // and served the same page): stop.
        if added == 0 {
            break;
        }
        if collected.len() >= cfg.max_results {
            break;
        }
        offset += shown;
        prev_url = Some(fetched.final_url.clone());
        // Prefer the page's own next-form; else the GET offset above advances.
        next_form = crate::engine::extract_next_form(&fetched.html, &base);
        tracing::debug!(
            "octo search: page {} added {} (total {})",
            page_idx,
            added,
            collected.len()
        );
        page_idx += 1;
    }

    let base = base_out.unwrap_or_else(|| Url::parse("https://invalid.local/").unwrap());
    Ok((collected, base))
}

/// Heuristic: does this SERP HTML look like an anti-bot / consent wall rather
/// than a real results page? Google ("unusual traffic") and Bing (captcha)
/// serve these to clients they do not trust.
fn looks_blocked(html: &str) -> bool {
    let lower = html.to_ascii_lowercase();
    const MARKERS: &[&str] = &[
        "unusual traffic",
        "/sorry/",
        "recaptcha",
        "captcha",
        "verify you are human",
        "are you a robot",
        "enablejs",
    ];
    MARKERS.iter().any(|m| lower.contains(m))
}

async fn scrape_into(
    result: &mut SearchResult,
    req: &SearchRequest,
    cfg: &ResolvedConfig,
    fetcher: &dyn Fetcher,
    referer: &str,
) {
    let page = match fetcher
        .fetch(
            &result.url,
            FetchOpts {
                timeout_secs: cfg.timeout,
                wait_secs: cfg.wait,
                eval: req.eval.as_deref(),
                referer: Some(referer),
            },
        )
        .await
    {
        Ok(p) => p,
        Err(_) => return,
    };
    result.eval = page.eval;
    if cfg.scrape != ScrapeKind::None {
        let base = Url::parse(&page.final_url).ok();
        result.scrape = Some(scrape_html(&page.html, cfg.scrape, base.as_ref()));
    }
}

/// Follow same-host (or same allowed-site) links found on already-scraped
/// pages, one level, until the result budget is exhausted.
async fn follow_internal_links(
    results: &mut Vec<SearchResult>,
    seen: &mut HashSet<String>,
    req: &SearchRequest,
    cfg: &ResolvedConfig,
    fetcher: &dyn Fetcher,
    sink: &mut dyn OutputSink,
    _serp_base: &Url,
) {
    // Snapshot the seed pages first; we append to `results` as we go.
    let seeds: Vec<(String, Vec<String>)> = results
        .iter()
        .filter_map(|r| {
            let links = r
                .scrape
                .as_ref()
                .and_then(|s| s.links.clone())
                .unwrap_or_default();
            Some((r.url.clone(), links))
        })
        .collect();

    for (seed_url, links) in seeds {
        let Some(seed_host) = host_of(&seed_url) else {
            continue;
        };
        for link in links {
            if results.len() >= cfg.max_results {
                return;
            }
            if !is_http(&link) {
                continue;
            }
            // Same host as the seed, and still inside any --site allow-list.
            if host_of(&link).as_deref() != Some(seed_host.as_str()) {
                continue;
            }
            if !passes_site_filters(&link, &cfg.site, &cfg.exclude_site, cfg.site_exact) {
                continue;
            }
            if !seen.insert(canonical(&link)) {
                continue;
            }
            let page = match fetcher
                .fetch(
                    &link,
                    FetchOpts {
                        timeout_secs: cfg.timeout,
                        wait_secs: cfg.wait,
                        eval: req.eval.as_deref(),
                        // A real click on an internal link sends the page it was
                        // found on as the referrer.
                        referer: Some(&seed_url),
                    },
                )
                .await
            {
                Ok(p) => p,
                Err(_) => continue,
            };
            let base = Url::parse(&page.final_url).ok();
            let scrape = if cfg.scrape != ScrapeKind::None {
                Some(scrape_html(&page.html, cfg.scrape, base.as_ref()))
            } else {
                None
            };
            let rank = results.len() + 1;
            let result = SearchResult {
                rank,
                title: page.title.clone(),
                url: page.final_url.clone(),
                snippet: String::new(),
                scrape,
                eval: page.eval,
                followed: true,
            };
            emit(sink, &result);
            results.push(result);
        }
    }
}

/// Pure scrape of rendered HTML into the requested shape.
fn scrape_html(html: &str, kind: ScrapeKind, base: Option<&Url>) -> ScrapeData {
    let dom = parse_html(html);
    match kind {
        ScrapeKind::None => ScrapeData { kind, text: None, html: None, links: None },
        ScrapeKind::Html => ScrapeData {
            kind,
            text: None,
            html: Some(html.to_string()),
            links: None,
        },
        ScrapeKind::Text => {
            let text = dom
                .query_selector("body")
                .ok()
                .flatten()
                .map(|b| dom.text_content(b).split_whitespace().collect::<Vec<_>>().join(" "))
                .unwrap_or_default();
            ScrapeData { kind, text: Some(text), html: None, links: None }
        }
        ScrapeKind::Links => {
            let mut links = Vec::new();
            let mut seen = HashSet::new();
            for a in dom.query_selector_all("a[href]").unwrap_or_default() {
                let Some(raw) = dom
                    .get_node(a)
                    .and_then(|n| n.get_attribute("href").map(|s| s.to_string()))
                else {
                    continue;
                };
                let resolved = match base {
                    Some(b) => resolve_href(&raw, b),
                    None => Some(raw.clone()),
                };
                if let Some(u) = resolved {
                    if is_http(&u) && seen.insert(u.clone()) {
                        links.push(u);
                    }
                }
            }
            ScrapeData { kind, text: None, html: None, links: Some(links) }
        }
    }
}

fn emit(sink: &mut dyn OutputSink, result: &SearchResult) {
    if let Ok(v) = serde_json::to_value(result) {
        sink.emit(&v);
    }
}
