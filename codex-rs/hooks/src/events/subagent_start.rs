// ecodex addition (goal f0004294): SubagentStart fires when the parent
// session successfully spawns a subagent thread via the spawn_agent
// tool. Plugin handlers track parent→child relationships so subagent
// findings can be merged back into the parent's epistemic context.

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
use crate::schema::SubagentStartCommandInput;

#[derive(Debug, Clone)]
pub struct SubagentStartRequest {
    pub session_id: ThreadId,
    pub turn_id: String,
    pub cwd: AbsolutePathBuf,
    pub transcript_path: Option<PathBuf>,
    pub model: String,
    pub permission_mode: String,
    /// The new subagent's thread id.
    pub child_thread_id: String,
    /// The role the subagent is filling (e.g., "code-reviewer"), if known.
    pub agent_role: Option<String>,
    /// The subagent's nickname (e.g., random alias), if known.
    pub agent_nickname: Option<String>,
    /// The model the subagent will use.
    pub child_model: String,
}

#[derive(Debug, Default)]
pub struct SubagentStartOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
}

pub(crate) fn preview(
    handlers: &[ConfiguredHandler],
    _request: &SubagentStartRequest,
) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(handlers, HookEventName::SubagentStart, None)
        .into_iter()
        .map(|handler| dispatcher::running_summary(&handler))
        .collect()
}

pub(crate) async fn run(
    handlers: &[ConfiguredHandler],
    shell: &CommandShell,
    request: SubagentStartRequest,
) -> SubagentStartOutcome {
    let matched = dispatcher::select_handlers(handlers, HookEventName::SubagentStart, None);
    if matched.is_empty() {
        return SubagentStartOutcome::default();
    }

    let input_json = match serde_json::to_string(&SubagentStartCommandInput {
        session_id: request.session_id.to_string(),
        turn_id: request.turn_id.clone(),
        transcript_path: NullableString::from_path(request.transcript_path.clone()),
        cwd: request.cwd.display().to_string(),
        hook_event_name: "SubagentStart".to_string(),
        model: request.model.clone(),
        permission_mode: request.permission_mode.clone(),
        child_thread_id: request.child_thread_id.clone(),
        agent_role: NullableString::from_string(request.agent_role.clone()),
        agent_nickname: NullableString::from_string(request.agent_nickname.clone()),
        child_model: request.child_model.clone(),
    }) {
        Ok(input_json) => input_json,
        Err(error) => {
            return SubagentStartOutcome {
                hook_events: common::serialization_failure_hook_events(
                    matched,
                    Some(request.turn_id),
                    format!("failed to serialize subagent_start hook input: {error}"),
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

    SubagentStartOutcome {
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
