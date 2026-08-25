// ecodex addition (goal f0004294): PostToolUseFailure fires when a tool
// invocation fails (non-zero exit, exception, timeout). Sibling to
// PostToolUse which only fires on success. Informational only — plugin
// handlers consume failures as dead-end artifacts for calibration; they
// cannot redirect the agent loop. Dispatch site is tools/registry.rs at
// the failure branch alongside the existing PostToolUse dispatch.

use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookRunSummary;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde_json::Value;

use super::common;
use crate::engine::ClaudeHooksEngine;
use crate::engine::ConfiguredHandler;
use crate::engine::HandlerRunResult;
use crate::engine::dispatcher;
use crate::schema::NullableString;
use crate::schema::PostToolUseFailureCommandInput;

#[derive(Debug, Clone)]
pub struct PostToolUseFailureRequest {
    pub session_id: ThreadId,
    pub turn_id: String,
    pub cwd: AbsolutePathBuf,
    pub transcript_path: Option<PathBuf>,
    pub model: String,
    pub permission_mode: String,
    pub tool_name: String,
    pub matcher_aliases: Vec<String>,
    pub tool_use_id: String,
    pub tool_input: Value,
    pub error_message: String,
    pub duration_ms: u64,
}

#[derive(Debug, Default)]
pub struct PostToolUseFailureOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
}

pub(crate) fn preview(
    handlers: &[ConfiguredHandler],
    request: &PostToolUseFailureRequest,
) -> Vec<HookRunSummary> {
    let matcher_inputs: Vec<&str> = std::iter::once(request.tool_name.as_str())
        .chain(request.matcher_aliases.iter().map(String::as_str))
        .collect();
    dispatcher::select_handlers_for_matcher_inputs(
        handlers,
        HookEventName::PostToolUseFailure,
        &matcher_inputs,
    )
    .into_iter()
    .map(|handler| dispatcher::running_summary(&handler))
    .collect()
}

pub(crate) async fn run(
    engine: &ClaudeHooksEngine,
    request: PostToolUseFailureRequest,
) -> PostToolUseFailureOutcome {
    let matcher_inputs: Vec<&str> = std::iter::once(request.tool_name.as_str())
        .chain(request.matcher_aliases.iter().map(String::as_str))
        .collect();
    let matched = dispatcher::select_handlers_for_matcher_inputs(
        &engine.handlers,
        HookEventName::PostToolUseFailure,
        &matcher_inputs,
    );
    if matched.is_empty() {
        return PostToolUseFailureOutcome::default();
    }

    let input_json = match serde_json::to_string(&PostToolUseFailureCommandInput {
        session_id: request.session_id.to_string(),
        turn_id: request.turn_id.clone(),
        transcript_path: NullableString::from_path(request.transcript_path.clone()),
        cwd: request.cwd.display().to_string(),
        hook_event_name: "PostToolUseFailure".to_string(),
        model: request.model.clone(),
        permission_mode: request.permission_mode.clone(),
        tool_name: request.tool_name.clone(),
        tool_use_id: request.tool_use_id.clone(),
        tool_input: request.tool_input.clone(),
        error_message: request.error_message.clone(),
        duration_ms: request.duration_ms,
    }) {
        Ok(input_json) => input_json,
        Err(error) => {
            return PostToolUseFailureOutcome {
                hook_events: common::serialization_failure_hook_events(
                    matched,
                    Some(request.turn_id),
                    format!("failed to serialize post_tool_use_failure hook input: {error}"),
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

    PostToolUseFailureOutcome {
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
