//! JSON-to-JSON translation between Responses API and Chat Completions wire formats.
//!
//! Field mappings derived from the resurrected reference implementations in
//! `vendored/chat_request.rs` and `vendored/chat_sse.rs` (originally
//! codex-api at commit d2394a2494^).
//!
//! Phase 2 scope: text streaming + system instructions + basic message types.
//! Phase 3 will add: tool/function calling, reasoning items, image content,
//! request_max_retries / stream_max_retries handling.

use anyhow::{Context, Result};
use serde_json::{json, Value};

/// Translate a Responses-API request body into a Chat-Completions request body.
///
/// Responses input chain → flat `messages` array.
/// `instructions` field → leading `system` message.
pub fn responses_to_chat_request(responses_req: &Value) -> Result<Value> {
    let model = responses_req
        .get("model")
        .and_then(Value::as_str)
        .context("Responses request missing required field 'model'")?;

    let mut messages: Vec<Value> = Vec::new();

    // `instructions` becomes the leading system message
    if let Some(instructions) = responses_req.get("instructions").and_then(Value::as_str) {
        if !instructions.is_empty() {
            messages.push(json!({
                "role": "system",
                "content": instructions,
            }));
        }
    }

    // Translate each input item
    let input = responses_req
        .get("input")
        .and_then(Value::as_array)
        .context("Responses request missing or non-array 'input'")?;

    for item in input {
        if let Some(msg) = translate_input_item(item)? {
            messages.push(msg);
        }
    }

    let mut chat_req = json!({
        "model": model,
        "messages": messages,
        "stream": responses_req.get("stream").and_then(Value::as_bool).unwrap_or(true),
    });

    // Pass through tools if present (function-calling support — Phase 3 polishes the shape)
    if let Some(tools) = responses_req.get("tools") {
        chat_req["tools"] = tools.clone();
    }
    if let Some(temp) = responses_req.get("temperature") {
        chat_req["temperature"] = temp.clone();
    }
    if let Some(max_tokens) = responses_req.get("max_output_tokens") {
        chat_req["max_tokens"] = max_tokens.clone();
    }

    Ok(chat_req)
}

/// Translate one Responses input item into a Chat Completions message (or None to skip).
fn translate_input_item(item: &Value) -> Result<Option<Value>> {
    let item_type = item.get("type").and_then(Value::as_str).unwrap_or("message");

    match item_type {
        "message" => {
            let role = item
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("user");
            let content = extract_content_text(item.get("content"));
            Ok(Some(json!({
                "role": role,
                "content": content,
            })))
        }
        "function_call" => {
            // Responses: {type:"function_call", call_id, name, arguments}
            // Chat:     assistant message with tool_calls array
            let call_id = item.get("call_id").and_then(Value::as_str).unwrap_or("");
            let name = item.get("name").and_then(Value::as_str).unwrap_or("");
            let arguments = item.get("arguments").and_then(Value::as_str).unwrap_or("{}");
            Ok(Some(json!({
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": call_id,
                    "type": "function",
                    "function": { "name": name, "arguments": arguments },
                }],
            })))
        }
        "function_call_output" => {
            // Responses: {type:"function_call_output", call_id, output}
            // Chat:     {role:"tool", tool_call_id, content}
            let call_id = item.get("call_id").and_then(Value::as_str).unwrap_or("");
            let output = item
                .get("output")
                .map(serialize_output)
                .unwrap_or_default();
            Ok(Some(json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": output,
            })))
        }
        "reasoning" => {
            // Chat completions has no place for reasoning items — drop them.
            // (This is the lossy direction of the translation. Reasoning round-trip
            // requires Responses-native providers, which is the whole point of
            // the dual-track support David called out.)
            Ok(None)
        }
        _ => Ok(None),
    }
}

/// Extract a flat text string from a Responses content array.
/// Phase 2: text only. Phase 3: image, audio, etc.
fn extract_content_text(content: Option<&Value>) -> String {
    let Some(content) = content else { return String::new() };

    if let Some(s) = content.as_str() {
        return s.to_string();
    }

    if let Some(arr) = content.as_array() {
        let mut out = String::new();
        for piece in arr {
            let ptype = piece.get("type").and_then(Value::as_str).unwrap_or("");
            match ptype {
                "input_text" | "output_text" | "text" => {
                    if let Some(t) = piece.get("text").and_then(Value::as_str) {
                        out.push_str(t);
                    }
                }
                _ => {} // image / audio / etc — Phase 3
            }
        }
        return out;
    }

    String::new()
}

fn serialize_output(v: &Value) -> String {
    v.as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| serde_json::to_string(v).unwrap_or_default())
}

/// Translate one Chat-Completions SSE chunk into zero or more Responses-format SSE events.
///
/// Returns the bytes to forward (already formatted as SSE `event:`+`data:` lines
/// terminated by `\n\n`), or empty if the chunk produces nothing for the
/// Responses stream.
pub fn chat_chunk_to_responses_sse(
    chat_chunk_data: &str,
    state: &mut StreamState,
) -> Result<Vec<u8>> {
    // Handle the [DONE] sentinel
    if chat_chunk_data.trim() == "[DONE]" || chat_chunk_data.trim() == "DONE" {
        let response_id = state.response_id.clone();
        let completed = json!({
            "type": "response.completed",
            "response": {
                "id": response_id,
                "status": "completed",
                "output": [{
                    "type": "message",
                    "role": "assistant",
                    "content": [{ "type": "output_text", "text": state.accumulated_text }],
                }],
            },
        });
        return Ok(format_sse_event("response.completed", &completed));
    }

    let chunk: Value = serde_json::from_str(chat_chunk_data)
        .with_context(|| format!("malformed chat completion chunk: {chat_chunk_data}"))?;

    // Capture response id from the first chunk that has one
    if state.response_id.is_empty() {
        if let Some(id) = chunk.get("id").and_then(Value::as_str) {
            state.response_id = id.to_string();
        }
    }

    let choices = chunk
        .get("choices")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::new();
    for choice in choices {
        let delta = choice.get("delta").cloned().unwrap_or(json!({}));

        if let Some(text) = delta.get("content").and_then(Value::as_str) {
            if !text.is_empty() {
                state.accumulated_text.push_str(text);
                let event = json!({
                    "type": "response.output_text.delta",
                    "delta": text,
                });
                out.extend(format_sse_event("response.output_text.delta", &event));
            }
        }

        // Tool call deltas — Phase 3 polishes the assembly logic.
        if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for tc in tool_calls {
                let event = json!({
                    "type": "response.function_call.delta",
                    "delta": tc,
                });
                out.extend(format_sse_event("response.function_call.delta", &event));
            }
        }
    }

    Ok(out)
}

fn format_sse_event(event_name: &str, payload: &Value) -> Vec<u8> {
    let body = serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_string());
    format!("event: {event_name}\ndata: {body}\n\n").into_bytes()
}

/// Per-request streaming state — accumulates id + text so the final
/// `response.completed` event has the full output.
#[derive(Default, Debug)]
pub struct StreamState {
    pub response_id: String,
    pub accumulated_text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instructions_become_system_message() {
        let req = json!({
            "model": "deepseek-chat",
            "instructions": "you are helpful",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "hi"}],
            }],
        });
        let chat = responses_to_chat_request(&req).unwrap();
        let messages = chat["messages"].as_array().unwrap();
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"], "you are helpful");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[1]["content"], "hi");
    }

    #[test]
    fn function_call_output_maps_to_tool_message() {
        let req = json!({
            "model": "x",
            "input": [{
                "type": "function_call_output",
                "call_id": "c_42",
                "output": "result",
            }],
        });
        let chat = responses_to_chat_request(&req).unwrap();
        let m = &chat["messages"][0];
        assert_eq!(m["role"], "tool");
        assert_eq!(m["tool_call_id"], "c_42");
        assert_eq!(m["content"], "result");
    }

    #[test]
    fn text_delta_chunk_translates() {
        let mut state = StreamState::default();
        let chunk = r#"{"id":"abc","choices":[{"delta":{"content":"Hel"}}]}"#;
        let bytes = chat_chunk_to_responses_sse(chunk, &mut state).unwrap();
        let s = String::from_utf8(bytes).unwrap();
        assert!(s.contains("response.output_text.delta"));
        assert!(s.contains("\"delta\":\"Hel\""));
        assert_eq!(state.response_id, "abc");
        assert_eq!(state.accumulated_text, "Hel");
    }

    #[test]
    fn done_sentinel_emits_completed_event() {
        let mut state = StreamState {
            response_id: "xyz".into(),
            accumulated_text: "hello world".into(),
        };
        let bytes = chat_chunk_to_responses_sse("[DONE]", &mut state).unwrap();
        let s = String::from_utf8(bytes).unwrap();
        assert!(s.contains("response.completed"));
        assert!(s.contains("hello world"));
    }
}
