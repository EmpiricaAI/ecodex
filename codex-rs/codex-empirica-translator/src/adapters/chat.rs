//! Chat Completions adapter (provider-side of the translator).
//!
//! `encode_request`: CIF Request → chat-completions JSON request body.
//! `parse_chunk`:    one chat-completions SSE chunk → CIF StreamEvents.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::cif::{Content, FinishReason, Message, Request, StreamEvent, ToolCall};

/// Encode a CIF Request as a chat-completions request body.
pub fn encode_request(req: &Request) -> Value {
    let mut messages: Vec<Value> = Vec::new();

    if let Some(system) = &req.system {
        messages.push(json!({"role": "system", "content": system}));
    }

    for m in &req.messages {
        match m {
            Message::User { content } => {
                messages.push(json!({"role": "user", "content": content_to_chat(content)}));
            }
            Message::Assistant {
                content,
                tool_calls,
            } => {
                let mut msg = json!({"role": "assistant"});
                let text = flatten_text(content);
                if !text.is_empty() {
                    msg["content"] = json!(text);
                } else {
                    msg["content"] = Value::Null;
                }
                if !tool_calls.is_empty() {
                    msg["tool_calls"] = json!(
                        tool_calls
                            .iter()
                            .map(|tc| json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {"name": tc.name, "arguments": tc.arguments},
                            }))
                            .collect::<Vec<_>>()
                    );
                }
                messages.push(msg);
            }
            Message::Tool {
                tool_call_id,
                content,
            } => {
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": tool_call_id,
                    "content": content,
                }));
            }
            Message::Reasoning { .. } => {
                // Drop reasoning items — chat-completions has no slot for them.
                // (Lossy by design; reasoning round-trip needs a Responses-native
                // provider, which is the dual-track support raison d'être.)
            }
        }
    }

    let mut out = json!({
        "model": req.model,
        "messages": messages,
        "stream": req.stream,
    });
    if !req.tools.is_empty() {
        out["tools"] = json!(
            req.tools
                .iter()
                .map(|t| json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    },
                }))
                .collect::<Vec<_>>()
        );
    }
    if let Some(t) = req.temperature {
        out["temperature"] = json!(t);
    }
    if let Some(m) = req.max_output_tokens {
        out["max_tokens"] = json!(m);
    }
    out
}

fn content_to_chat(content: &[Content]) -> Value {
    // If it's a single text item, emit as plain string. Else multi-part array.
    if content.len() == 1
        && let Content::Text { text } = &content[0]
    {
        return json!(text);
    }
    let parts: Vec<Value> = content
        .iter()
        .map(|c| match c {
            Content::Text { text } => json!({"type": "text", "text": text}),
            Content::Image { url, .. } => json!({
                "type": "image_url",
                "image_url": {"url": url},
            }),
        })
        .collect();
    json!(parts)
}

fn flatten_text(content: &[Content]) -> String {
    let mut out = String::new();
    for c in content {
        if let Content::Text { text } = c {
            out.push_str(text);
        }
    }
    out
}

// ─────────────────────────── PARSE PATH ────────────────────────────────

/// Parse one chat-completions SSE `data: ...` payload into zero-or-more CIF
/// stream events. The state accumulator stitches deltas across chunks.
pub fn parse_chunk(data: &str, state: &mut ChunkState) -> Result<Vec<StreamEvent>> {
    let trimmed = data.trim();
    if trimmed == "[DONE]" || trimmed == "DONE" {
        return Ok(vec![StreamEvent::Completed {
            text: std::mem::take(&mut state.text),
            tool_calls: std::mem::take(&mut state.tool_calls),
            finish_reason: state.finish_reason.take().unwrap_or(FinishReason::Stop),
            response_id: state.response_id.clone(),
        }]);
    }

    let chunk: Value = serde_json::from_str(trimmed)
        .with_context(|| format!("malformed chat completion chunk: {trimmed}"))?;

    if state.response_id.is_none()
        && let Some(id) = chunk.get("id").and_then(Value::as_str)
    {
        state.response_id = Some(id.to_string());
    }

    let mut events = Vec::new();
    for choice in chunk
        .get("choices")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        if let Some(fr) = choice.get("finish_reason").and_then(Value::as_str) {
            state.finish_reason = Some(FinishReason::from_chat_str(fr));
        }
        let delta = choice.get("delta").cloned().unwrap_or(json!({}));

        if let Some(text) = delta.get("content").and_then(Value::as_str)
            && !text.is_empty()
        {
            state.text.push_str(text);
            events.push(StreamEvent::TextDelta {
                text: text.to_string(),
            });
        }

        if let Some(tcs) = delta.get("tool_calls").and_then(Value::as_array) {
            for tc in tcs {
                let index = tc.get("index").and_then(Value::as_u64).unwrap_or(0) as u32;
                let id = tc.get("id").and_then(Value::as_str).map(str::to_string);
                let function = tc.get("function");
                let name = function
                    .and_then(|f| f.get("name"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let args_delta = function
                    .and_then(|f| f.get("arguments"))
                    .and_then(Value::as_str)
                    .map(str::to_string);

                // Maintain accumulator entry for this tool-call index
                while state.tool_calls.len() <= index as usize {
                    state.tool_calls.push(ToolCall::default_for_index());
                }
                let acc = &mut state.tool_calls[index as usize];
                if let Some(id) = &id
                    && !id.is_empty()
                {
                    acc.id = id.clone();
                }
                if let Some(name) = &name
                    && !name.is_empty()
                {
                    acc.name = name.clone();
                }
                if let Some(args) = &args_delta {
                    acc.arguments.push_str(args);
                }

                events.push(StreamEvent::ToolCallDelta {
                    index,
                    id,
                    name,
                    arguments_delta: args_delta,
                });
            }
        }
    }

    Ok(events)
}

#[derive(Default, Debug)]
pub struct ChunkState {
    pub response_id: Option<String>,
    /// Accumulated text across deltas (for the final Completed event).
    pub text: String,
    /// Per-index tool-call accumulators. Chat-completions can interleave
    /// fragments of multiple tool calls; we assemble them by index.
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: Option<FinishReason>,
}

impl ToolCall {
    fn default_for_index() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            arguments: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cif::Tool;

    #[test]
    fn encodes_system_and_user() {
        let req = Request {
            model: "x".into(),
            system: Some("you are helpful".into()),
            messages: vec![Message::User {
                content: vec![Content::Text { text: "hi".into() }],
            }],
            tools: vec![],
            temperature: None,
            max_output_tokens: None,
            stream: true,
        };
        let body = encode_request(&req);
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "you are helpful");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["messages"][1]["content"], "hi");
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn encodes_tool_message_correctly() {
        let req = Request {
            model: "x".into(),
            system: None,
            messages: vec![Message::Tool {
                tool_call_id: "c_1".into(),
                content: "result".into(),
            }],
            tools: vec![],
            temperature: None,
            max_output_tokens: None,
            stream: true,
        };
        let body = encode_request(&req);
        assert_eq!(body["messages"][0]["role"], "tool");
        assert_eq!(body["messages"][0]["tool_call_id"], "c_1");
        assert_eq!(body["messages"][0]["content"], "result");
    }

    #[test]
    fn encodes_tools_array() {
        let req = Request {
            model: "x".into(),
            system: None,
            messages: vec![],
            tools: vec![Tool {
                name: "shell".into(),
                description: Some("run shell".into()),
                parameters: json!({"type": "object"}),
            }],
            temperature: None,
            max_output_tokens: None,
            stream: true,
        };
        let body = encode_request(&req);
        assert_eq!(body["tools"][0]["type"], "function");
        assert_eq!(body["tools"][0]["function"]["name"], "shell");
    }

    #[test]
    fn parses_text_delta_chunk() {
        let mut state = ChunkState::default();
        let data = r#"{"id":"abc","choices":[{"delta":{"content":"Hi"}}]}"#;
        let events = parse_chunk(data, &mut state).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::TextDelta { text } => assert_eq!(text, "Hi"),
            _ => panic!("expected text delta"),
        }
        assert_eq!(state.response_id.as_deref(), Some("abc"));
        assert_eq!(state.text, "Hi");
    }

    #[test]
    fn assembles_tool_call_across_chunks() {
        let mut state = ChunkState::default();
        // Chunk 1: tool call name announcement
        let _ = parse_chunk(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"t_1","function":{"name":"shell"}}]}}]}"#,
            &mut state,
        )
        .unwrap();
        // Chunk 2: arguments fragment
        let _ = parse_chunk(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"cmd\":\"ls"}}]}}]}"#,
            &mut state,
        )
        .unwrap();
        // Chunk 3: rest of arguments + finish
        let _ = parse_chunk(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"}"}}]},"finish_reason":"tool_calls"}]}"#,
            &mut state,
        )
        .unwrap();
        // [DONE] flushes
        let final_events = parse_chunk("[DONE]", &mut state).unwrap();
        match &final_events[0] {
            StreamEvent::Completed {
                tool_calls,
                finish_reason,
                ..
            } => {
                assert_eq!(tool_calls.len(), 1);
                assert_eq!(tool_calls[0].id, "t_1");
                assert_eq!(tool_calls[0].name, "shell");
                assert_eq!(tool_calls[0].arguments, r#"{"cmd":"ls"}"#);
                assert!(matches!(finish_reason, FinishReason::ToolCalls));
            }
            _ => panic!("expected completed event"),
        }
    }

    #[test]
    fn done_emits_completed_with_accumulated_text() {
        let mut state = ChunkState {
            response_id: Some("r_1".into()),
            text: "hello world".into(),
            tool_calls: vec![],
            finish_reason: Some(FinishReason::Stop),
        };
        let events = parse_chunk("[DONE]", &mut state).unwrap();
        match &events[0] {
            StreamEvent::Completed {
                text, response_id, ..
            } => {
                assert_eq!(text, "hello world");
                assert_eq!(response_id.as_deref(), Some("r_1"));
            }
            _ => panic!("expected completed event"),
        }
    }
}
