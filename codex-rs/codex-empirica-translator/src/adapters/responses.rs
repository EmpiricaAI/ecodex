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

/// Encode a single CIF StreamEvent as Responses-format SSE bytes.
/// Returns `None` for events that don't produce a wire emission for this
/// adapter (e.g. ReasoningDelta when the downstream codex didn't ask for
/// reasoning visibility).
pub fn encode_event(event: &StreamEvent) -> Option<Vec<u8>> {
    let (event_name, payload) = match event {
        StreamEvent::TextDelta { text } if !text.is_empty() => (
            "response.output_text.delta",
            json!({"type": "response.output_text.delta", "delta": text}),
        ),
        StreamEvent::TextDelta { .. } => return None,
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
            ("response.function_call.delta", payload)
        }
        StreamEvent::ReasoningDelta { text } => (
            "response.reasoning.delta",
            json!({"type": "response.reasoning.delta", "delta": text}),
        ),
        StreamEvent::Completed {
            text,
            tool_calls,
            finish_reason,
            response_id,
        } => {
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
            (
                "response.completed",
                json!({
                    "type": "response.completed",
                    "response": {
                        "id": response_id.clone().unwrap_or_default(),
                        "status": "completed",
                        "finish_reason": finish_reason_str(finish_reason),
                        "output": output,
                    },
                }),
            )
        }
        StreamEvent::Error { message } => (
            "response.error",
            json!({"type": "response.error", "message": message}),
        ),
    };

    let body = serde_json::to_string(&payload).ok()?;
    Some(format!("event: {event_name}\ndata: {body}\n\n").into_bytes())
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
}
