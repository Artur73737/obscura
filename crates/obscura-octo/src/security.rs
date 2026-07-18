//! Network-safety helpers shared by the octo features. The real SSRF gate
//! (loopback / RFC1918 / link-local blocking) lives in obscura-net and is
//! enforced by the Page fetcher; these helpers add the feature-level rules:
//! only http(s) result URLs, site allow/deny filtering, and dropping links
//! that point back at the search engine itself.

use url::Url;

/// Host of `url`, lowercased and with a leading `www.` stripped so
/// `www.example.com` and `example.com` compare equal.
pub fn host_of(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    parsed
        .host_str()
        .map(|h| h.trim_start_matches("www.").to_ascii_lowercase())
}

/// True when `url` is http or https. Everything else (javascript:, data:,
/// about:, file:, mailto:) is rejected before we ever navigate to it.
pub fn is_http(url: &str) -> bool {
    matches!(
        Url::parse(url).ok().as_ref().map(|u| u.scheme()),
        Some("http") | Some("https")
    )
}

/// Whether `host` matches `site`. Non-exact match also accepts subdomains
/// (`example.com` matches `docs.example.com`).
pub fn site_matches(host: &str, site: &str, exact: bool) -> bool {
    let site = site.trim().trim_start_matches("www.").to_ascii_lowercase();
    if site.is_empty() {
        return false;
    }
    if exact {
        host == site
    } else {
        host == site || host.ends_with(&format!(".{site}"))
    }
}

/// Apply the `--site` allow-list and `--exclude-site` deny-list. An empty
/// allow-list means "any host".
pub fn passes_site_filters(url: &str, allow: &[String], exclude: &[String], exact: bool) -> bool {
    let Some(host) = host_of(url) else {
        return false;
    };
    if !allow.is_empty() && !allow.iter().any(|s| site_matches(&host, s, exact)) {
        return false;
    }
    if exclude.iter().any(|s| site_matches(&host, s, exact)) {
        return false;
    }
    true
}

/// Search-engine hosts we never surface as results (their own nav/help links).
const ENGINE_HOSTS: &[&str] = &[
    "google.com",
    "bing.com",
    "duckduckgo.com",
    "microsoft.com",
    "gstatic.com",
    "googleusercontent.com",
];

pub fn is_engine_host(url: &str) -> bool {
    match host_of(url) {
        Some(h) => ENGINE_HOSTS
            .iter()
            .any(|e| h == *e || h.ends_with(&format!(".{e}"))),
        None => true,
    }
}

/// A stable key for de-duplicating result URLs: scheme + host + path, dropping
/// the fragment and trailing slash so trivially different URLs collapse.
pub fn canonical(url: &str) -> String {
    match Url::parse(url) {
        Ok(u) => {
            let host = u.host_str().unwrap_or("").trim_start_matches("www.");
            let path = u.path().trim_end_matches('/');
            let query = u.query().map(|q| format!("?{q}")).unwrap_or_default();
            format!("{}://{}{}{}", u.scheme(), host, path, query)
        }
        Err(_) => url.to_string(),
    }
}
