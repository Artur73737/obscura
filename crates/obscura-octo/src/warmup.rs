//! Session warm-up. Drive a real browsing session on an engine for a set
//! duration so the cookie jar matures: SERP queries, opening a few results,
//! natural pauses between navigations. Everything here is genuine navigation —
//! real requests, real Set-Cookie, real history — not fabricated signals. The
//! resulting `--session` starts later runs as a returning visitor, which is a
//! legitimate trust signal that tends to raise anti-bot scores.

use std::time::{Duration, Instant};

use url::Url;

use crate::engine::{build_serp_url, parse_serp};
use crate::fetcher::{FetchOpts, Fetcher};
use crate::schema::Engine;

/// A small, deliberately mundane set of queries to browse. Generic, evergreen
/// topics — the point is to accumulate a real session, not to search anything
/// in particular.
const SEED_QUERIES: &[&str] = &[
    "weather forecast this week",
    "best books 2026",
    "how to cook pasta",
    "premier league table",
    "python vs rust performance",
    "cheap flights europe",
    "healthy breakfast ideas",
    "latest tech news",
    "how does a cpu work",
    "top movies this year",
    "world news today",
    "electric cars 2026",
];

/// Progress callback: `(elapsed_secs, total_secs, message)`.
pub type Progress<'a> = dyn FnMut(u64, u64, &str) + 'a;

/// Warm a session on `engine` for `minutes`, optionally seeded with the caller's
/// own `queries` (else a built-in generic set). Navigates real SERPs and opens a
/// couple of results per query, pausing naturally between requests, until the
/// budget elapses. The fetcher must be built with a `--session` dir so its jar is
/// persisted after each navigation.
pub async fn run_warmup(
    engine: Engine,
    minutes: f64,
    queries: &[String],
    fetcher: &dyn Fetcher,
    mut progress: Option<&mut Progress<'_>>,
) -> WarmupStats {
    let total_secs = (minutes * 60.0).max(1.0) as u64;
    let deadline = Instant::now() + Duration::from_secs(total_secs);
    let lang = "en";

    let owned: Vec<String>;
    let list: &[String] = if queries.is_empty() {
        owned = SEED_QUERIES.iter().map(|s| s.to_string()).collect();
        &owned
    } else {
        queries
    };

    let mut stats = WarmupStats::default();
    let mut qi = 0usize;

    let report = |progress: &mut Option<&mut Progress<'_>>, start: Instant, msg: &str| {
        if let Some(cb) = progress.as_mut() {
            let elapsed = start.elapsed().as_secs().min(total_secs);
            cb(elapsed, total_secs, msg);
        }
    };
    let start = Instant::now();

    while Instant::now() < deadline {
        let query = &list[qi % list.len()];
        qi += 1;

        // 1) The SERP itself — this is where the engine sets its session cookies.
        let serp_url = build_serp_url(engine, query, lang, &[], None, 0);
        report(&mut progress, start, &format!("search: {query}"));
        let serp = match fetcher.fetch(&serp_url, warm_opts(None)).await {
            Ok(p) => {
                stats.pages += 1;
                p
            }
            Err(e) => {
                stats.errors += 1;
                tracing::debug!("warmup serp error: {e}");
                if sleep_until("", Duration::from_secs(4), deadline).await {
                    break;
                }
                continue;
            }
        };

        if sleep_until("dwell on results", human_pause(2, 6), deadline).await {
            break;
        }

        // 2) Open a couple of real results, as a person skimming would. Real
        // navigations to real hosts — more genuine cookies and history.
        let base = Url::parse(&serp.final_url)
            .unwrap_or_else(|_| Url::parse("https://invalid.local/").unwrap());
        let results = parse_serp(engine, &serp.html, &base);
        for r in results.into_iter().filter(|r| r.url.starts_with("http")).take(2) {
            if Instant::now() >= deadline {
                break;
            }
            report(&mut progress, start, &format!("open: {}", r.url));
            match fetcher.fetch(&r.url, warm_opts(Some(&serp.final_url))).await {
                Ok(_) => stats.pages += 1,
                Err(e) => {
                    stats.errors += 1;
                    tracing::debug!("warmup result error: {e}");
                }
            }
            if sleep_until("read page", human_pause(4, 12), deadline).await {
                break;
            }
        }

        stats.queries += 1;
        // A longer pause between search "sessions".
        if sleep_until("idle", human_pause(5, 15), deadline).await {
            break;
        }
    }

    stats.elapsed_secs = start.elapsed().as_secs();
    report(&mut progress, start, "done");
    stats
}

fn warm_opts(referer: Option<&str>) -> FetchOpts<'_> {
    FetchOpts {
        timeout_secs: 30,
        wait_secs: 1,
        eval: None,
        referer,
    }
}

/// A pseudo-random pause in `[lo, hi]` seconds. Derived from the wall clock so
/// intervals vary run to run without pulling in an rng dependency; this is just
/// pacing, not a security primitive.
fn human_pause(lo: u64, hi: u64) -> Duration {
    let span = hi.saturating_sub(lo).max(1);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    Duration::from_secs(lo + (nanos % span))
}

/// Sleep for `dur`, but never past `deadline`. Returns true if the deadline was
/// reached (caller should stop).
async fn sleep_until(_label: &str, dur: Duration, deadline: Instant) -> bool {
    let now = Instant::now();
    if now >= deadline {
        return true;
    }
    let remaining = deadline.saturating_duration_since(now);
    tokio::time::sleep(dur.min(remaining)).await;
    Instant::now() >= deadline
}

/// What a warm-up run did.
#[derive(Debug, Default, Clone, Copy)]
pub struct WarmupStats {
    pub queries: usize,
    pub pages: usize,
    pub errors: usize,
    pub elapsed_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_pause_within_bounds() {
        for _ in 0..50 {
            let d = human_pause(4, 12).as_secs();
            assert!((4..=12).contains(&d), "pause {d} out of [4,12]");
        }
    }

    #[tokio::test]
    async fn sleep_until_stops_at_deadline() {
        let past = Instant::now();
        assert!(sleep_until("x", Duration::from_secs(10), past).await);
    }
}
