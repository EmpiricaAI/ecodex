// ecodex addition (goal f0004294): SessionEnd is the symmetric
// counterpart to SessionStart. Fires when a codex session terminates
// (user /quit, signal, parent process death). Plugin handlers run
// final POSTFLIGHT, capture session snapshots, curate the rollout.
// Informational only.

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
use crate::engine::CommandShell;
use crate::engine::ConfiguredHandler;
use crate::engine::command_runner::CommandRunResult;
use crate::engine::dispatcher;
use crate::schema::NullableString;
use crate::schema::SessionEndCommandInput;

#[derive(Debug, Clone)]
pub struct SessionEndRequest {
    pub session_id: ThreadId,
    pub turn_id: String,
    pub cwd: AbsolutePathBuf,
    pub transcript_path: Option<PathBuf>,
    pub model: String,
    pub permission_mode: String,
    /// Approximate number of user-turn boundaries the session held at
    /// shutdown. Plugins can use this for session-summary thresholds.
    pub turn_count: u64,
}

#[derive(Debug, Default)]
pub struct SessionEndOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
}

pub(crate) fn preview(
    handlers: &[ConfiguredHandler],
    _request: &SessionEndRequest,
) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(handlers, HookEventName::SessionEnd, None)
        .into_iter()
        .map(|handler| dispatcher::running_summary(&handler))
        .collect()
}

pub(crate) async fn run(
    handlers: &[ConfiguredHandler],
    shell: &CommandShell,
    request: SessionEndRequest,
) -> SessionEndOutcome {
    let matched = dispatcher::select_handlers(handlers, HookEventName::SessionEnd, None);
    if matched.is_empty() {
        return SessionEndOutcome::default();
    }

    let input_json = match serde_json::to_string(&SessionEndCommandInput {
        session_id: request.session_id.to_string(),
        turn_id: request.turn_id.clone(),
        transcript_path: NullableString::from_path(request.transcript_path.clone()),
        cwd: request.cwd.display().to_string(),
        hook_event_name: "SessionEnd".to_string(),
        model: request.model.clone(),
        permission_mode: request.permission_mode.clone(),
        turn_count: request.turn_count,
    }) {
        Ok(input_json) => input_json,
        Err(error) => {
            return SessionEndOutcome {
                hook_events: common::serialization_failure_hook_events(
                    matched,
                    Some(request.turn_id),
                    format!("failed to serialize session_end hook input: {error}"),
                ),
            };
        }
    };

    let results = dispatcher::execute_handlers(
        shell,
        matched,
        input_json,
        request.cwd.as_path(),
        Some(request.turn_id),
        parse_completed,
    )
    .await;

    SessionEndOutcome {
        hook_events: results.into_iter().map(|result| result.completed).collect(),
    }
}

fn parse_completed(
    handler: &ConfiguredHandler,
    run_result: CommandRunResult,
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
        data: (),
    }
}
