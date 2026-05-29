//! Event tap: emit CIF stream events + request lifecycle events as JSONL
//! to a configurable sink. Subscribers (Empirica MCP, Cockpit TUI, replay
//! log, Brier-grounded routing) consume by tailing or by reading at end.
//!
//! Per the architecture decision against ChatGPT's "fat epistemic choke
//! point" framing: translator stays thin, observers subscribe externally.
//! All epistemic logic lives in the consumers — translator's only job
//! here is faithfully emitting the timeline of every request.
//!
//! v1: file-based JSONL append (works on all platforms, subscribers
//! can `tail -F` it). v2 may add a Unix-socket pub-sub for live
//! multi-subscriber broadcast — out of scope here.

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{Value, json};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cif::{Request, StreamEvent};

/// One observable event in the translator's timeline. JSON-serialized to
/// the tap as `{ts_ms, event_id, kind, ...}` — kind discriminates the
/// payload shape.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TapEvent {
    /// A request arrived from the codex client and was parsed into CIF.
    /// Includes the model + message count + tool count for cheap subscriber
    /// indexing without re-parsing the body.
    RequestStarted {
        request_id: String,
        model: String,
        adapter: String,
        message_count: usize,
        tool_count: usize,
        upstream_url: String,
    },
    /// One CIF StreamEvent flowed through the translator (any direction).
    StreamEvent {
        request_id: String,
        event: StreamEvent,
    },
    /// Request completed successfully — gives subscribers a clean boundary
    /// for per-request analysis (latency, token volume, finish reason, etc).
    RequestCompleted {
        request_id: String,
        duration_ms: u64,
        text_chars: usize,
        tool_calls_count: usize,
    },
    /// Request errored. `stage` tells you where it died (parse / upstream /
    /// stream-decode / etc.) so subscribers can categorize failures.
    RequestErrored {
        request_id: String,
        stage: String,
        message: String,
        duration_ms: u64,
    },
}

/// A sink for emitted events. Trait so subscribers (Cockpit, Empirica) can
/// later be wired in-process if needed; default impl is JSONL-to-file.
pub trait EventEmitter: Send + Sync {
    fn emit(&self, event: &TapEvent);
}

/// No-op emitter — used when --event-log is not configured.
pub struct NoopEmitter;
impl EventEmitter for NoopEmitter {
    fn emit(&self, _event: &TapEvent) {}
}

/// Append JSONL to a file. One line per event. Subscribers tail it.
pub struct JsonlFileEmitter {
    file: Mutex<File>,
    path: PathBuf,
}

impl JsonlFileEmitter {
    pub fn new(path: PathBuf) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("opening event log at {}", path.display()))?;
        Ok(Self {
            file: Mutex::new(file),
            path,
        })
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl EventEmitter for JsonlFileEmitter {
    fn emit(&self, event: &TapEvent) {
        // Best-effort: a tap failure must NEVER fail the actual translation.
        let mut envelope = match serde_json::to_value(event) {
            Ok(v) => v,
            Err(_) => return,
        };
        if let Value::Object(map) = &mut envelope {
            map.insert("ts_ms".into(), json!(now_ms()));
        }
        let Ok(line) = serde_json::to_string(&envelope) else {
            return;
        };
        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "{line}");
            let _ = f.flush();
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Build a TapEvent::RequestStarted from a parsed CIF request.
pub fn build_request_started(
    request_id: &str,
    req: &Request,
    adapter: &str,
    upstream_url: &str,
) -> TapEvent {
    TapEvent::RequestStarted {
        request_id: request_id.to_string(),
        model: req.model.clone(),
        adapter: adapter.to_string(),
        message_count: req.messages.len(),
        tool_count: req.tools.len(),
        upstream_url: upstream_url.to_string(),
    }
}

/// Generate a per-request id. UUID-ish but cheap (no uuid crate dep).
pub fn new_request_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("req_{nanos:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cif::{Content, FinishReason, Message};
    use std::io::Read;
    use tempfile::tempdir;

    fn sample_request() -> Request {
        Request {
            model: "deepseek-chat".into(),
            system: Some("be helpful".into()),
            messages: vec![Message::User {
                content: vec![Content::Text { text: "hi".into() }],
            }],
            tools: vec![],
            temperature: None,
            max_output_tokens: None,
            stream: true,
        }
    }

    #[test]
    fn jsonl_emitter_writes_request_started_with_model_and_counts() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let emitter = JsonlFileEmitter::new(path.clone()).unwrap();
        let req = sample_request();
        let event = build_request_started("req_1", &req, "chat", "https://api.deepseek.com/v1");
        emitter.emit(&event);

        let mut contents = String::new();
        File::open(&path)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        let line = contents.lines().next().unwrap();
        let parsed: Value = serde_json::from_str(line).unwrap();
        assert_eq!(parsed["kind"], "request_started");
        assert_eq!(parsed["request_id"], "req_1");
        assert_eq!(parsed["model"], "deepseek-chat");
        assert_eq!(parsed["adapter"], "chat");
        assert_eq!(parsed["message_count"], 1);
        assert_eq!(parsed["tool_count"], 0);
        assert!(parsed["ts_ms"].as_u64().unwrap() > 0);
    }

    #[test]
    fn jsonl_emitter_writes_stream_events() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let emitter = JsonlFileEmitter::new(path.clone()).unwrap();
        emitter.emit(&TapEvent::StreamEvent {
            request_id: "req_1".into(),
            event: StreamEvent::TextDelta { text: "Hi".into() },
        });
        emitter.emit(&TapEvent::StreamEvent {
            request_id: "req_1".into(),
            event: StreamEvent::Completed {
                text: "Hi".into(),
                tool_calls: vec![],
                finish_reason: FinishReason::Stop,
                response_id: Some("r_1".into()),
            },
        });

        let mut contents = String::new();
        File::open(&path)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        let lines: Vec<_> = contents.lines().collect();
        assert_eq!(lines.len(), 2);
        let first: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["kind"], "stream_event");
        assert_eq!(first["event"]["type"], "text_delta");
        assert_eq!(first["event"]["text"], "Hi");
        let second: Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(second["event"]["type"], "completed");
        assert_eq!(second["event"]["response_id"], "r_1");
    }

    #[test]
    fn noop_emitter_is_silent() {
        let emitter = NoopEmitter;
        emitter.emit(&TapEvent::RequestStarted {
            request_id: "x".into(),
            model: "y".into(),
            adapter: "z".into(),
            message_count: 0,
            tool_count: 0,
            upstream_url: "".into(),
        });
        // Reaching here without panic = success
    }

    #[test]
    fn request_id_is_unique() {
        let a = new_request_id();
        // Sleep nano via spin (avoids std::thread import bloat)
        for _ in 0..100 {
            std::hint::black_box(0);
        }
        let b = new_request_id();
        assert_ne!(a, b);
        assert!(a.starts_with("req_"));
    }
}
