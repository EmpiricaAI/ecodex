// ecodex: TaskCompleted is informational. These tests pin that contract: only
// TaskCompleted handlers run, and a handler can report success or failure but
// cannot block or stop the turn the way a Stop hook can.

use std::collections::HashMap;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookSource;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;

use super::TaskCompletedRequest;
use super::parse_completed;
use super::preview;
use crate::engine::ConfiguredHandler;
use crate::engine::HandlerRunResult;

#[test]
fn preview_selects_only_task_completed_handlers() {
    let selected = preview(
        &[
            ConfiguredHandler {
                display_order: 0,
                ..handler(HookEventName::Stop)
            },
            ConfiguredHandler {
                display_order: 1,
                ..handler(HookEventName::TaskCompleted)
            },
            ConfiguredHandler {
                display_order: 2,
                ..handler(HookEventName::SessionEnd)
            },
        ],
        &request(),
    );

    assert_eq!(
        selected
            .iter()
            .map(|run| run.display_order)
            .collect::<Vec<_>>(),
        vec![1]
    );
}

#[test]
fn successful_handler_completes_and_cannot_steer_the_turn() {
    let parsed = parse_completed(
        &handler(HookEventName::TaskCompleted),
        run_result(
            Some(0),
            r#"{"continue":false,"decision":"block","reason":"ignored"}"#,
        ),
        Some("turn-1".to_string()),
    );

    assert_eq!(parsed.completed.turn_id, Some("turn-1".to_string()));
    assert_eq!(parsed.completed.run.status, HookRunStatus::Completed);
    assert_eq!(parsed.completed.run.entries, Vec::new());
}

#[test]
fn non_zero_exit_fails_with_an_error_entry() {
    let parsed = parse_completed(
        &handler(HookEventName::TaskCompleted),
        run_result(Some(2), ""),
        Some("turn-1".to_string()),
    );

    assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
    assert_eq!(
        parsed.completed.run.entries,
        vec![HookOutputEntry {
            kind: HookOutputEntryKind::Error,
            text: "hook exited with code 2".to_string(),
        }]
    );
}

fn request() -> TaskCompletedRequest {
    TaskCompletedRequest {
        session_id: ThreadId::new(),
        turn_id: "turn-1".to_string(),
        cwd: test_path_buf("/tmp").abs(),
        transcript_path: None,
        model: "test-model".to_string(),
        permission_mode: "default".to_string(),
        last_assistant_message: Some("done".to_string()),
    }
}

fn run_result(exit_code: Option<i32>, stdout: &str) -> HandlerRunResult {
    HandlerRunResult {
        structured_content: None,
        context_metadata: codex_protocol::models::ContentItemMetadata::harness(),
        started_at: 1,
        completed_at: 2,
        duration_ms: 1,
        exit_code,
        stdout: stdout.to_string(),
        stderr: String::new(),
        error: None,
    }
}

fn handler(event_name: HookEventName) -> ConfiguredHandler {
    ConfiguredHandler {
        builtin: false,
        plugin_trust: None,
        event_name,
        matcher: None,
        timeout_sec: 2,
        status_message: None,
        additional_context_limit: Default::default(),
        source_path: test_path_buf("/tmp/hooks.json").abs().into(),
        source: HookSource::User,
        display_order: 0,
        kind: crate::engine::ConfiguredHandlerKind::Command {
            command: "echo hook".to_string(),
            r#async: false,
            env: HashMap::new(),
        },
    }
}
