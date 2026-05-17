// ecodex addition (goal f0004294): SubagentStop fires when a subagent
// reports done via the report_agent_job_result tool. Fires from the
// SUBAGENT'S session (session_id = subagent's thread_id). Plugin
// handlers correlate via shared state (~/.empirica/subagents/) with
// the SubagentStart event the parent saw at spawn time.

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
use crate::schema::SubagentStopCommandInput;

#[derive(Debug, Clone)]
pub struct SubagentStopRequest {
    pub session_id: ThreadId,
    pub turn_id: String,
    pub cwd: AbsolutePathBuf,
    pub transcript_path: Option<PathBuf>,
    pub model: String,
    pub permission_mode: String,
    pub job_id: String,
    pub item_id: String,
    /// True if the result was accepted by the agent-jobs store.
    pub accepted: bool,
}

#[derive(Debug, Default)]
pub struct SubagentStopOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
}

pub(crate) fn preview(
    handlers: &[ConfiguredHandler],
    _request: &SubagentStopRequest,
) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(handlers, HookEventName::SubagentStop, None)
        .into_iter()
        .map(|handler| dispatcher::running_summary(&handler))
        .collect()
}

pub(crate) async fn run(
    handlers: &[ConfiguredHandler],
    shell: &CommandShell,
    request: SubagentStopRequest,
) -> SubagentStopOutcome {
    let matched = dispatcher::select_handlers(handlers, HookEventName::SubagentStop, None);
    if matched.is_empty() {
        return SubagentStopOutcome::default();
    }

    let input_json = match serde_json::to_string(&SubagentStopCommandInput {
        session_id: request.session_id.to_string(),
        turn_id: request.turn_id.clone(),
        transcript_path: NullableString::from_path(request.transcript_path.clone()),
        cwd: request.cwd.display().to_string(),
        hook_event_name: "SubagentStop".to_string(),
        model: request.model.clone(),
        permission_mode: request.permission_mode.clone(),
        job_id: request.job_id.clone(),
        item_id: request.item_id.clone(),
        accepted: request.accepted,
    }) {
        Ok(input_json) => input_json,
        Err(error) => {
            return SubagentStopOutcome {
                hook_events: common::serialization_failure_hook_events(
                    matched,
                    Some(request.turn_id),
                    format!("failed to serialize subagent_stop hook input: {error}"),
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

    SubagentStopOutcome {
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
