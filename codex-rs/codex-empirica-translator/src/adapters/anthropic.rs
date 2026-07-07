//! Anthropic Messages API adapter (alternative provider-side of the translator).
//!
//! Validates that the CIF design is genuinely neutral by mapping a third
//! provider's wire format with no schema additions.
//!
//! Scope: direct-API path only (api.anthropic.com + ANTHROPIC_API_KEY).
//! TOS-clean third-party client model. Bedrock/Vertex/Claude.ai-subscription
//! routing are explicitly out of scope (separate auth/protocol surfaces).
//!
//! `encode_request`: CIF Request → Anthropic /v1/messages JSON body.
//! `parse_chunk`:    one Anthropic SSE event payload → CIF StreamEvents.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::cif::{Content, FinishReason, Message, Request, StreamEvent, ToolCall};

/// Encode a CIF Request as an Anthropic Messages API request body.
///
/// Anthropic-specific shapes:
///   - `system` is a separate top-level field (not a message turn).
///   - User and assistant messages have `content: [block]` where blocks
///     are `{type: "text", text}` or `{type: "tool_use", id, name, input}`
///     or `{type: "tool_result", tool_use_id, content}`.
///   - Tool definitions use `input_schema` (not `parameters`).
///   - `max_tokens` is required (Anthropic enforces); we default if absent.
pub fn encode_request(req: &Request) -> Value {
    // First pass: convert CIF messages to (role, content-blocks) pairs.
    // Each CIF message produces exactly one (role, blocks) entry — the
    // merging happens in the second pass.
    let mut staged: Vec<(&'static str, Vec<Value>)> = Vec::new();

    for m in &req.messages {
        match m {
            Message::User { content } => {
                let blocks: Vec<Value> = match content_to_anthropic(content) {
                    Value::Array(a) => a,
                    other => vec![other],
                };
                staged.push(("user", blocks));
            }
            Message::Assistant {
                content,
                tool_calls,
            } => {
                let mut blocks = content_to_anthropic_assistant(content);
                for tc in tool_calls {
                    let input: Value =
                        serde_json::from_str(&tc.arguments).unwrap_or_else(|_| json!({}));
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": tc.id,
                        "name": tc.name,
                        "input": input,
                    }));
                }
                staged.push(("assistant", blocks));
            }
            Message::Tool {
                tool_call_id,
                content,
            } => {
                // Anthropic tool results live inside a USER turn, not a tool role.
                staged.push((
                    "user",
                    vec![json!({
                        "type": "tool_result",
                        "tool_use_id": tool_call_id,
                        "content": content,
                    })],
                ));
            }
            Message::Reasoning { content, .. } => {
                // Anthropic round-trips reasoning as `thinking` blocks on
                // assistant turns. CIF stores them as standalone Reasoning
                // messages; the merge pass below folds them into adjacent
                // assistant turns automatically.
                staged.push((
                    "assistant",
                    vec![json!({"type": "thinking", "thinking": content})],
                ));
            }
        }
    }

    // Second pass: merge consecutive same-role messages.
    //
    // Anthropic's /v1/messages API rejects any sequence where two assistant
    // messages are adjacent or two user messages are adjacent (it expects a
    // strict user/assistant alternation). When the model issues parallel
    // tool calls, codex emits one `function_call` ResponseItem per call —
    // each becomes a separate CIF Assistant message — which without merging
    // produces N consecutive assistant entries. Anthropic's validator then
    // complains that the tool_call_ids "did not have response messages",
    // because it can only pair the FIRST assistant's tool_use blocks with
    // the matching user tool_result blocks; the second assistant looks
    // unanswered.
    //
    // Same logic applies to consecutive `function_call_output` items
    // (multiple tool results in a row) — those collapse into a single user
    // message with multiple tool_result content blocks.
    //
    // Diagnosis case: ecodex T81 Tx-S, 2026-05-06. Translator emitted two
    // consecutive assistant messages (one tool_use each) followed by two
    // consecutive user messages (one tool_result each). Kimi rejected with
    // "tool_call_ids did not have response messages: exec_command:0".
    let mut messages: Vec<Value> = Vec::new();
    for (role, blocks) in staged {
        if blocks.is_empty() {
            continue;
        }
        if let Some(last) = messages.last_mut()
            && last.get("role").and_then(Value::as_str) == Some(role)
            && let Some(content_arr) = last.get_mut("content").and_then(|c| c.as_array_mut())
        {
            content_arr.extend(blocks);
            continue;
        }
        messages.push(json!({"role": role, "content": blocks}));
    }

    let mut out = json!({
        "model": req.model,
        "messages": messages,
        "max_tokens": req.max_output_tokens.unwrap_or(4096),
        "stream": req.stream,
    });
    if let Some(system) = &req.system {
        out["system"] = json!(system);
    }
    if !req.tools.is_empty() {
        out["tools"] = json!(
            req.tools
                .iter()
                .map(|t| json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters,
                }))
                .collect::<Vec<_>>()
        );
    }
    if let Some(t) = req.temperature {
        out["temperature"] = json!(t);
    }
    out
}

fn content_to_anthropic(content: &[Content]) -> Value {
    let blocks: Vec<Value> = content
        .iter()
        .map(|c| match c {
            Content::Text { text } => json!({"type": "text", "text": text}),
            Content::Image { url, mime } => json!({
                "type": "image",
                "source": {
                    "type": "url",
                    "media_type": mime.clone().unwrap_or_else(|| "image/jpeg".into()),
                    "url": url,
                },
            }),
        })
        .collect();
    json!(blocks)
}

fn content_to_anthropic_assistant(content: &[Content]) -> Vec<Value> {
    content
        .iter()
        .filter_map(|c| match c {
            Content::Text { text } if !text.is_empty() => {
                Some(json!({"type": "text", "text": text}))
            }
            Content::Text { .. } => None,
            // Assistant turns rarely carry images; skip for safety.
            Content::Image { .. } => None,
        })
        .collect()
}

// ─────────────────────────── PARSE PATH ────────────────────────────────

/// Parse one Anthropic SSE event payload into zero-or-more CIF stream events.
///
/// Anthropic's SSE protocol is more structured than chat-completions. Events:
///   - `message_start`     — gives us the response id
///   - `content_block_start` — starts a block at index N (text|tool_use|thinking)
///   - `content_block_delta` — text_delta | input_json_delta | thinking_delta
///   - `content_block_stop` — closes block N
///   - `message_delta`     — gives us stop_reason
///   - `message_stop`      — final, equivalent to chat's [DONE]
///
/// State must track the "type" of each indexed block so deltas know whether
/// they're text, tool_use, or thinking.
pub fn parse_chunk(payload: &str, state: &mut ChunkState) -> Result<Vec<StreamEvent>> {
    let value: Value = serde_json::from_str(payload)
        .with_context(|| format!("malformed anthropic SSE payload: {payload}"))?;
    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .context("anthropic SSE event missing 'type'")?;

    let mut events = Vec::new();
    match event_type {
        "message_start" => {
            if let Some(id) = value
                .get("message")
                .and_then(|m| m.get("id"))
                .and_then(Value::as_str)
            {
                state.response_id = Some(id.to_string());
            }
        }
        "content_block_start" => {
            let index = value.get("index").and_then(Value::as_u64).unwrap_or(0) as u32;
            let block = value.get("content_block").cloned().unwrap_or(json!({}));
            let block_type = block.get("type").and_then(Value::as_str).unwrap_or("text");
            while state.blocks.len() <= index as usize {
                state.blocks.push(BlockState::default());
            }
            state.blocks[index as usize].kind = block_type.to_string();
            // tool_use block carries its id+name in the start event
            if block_type == "tool_use" {
                let id = block
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let name = block
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                state.blocks[index as usize].tool_id = id.clone();
                state.blocks[index as usize].tool_name = name.clone();
                events.push(StreamEvent::ToolCallDelta {
                    index,
                    id: Some(id),
                    name: Some(name),
                    arguments_delta: None,
                });
            }
        }
        "content_block_delta" => {
            let index = value.get("index").and_then(Value::as_u64).unwrap_or(0) as u32;
            let delta = value.get("delta").cloned().unwrap_or(json!({}));
            let delta_type = delta.get("type").and_then(Value::as_str).unwrap_or("");
            match delta_type {
                "text_delta" => {
                    if let Some(text) = delta.get("text").and_then(Value::as_str)
                        && !text.is_empty() {
                            if let Some(b) = state.blocks.get_mut(index as usize) {
                                b.text.push_str(text);
                            }
                            state.text.push_str(text);
                            events.push(StreamEvent::TextDelta {
                                text: text.to_string(),
                            });
                        }
                }
                "input_json_delta" => {
                    if let Some(partial) = delta.get("partial_json").and_then(Value::as_str) {
                        if let Some(b) = state.blocks.get_mut(index as usize) {
                            b.tool_args.push_str(partial);
                        }
                        events.push(StreamEvent::ToolCallDelta {
                            index,
                            id: None,
                            name: None,
                            arguments_delta: Some(partial.to_string()),
                        });
                    }
                }
                "thinking_delta" => {
                    if let Some(thinking) = delta.get("thinking").and_then(Value::as_str)
                        && !thinking.is_empty() {
                            events.push(StreamEvent::ReasoningDelta {
                                text: thinking.to_string(),
                            });
                        }
                }
                _ => {} // unknown delta type — ignore
            }
        }
        "content_block_stop" => {
            // Could surface a per-block close signal — Phase 4 territory.
        }
        "message_delta" => {
            if let Some(stop) = value
                .get("delta")
                .and_then(|d| d.get("stop_reason"))
                .and_then(Value::as_str)
            {
                state.finish_reason = Some(map_anthropic_stop_reason(stop));
            }
        }
        "message_stop" => {
            // Assemble final tool calls from accumulated block state
            let tool_calls: Vec<ToolCall> = state
                .blocks
                .iter()
                .filter(|b| b.kind == "tool_use")
                .map(|b| ToolCall {
                    id: b.tool_id.clone(),
                    name: b.tool_name.clone(),
                    arguments: if b.tool_args.is_empty() {
                        "{}".into()
                    } else {
                        b.tool_args.clone()
                    },
                })
                .collect();
            events.push(StreamEvent::Completed {
                text: std::mem::take(&mut state.text),
                tool_calls,
                finish_reason: state.finish_reason.take().unwrap_or(FinishReason::Stop),
                response_id: state.response_id.clone(),
            });
        }
        "ping" | "error"
            // ping = keepalive (ignore). error = surface to caller.
            if event_type == "error" => {
                let message = value
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("anthropic stream error")
                    .to_string();
                events.push(StreamEvent::Error { message });
            }
        _ => {} // unknown event — ignore
    }

    Ok(events)
}

fn map_anthropic_stop_reason(s: &str) -> FinishReason {
    match s {
        "end_turn" | "stop_sequence" => FinishReason::Stop,
        "max_tokens" => FinishReason::Length,
        "tool_use" => FinishReason::ToolCalls,
        other => FinishReason::Other(other.to_string()),
    }
}

/// Per-content-block accumulator. Anthropic streams blocks by index; deltas
/// carry the index but not the type, so we have to remember what each index is.
#[derive(Default, Debug, Clone)]
pub struct BlockState {
    /// "text", "tool_use", or "thinking"
    pub kind: String,
    pub text: String,
    pub tool_id: String,
    pub tool_name: String,
    pub tool_args: String,
}

#[derive(Default, Debug)]
pub struct ChunkState {
    pub response_id: Option<String>,
    pub text: String,
    pub blocks: Vec<BlockState>,
    pub finish_reason: Option<FinishReason>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cif::Tool;

    #[test]
    fn encodes_system_separately_not_as_message() {
        let req = Request {
            model: "claude-sonnet-4-5".into(),
            system: Some("be helpful".into()),
            messages: vec![Message::User {
                content: vec![Content::Text { text: "hi".into() }],
            }],
            tools: vec![],
            temperature: None,
            max_output_tokens: Some(1024),
            stream: true,
        };
        let body = encode_request(&req);
        assert_eq!(body["system"], "be helpful");
        assert_eq!(body["max_tokens"], 1024);
        // messages array does NOT have a system entry
        assert_eq!(body["messages"].as_array().unwrap().len(), 1);
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"][0]["type"], "text");
    }

    #[test]
    fn encodes_tools_with_input_schema() {
        let req = Request {
            model: "x".into(),
            system: None,
            messages: vec![],
            tools: vec![Tool {
                name: "shell".into(),
                description: Some("run shell".into()),
                parameters: json!({"type": "object", "properties": {}}),
            }],
            temperature: None,
            max_output_tokens: None,
            stream: true,
        };
        let body = encode_request(&req);
        assert_eq!(body["tools"][0]["name"], "shell");
        assert_eq!(body["tools"][0]["input_schema"]["type"], "object");
        // NOT "parameters"
        assert!(body["tools"][0].get("parameters").is_none());
    }

    #[test]
    fn tool_message_becomes_user_with_tool_result_block() {
        let req = Request {
            model: "x".into(),
            system: None,
            messages: vec![Message::Tool {
                tool_call_id: "t_1".into(),
                content: "result".into(),
            }],
            tools: vec![],
            temperature: None,
            max_output_tokens: None,
            stream: true,
        };
        let body = encode_request(&req);
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"][0]["type"], "tool_result");
        assert_eq!(body["messages"][0]["content"][0]["tool_use_id"], "t_1");
        assert_eq!(body["messages"][0]["content"][0]["content"], "result");
    }

    /// Regression (ecodex T81 Tx-S): parallel tool calls from one assistant
    /// turn must encode as ONE assistant message with multiple tool_use
    /// blocks and ONE following user message with multiple tool_result
    /// blocks — not as N consecutive assistant + N consecutive user
    /// messages. Anthropic's /v1/messages API rejects consecutive same-role
    /// messages with "tool_call_ids did not have response messages".
    /// Diagnosis case 2026-05-06: Kimi 400 on follow-up turn after parallel
    /// `find` + `Read` tool calls.
    #[test]
    fn parallel_tool_calls_merge_into_one_assistant_then_one_user() {
        use crate::cif::ToolCall;
        let req = Request {
            model: "x".into(),
            system: None,
            messages: vec![
                Message::User {
                    content: vec![Content::Text {
                        text: "do two things".into(),
                    }],
                },
                // Codex emits one Assistant per function_call ResponseItem,
                // so parallel tool calls arrive as TWO consecutive Assistants.
                Message::Assistant {
                    content: vec![],
                    tool_calls: vec![ToolCall {
                        id: "tool_a".into(),
                        name: "exec".into(),
                        arguments: "{}".into(),
                    }],
                },
                Message::Assistant {
                    content: vec![],
                    tool_calls: vec![ToolCall {
                        id: "tool_b".into(),
                        name: "exec".into(),
                        arguments: "{}".into(),
                    }],
                },
                // And TWO consecutive Tools for the matching results.
                Message::Tool {
                    tool_call_id: "tool_a".into(),
                    content: "result-a".into(),
                },
                Message::Tool {
                    tool_call_id: "tool_b".into(),
                    content: "result-b".into(),
                },
            ],
            tools: vec![],
            temperature: None,
            max_output_tokens: None,
            stream: false,
        };
        let body = encode_request(&req);
        let messages = body["messages"].as_array().expect("messages array");

        // Expected after merge: 3 messages — user (initial), assistant (two
        // tool_use blocks), user (two tool_result blocks).
        assert_eq!(
            messages.len(),
            3,
            "merge must collapse consecutive same-role messages; got {} messages: {}",
            messages.len(),
            serde_json::to_string_pretty(&messages).unwrap_or_default()
        );

        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[0]["content"][0]["type"], "text");

        // Assistant message: ONE message containing TWO tool_use blocks in order.
        assert_eq!(messages[1]["role"], "assistant");
        let asst_blocks = messages[1]["content"].as_array().unwrap();
        assert_eq!(
            asst_blocks.len(),
            2,
            "merged assistant must hold both tool_use blocks"
        );
        assert_eq!(asst_blocks[0]["type"], "tool_use");
        assert_eq!(asst_blocks[0]["id"], "tool_a");
        assert_eq!(asst_blocks[1]["type"], "tool_use");
        assert_eq!(asst_blocks[1]["id"], "tool_b");

        // User message: ONE message containing TWO tool_result blocks linked by
        // tool_use_id, in the same order as the tool_use blocks above.
        assert_eq!(messages[2]["role"], "user");
        let user_blocks = messages[2]["content"].as_array().unwrap();
        assert_eq!(
            user_blocks.len(),
            2,
            "merged user must hold both tool_result blocks"
        );
        assert_eq!(user_blocks[0]["type"], "tool_result");
        assert_eq!(user_blocks[0]["tool_use_id"], "tool_a");
        assert_eq!(user_blocks[0]["content"], "result-a");
        assert_eq!(user_blocks[1]["type"], "tool_result");
        assert_eq!(user_blocks[1]["tool_use_id"], "tool_b");
        assert_eq!(user_blocks[1]["content"], "result-b");
    }

    /// Sanity: a sequential turn pattern (assistant text → user → assistant)
    /// must NOT merge across the user boundary. Only adjacent same-role
    /// pairs collapse.
    #[test]
    fn merge_does_not_collapse_across_role_boundaries() {
        let req = Request {
            model: "x".into(),
            system: None,
            messages: vec![
                Message::User {
                    content: vec![Content::Text { text: "hi".into() }],
                },
                Message::Assistant {
                    content: vec![Content::Text {
                        text: "hello".into(),
                    }],
                    tool_calls: vec![],
                },
                Message::User {
                    content: vec![Content::Text {
                        text: "more".into(),
                    }],
                },
                Message::Assistant {
                    content: vec![Content::Text { text: "ok".into() }],
                    tool_calls: vec![],
                },
            ],
            tools: vec![],
            temperature: None,
            max_output_tokens: None,
            stream: false,
        };
        let body = encode_request(&req);
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 4, "no merging when roles alternate");
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[2]["role"], "user");
        assert_eq!(messages[3]["role"], "assistant");
    }

    #[test]
    fn parses_message_start_captures_id() {
        let mut state = ChunkState::default();
        let _ = parse_chunk(
            r#"{"type":"message_start","message":{"id":"msg_abc"}}"#,
            &mut state,
        )
        .unwrap();
        assert_eq!(state.response_id.as_deref(), Some("msg_abc"));
    }

    #[test]
    fn parses_text_delta_stream() {
        let mut state = ChunkState::default();
        let _ = parse_chunk(
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
            &mut state,
        )
        .unwrap();
        let events = parse_chunk(
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}}"#,
            &mut state,
        )
        .unwrap();
        match &events[0] {
            StreamEvent::TextDelta { text } => assert_eq!(text, "Hi"),
            _ => panic!("expected text delta"),
        }
        assert_eq!(state.text, "Hi");
    }

    #[test]
    fn assembles_tool_use_block_across_chunks() {
        let mut state = ChunkState::default();
        let _ = parse_chunk(
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"t_1","name":"shell"}}"#,
            &mut state,
        )
        .unwrap();
        let _ = parse_chunk(
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"cmd\":\"l"}}"#,
            &mut state,
        )
        .unwrap();
        let _ = parse_chunk(
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"s\"}"}}"#,
            &mut state,
        )
        .unwrap();
        let _ = parse_chunk(
            r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"}}"#,
            &mut state,
        )
        .unwrap();
        let final_events = parse_chunk(r#"{"type":"message_stop"}"#, &mut state).unwrap();
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
    fn thinking_delta_emits_reasoning_event() {
        let mut state = ChunkState::default();
        let _ = parse_chunk(
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}"#,
            &mut state,
        )
        .unwrap();
        let events = parse_chunk(
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hmm"}}"#,
            &mut state,
        )
        .unwrap();
        match &events[0] {
            StreamEvent::ReasoningDelta { text } => assert_eq!(text, "hmm"),
            _ => panic!("expected reasoning delta"),
        }
    }
}
