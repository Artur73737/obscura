//! The monitor core: poll a page on an interval, evaluate a condition, and emit
//! a change event whenever the captured value differs from the last one. Pure
//! over a `Fetcher` and an `OutputSink` (same seams as search), so it drives the
//! CLI (NDJSON to file/stdout) and the HTTP/WS server identically, and is
//! testable offline.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::fetcher::{FetchOpts, Fetcher};
use crate::output::OutputSink;
use crate::schema::{MonitorEvent, MonitorRequest};

#[derive(Clone, Debug)]
pub struct ResolvedMonitor {
    pub url: String,
    pub eval: String,
    pub interval: u64,
    pub timeout: u64,
    pub wait: u64,
    pub max_runs: u64,
    pub min_change_interval: u64,
}

/// Signal used to stop a running monitor loop early (tests, `serve` shutdown).
pub type Stop = std::rc::Rc<std::cell::Cell<bool>>;

pub fn resolve(req: &MonitorRequest) -> ResolvedMonitor {
    ResolvedMonitor {
        url: req.url.clone(),
        eval: build_eval(req),
        interval: req.interval.unwrap_or(60).max(1),
        timeout: req.timeout.unwrap_or(20).max(1),
        wait: req.wait.unwrap_or(2),
        max_runs: req.max_runs.unwrap_or(0),
        min_change_interval: req.min_change_interval.unwrap_or(0),
    }
}

/// Build the single JS expression the poll evaluates. It resolves the watched
/// element, runs the condition with the element in scope, and returns
/// `{value: ...}` on a truthy condition, `null` otherwise, or `{error: msg}`.
/// User JS is embedded as data via serde for the selector; condition/on_change
/// are arbitrary user expressions (bounded by the eval watchdog).
fn build_eval(req: &MonitorRequest) -> String {
    let sel = match &req.selector {
        Some(s) => serde_json::to_string(s).unwrap_or_else(|_| "null".into()),
        None => "null".into(),
    };
    let cond = req.condition.as_deref().unwrap_or("true");
    let on_change = req
        .on_change
        .as_deref()
        .unwrap_or("(el.textContent||'').trim()");
    format!(
        r#"(function(){{
  var el = {sel} ? document.querySelector({sel}) : document.documentElement;
  if (!el) return null;
  try {{
    with (el) {{
      if (!({cond})) return null;
      return {{ value: ({on_change}) }};
    }}
  }} catch (e) {{ return {{ error: String((e && e.message) || e) }}; }}
}})()"#
    )
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn hash_value(v: &Value) -> String {
    let s = serde_json::to_string(v).unwrap_or_default();
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Run the poll loop. Emits a `MonitorEvent` to `sink` on each detected change
/// (and on poll errors). Returns when `max_runs` is reached or `stop` is set.
pub async fn run_monitor(
    req: &MonitorRequest,
    fetcher: &dyn Fetcher,
    sink: &mut dyn OutputSink,
    stop: Option<Stop>,
) {
    let cfg = resolve(req);
    let mut last_hash: Option<String> = None;
    let mut last_emit_ms: u64 = 0;
    let mut run: u64 = 0;
    let mut consecutive_errors: u32 = 0;
    let debounce_ms = cfg.min_change_interval.saturating_mul(1000);

    loop {
        if stop.as_ref().map(|s| s.get()).unwrap_or(false) {
            break;
        }
        run += 1;

        let opts = FetchOpts {
            timeout_secs: cfg.timeout,
            wait_secs: cfg.wait,
            eval: Some(&cfg.eval),
            referer: None,
        };

        match fetcher.fetch(&cfg.url, opts).await {
            Ok(page) => {
                consecutive_errors = 0;
                if let Some(result) = page.eval.as_ref().and_then(|v| v.as_object()) {
                    if let Some(err) = result.get("error").and_then(|e| e.as_str()) {
                        emit(sink, MonitorEvent {
                            run,
                            ts: now_ms(),
                            value: None,
                            hash: None,
                            error: Some(format!("eval error: {err}")),
                        });
                    } else if let Some(value) = result.get("value") {
                        let h = hash_value(value);
                        let changed = last_hash.as_deref() != Some(h.as_str());
                        let ts = now_ms();
                        let debounced = debounce_ms > 0 && ts.saturating_sub(last_emit_ms) < debounce_ms;
                        if changed && !debounced {
                            last_hash = Some(h.clone());
                            last_emit_ms = ts;
                            emit(sink, MonitorEvent {
                                run,
                                ts,
                                value: Some(value.clone()),
                                hash: Some(h),
                                error: None,
                            });
                        }
                    }
                }
            }
            Err(e) => {
                consecutive_errors = consecutive_errors.saturating_add(1);
                emit(sink, MonitorEvent {
                    run,
                    ts: now_ms(),
                    value: None,
                    hash: None,
                    error: Some(e),
                });
            }
        }

        if cfg.max_runs > 0 && run >= cfg.max_runs {
            break;
        }

        // Sleep the interval, backing off (capped) after repeated failures so a
        // down site is not hammered.
        let mut delay = cfg.interval;
        if consecutive_errors > 0 {
            let factor = 1u64 << consecutive_errors.min(4); // 2,4,8,16
            delay = delay.saturating_mul(factor).min(cfg.interval.saturating_mul(16).max(cfg.interval));
        }
        sleep_interruptible(Duration::from_secs(delay), stop.as_ref()).await;
    }
}

/// Sleep, waking early (every 200ms) to check the stop flag so shutdown is
/// responsive even with a long interval.
async fn sleep_interruptible(total: Duration, stop: Option<&Stop>) {
    let step = Duration::from_millis(200);
    let mut remaining = total;
    while remaining > Duration::ZERO {
        if stop.map(|s| s.get()).unwrap_or(false) {
            return;
        }
        let d = remaining.min(step);
        tokio::time::sleep(d).await;
        remaining = remaining.saturating_sub(d);
    }
}

fn emit(sink: &mut dyn OutputSink, ev: MonitorEvent) {
    if let Ok(v) = serde_json::to_value(&ev) {
        sink.emit(&v);
    }
}
