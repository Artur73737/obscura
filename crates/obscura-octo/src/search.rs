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
    // The summary handed to `finish` is the aggregate for the trailing NDJSON
    // line / WS `summary` frame — NOT a second copy of every result. The results
    // are already streamed via `emit`, so we drop the `results` array here and
    // replace it with a `count`; keeping it would duplicate all scraped text and
    // double the output size. Full-response consumers use the returned `resp`.
    if let Ok(mut v) = serde_json::to_value(&resp) {
        if let Some(map) = v.as_object_mut() {
            map.remove("results");
            map.insert("count".to_string(), serde_json::json!(resp.results.len()));
        }
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
        let block = classify_block(&fetched.html, &fetched.final_url);
        let blocked = block.is_some();
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
                if let Some(block) = block {
                    return Err(block.describe(engine));
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

/// The kind of wall an engine served instead of results. Naming the mechanism
/// lets the caller know whether it is a solvable challenge (image grid), an
/// invisible score check, or an IP-reputation block that no browser tweak fixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// reCAPTCHA v2 — the visible checkbox / "select all images" grid.
    RecaptchaV2,
    /// reCAPTCHA v3 / invisible — a background score, no user challenge.
    RecaptchaV3,
    /// hCaptcha — visible image-grid challenge.
    HCaptcha,
    /// Google `/sorry/` "unusual traffic" interstitial (usually reCAPTCHA-backed).
    GoogleSorry,
    /// Cloudflare "Just a moment" / "checking your browser" JS challenge.
    CloudflareChallenge,
    /// A cookie/consent wall shown before results (not a bot check).
    ConsentWall,
    /// A generic captcha/anti-bot page we could not attribute more precisely.
    UnknownCaptcha,
}

impl BlockKind {
    fn label(self) -> &'static str {
        match self {
            BlockKind::RecaptchaV2 => "reCAPTCHA v2 (visible checkbox / image-grid challenge)",
            BlockKind::RecaptchaV3 => "reCAPTCHA v3 (invisible score, no image grid)",
            BlockKind::HCaptcha => "hCaptcha (visible image-grid challenge)",
            BlockKind::GoogleSorry => "Google \"unusual traffic\" interstitial (/sorry/, reCAPTCHA-backed)",
            BlockKind::CloudflareChallenge => "Cloudflare JS challenge (\"Just a moment\")",
            BlockKind::ConsentWall => "cookie/consent wall (not a bot check)",
            BlockKind::UnknownCaptcha => "generic anti-bot / captcha page",
        }
    }
}

/// A classified wall plus the most likely reason it fired.
#[derive(Debug, Clone)]
pub struct BlockInfo {
    pub kind: BlockKind,
    /// Short, human reason (IP reputation, TLS fingerprint, rate-limit, ...).
    pub reason: &'static str,
}

impl BlockInfo {
    /// The user-facing error: what wall it is, why it most likely triggered,
    /// and how to recover.
    fn describe(&self, engine: crate::schema::Engine) -> String {
        let recover = match self.kind {
            BlockKind::ConsentWall => {
                "This is a consent screen, not a bot block. Try --engine duckduckgo, \
                 or a region/--lang that does not gate results behind consent."
            }
            BlockKind::RecaptchaV3 | BlockKind::GoogleSorry => {
                "Score/IP-reputation blocks do not clear by tweaking the browser: \
                 use a clean residential --proxy, and add --fallback duckduckgo to auto-recover."
            }
            _ => {
                "DuckDuckGo works without stealth; Bing works with a stealth build \
                 (--features stealth). For Google use a residential --proxy, and add \
                 --fallback duckduckgo to auto-recover."
            }
        };
        format!(
            "engine {engine:?} served a wall instead of results.\n  \
             captcha type: {}\n  \
             likely reason: {}\n  \
             recover: {}",
            self.kind.label(),
            self.reason,
            recover
        )
    }
}

/// Inspect a SERP page and, if it is a wall rather than results, classify the
/// captcha mechanism and infer why it fired. Ordered most-specific first so a
/// page that carries several markers is attributed to its strongest signal.
fn classify_block(html: &str, final_url: &str) -> Option<BlockInfo> {
    let lower = html.to_ascii_lowercase();
    let url = final_url.to_ascii_lowercase();
    let has = |m: &str| lower.contains(m);

    // Google "unusual traffic" — the redirect to /sorry/ is the clearest tell,
    // and it is IP-reputation driven (datacenter/VPN ranges), not fingerprint.
    if url.contains("/sorry/") || has("unusual traffic") || has("our systems have detected") {
        return Some(BlockInfo {
            kind: BlockKind::GoogleSorry,
            reason: "this IP's reputation (datacenter/VPN range or a burst of \
                     queries) tripped Google's rate/abuse check — not a browser-fingerprint issue",
        });
    }

    // reCAPTCHA: v3/invisible loads the api with ?render=<sitekey> or calls
    // grecaptcha.execute; v2 renders a widget (g-recaptcha / api2 / the prompt).
    let recaptcha = has("recaptcha") || has("grecaptcha");
    if recaptcha {
        let v3 = has("render=") || has("grecaptcha.execute") || has("recaptcha/api.js?render");
        let v2 = has("g-recaptcha")
            || has("/recaptcha/api2")
            || has("i'm not a robot")
            || has("select all images");
        if v3 && !v2 {
            return Some(BlockInfo {
                kind: BlockKind::RecaptchaV3,
                reason: "an invisible behavioral score came back too low: cold session \
                         (no cookie/history), no human input signals, or a flagged IP",
            });
        }
        return Some(BlockInfo {
            kind: BlockKind::RecaptchaV2,
            reason: "the client was distrusted enough to demand an interactive challenge, \
                     usually IP reputation combined with a cold, historyless session",
        });
    }

    if has("hcaptcha") || has("h-captcha") {
        return Some(BlockInfo {
            kind: BlockKind::HCaptcha,
            reason: "the site's anti-bot provider distrusted this client — typically \
                     IP reputation or request velocity",
        });
    }

    // Cloudflare interstitial before the real page.
    if has("just a moment")
        || has("checking your browser")
        || has("cf-chl")
        || has("cf_chl")
        || (has("cloudflare") && has("challenge"))
    {
        return Some(BlockInfo {
            kind: BlockKind::CloudflareChallenge,
            reason: "Cloudflare issued a JS/interstitial challenge, driven by IP \
                     reputation or a managed-challenge rule on the zone",
        });
    }

    // Consent / cookie wall (common on google.* in the EU) shown before results.
    if (has("before you continue") || has("consent") || has("accept all"))
        && (url.contains("consent.") || has("cookies"))
    {
        return Some(BlockInfo {
            kind: BlockKind::ConsentWall,
            reason: "a regional cookie/consent gate is being shown before results \
                     (common on google.* in the EU)",
        });
    }

    // Generic fallbacks: a captcha page we could not attribute precisely.
    if has("captcha") || has("verify you are human") || has("are you a robot") || has("enablejs") {
        return Some(BlockInfo {
            kind: BlockKind::UnknownCaptcha,
            reason: "the engine served an anti-bot page; the most common cause is IP \
                     reputation or request velocity from this address",
        });
    }

    None
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

#[cfg(test)]
mod block_tests {
    use super::{classify_block, BlockKind};

    #[test]
    fn google_sorry_is_ip_reputation() {
        let b = classify_block("... Our systems have detected unusual traffic ...", "https://www.google.com/sorry/index?continue=x").unwrap();
        assert_eq!(b.kind, BlockKind::GoogleSorry);
        assert!(b.reason.contains("reputation"));
    }

    #[test]
    fn recaptcha_v3_is_invisible_score() {
        let html = r#"<script src="https://www.google.com/recaptcha/api.js?render=6Lxyz"></script><script>grecaptcha.execute()</script>"#;
        let b = classify_block(html, "https://example.com/").unwrap();
        assert_eq!(b.kind, BlockKind::RecaptchaV3);
    }

    #[test]
    fn recaptcha_v2_is_visible_grid() {
        let html = r#"<div class="g-recaptcha" data-sitekey="k"></div> please select all images"#;
        let b = classify_block(html, "https://example.com/").unwrap();
        assert_eq!(b.kind, BlockKind::RecaptchaV2);
    }

    #[test]
    fn hcaptcha_detected() {
        let b = classify_block(r#"<div class="h-captcha"></div>"#, "https://example.com/").unwrap();
        assert_eq!(b.kind, BlockKind::HCaptcha);
    }

    #[test]
    fn cloudflare_challenge_detected() {
        let b = classify_block("<title>Just a moment...</title> checking your browser", "https://example.com/").unwrap();
        assert_eq!(b.kind, BlockKind::CloudflareChallenge);
    }

    #[test]
    fn consent_wall_detected() {
        let b = classify_block("Before you continue to Google — we use cookies. Accept all?", "https://consent.google.com/").unwrap();
        assert_eq!(b.kind, BlockKind::ConsentWall);
    }

    #[test]
    fn real_results_are_not_blocked() {
        assert!(classify_block("<html><body><a href=\"https://x.com\">A result</a></body></html>", "https://duckduckgo.com/").is_none());
    }

    #[test]
    fn describe_names_type_and_reason() {
        let b = classify_block("recaptcha unusual traffic", "https://google.com/sorry/").unwrap();
        let msg = b.describe(crate::schema::Engine::Google);
        assert!(msg.contains("captcha type:"));
        assert!(msg.contains("likely reason:"));
        assert!(msg.contains("recover:"));
    }
}
