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
use crate::tools::context::boxed_tool_output;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_tools::AdditionalProperties;
use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde::Deserialize;
use serde_json::Value as JsonValue;

pub struct MonitorHandler;

pub struct MonitorToolOutput {
    text: String,
}

impl ToolOutput for MonitorToolOutput {
    fn log_output(&self) -> String {
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

impl ToolExecutor<ToolInvocation> for MonitorHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("monitor")
    }

    fn spec(&self) -> ToolSpec {
        monitor_tool_spec()
    }

    fn handle<'a>(&'a self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'a>
    where
        ToolInvocation: 'a,
    {
        Box::pin(async move {
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
                    let monitor_id =
                        spawn_monitor(session.clone(), options).await.map_err(|err| {
                            FunctionCallError::RespondToModel(format!("monitor arm failed: {err}"))
                        })?;
                    let text = serde_json::json!({
                        "ok": true,
                        "monitor_id": monitor_id,
                        "armed": true,
                    })
                    .to_string();
                    Ok(boxed_tool_output(MonitorToolOutput { text }))
                }
                MonitorAction::Kill { monitor_id } => {
                    let removed = session.services.monitor_registry.kill(&monitor_id).await;
                    let text = serde_json::json!({
                        "ok": true,
                        "killed": removed,
                        "monitor_id": monitor_id,
                    })
                    .to_string();
                    Ok(boxed_tool_output(MonitorToolOutput { text }))
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
                    Ok(boxed_tool_output(MonitorToolOutput { text }))
                }
            }
        })
    }
}

impl CoreToolRuntime for MonitorHandler {}

// ecodex addition: build the ToolSpec describing the `monitor` tool's
// JSON-schema argument surface so the model can discover + call it.
// (Restored into the handler's spec() after the 2026-05 sync moved tool
// specs from the old tools/spec.rs builder into per-handler spec() methods.)
fn monitor_tool_spec() -> ToolSpec {
    let mut properties = std::collections::BTreeMap::<String, JsonSchema>::new();
    properties.insert(
        "action".to_string(),
        JsonSchema::string_enum(
            vec![
                JsonValue::String("arm".to_string()),
                JsonValue::String("kill".to_string()),
                JsonValue::String("list".to_string()),
            ],
            Some("What to do: arm a new watch, kill an existing one, or list count".to_string()),
        ),
    );
    properties.insert(
        "command".to_string(),
        JsonSchema::array(
            JsonSchema::string(None),
            Some(
                "Argv to spawn (action=arm only). First element is the program, rest are args."
                    .to_string(),
            ),
        ),
    );
    properties.insert(
        "pattern".to_string(),
        JsonSchema::string(Some(
            "Regex pattern to match against subprocess output lines (action=arm only).".to_string(),
        )),
    );
    properties.insert(
        "persistent".to_string(),
        JsonSchema::boolean(Some(
            "When true, the watch stays armed after each match (action=arm only). Default false."
                .to_string(),
        )),
    );
    properties.insert(
        "stream".to_string(),
        JsonSchema::string_enum(
            vec![
                JsonValue::String("stdout".to_string()),
                JsonValue::String("stderr".to_string()),
                JsonValue::String("both".to_string()),
            ],
            Some("Which stream to watch (action=arm only). Default stdout.".to_string()),
        ),
    );
    properties.insert(
        "cwd".to_string(),
        JsonSchema::string(Some(
            "Working directory for the spawned command (action=arm only).".to_string(),
        )),
    );
    properties.insert(
        "monitor_id".to_string(),
        JsonSchema::string(Some(
            "The id returned by a prior arm (action=kill only).".to_string(),
        )),
    );

    let parameters = JsonSchema::object(
        properties,
        Some(vec!["action".to_string()]),
        Some(AdditionalProperties::Boolean(false)),
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "monitor".to_string(),
        description:
            "Arm a watch on a background subprocess. On each line matching the regex \
             `pattern`, a <task-notification> message is injected into your pending \
             input. Use for sub-second wake on cortex mesh events (held ntfy \
             connection) or any long-running stream you want to react to. Returns \
             {monitor_id} from arm; pair with kill to disarm. persistent=true keeps \
             the watch armed across multiple matches."
                .to_string(),
        strict: false,
        defer_loading: None,
        parameters,
        output_schema: None,
    })
}
