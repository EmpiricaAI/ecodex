use crate::function_tool::FunctionCallError;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::hook_names::HookToolName;
use crate::tools::registry::PostToolUsePayload;
use crate::tools::registry::PreToolUsePayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::dynamic_tools::DynamicToolCallRequest;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::protocol::DynamicToolCallResponseEvent;
use codex_protocol::protocol::EventMsg;
use codex_tools::ToolName;
use serde_json::Value;
use std::time::Instant;
use tokio::sync::oneshot;
use tracing::warn;

pub struct DynamicToolHandler;

impl ToolHandler for DynamicToolHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn pre_tool_use_payload(&self, invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        Some(PreToolUsePayload {
            tool_name: HookToolName::for_dynamic_tool(&invocation.tool_name),
            tool_input: dynamic_tool_input(invocation).ok()?,
        })
    }

    fn post_tool_use_payload(
        &self,
        invocation: &ToolInvocation,
        result: &Self::Output,
    ) -> Option<PostToolUsePayload> {
        Some(PostToolUsePayload {
            tool_name: HookToolName::for_dynamic_tool(&invocation.tool_name),
            tool_use_id: invocation.call_id.clone(),
            tool_input: dynamic_tool_input(invocation).ok()?,
            tool_response: result
                .post_tool_use_response(&invocation.call_id, &invocation.payload)?,
        })
    }

    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        true
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            tool_name,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "dynamic tool handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: Value = parse_arguments(&arguments)?;
        let response = request_dynamic_tool(&session, turn.as_ref(), call_id, tool_name, args)
            .await
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(
                    "dynamic tool call was cancelled before receiving a response".to_string(),
                )
            })?;

        let DynamicToolResponse {
            content_items,
            success,
        } = response;
        let body = content_items
            .into_iter()
            .map(FunctionCallOutputContentItem::from)
            .collect::<Vec<_>>();
        Ok(FunctionToolOutput {
            post_tool_use_response: Some(dynamic_tool_post_tool_use_response(&body)?),
            body,
            success: Some(success),
        })
    }
}

fn dynamic_tool_input(invocation: &ToolInvocation) -> Result<Value, FunctionCallError> {
    let ToolPayload::Function { arguments } = &invocation.payload else {
        return Err(FunctionCallError::RespondToModel(
            "dynamic tool handler received unsupported payload".to_string(),
        ));
    };

    parse_arguments(arguments)
}

fn dynamic_tool_post_tool_use_response(
    body: &[FunctionCallOutputContentItem],
) -> Result<Value, FunctionCallError> {
    match body {
        [FunctionCallOutputContentItem::InputText { text }] => Ok(Value::String(text.clone())),
        _ => serde_json::to_value(body).map_err(|error| {
            FunctionCallError::RespondToModel(format!(
                "failed to serialize dynamic tool response for PostToolUse: {error}"
            ))
        }),
    }
}

#[expect(
    clippy::await_holding_invalid_type,
    reason = "active turn checks and dynamic tool response registration must remain atomic"
)]
async fn request_dynamic_tool(
    session: &Session,
    turn_context: &TurnContext,
    call_id: String,
    tool_name: ToolName,
    arguments: Value,
) -> Option<DynamicToolResponse> {
    let namespace = tool_name.namespace;
    let tool = tool_name.name;
    let turn_id = turn_context.sub_id.clone();
    let (tx_response, rx_response) = oneshot::channel();
    let event_id = call_id.clone();
    let prev_entry = {
        let mut active = session.active_turn.lock().await;
        match active.as_mut() {
            Some(at) => {
                let mut ts = at.turn_state.lock().await;
                ts.insert_pending_dynamic_tool(call_id.clone(), tx_response)
            }
            None => None,
        }
    };
    if prev_entry.is_some() {
        warn!("Overwriting existing pending dynamic tool call for call_id: {event_id}");
    }

    let started_at = Instant::now();
    let event = EventMsg::DynamicToolCallRequest(DynamicToolCallRequest {
        call_id: call_id.clone(),
        turn_id: turn_id.clone(),
        namespace: namespace.clone(),
        tool: tool.clone(),
        arguments: arguments.clone(),
    });
    session.send_event(turn_context, event).await;
    let response = rx_response.await.ok();

    let response_event = match &response {
        Some(response) => EventMsg::DynamicToolCallResponse(DynamicToolCallResponseEvent {
            call_id,
            turn_id,
            namespace,
            tool,
            arguments,
            content_items: response.content_items.clone(),
            success: response.success,
            error: None,
            duration: started_at.elapsed(),
        }),
        None => EventMsg::DynamicToolCallResponse(DynamicToolCallResponseEvent {
            call_id,
            turn_id,
            namespace,
            tool,
            arguments,
            content_items: Vec::new(),
            success: false,
            error: Some("dynamic tool call was cancelled before receiving a response".to_string()),
            duration: started_at.elapsed(),
        }),
    };
    session.send_event(turn_context, response_event).await;

    response
}

#[cfg(test)]
mod tests {
    use super::DynamicToolHandler;
    use super::dynamic_tool_post_tool_use_response;
    use crate::session::tests::make_session_and_context;
    use crate::tools::context::FunctionToolOutput;
    use crate::tools::context::ToolCallSource;
    use crate::tools::context::ToolInvocation;
    use crate::tools::context::ToolPayload;
    use crate::tools::hook_names::HookToolName;
    use crate::tools::registry::PostToolUsePayload;
    use crate::tools::registry::PreToolUsePayload;
    use crate::tools::registry::ToolHandler;
    use crate::turn_diff_tracker::TurnDiffTracker;
    use codex_protocol::models::FunctionCallOutputContentItem;
    use codex_tools::ToolName;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    async fn dynamic_invocation(
        tool_name: ToolName,
        arguments: serde_json::Value,
    ) -> ToolInvocation {
        let (session, turn) = make_session_and_context().await;
        ToolInvocation {
            session: session.into(),
            turn: turn.into(),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            tracker: Arc::new(Mutex::new(TurnDiffTracker::new())),
            call_id: "call-dynamic".to_string(),
            tool_name,
            source: ToolCallSource::Direct,
            payload: ToolPayload::Function {
                arguments: arguments.to_string(),
            },
        }
    }

    #[tokio::test]
    async fn dynamic_pre_tool_use_payload_uses_plain_tool_name() {
        let invocation =
            dynamic_invocation(ToolName::plain("automation_update"), json!({"id": 1})).await;

        assert_eq!(
            DynamicToolHandler.pre_tool_use_payload(&invocation),
            Some(PreToolUsePayload {
                tool_name: HookToolName::new("automation_update"),
                tool_input: json!({ "id": 1 }),
            })
        );
    }

    #[tokio::test]
    async fn dynamic_post_tool_use_payload_uses_namespaced_hook_name() {
        let invocation = dynamic_invocation(
            ToolName::namespaced("codex_app", "automation_update"),
            json!({ "job": "sync" }),
        )
        .await;
        let output = FunctionToolOutput {
            body: vec![FunctionCallOutputContentItem::InputText {
                text: "ok".to_string(),
            }],
            success: Some(true),
            post_tool_use_response: Some(json!("ok")),
        };

        assert_eq!(
            DynamicToolHandler.post_tool_use_payload(&invocation, &output),
            Some(PostToolUsePayload {
                tool_name: HookToolName::new("dynamic__codex_app__automation_update"),
                tool_use_id: "call-dynamic".to_string(),
                tool_input: json!({ "job": "sync" }),
                tool_response: json!("ok"),
            })
        );
    }

    #[test]
    fn dynamic_post_tool_use_response_uses_text_for_single_text_item() {
        assert_eq!(
            dynamic_tool_post_tool_use_response(&[FunctionCallOutputContentItem::InputText {
                text: "done".to_string(),
            }]),
            Ok(json!("done"))
        );
    }

    #[test]
    fn dynamic_post_tool_use_response_uses_content_items_for_mixed_output() {
        let response = dynamic_tool_post_tool_use_response(&[
            FunctionCallOutputContentItem::InputText {
                text: "done".to_string(),
            },
            FunctionCallOutputContentItem::InputImage {
                image_url: "https://example.com/image.png".to_string(),
                detail: None,
            },
        ])
        .expect("serialize mixed dynamic tool output");

        assert_eq!(
            response,
            json!([
                {
                    "type": "input_text",
                    "text": "done",
                },
                {
                    "type": "input_image",
                    "image_url": "https://example.com/image.png",
                }
            ])
        );
    }
}
