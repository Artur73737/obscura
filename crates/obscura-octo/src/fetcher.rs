//! The one thing the search core needs from the outside world: fetch a URL and
//! return its rendered HTML. Abstracted behind a trait so the core is testable
//! offline (a map of url -> html) while production renders through a real
//! `obscura_browser::Page`. The trait is `?Send` because a Page owns a V8
//! isolate that is single-threaded and never crosses threads.

use async_trait::async_trait;
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct FetchedPage {
    pub final_url: String,
    pub html: String,
    pub title: String,
    pub eval: Option<Value>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FetchOpts<'a> {
    pub timeout_secs: u64,
    pub wait_secs: u64,
    pub eval: Option<&'a str>,
    /// Referer to send. Search engines validate their "next page" POST against
    /// the page it came from; without it they return an empty results page.
    pub referer: Option<&'a str>,
}

#[async_trait(?Send)]
pub trait Fetcher {
    async fn fetch(&self, url: &str, opts: FetchOpts<'_>) -> Result<FetchedPage, String>;

    /// POST a urlencoded form body and return the rendered result page. Used to
    /// page engines (DuckDuckGo) whose "next" control is a form submit rather
    /// than a GET offset. Default: unsupported (offline test fetchers).
    async fn fetch_post(
        &self,
        url: &str,
        _body: &str,
        _opts: FetchOpts<'_>,
    ) -> Result<FetchedPage, String> {
        Err(format!("POST not supported by this fetcher (url={url})"))
    }
}

/// Production fetcher: renders each URL through a single-use Page. Pages are
/// built and dropped sequentially so at most one V8 isolate is ever live,
/// honouring the engine's single-isolate-per-process rule. The BrowserContext
/// (and its cookie jar) is shared across calls so cookies set on one page —
/// e.g. a search engine's session token that its "next page" POST is validated
/// against — carry forward to the following fetch.
pub struct PageFetcher {
    context: std::sync::Arc<obscura_browser::BrowserContext>,
    user_agent: Option<String>,
    /// When set, cookies are loaded from `{dir}/cookies.json` at construction and
    /// written back by `save_session()`. This is the "warm session": the same
    /// real cookies/history carry across separate runs, which is a legitimate
    /// trust signal (a returning visitor) rather than a fabricated one.
    session_dir: Option<std::path::PathBuf>,
}

impl PageFetcher {
    /// Build a fetcher that presents Obscura as a real, hidden Chrome: stealth
    /// is always on. With the `stealth` build feature this means full TLS
    /// impersonation (so engines that fingerprint the handshake serve real
    /// results); without it, a consistent browser fingerprint plus tracker
    /// blocking. This is the identity the octo tools use by default — automated
    /// but indistinguishable from ordinary browsing.
    pub fn hidden_chrome(
        proxy: Option<String>,
        user_agent: Option<String>,
        allow_private_network: bool,
    ) -> Self {
        Self::new(proxy, true, user_agent, allow_private_network, None)
    }

    /// Like `hidden_chrome`, but persists the cookie jar under `session_dir` so a
    /// real browsing session matures across runs (see the `session_dir` field).
    pub fn hidden_chrome_session(
        proxy: Option<String>,
        user_agent: Option<String>,
        allow_private_network: bool,
        session_dir: Option<std::path::PathBuf>,
    ) -> Self {
        Self::new(proxy, true, user_agent, allow_private_network, session_dir)
    }

    pub fn new(
        proxy: Option<String>,
        stealth: bool,
        user_agent: Option<String>,
        allow_private_network: bool,
        session_dir: Option<std::path::PathBuf>,
    ) -> Self {
        // Ensure the session dir exists so the first run can save into it.
        if let Some(ref dir) = session_dir {
            let _ = std::fs::create_dir_all(dir);
        }
        let context = std::sync::Arc::new(obscura_browser::BrowserContext::with_storage_and_network(
            "octo-search".to_string(),
            proxy,
            stealth,
            user_agent.clone(),
            session_dir.clone(),
            allow_private_network,
        ));
        PageFetcher { context, user_agent, session_dir }
    }

    /// Persist the current cookie jar to `{session_dir}/cookies.json`. No-op when
    /// no session dir was configured. Call once after a run so the session the
    /// engines built up (consent, preference, and rate/reputation cookies) is
    /// there for the next invocation.
    pub fn save_session(&self) {
        if self.session_dir.is_some() {
            self.context.save_cookies();
        }
    }
}

impl PageFetcher {
    async fn render(
        &self,
        url: &str,
        method: &str,
        body: &str,
        opts: FetchOpts<'_>,
    ) -> Result<FetchedPage, String> {
        use obscura_browser::{lifecycle::WaitUntil, Page};
        use std::time::Duration;

        let mut page = Page::new("octo-search-page".to_string(), self.context.clone());
        if let Some(ref ua) = self.user_agent {
            page.http_client.set_user_agent(ua).await;
        }
        // Do NOT synthesize a browser header set here. The search tool is native
        // to Obscura's browser: its requests must be byte-for-byte the ones the
        // (stealth) browser already sends, so the fingerprint is identical to a
        // normal Obscura navigation. The only thing we add is Referer, because a
        // real Chrome sends it when you click a "next page" link or a result —
        // that is real navigation, not a spoofed identity.
        if let Some(referer) = opts.referer {
            let mut h = std::collections::HashMap::new();
            h.insert("Referer".to_string(), referer.to_string());
            page.http_client.set_extra_headers(h).await;
        }

        let timeout = Duration::from_secs(opts.timeout_secs.max(1));
        let nav = page.navigate_with_wait_post(url, WaitUntil::Load, method, body);
        match tokio::time::timeout(timeout, nav).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(format!("navigate failed: {e}")),
            Err(_) => return Err(format!("timed out after {}s", opts.timeout_secs)),
        }

        page.settle(opts.wait_secs.saturating_mul(1000)).await;

        let eval = opts
            .eval
            .map(|expr| page.evaluate_with_timeout(expr, timeout));

        let html = page
            .with_dom(|dom| {
                if let Ok(Some(h)) = dom.query_selector("html") {
                    format!("<!DOCTYPE html>\n{}", dom.outer_html(h))
                } else {
                    dom.inner_html(dom.document())
                }
            })
            .unwrap_or_default();

        Ok(FetchedPage {
            final_url: page.url_string(),
            html,
            title: page.title.clone(),
            eval,
        })
    }
}

#[async_trait(?Send)]
impl Fetcher for PageFetcher {
    async fn fetch(&self, url: &str, opts: FetchOpts<'_>) -> Result<FetchedPage, String> {
        self.render(url, "GET", "", opts).await
    }

    async fn fetch_post(
        &self,
        url: &str,
        body: &str,
        opts: FetchOpts<'_>,
    ) -> Result<FetchedPage, String> {
        self.render(url, "POST", body, opts).await
    }
}
