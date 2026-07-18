//! Core + engine-parsing tests, offline via MapFetcher.

mod common;

use common::{MapFetcher, DDG_SERP, PAGE_A};
use obscura_octo::engine::{build_serp_url, parse_serp};
use obscura_octo::schema::{Depth, Engine, ScrapeKind};
use obscura_octo::{run_search, CollectSink, SearchRequest};
use url::Url;

fn ddg_url(query: &str) -> String {
    build_serp_url(Engine::Duckduckgo, query, "en", &[], None, 0)
}

async fn search(req: SearchRequest, fetcher: &MapFetcher) -> (obscura_octo::SearchResponse, CollectSink) {
    let mut sink = CollectSink::default();
    let resp = run_search(&req, fetcher, &mut sink).await;
    (resp, sink)
}

#[tokio::test]
async fn serp_extracts_dedupes_and_ranks() {
    let url = ddg_url("rust");
    let fetcher = MapFetcher::default().with(&url, DDG_SERP);
    let req = SearchRequest { query: "rust".into(), ..Default::default() };

    let (resp, sink) = search(req, &fetcher).await;

    assert!(resp.error.is_none(), "unexpected error: {:?}", resp.error);
    // Third result is a duplicate of the first -> deduped to 2.
    assert_eq!(resp.results.len(), 2);
    assert_eq!(resp.results[0].rank, 1);
    assert_eq!(resp.results[0].title, "Result A");
    assert_eq!(resp.results[0].url, "https://example.com/a");
    assert_eq!(resp.results[0].snippet, "Snippet about A");
    assert_eq!(resp.results[1].url, "https://example.org/b");
    // Sink saw one record per result plus the summary.
    assert_eq!(sink.records.len(), 2);
    assert!(sink.summary.is_some());
}

#[tokio::test]
async fn site_filter_limits_to_domain() {
    // The site operator is injected into the query, so the SERP URL differs
    // from the plain one; key the fixture on the actual URL the core builds.
    let url = build_serp_url(Engine::Duckduckgo, "rust", "en", &["example.com".to_string()], None, 0);
    let fetcher = MapFetcher::default().with(&url, DDG_SERP);
    let req = SearchRequest {
        query: "rust".into(),
        site: vec!["example.com".into()],
        ..Default::default()
    };

    let (resp, _) = search(req, &fetcher).await;
    assert_eq!(resp.results.len(), 1);
    assert_eq!(resp.results[0].url, "https://example.com/a");
}

#[tokio::test]
async fn depth_page_scrapes_result_text() {
    let url = ddg_url("rust");
    let fetcher = MapFetcher::default()
        .with(&url, DDG_SERP)
        .with("https://example.com/a", PAGE_A)
        .with("https://example.org/b", "<html><body><p>Bravo body</p></body></html>");
    let req = SearchRequest {
        query: "rust".into(),
        depth: Some(Depth::Page),
        scrape: Some(ScrapeKind::Text),
        ..Default::default()
    };

    let (resp, _) = search(req, &fetcher).await;
    let a = &resp.results[0];
    let scrape = a.scrape.as_ref().expect("scrape present");
    assert!(scrape.text.as_deref().unwrap_or("").contains("Hello A world"));
}

#[tokio::test]
async fn missing_serp_fixture_yields_error_not_panic() {
    let fetcher = MapFetcher::default();
    let req = SearchRequest { query: "nothing".into(), ..Default::default() };
    let (resp, _) = search(req, &fetcher).await;
    assert!(resp.error.is_some());
    assert!(resp.results.is_empty());
}

// ---- pure engine parsing ----

#[test]
fn google_parse_unwraps_redirect() {
    let html = r#"<html><body>
        <div class="g"><a href="/url?q=https://foo.example/x&sa=U"><h3>Foo Title</h3></a></div>
    </body></html>"#;
    let base = Url::parse("https://www.google.com/search").unwrap();
    let results = parse_serp(Engine::Google, html, &base);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://foo.example/x");
    assert_eq!(results[0].title, "Foo Title");
}

#[test]
fn bing_parse_extracts_result() {
    let html = r#"<html><body>
        <li class="b_algo"><h2><a href="https://bar.example/y">Bar Title</a></h2>
        <div class="b_caption"><p>bing caption</p></div></li>
    </body></html>"#;
    let base = Url::parse("https://www.bing.com/search").unwrap();
    let results = parse_serp(Engine::Bing, html, &base);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://bar.example/y");
    assert_eq!(results[0].title, "Bar Title");
    assert_eq!(results[0].snippet, "bing caption");
}

#[test]
fn extract_next_form_reads_paging_form() {
    // DuckDuckGo-style next-page form; action resolves to /html/ (query dropped)
    // and every named input is carried into the POST body.
    let html = r#"<html><body>
        <form action="/html/" method="post">
            <input name="q" value="rust"><input name="s" value="10"><input name="vqd" value="abc">
        </form>
    </body></html>"#;
    let base = Url::parse("https://html.duckduckgo.com/html/?q=rust").unwrap();
    let nf = obscura_octo::engine::extract_next_form(html, &base).expect("next form");
    assert_eq!(nf.action, "https://html.duckduckgo.com/html/");
    assert!(nf.body.contains("s=10"), "body: {}", nf.body);
    assert!(nf.body.contains("q=rust"), "body: {}", nf.body);
    assert!(nf.body.contains("vqd=abc"), "body: {}", nf.body);
}

#[test]
fn extract_next_form_none_without_paging_input() {
    let html = r#"<html><body><form action="/x"><input name="q" value="a"></form></body></html>"#;
    let base = Url::parse("https://example.com/").unwrap();
    assert!(obscura_octo::engine::extract_next_form(html, &base).is_none());
}

#[test]
fn bing_parse_decodes_ck_redirect() {
    // Bing wraps the destination in /ck/a?...&u=a1<base64url(url)>.
    use base64::Engine as _;
    let target = "https://www.w3schools.com/python/";
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(target);
    let href = format!("https://www.bing.com/ck/a?ptn=3&u=a1{b64}&ntb=1");
    let html = format!(
        r#"<html><body><li class="b_algo"><h2><a href="{href}">W3</a></h2>
        <div class="b_caption"><p>cap</p></div></li></body></html>"#
    );
    let base = Url::parse("https://www.bing.com/search").unwrap();
    let r = parse_serp(Engine::Bing, &html, &base);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].url, target);
}

#[test]
fn ddg_parse_unwraps_uddg_redirect() {
    let html = r#"<html><body><div class="result">
        <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fz.example%2Fp&rut=abc">Z</a>
    </div></body></html>"#;
    let base = Url::parse("https://html.duckduckgo.com/html/").unwrap();
    let results = parse_serp(Engine::Duckduckgo, html, &base);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://z.example/p");
}
