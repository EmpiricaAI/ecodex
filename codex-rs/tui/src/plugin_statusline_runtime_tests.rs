use super::*;
use pretty_assertions::assert_eq;
use std::fs;

fn write_instance(dir: &Path, stem: &str, project_path: &str, session_id: &str) {
    fs::write(
        dir.join(format!("{stem}.json")),
        format!(r#"{{"project_path":"{project_path}","empirica_session_id":"{session_id}"}}"#),
    )
    .expect("write instance file");
}

#[test]
fn payload_carries_context_use_and_model_in_the_claude_code_shape() {
    let live = LiveContext {
        instance_id: Some("thread-1".to_string()),
        context_used_percentage: Some(31),
        model: Some("devstral-latest".to_string()),
    };
    let payload = statusline_payload_json(
        Some("sess-1".to_string()),
        Some("/tmp/project".to_string()),
        &live,
    );
    let parsed: serde_json::Value = serde_json::from_str(&payload).expect("valid JSON");
    assert_eq!(
        parsed,
        serde_json::json!({
            "session_id": "sess-1",
            "cwd": "/tmp/project",
            "context_window": { "used_percentage": 31 },
            "model": { "id": "devstral-latest", "display_name": "devstral-latest" },
        })
    );
}

#[test]
fn payload_omits_what_the_tui_does_not_know_yet() {
    let payload = statusline_payload_json(None, None, &LiveContext::default());
    assert_eq!(payload, "{}");
    let payload = statusline_payload_json(
        Some("sess-1".to_string()),
        None,
        &LiveContext {
            model: Some("gpt-6.1-sol".to_string()),
            ..LiveContext::default()
        },
    );
    let parsed: serde_json::Value = serde_json::from_str(&payload).expect("valid JSON");
    assert_eq!(
        parsed,
        serde_json::json!({
            "session_id": "sess-1",
            "model": { "id": "gpt-6.1-sol", "display_name": "gpt-6.1-sol" },
        })
    );
}

#[test]
fn the_thread_id_wins_over_a_directory_match() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_instance(dir.path(), "tmux_7", "/work/project", "session-from-pane");
    write_instance(dir.path(), "thread-a", "/work/project", "session-a");
    write_instance(dir.path(), "thread-b", "/work/project", "session-b");

    assert_eq!(
        resolve_in_dir(
            dir.path(),
            &["thread-b".to_string()],
            Some(Path::new("/work/project/src")),
        ),
        Some("session-b".to_string())
    );
}

#[test]
fn candidates_are_tried_in_order_before_the_directory_match() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_instance(dir.path(), "second", "/work/project", "session-second");
    write_instance(dir.path(), "other", "/work/project", "session-other");

    assert_eq!(
        resolve_in_dir(
            dir.path(),
            &["first".to_string(), "second".to_string()],
            Some(Path::new("/work/project")),
        ),
        Some("session-second".to_string())
    );
}

#[test]
fn falls_back_to_the_directory_match_and_then_to_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_instance(dir.path(), "thread-a", "/work/project", "session-a");

    assert_eq!(
        [
            resolve_in_dir(
                dir.path(),
                &["thread-missing".to_string()],
                Some(Path::new("/work/project/src")),
            ),
            resolve_in_dir(dir.path(), &[], Some(Path::new("/elsewhere"))),
            resolve_in_dir(dir.path(), &[], /*cwd*/ None),
        ],
        [Some("session-a".to_string()), None, None]
    );
}
