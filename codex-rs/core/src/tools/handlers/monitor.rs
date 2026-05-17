// ecodex addition: `monitor` tool — agents call this to arm a watch on a
// background subprocess so the conversation wakes when the subprocess
// emits matching output. Mirrors Claude Code's Monitor primitive,
// enabling cross-AI mesh participation (e.g., held ntfy connection that
// pushes cortex inbox events).
//
// Action surface:
//   {"action":"arm","command":[...],"pattern":"...","persistent":true|false,
//    "stream":"stdout"|"stderr"|"both","cwd":"..."}  → returns monitor_id
//   {"action":"kill","monitor_id":"..."}            → returns ok flag
//   {"action":"list"}                                → returns array of armed
//
// Wake mechanism: matched line is injected into the session as a user-role
// <task-notification> message via `Session::inject_response_items`. The
// agent picks it up on the next turn.

use crate::function_tool::FunctionCallError;
use crate::monitor::ArmMonitorOptions;
use crate::monitor::spawn_monitor;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_tools::ToolName;
use serde::Deserialize;
use serde_json::Value as JsonValue;

pub struct MonitorHandler;

pub struct MonitorToolOutput {
    text: String,
}

impl ToolOutput for MonitorToolOutput {
    fn log_preview(&self) -> String {
        self.text.clone()
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        let mut output = FunctionCallOutputPayload::from_text(self.text.clone());
        output.success = Some(true);
        ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output,
        }
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
        serde_json::from_str(&self.text).unwrap_or(JsonValue::Null)
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum MonitorAction {
    Arm(ArmMonitorOptions),
    Kill { monitor_id: String },
    List,
}

impl ToolHandler for MonitorHandler {
    type Output = MonitorToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain("monitor")
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session, payload, ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "monitor handler received unsupported payload".to_string(),
                ));
            }
        };

        let action: MonitorAction = serde_json::from_str(&arguments).map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "monitor: invalid arguments: {err}. Expected one of: \
                 {{\"action\":\"arm\",\"command\":[...],\"pattern\":\"...\"}} | \
                 {{\"action\":\"kill\",\"monitor_id\":\"...\"}} | \
                 {{\"action\":\"list\"}}"
            ))
        })?;

        match action {
            MonitorAction::Arm(options) => {
                let monitor_id = spawn_monitor(session.clone(), options).await.map_err(|err| {
                    FunctionCallError::RespondToModel(format!("monitor arm failed: {err}"))
                })?;
                let text = serde_json::json!({
                    "ok": true,
                    "monitor_id": monitor_id,
                    "armed": true,
                })
                .to_string();
                Ok(MonitorToolOutput { text })
            }
            MonitorAction::Kill { monitor_id } => {
                let removed = session.services.monitor_registry.kill(&monitor_id).await;
                let text = serde_json::json!({
                    "ok": true,
                    "killed": removed,
                    "monitor_id": monitor_id,
                })
                .to_string();
                Ok(MonitorToolOutput { text })
            }
            MonitorAction::List => {
                // Hold the lock briefly to snapshot id+meta for each entry.
                let guard = session.services.monitor_registry.inner.lock().await;
                let entries: Vec<serde_json::Value> = guard
                    .iter()
                    .map(|(id, entry)| {
                        serde_json::json!({
                            "monitor_id": id,
                            "command": entry.command,
                            "pattern": entry.pattern,
                            "persistent": entry.persistent,
                        })
                    })
                    .collect();
                let count = entries.len();
                drop(guard);
                let text = serde_json::json!({
                    "ok": true,
                    "count": count,
                    "monitors": entries,
                })
                .to_string();
                Ok(MonitorToolOutput { text })
            }
        }
    }
}
