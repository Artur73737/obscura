//! Monitor core tests, offline via a fetcher that returns a scripted sequence
//! of eval values (simulating a page whose watched value changes over time).

use std::cell::RefCell;

use async_trait::async_trait;
use obscura_octo::{run_monitor, CollectSink, FetchOpts, FetchedPage, Fetcher, MonitorRequest};
use serde_json::json;

struct SeqFetcher {
    values: Vec<serde_json::Value>,
    idx: RefCell<usize>,
}

#[async_trait(?Send)]
impl Fetcher for SeqFetcher {
    async fn fetch(&self, _url: &str, _opts: FetchOpts<'_>) -> Result<FetchedPage, String> {
        let i = *self.idx.borrow();
        *self.idx.borrow_mut() = i + 1;
        let v = self.values.get(i).cloned().unwrap_or(serde_json::Value::Null);
        // The monitor's combined eval returns {value: ...}; mirror that shape.
        Ok(FetchedPage {
            final_url: "http://x/".to_string(),
            html: String::new(),
            title: String::new(),
            eval: Some(json!({ "value": v })),
        })
    }
}

fn req(max_runs: u64) -> MonitorRequest {
    MonitorRequest {
        url: "http://x/".into(),
        interval: Some(1),
        max_runs: Some(max_runs),
        ..Default::default()
    }
}

#[tokio::test]
async fn emits_on_change_and_dedupes_repeats() {
    // A, A (dup), B -> two events (A at run 1, B at run 3).
    let fetcher = SeqFetcher {
        values: vec![json!("A"), json!("A"), json!("B")],
        idx: RefCell::new(0),
    };
    let mut sink = CollectSink::default();
    run_monitor(&req(3), &fetcher, &mut sink, None).await;

    assert_eq!(sink.records.len(), 2, "records: {:?}", sink.records);
    assert_eq!(sink.records[0]["value"], json!("A"));
    assert_eq!(sink.records[0]["run"], 1);
    assert_eq!(sink.records[1]["value"], json!("B"));
    assert_eq!(sink.records[1]["run"], 3);
    // Each event carries a stable hash and no error.
    assert!(sink.records[0]["hash"].is_string());
    assert!(sink.records[0].get("error").is_none());
}

#[tokio::test]
async fn first_value_is_a_baseline_event() {
    let fetcher = SeqFetcher {
        values: vec![json!({ "n": 1 })],
        idx: RefCell::new(0),
    };
    let mut sink = CollectSink::default();
    run_monitor(&req(1), &fetcher, &mut sink, None).await;
    assert_eq!(sink.records.len(), 1);
    assert_eq!(sink.records[0]["value"], json!({ "n": 1 }));
}

struct ErrFetcher;
#[async_trait(?Send)]
impl Fetcher for ErrFetcher {
    async fn fetch(&self, _url: &str, _opts: FetchOpts<'_>) -> Result<FetchedPage, String> {
        Err("navigate failed: boom".to_string())
    }
}

#[tokio::test]
async fn navigation_error_is_reported_as_event() {
    let mut sink = CollectSink::default();
    run_monitor(&req(1), &ErrFetcher, &mut sink, None).await;
    assert_eq!(sink.records.len(), 1);
    assert!(sink.records[0]["error"].as_str().unwrap().contains("boom"));
}
