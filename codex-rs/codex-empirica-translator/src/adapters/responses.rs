//! Responses API adapter (codex-side of the translator).
//!
//! `parse_request`: incoming Responses-format JSON from codex → CIF Request.
//! `encode_event`:  CIF StreamEvent → Responses-format SSE bytes.

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::cif::{Content, FinishReason, Message, Request, StreamEvent, Tool, ToolCall};

/// Parse an incoming Responses-format request body into CIF.
pub fn parse_request(body: &Value) -> Result<Request> {
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .context("Responses request missing required field 'model'")?
        .to_string();

    let system = body
        .get("instructions")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let mut messages = Vec::new();
    let input = body
        .get("input")
        .and_then(Value::as_array)
        .context("Responses request missing or non-array 'input'")?;
    for item in input {
        if let Some(m) = parse_input_item(item)? {
            messages.push(m);
        }
    }

    let tools = body
        .get("tools")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(parse_tool).collect())
        .unwrap_or_default();

    Ok(Request {
        model,
        system,
        messages,
        tools,
        temperature: body.get("temperature").and_then(Value::as_f64).map(|v| v as f32),
        max_output_tokens: body
            .get("max_output_tokens")
            .and_then(Value::as_u64)
            .map(|v| v as u32),
        stream: body.get("stream").and_then(Value::as_bool).unwrap_or(true),
    })
}

fn parse_input_item(item: &Value) -> Result<Option<Message>> {
    let item_type = item.get("type").and_then(Value::as_str).unwrap_or("message");
    match item_type {
        "message" => {
            let role = item.get("role").and_then(Value::as_str).unwrap_or("user");
            let content = parse_content(item.get("content"));
            Ok(Some(match role {
                "user" => Message::User { content },
                "assistant" => Message::Assistant {
                    content,
                    tool_calls: Vec::new(),
                },
                _ => Message::User { content }, // unknown role → treat as user
            }))
        }
        "function_call" => {
            let id = item.get("call_id").and_then(Value::as_str).unwrap_or("").to_string();
            let name = item.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            let arguments = item.get("arguments").and_then(Value::as_str).unwrap_or("{}").to_string();
            Ok(Some(Message::Assistant {
                content: Vec::new(),
                tool_calls: vec![ToolCall { id, name, arguments }],
            }))
        }
        "function_call_output" => {
            let tool_call_id = item.get("call_id").and_then(Value::as_str).unwrap_or("").to_string();
            let content = item
                .get("output")
                .map(serialize_output)
                .unwrap_or_default();
            Ok(Some(Message::Tool { tool_call_id, content }))
        }
        "reasoning" => {
            let content = item
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let summary = item
                .get("summary")
                .and_then(Value::as_str)
                .map(str::to_string);
            Ok(Some(Message::Reasoning { content, summary }))
        }
        _ => Ok(None),
    }
}

fn parse_content(value: Option<&Value>) -> Vec<Content> {
    let Some(v) = value else {
        return Vec::new();
    };
    if let Some(s) = v.as_str() {
        return vec![Content::Text { text: s.to_string() }];
    }
    if let Some(arr) = v.as_array() {
        return arr
            .iter()
            .filter_map(|piece| {
                let ptype = piece.get("type").and_then(Value::as_str).unwrap_or("");
                match ptype {
                    "input_text" | "output_text" | "text" => piece
                        .get("text")
                        .and_then(Value::as_str)
                        .map(|t| Content::Text { text: t.to_string() }),
                    "input_image" | "image" | "image_url" => {
                        let url = piece
                            .get("image_url")
                            .and_then(|u| u.get("url"))
                            .or_else(|| piece.get("url"))
                            .and_then(Value::as_str)?
                            .to_string();
                        let mime = piece
                            .get("mime_type")
                            .and_then(Value::as_str)
                            .map(str::to_string);
                        Some(Content::Image { url, mime })
                    }
                    _ => None,
                }
            })
            .collect();
    }
    Vec::new()
}

fn parse_tool(t: &Value) -> Option<Tool> {
    // Responses tool shape: {type:"function", name, description, parameters}
    let name = t.get("name").and_then(Value::as_str)?.to_string();
    let description = t.get("description").and_then(Value::as_str).map(str::to_string);
    let parameters = t.get("parameters").cloned().unwrap_or(json!({}));
    Some(Tool { name, description, parameters })
}

fn serialize_output(v: &Value) -> String {
    v.as_str()
        .map(str::to_string)
        .unwrap_or_else(|| serde_json::to_string(v).unwrap_or_default())
}

// ─────────────────────────── ENCODE PATH ───────────────────────────────

/// Per-stream state carried across `encode_event` calls. Tracks whether a
/// `response.output_item.added` (assistant message) has been emitted for the
/// current turn so we know when to open / close it.
///
/// Codex's Responses-format parser (`session/turn.rs:OutputTextDelta`) requires
/// that every `response.output_text.delta` arrive while an active item is
/// open — that item is established by `response.output_item.added`. Without
/// the open, codex errors `OutputTextDelta without active item` and silently
/// drops the assistant text. The state machine here ensures the open/close
/// envelope wraps every delta sequence.
#[derive(Default)]
pub struct EncoderState {
    /// Some(id) once a `response.output_item.added` for the assistant message
    /// has been emitted in the current stream. Cleared when the matching
    /// `response.output_item.done` is emitted (at Completed).
    message_item_id: Option<String>,
    /// Accumulated text so the closing `response.output_item.done` carries
    /// the full content (codex's parser uses this to seed the item's text).
    accumulated_text: String,
    /// Counter for generating unique message ids per stream. The id format
    /// matches codex's test fixtures (`msg-N`); codex doesn't validate the
    /// format strictly, but stable ids help log readability.
    next_message_seq: u32,
}

impl EncoderState {
    fn open_message_item(&mut self) -> String {
        self.next_message_seq += 1;
        let id = format!("msg-{}", self.next_message_seq);
        self.message_item_id = Some(id.clone());
        id
    }
}

/// Format a single SSE frame.
fn sse_frame(event_name: &str, payload: &Value) -> Vec<u8> {
    let body = serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_string());
    format!("event: {event_name}\ndata: {body}\n\n").into_bytes()
}

/// Encode a CIF StreamEvent as a sequence of Responses-format SSE frames.
///
/// Returns zero or more frames per call. Most events produce exactly one
/// frame; the FIRST `TextDelta` in a stream produces TWO (an opening
/// `response.output_item.added` followed by the actual delta), and
/// `Completed` produces TWO (a closing `response.output_item.done` followed
/// by `response.completed`) when an item was opened during the stream.
///
/// Empty `TextDelta` events produce zero frames (filter no-op).
pub fn encode_events(event: &StreamEvent, state: &mut EncoderState) -> Vec<Vec<u8>> {
    let mut frames: Vec<Vec<u8>> = Vec::new();

    match event {
        StreamEvent::TextDelta { text } if !text.is_empty() => {
            // Open an assistant-message item if this is the first text-delta
            // in the stream. Codex needs the item before any delta to set
            // active_item; without it, deltas error as "OutputTextDelta
            // without active item" and silently drop.
            if state.message_item_id.is_none() {
                let id = state.open_message_item();
                let added = json!({
                    "type": "response.output_item.added",
                    "item": {
                        "type": "message",
                        "role": "assistant",
                        "id": id,
                        "content": []
                    }
                });
                frames.push(sse_frame("response.output_item.added", &added));
            }
            state.accumulated_text.push_str(text);
            let payload = json!({"type": "response.output_text.delta", "delta": text});
            frames.push(sse_frame("response.output_text.delta", &payload));
        }
        StreamEvent::TextDelta { .. } => {} // empty text — filter
        StreamEvent::ToolCallDelta {
            index,
            id,
            name,
            arguments_delta,
        } => {
            let mut payload = json!({
                "type": "response.function_call.delta",
                "index": index,
            });
            if let Some(id) = id {
                payload["id"] = json!(id);
            }
            if let Some(name) = name {
                payload["name"] = json!(name);
            }
            if let Some(args) = arguments_delta {
                payload["arguments_delta"] = json!(args);
            }
            frames.push(sse_frame("response.function_call.delta", &payload));
        }
        StreamEvent::ReasoningDelta { text } => {
            let payload =
                json!({"type": "response.reasoning.delta", "delta": text});
            frames.push(sse_frame("response.reasoning.delta", &payload));
        }
        StreamEvent::Completed {
            text,
            tool_calls,
            finish_reason,
            response_id,
        } => {
            // Close the assistant-message item if one was opened during this
            // stream. The done event carries the full accumulated text so
            // codex can seed the item content.
            if let Some(id) = state.message_item_id.take() {
                let final_text = if !text.is_empty() {
                    text.clone()
                } else {
                    std::mem::take(&mut state.accumulated_text)
                };
                let done = json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "message",
                        "role": "assistant",
                        "id": id,
                        "content": [{"type": "output_text", "text": final_text}]
                    }
                });
                frames.push(sse_frame("response.output_item.done", &done));
            }
            // ecodex T81 fix (tool-call SSE envelope): for each tool_call,
            // emit a complete output_item.added/done lifecycle BEFORE the
            // response.completed envelope. Without this, codex's parser sees
            // function_call.delta events with no active item (since codex
            // sets active_tool_argument_diff_consumer in OutputItemAdded only
            // when the item is a CustomToolCall or FunctionCall), and the
            // tool calls silently drop. Symptom: model says "I'll run X" then
            // hangs because the dispatched call never reached codex's tool
            // runtime. Diagnosis case: Kimi sent two exec_command calls,
            // event-tap captured them, codex saw nothing actionable.
            //
            // Item shape matches codex/tests/common/responses.rs::ev_function_call:
            // {"type":"function_call","call_id":..,"name":..,"arguments":..}
            for tc in tool_calls {
                let added = json!({
                    "type": "response.output_item.added",
                    "item": {
                        "type": "function_call",
                        "call_id": tc.id,
                        "name": tc.name,
                        "arguments": "",
                    }
                });
                frames.push(sse_frame("response.output_item.added", &added));
                let done = json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "function_call",
                        "call_id": tc.id,
                        "name": tc.name,
                        "arguments": tc.arguments,
                    }
                });
                frames.push(sse_frame("response.output_item.done", &done));
            }
            let mut output = vec![json!({
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": text}],
            })];
            for tc in tool_calls {
                output.push(json!({
                    "type": "function_call",
                    "call_id": tc.id,
                    "name": tc.name,
                    "arguments": tc.arguments,
                }));
            }
            let payload = json!({
                "type": "response.completed",
                "response": {
                    "id": response_id.clone().unwrap_or_default(),
                    "status": "completed",
                    "finish_reason": finish_reason_str(finish_reason),
                    "output": output,
                },
            });
            frames.push(sse_frame("response.completed", &payload));
        }
        StreamEvent::Error { message } => {
            let payload = json!({"type": "response.error", "message": message});
            frames.push(sse_frame("response.error", &payload));
        }
    }

    frames
}

/// Backwards-compatible single-frame encoder. Returns the LAST frame produced
/// by `encode_events` if any (most events produce exactly one frame). Kept
/// for existing call sites (e.g. tests) that don't carry stream state.
///
/// New call sites in the streaming server path should use `encode_events`
/// with an `EncoderState` so the open/close item envelope is emitted
/// correctly. This single-frame variant cannot emit the
/// `response.output_item.added` that the FIRST text-delta needs, so it's
/// only suitable for unit tests that don't exercise the active-item
/// requirement.
#[cfg(test)]
pub fn encode_event(event: &StreamEvent) -> Option<Vec<u8>> {
    let mut state = EncoderState::default();
    let frames = encode_events(event, &mut state);
    // For TextDelta, encode_events produces [output_item.added, text.delta] —
    // the test wants the delta itself, so return the last frame. For other
    // events, there's exactly one frame so .last() is always correct.
    frames.into_iter().last()
}

fn finish_reason_str(fr: &FinishReason) -> &str {
    match fr {
        FinishReason::Stop => "stop",
        FinishReason::Length => "length",
        FinishReason::ToolCalls => "tool_calls",
        FinishReason::ContentFilter => "content_filter",
        FinishReason::Other(s) => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_user_message() {
        let req = json!({
            "model": "deepseek-chat",
            "instructions": "be helpful",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "hello"}],
            }],
        });
        let cif = parse_request(&req).unwrap();
        assert_eq!(cif.model, "deepseek-chat");
        assert_eq!(cif.system.as_deref(), Some("be helpful"));
        assert_eq!(cif.messages.len(), 1);
        match &cif.messages[0] {
            Message::User { content } => match &content[0] {
                Content::Text { text } => assert_eq!(text, "hello"),
                _ => panic!("expected text content"),
            },
            _ => panic!("expected user message"),
        }
    }

    #[test]
    fn parses_function_call_output_to_tool_message() {
        let req = json!({
            "model": "x",
            "input": [{"type": "function_call_output", "call_id": "c_1", "output": "result"}],
        });
        let cif = parse_request(&req).unwrap();
        assert!(matches!(cif.messages[0], Message::Tool { .. }));
    }

    #[test]
    fn encodes_text_delta_as_responses_sse() {
        let bytes = encode_event(&StreamEvent::TextDelta { text: "Hi".into() }).unwrap();
        let s = String::from_utf8(bytes).unwrap();
        assert!(s.starts_with("event: response.output_text.delta\n"));
        assert!(s.contains("\"delta\":\"Hi\""));
    }

    #[test]
    fn encodes_completed_event() {
        let bytes = encode_event(&StreamEvent::Completed {
            text: "done".into(),
            tool_calls: vec![],
            finish_reason: FinishReason::Stop,
            response_id: Some("r_1".into()),
        })
        .unwrap();
        let s = String::from_utf8(bytes).unwrap();
        assert!(s.contains("response.completed"));
        assert!(s.contains("\"finish_reason\":\"stop\""));
        assert!(s.contains("\"id\":\"r_1\""));
    }

    /// Regression: codex's parser errors "OutputTextDelta without active item"
    /// if a text-delta arrives before the matching `response.output_item.added`.
    /// `encode_events` must wrap the first text-delta in an item.added, and
    /// emit a closing item.done before `response.completed`. Diagnosis canon:
    /// translator caused silent-quit on Kimi responses because deltas reached
    /// codex without an open item — content was visible in raw logs but
    /// dropped by the parser.
    #[test]
    fn encode_events_wraps_text_deltas_with_open_close_item_envelope() {
        let mut state = EncoderState::default();

        let frames_a = encode_events(&StreamEvent::TextDelta { text: "Hello".into() }, &mut state);
        let frames_b = encode_events(&StreamEvent::TextDelta { text: " world".into() }, &mut state);
        let frames_c = encode_events(
            &StreamEvent::Completed {
                text: "Hello world".into(),
                tool_calls: vec![],
                finish_reason: FinishReason::Stop,
                response_id: Some("r-1".into()),
            },
            &mut state,
        );

        let s = |fs: &[Vec<u8>]| -> Vec<String> {
            fs.iter()
                .map(|f| String::from_utf8(f.clone()).unwrap())
                .collect()
        };
        let a = s(&frames_a);
        let b = s(&frames_b);
        let c = s(&frames_c);

        // First text-delta produces TWO frames: the item.added envelope, then
        // the actual delta. Order matters — codex requires the item open
        // before any delta references its slot.
        assert_eq!(a.len(), 2, "first text-delta must emit item.added + delta");
        assert!(
            a[0].starts_with("event: response.output_item.added\n"),
            "first frame must be item.added; got: {}",
            a[0]
        );
        assert!(
            a[0].contains("\"type\":\"message\"")
                && a[0].contains("\"role\":\"assistant\""),
            "item.added must declare an assistant message item; got: {}",
            a[0]
        );
        assert!(
            a[1].starts_with("event: response.output_text.delta\n"),
            "second frame must be text.delta; got: {}",
            a[1]
        );

        // Subsequent text-deltas produce ONE frame — item is already open.
        assert_eq!(b.len(), 1, "subsequent text-delta must emit only the delta");
        assert!(b[0].starts_with("event: response.output_text.delta\n"));

        // Completed produces TWO frames: item.done envelope (carrying full
        // text) + response.completed. Same order requirement, reversed.
        assert_eq!(c.len(), 2, "completed must emit item.done + response.completed");
        assert!(
            c[0].starts_with("event: response.output_item.done\n"),
            "first frame must be item.done; got: {}",
            c[0]
        );
        assert!(
            c[0].contains("\"text\":\"Hello world\""),
            "item.done must carry the full accumulated text; got: {}",
            c[0]
        );
        assert!(
            c[1].starts_with("event: response.completed\n"),
            "second frame must be response.completed; got: {}",
            c[1]
        );

        // After Completed, the encoder state is cleared so a follow-up turn
        // would open a fresh item.
        assert!(
            state.message_item_id.is_none(),
            "Completed must close out the message item from state"
        );
    }

    /// Streams that produce no text (e.g. tool-only turns) must not emit a
    /// stray item.done. Currently we only open an item on the first
    /// text-delta, so this should be naturally true — but guard against
    /// future regressions.
    #[test]
    fn encode_events_no_orphan_item_done_when_no_text() {
        let mut state = EncoderState::default();
        let frames = encode_events(
            &StreamEvent::Completed {
                text: "".into(),
                tool_calls: vec![],
                finish_reason: FinishReason::Stop,
                response_id: Some("r-2".into()),
            },
            &mut state,
        );
        // Only the response.completed frame; no item.done since no item was opened.
        assert_eq!(frames.len(), 1);
        let s = String::from_utf8(frames[0].clone()).unwrap();
        assert!(s.starts_with("event: response.completed\n"));
    }

    /// Regression (ecodex T81): Kimi-style tool-only turns must produce a
    /// complete output_item.added/done lifecycle for each tool_call BEFORE
    /// response.completed. Without this lifecycle, codex's parser silently
    /// drops the tool calls (`active_tool_argument_diff_consumer` is only
    /// set in OutputItemAdded for FunctionCall items), and the agent loop
    /// hangs because the dispatched tools never reached codex's runtime.
    /// Diagnosis case: Kimi sent two exec_command calls via the translator,
    /// event-tap captured them, codex saw nothing actionable.
    #[test]
    fn encode_events_emits_lifecycle_for_each_tool_call_before_completed() {
        use crate::cif::ToolCall;
        let mut state = EncoderState::default();
        let frames = encode_events(
            &StreamEvent::Completed {
                text: "".into(),
                tool_calls: vec![
                    ToolCall {
                        id: "tool_aaa".into(),
                        name: "exec_command".into(),
                        arguments: "{\"cmd\":\"ls\"}".into(),
                    },
                    ToolCall {
                        id: "tool_bbb".into(),
                        name: "exec_command".into(),
                        arguments: "{\"cmd\":\"pwd\"}".into(),
                    },
                ],
                finish_reason: FinishReason::ToolCalls,
                response_id: Some("r-3".into()),
            },
            &mut state,
        );

        let s: Vec<String> = frames
            .iter()
            .map(|f| String::from_utf8(f.clone()).unwrap())
            .collect();

        // Expected order: added(aaa), done(aaa), added(bbb), done(bbb), completed.
        assert_eq!(
            s.len(),
            5,
            "tool-only stream with 2 tool_calls must emit 5 frames \
             (added+done per call + completed); got {}",
            s.len()
        );

        // Frame 0: added for aaa, function_call shape with empty args.
        assert!(s[0].starts_with("event: response.output_item.added\n"), "frame 0 must be added; got: {}", s[0]);
        assert!(s[0].contains("\"type\":\"function_call\""), "frame 0 must declare function_call item; got: {}", s[0]);
        assert!(s[0].contains("\"call_id\":\"tool_aaa\""), "frame 0 must carry call_id; got: {}", s[0]);
        assert!(s[0].contains("\"name\":\"exec_command\""), "frame 0 must carry name; got: {}", s[0]);

        // Frame 1: done for aaa with full arguments.
        assert!(s[1].starts_with("event: response.output_item.done\n"), "frame 1 must be done; got: {}", s[1]);
        assert!(s[1].contains("\"call_id\":\"tool_aaa\""));
        assert!(s[1].contains("\"arguments\":\"{\\\"cmd\\\":\\\"ls\\\"}\""), "frame 1 must carry full arguments; got: {}", s[1]);

        // Frames 2-3: same pair for bbb.
        assert!(s[2].starts_with("event: response.output_item.added\n") && s[2].contains("tool_bbb"));
        assert!(s[3].starts_with("event: response.output_item.done\n") && s[3].contains("tool_bbb") && s[3].contains("pwd"));

        // Frame 4: response.completed (the last frame, after all tool lifecycles).
        assert!(s[4].starts_with("event: response.completed\n"), "frame 4 must be response.completed; got: {}", s[4]);
        assert!(s[4].contains("\"finish_reason\":\"tool_calls\""));
    }

    /// Mixed turn (assistant text AND tool_calls): the message item.done
    /// closes first, then each tool_call lifecycle, then response.completed.
    #[test]
    fn encode_events_mixed_text_and_tools_orders_message_first_then_tools() {
        use crate::cif::ToolCall;
        let mut state = EncoderState::default();

        // Open with a text-delta so the message item gets opened.
        let _ = encode_events(&StreamEvent::TextDelta { text: "Running...".into() }, &mut state);

        // Then a Completed event with both text and tool_calls.
        let frames = encode_events(
            &StreamEvent::Completed {
                text: "Running...".into(),
                tool_calls: vec![ToolCall {
                    id: "tool_ccc".into(),
                    name: "shell".into(),
                    arguments: "{\"cmd\":\"echo hi\"}".into(),
                }],
                finish_reason: FinishReason::ToolCalls,
                response_id: Some("r-4".into()),
            },
            &mut state,
        );

        let s: Vec<String> = frames
            .iter()
            .map(|f| String::from_utf8(f.clone()).unwrap())
            .collect();

        // Order: message done, tool added, tool done, response.completed.
        assert_eq!(s.len(), 4, "expected 4 frames (msg.done + tool.added + tool.done + completed); got {}", s.len());
        assert!(s[0].starts_with("event: response.output_item.done\n") && s[0].contains("\"type\":\"message\""), "frame 0 must close the message item: {}", s[0]);
        assert!(s[1].starts_with("event: response.output_item.added\n") && s[1].contains("\"type\":\"function_call\""), "frame 1 must open the tool: {}", s[1]);
        assert!(s[2].starts_with("event: response.output_item.done\n") && s[2].contains("\"type\":\"function_call\""), "frame 2 must close the tool: {}", s[2]);
        assert!(s[3].starts_with("event: response.completed\n"), "frame 3 must be completed: {}", s[3]);
    }
}
