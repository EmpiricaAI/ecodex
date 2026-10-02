use super::*;
use pretty_assertions::assert_eq;

#[test]
fn bare_justification_is_dropped() {
    let normalized = normalize_tool_arguments(
        r#"{"cmd":"ls -la","justification":"list the files to inspect them"}"#,
    );

    assert_eq!(
        serde_json::from_str::<Value>(&normalized).expect("json"),
        serde_json::json!({"cmd": "ls -la"})
    );
}

#[test]
fn arguments_needing_no_repair_pass_through_unchanged() {
    let escalation = r#"{"cmd":"cargo build","sandbox_permissions":"require_escalated","justification":"needs network"}"#;
    let plain = r#"{"cmd":"ls"}"#;
    let not_an_object = r#"["ls"]"#;
    let not_json = "ls -la";

    assert_eq!(
        [escalation, plain, not_an_object, not_json].map(|arguments| matches!(
            normalize_tool_arguments(arguments),
            Cow::Borrowed(unchanged) if unchanged == arguments
        )),
        [true, true, true, true]
    );
}
