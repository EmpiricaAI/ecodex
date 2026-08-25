// ecodex addition (goal f0004294): TaskCompleted is an informational hook
// that fires when the agent's turn ends with no follow-up tool calls
// (semantically: "agent claimed done"). Unlike Stop, it carries no
// continuation/block semantics — plugin handlers consume it to enforce
// discipline (e.g., force POSTFLIGHT) but cannot redirect the conversation.
// Codex has no explicit task-completion marker, so dispatch site mirrors
// Stop's location in session/turn.rs; the distinct event name lets plugins
// attach POSTFLIGHT-enforcement handlers without changing Stop semantics.

use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookRunSummary;
use codex_utils_absolute_path::AbsolutePathBuf;

use super::common;
use crate::engine::ClaudeHooksEngine;
use crate::engine::ConfiguredHandler;
use crate::engine::HandlerRunResult;
use crate::engine::dispatcher;
use crate::schema::NullableString;
use crate::schema::TaskCompletedCommandInput;

#[derive(Debug, Clone)]
pub struct TaskCompletedRequest {
    pub session_id: ThreadId,
    pub turn_id: String,
    pub cwd: AbsolutePathBuf,
    pub transcript_path: Option<PathBuf>,
    pub model: String,
    pub permission_mode: String,
    pub last_assistant_message: Option<String>,
}

#[derive(Debug, Default)]
pub struct TaskCompletedOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
}

pub(crate) fn preview(
    handlers: &[ConfiguredHandler],
    _request: &TaskCompletedRequest,
) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(handlers, HookEventName::TaskCompleted, None)
        .into_iter()
        .map(|handler| dispatcher::running_summary(&handler))
        .collect()
}

pub(crate) async fn run(
    engine: &ClaudeHooksEngine,
    request: TaskCompletedRequest,
) -> TaskCompletedOutcome {
    let matched = dispatcher::select_handlers(&engine.handlers, HookEventName::TaskCompleted, None);
    if matched.is_empty() {
        return TaskCompletedOutcome::default();
    }

    let input_json = match serde_json::to_string(&TaskCompletedCommandInput {
        session_id: request.session_id.to_string(),
        turn_id: request.turn_id.clone(),
        transcript_path: NullableString::from_path(request.transcript_path.clone()),
        cwd: request.cwd.display().to_string(),
        hook_event_name: "TaskCompleted".to_string(),
        model: request.model.clone(),
        permission_mode: request.permission_mode.clone(),
        last_assistant_message: NullableString::from_string(request.last_assistant_message.clone()),
    }) {
        Ok(input_json) => input_json,
        Err(error) => {
            return TaskCompletedOutcome {
                hook_events: common::serialization_failure_hook_events(
                    matched,
                    Some(request.turn_id),
                    format!("failed to serialize task_completed hook input: {error}"),
                ),
            };
        }
    };

    let results = dispatcher::execute_handlers(
        engine,
        matched,
        input_json,
        request.cwd.as_path(),
        Some(request.turn_id),
        parse_completed,
    )
    .await;

    TaskCompletedOutcome {
        hook_events: results.into_iter().map(|result| result.completed).collect(),
    }
}

fn parse_completed(
    handler: &ConfiguredHandler,
    run_result: HandlerRunResult,
    turn_id: Option<String>,
) -> dispatcher::ParsedHandler<()> {
    let mut entries = Vec::new();
    let status = match run_result.error.as_deref() {
        Some(error) => {
            entries.push(HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: error.to_string(),
            });
            HookRunStatus::Failed
        }
        None => match run_result.exit_code {
            Some(0) => HookRunStatus::Completed,
            Some(exit_code) => {
                entries.push(HookOutputEntry {
                    kind: HookOutputEntryKind::Error,
                    text: format!("hook exited with code {exit_code}"),
                });
                HookRunStatus::Failed
            }
            None => {
                entries.push(HookOutputEntry {
                    kind: HookOutputEntryKind::Error,
                    text: "hook exited without a status code".to_string(),
                });
                HookRunStatus::Failed
            }
        },
    };

    let completed = HookCompletedEvent {
        turn_id,
        run: dispatcher::completed_summary(handler, &run_result, status, entries),
    };
    dispatcher::ParsedHandler {
        completed,
        completion_order: 0,
        data: (),
    }
}
