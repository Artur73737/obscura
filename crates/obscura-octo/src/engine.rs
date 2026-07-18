//! Per-engine SERP URL building and result extraction. Extraction is a pure
//! function over a parsed `DomTree`, so it is unit-tested offline against saved
//! SERP fixtures with no browser or network. Selectors live in one place per
//! engine; when a layout changes only the spec moves.

use obscura_dom::{parse_html, DomTree, NodeId};
use url::form_urlencoded::byte_serialize;
use url::Url;

use crate::schema::Engine;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

fn enc(s: &str) -> String {
    byte_serialize(s.as_bytes()).collect()
}

/// Build the SERP URL for result page starting at `offset` (0 = first page).
/// `site:` operators are injected into the query so the engine narrows results
/// up front; we still filter by host afterwards because engines do not always
/// honour the operator (see security::passes_site_filters). `offset` is the
/// number of results already seen; each engine maps it to its own paging param.
pub fn build_serp_url(
    engine: Engine,
    query: &str,
    lang: &str,
    sites: &[String],
    custom: Option<&str>,
    offset: usize,
) -> String {
    let mut q = query.to_string();
    if !sites.is_empty() {
        let joined = sites
            .iter()
            .map(|s| format!("site:{}", s.trim()))
            .collect::<Vec<_>>()
            .join(" OR ");
        q = format!("{q} ({joined})");
    }
    let q = enc(&q);
    match engine {
        Engine::Duckduckgo => {
            let mut u = format!("https://html.duckduckgo.com/html/?q={q}");
            if offset > 0 {
                u.push_str(&format!("&s={offset}&dc={}", offset + 1));
            }
            u
        }
        Engine::Google => {
            let mut u = format!("https://www.google.com/search?q={q}&hl={}", enc(lang));
            if offset > 0 {
                u.push_str(&format!("&start={offset}"));
            }
            u
        }
        Engine::Bing => {
            let mut u = format!("https://www.bing.com/search?q={q}&setlang={}", enc(lang));
            if offset > 0 {
                u.push_str(&format!("&first={}", offset + 1));
            }
            u
        }
        Engine::Custom => custom
            .unwrap_or("")
            .replace("{query}", &q)
            .replace("{lang}", &enc(lang))
            .replace("{offset}", &offset.to_string()),
    }
}

/// Extract results from raw SERP HTML for the given engine.
pub fn parse_serp(engine: Engine, html: &str, base: &Url) -> Vec<RawResult> {
    let dom = parse_html(html);
    match engine {
        Engine::Google => parse_google(&dom, base),
        Engine::Bing => parse_spec(&dom, &BING, base),
        // Custom engines default to the DuckDuckGo-style result markup; a
        // self-hosted meta engine can mimic it, and it is the most forgiving.
        Engine::Duckduckgo | Engine::Custom => parse_spec(&dom, &DDG, base),
    }
}

struct SerpSpec {
    container: &'static str,
    link: &'static str,
    snippet: &'static str,
}

const DDG: SerpSpec = SerpSpec {
    container: ".result",
    link: "a.result__a",
    snippet: "a.result__snippet",
};

const BING: SerpSpec = SerpSpec {
    container: "li.b_algo",
    link: "h2 a",
    snippet: ".b_caption p",
};

fn parse_spec(dom: &DomTree, spec: &SerpSpec, base: &Url) -> Vec<RawResult> {
    let mut out = Vec::new();
    let containers = dom.query_selector_all(spec.container).unwrap_or_default();
    for c in containers {
        let Some(link) = dom.query_selector_from(c, spec.link).ok().flatten() else {
            continue;
        };
        let Some(raw_href) = attr(dom, link, "href") else {
            continue;
        };
        let Some(url) = resolve_href(&raw_href, base) else {
            continue;
        };
        let title = dom.text_content(link).trim().to_string();
        if title.is_empty() {
            continue;
        }
        let snippet = dom
            .query_selector_from(c, spec.snippet)
            .ok()
            .flatten()
            .map(|s| collapse_ws(&dom.text_content(s)))
            .unwrap_or_default();
        out.push(RawResult { title, url, snippet });
    }
    out
}

/// Google buries results in noisy, frequently-changing markup. Anchor a match
/// on `a:has(h3)` (the result link always wraps the visible title heading),
/// then read the title from the `<h3>`. Snippet extraction is best-effort.
fn parse_google(dom: &DomTree, base: &Url) -> Vec<RawResult> {
    let mut out = Vec::new();
    let links = dom.query_selector_all("a:has(h3)").unwrap_or_default();
    for a in links {
        let Some(raw_href) = attr(dom, a, "href") else {
            continue;
        };
        let Some(url) = resolve_href(&raw_href, base) else {
            continue;
        };
        let title = dom
            .query_selector_from(a, "h3")
            .ok()
            .flatten()
            .map(|h| collapse_ws(&dom.text_content(h)))
            .unwrap_or_default();
        if title.is_empty() {
            continue;
        }
        out.push(RawResult {
            title,
            url,
            snippet: String::new(),
        });
    }
    out
}

/// A "next page" form to resubmit (DuckDuckGo-style paging). `action` is the
/// absolute POST target, `body` the urlencoded form fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NextForm {
    pub action: String,
    pub body: String,
}

/// Find the results paging form that advances to the NEXT page. DuckDuckGo html
/// renders one form per direction, each with a hidden `input[name="s"]` (result
/// offset); the next-page form has the largest `s`. Collect that form's inputs
/// verbatim and urlencode them for a POST. Returns None when no such form exists
/// (engines that page via a GET offset instead).
pub fn extract_next_form(html: &str, base: &Url) -> Option<NextForm> {
    let dom = parse_html(html);
    let forms = dom.query_selector_all("form").unwrap_or_default();
    let mut best: Option<(i64, NextForm)> = None;

    for f in forms {
        let s_input = dom.query_selector_from(f, "input[name=\"s\"]").ok().flatten();
        let Some(s_input) = s_input else {
            continue;
        };
        let s_val: i64 = attr(&dom, s_input, "value")
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0);

        let action_raw = attr(&dom, f, "action").unwrap_or_default();
        let action = if action_raw.trim().is_empty() {
            base.to_string()
        } else {
            base.join(action_raw.trim())
                .map(|u| u.to_string())
                .unwrap_or_else(|_| base.to_string())
        };

        let inputs = dom.query_selector_all_from(f, "input[name]").unwrap_or_default();
        let mut pairs = Vec::new();
        for inp in inputs {
            if let Some(name) = attr(&dom, inp, "name") {
                let val = attr(&dom, inp, "value").unwrap_or_default();
                pairs.push(format!("{}={}", enc(&name), enc(&val)));
            }
        }
        if pairs.is_empty() {
            continue;
        }
        let form = NextForm { action, body: pairs.join("&") };
        if best.as_ref().map(|(bs, _)| s_val > *bs).unwrap_or(true) {
            best = Some((s_val, form));
        }
    }

    best.map(|(_, f)| f)
}

fn attr(dom: &DomTree, nid: NodeId, name: &str) -> Option<String> {
    dom.get_node(nid)
        .and_then(|n| n.get_attribute(name).map(|s| s.to_string()))
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Resolve a SERP anchor href to an absolute http(s) URL, unwrapping the
/// redirect wrappers the engines use (DuckDuckGo `l/?uddg=`, Google `/url?q=`).
pub fn resolve_href(raw: &str, base: &Url) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Some(u) = query_param(raw, "uddg") {
        if u.starts_with("http") {
            return Some(u);
        }
    }
    // Bing wraps every result in https://www.bing.com/ck/a?...&u=a1<base64url>&...
    // where the real destination is base64url of the URL after the "a1" prefix.
    if raw.contains("/ck/a") {
        if let Some(u) = query_param(raw, "u") {
            if let Some(dec) = decode_bing_u(&u) {
                if dec.starts_with("http") {
                    return Some(dec);
                }
            }
        }
    }
    if raw.starts_with("/url?") || raw.contains("google.") && raw.contains("/url?") {
        if let Some(q) = query_param(raw, "q") {
            if q.starts_with("http") {
                return Some(q);
            }
        }
    }
    if raw.starts_with("http://") || raw.starts_with("https://") {
        return Some(raw.to_string());
    }
    if let Some(rest) = raw.strip_prefix("//") {
        return Some(format!("https://{rest}"));
    }
    base.join(raw).ok().map(|u| u.to_string())
}

/// Read query parameter `key` from a URL or URL fragment, percent-decoding the
/// value. Works on absolute URLs and on bare `path?query` strings alike.
fn query_param(raw: &str, key: &str) -> Option<String> {
    let query = raw.split('?').nth(1)?;
    // Drop any fragment tail.
    let query = query.split('#').next().unwrap_or(query);
    for pair in query.split('&') {
        let mut it = pair.splitn(2, '=');
        let k = it.next()?;
        if k == key {
            return it.next().map(pct_decode);
        }
    }
    None
}

fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                    out.push((h << 4) | l);
                    i += 3;
                    continue;
                }
                out.push(b[i]);
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Decode a Bing `u=a1<base64url>` redirect parameter to its target URL.
/// The value is base64url (sometimes unpadded) of the destination, prefixed
/// with a two-char scheme tag ("a1").
fn decode_bing_u(u: &str) -> Option<String> {
    use base64::Engine as _;
    let b64 = u.strip_prefix("a1").unwrap_or(u);
    let engines = [
        base64::engine::general_purpose::URL_SAFE_NO_PAD,
        base64::engine::general_purpose::URL_SAFE,
    ];
    for eng in engines {
        if let Ok(bytes) = eng.decode(b64) {
            if let Ok(s) = String::from_utf8(bytes) {
                return Some(s);
            }
        }
    }
    None
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
