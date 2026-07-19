//! Output sinks. The search core is agnostic about where records go: it calls
//! `emit` per result and `finish` once with the aggregate. Each surface plugs
//! in its own sink (file/stdout for CLI, response body for HTTP, frames for
//! WS). A `CollectSink` backs tests and the buffered surfaces.

use serde_json::Value;

pub trait OutputSink {
    /// One search result, as soon as it is produced.
    fn emit(&mut self, record: &Value);
    /// The aggregate summary, once, after the last `emit`: response metadata
    /// (query, engine, count, took_ms, error) WITHOUT the `results` array, which
    /// was already streamed via `emit`. Used as the trailing NDJSON line / WS
    /// `summary` frame. Full-response callers use the returned `SearchResponse`.
    fn finish(&mut self, summary: &Value) {
        let _ = summary;
    }
}

/// Buffers every record and the summary. Used by tests and by surfaces that
/// send everything after the run completes (HTTP one-shot, WS framing).
#[derive(Default)]
pub struct CollectSink {
    pub records: Vec<Value>,
    pub summary: Option<Value>,
}

impl OutputSink for CollectSink {
    fn emit(&mut self, record: &Value) {
        self.records.push(record.clone());
    }
    fn finish(&mut self, summary: &Value) {
        self.summary = Some(summary.clone());
    }
}

/// A sink that ignores records and keeps only the summary. For JSON one-shot
/// callers that serialize the returned `SearchResponse` directly.
#[derive(Default)]
pub struct NullSink;

impl OutputSink for NullSink {
    fn emit(&mut self, _record: &Value) {}
}
