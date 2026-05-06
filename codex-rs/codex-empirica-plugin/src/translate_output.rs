//! Translate empirica/Claude-Code-shape hook output → codex-shape hook output.
//!
//! Codex's hook engine validates each event's output JSON against a strict
//! `additionalProperties: false` schema (in
//! `codex-rs/hooks/schema/generated/<event>.command.output.schema.json`).
//! Empirica's bundled hook scripts emit Claude-Code shapes — flat
//! `{continue, context, decision, suppressOutput}` — which codex rejects with
//! "invalid hook JSON output" or "unsupported suppressOutput".
//!
//! Translation rules per event:
//!
//! | event              | CC shape                                         | codex shape                                                                                          |
//! |--------------------|--------------------------------------------------|------------------------------------------------------------------------------------------------------|
//! | SessionStart       | `{continue, context}`                            | `{continue, hookSpecificOutput:{hookEventName:"SessionStart",additionalContext}}`                    |
//! | UserPromptSubmit   | `{continue, context, decision?:"block"}`         | `{continue, hookSpecificOutput:{hookEventName:"UserPromptSubmit",additionalContext}, decision?}`     |
//! | PreToolUse         | `{continue, decision?:"block"|"approve", stopReason?, suppressOutput?}` | `{continue, hookSpecificOutput:{hookEventName:"PreToolUse", permissionDecision?, permissionDecisionReason?, additionalContext?}}`  |
//! | PostToolUse        | `{continue, context?, decision?:"block"}`        | `{continue, hookSpecificOutput:{hookEventName:"PostToolUse",additionalContext}, decision?}`          |
//! | Stop               | `{continue, decision?:"block", reason?}`         | `{continue, decision?:"block", reason?, stopReason?}` (already-compatible, light pass-through)       |
//!
//! Unknown CC fields are dropped silently. Empty-context flat outputs become
//! `{continue: true}` with no hookSpecificOutput.
//!
//! Diagnosis case 2026-05-06: David's ecodex session reported
//! "SessionStart hook returned invalid JSON output" + "PreToolUse hook
//! returned unsupported suppressOutput" — both fixed by routing every
//! handler's output through this translator.

use serde_json::{json, Map, Value};

/// Translate empirica/CC-shape JSON to codex-shape for the given event.
///
/// Accepts any JSON; returns codex-valid JSON. On parse failure or
/// unrecognized event, returns a minimal `{"continue": true}` so codex
/// stays happy even if the script returned garbage (fail-open).
pub fn translate(event: &str, cc_output: &str) -> String {
    let cc: Value = serde_json::from_str(cc_output.trim()).unwrap_or_else(|_| json!({}));
    let codex = match event {
        "session-start" | "SessionStart" => translate_session_start(&cc),
        "user-prompt-submit" | "UserPromptSubmit" => translate_user_prompt_submit(&cc),
        "pre-tool-use" | "PreToolUse" => translate_pre_tool_use(&cc),
        "post-tool-use" | "PostToolUse" => translate_post_tool_use(&cc),
        "stop" | "Stop" => translate_stop(&cc),
        _ => json!({"continue": true}),
    };
    serde_json::to_string(&codex).unwrap_or_else(|_| r#"{"continue":true}"#.to_string())
}

fn pluck_continue(cc: &Value) -> bool {
    cc.get("continue").and_then(Value::as_bool).unwrap_or(true)
}

fn pluck_context(cc: &Value) -> Option<String> {
    cc.get("context")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn translate_session_start(cc: &Value) -> Value {
    let mut out = Map::new();
    out.insert("continue".into(), json!(pluck_continue(cc)));
    let mut hso = Map::new();
    hso.insert("hookEventName".into(), json!("SessionStart"));
    if let Some(ctx) = pluck_context(cc) {
        hso.insert("additionalContext".into(), json!(ctx));
    }
    out.insert("hookSpecificOutput".into(), Value::Object(hso));
    Value::Object(out)
}

fn translate_user_prompt_submit(cc: &Value) -> Value {
    let mut out = Map::new();
    out.insert("continue".into(), json!(pluck_continue(cc)));
    let mut hso = Map::new();
    hso.insert("hookEventName".into(), json!("UserPromptSubmit"));
    if let Some(ctx) = pluck_context(cc) {
        hso.insert("additionalContext".into(), json!(ctx));
    }
    out.insert("hookSpecificOutput".into(), Value::Object(hso));
    // CC's top-level decision: "block" maps to codex's top-level decision: "block"
    if cc.get("decision").and_then(Value::as_str) == Some("block") {
        out.insert("decision".into(), json!("block"));
        if let Some(reason) = cc.get("reason").and_then(Value::as_str) {
            out.insert("reason".into(), json!(reason));
        }
    }
    Value::Object(out)
}

fn translate_pre_tool_use(cc: &Value) -> Value {
    let mut out = Map::new();
    out.insert("continue".into(), json!(pluck_continue(cc)));
    let mut hso = Map::new();
    hso.insert("hookEventName".into(), json!("PreToolUse"));
    // CC -> codex: top-level `decision` becomes `hookSpecificOutput.permissionDecision`.
    // CC values: "block" / "approve" — codex accepts both at PreToolUse.
    if let Some(decision) = cc.get("decision").and_then(Value::as_str)
        && (decision == "block" || decision == "approve")
    {
        hso.insert("permissionDecision".into(), json!(decision));
    }
    // CC's `stopReason` (the user-visible reason text) maps to codex's
    // `permissionDecisionReason`.
    if let Some(reason) = cc
        .get("stopReason")
        .and_then(Value::as_str)
        .or_else(|| cc.get("reason").and_then(Value::as_str))
    {
        hso.insert("permissionDecisionReason".into(), json!(reason));
    }
    if let Some(ctx) = pluck_context(cc) {
        hso.insert("additionalContext".into(), json!(ctx));
    }
    out.insert("hookSpecificOutput".into(), Value::Object(hso));
    // CC's `suppressOutput` is silently dropped — PreToolUse codex schema
    // rejects it. Other top-level CC fields (systemMessage, etc.) also
    // dropped to satisfy `additionalProperties: false`.
    Value::Object(out)
}

fn translate_post_tool_use(cc: &Value) -> Value {
    let mut out = Map::new();
    out.insert("continue".into(), json!(pluck_continue(cc)));
    let mut hso = Map::new();
    hso.insert("hookEventName".into(), json!("PostToolUse"));
    if let Some(ctx) = pluck_context(cc) {
        hso.insert("additionalContext".into(), json!(ctx));
    }
    out.insert("hookSpecificOutput".into(), Value::Object(hso));
    if cc.get("decision").and_then(Value::as_str) == Some("block") {
        out.insert("decision".into(), json!("block"));
        if let Some(reason) = cc.get("reason").and_then(Value::as_str) {
            out.insert("reason".into(), json!(reason));
        }
    }
    Value::Object(out)
}

fn translate_stop(cc: &Value) -> Value {
    // Stop's codex schema is mostly compatible with CC: continue, decision,
    // reason, stopReason, suppressOutput, systemMessage all permitted.
    // Whitelist-pass through to drop unknown fields.
    let mut out = Map::new();
    out.insert("continue".into(), json!(pluck_continue(cc)));
    if cc.get("decision").and_then(Value::as_str) == Some("block") {
        out.insert("decision".into(), json!("block"));
    }
    for key in ["reason", "stopReason", "systemMessage"] {
        if let Some(v) = cc.get(key).and_then(Value::as_str) {
            out.insert(key.into(), json!(v));
        }
    }
    if cc.get("suppressOutput").and_then(Value::as_bool) == Some(true) {
        out.insert("suppressOutput".into(), json!(true));
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn session_start_wraps_context_in_hook_specific_output() {
        let out = translate("SessionStart", r#"{"continue":true,"context":"hello"}"#);
        let v = parse(&out);
        assert_eq!(v["continue"], json!(true));
        assert_eq!(v["hookSpecificOutput"]["hookEventName"], json!("SessionStart"));
        assert_eq!(v["hookSpecificOutput"]["additionalContext"], json!("hello"));
    }

    #[test]
    fn user_prompt_submit_wraps_context_and_carries_block_decision() {
        let out = translate(
            "UserPromptSubmit",
            r#"{"continue":true,"context":"<x>","decision":"block","reason":"why"}"#,
        );
        let v = parse(&out);
        assert_eq!(v["hookSpecificOutput"]["hookEventName"], json!("UserPromptSubmit"));
        assert_eq!(v["hookSpecificOutput"]["additionalContext"], json!("<x>"));
        assert_eq!(v["decision"], json!("block"));
        assert_eq!(v["reason"], json!("why"));
    }

    #[test]
    fn pre_tool_use_remaps_decision_and_drops_suppress_output() {
        // CC sentinel emits decision=block + suppressOutput. codex rejects
        // suppressOutput on PreToolUse and wants permissionDecision under
        // hookSpecificOutput.
        let out = translate(
            "PreToolUse",
            r#"{"continue":true,"decision":"block","stopReason":"praxic without check","suppressOutput":true}"#,
        );
        let v = parse(&out);
        assert_eq!(v["continue"], json!(true));
        assert_eq!(v["hookSpecificOutput"]["hookEventName"], json!("PreToolUse"));
        assert_eq!(v["hookSpecificOutput"]["permissionDecision"], json!("block"));
        assert_eq!(
            v["hookSpecificOutput"]["permissionDecisionReason"],
            json!("praxic without check")
        );
        // suppressOutput must NOT survive — codex's PreToolUse schema rejects it.
        assert!(v.get("suppressOutput").is_none());
        // decision must NOT be at top level — codex puts it inside hookSpecificOutput.
        assert!(v.get("decision").is_none());
    }

    #[test]
    fn pre_tool_use_approve_passes_through() {
        let out = translate(
            "PreToolUse",
            r#"{"continue":true,"decision":"approve","stopReason":"sentinel allowed"}"#,
        );
        let v = parse(&out);
        assert_eq!(v["hookSpecificOutput"]["permissionDecision"], json!("approve"));
        assert_eq!(
            v["hookSpecificOutput"]["permissionDecisionReason"],
            json!("sentinel allowed")
        );
    }

    #[test]
    fn empty_input_yields_minimal_continue_true() {
        let out = translate("SessionStart", "");
        let v = parse(&out);
        assert_eq!(v["continue"], json!(true));
        // hookSpecificOutput is still present so codex can route it as a SessionStart
        assert_eq!(v["hookSpecificOutput"]["hookEventName"], json!("SessionStart"));
    }

    #[test]
    fn malformed_input_yields_continue_true() {
        let out = translate("PreToolUse", "{not json}");
        let v = parse(&out);
        assert_eq!(v["continue"], json!(true));
    }

    #[test]
    fn unknown_event_returns_continue_true() {
        let out = translate("UnknownEvent", r#"{"context":"ignored"}"#);
        let v = parse(&out);
        assert_eq!(v["continue"], json!(true));
        // No hookSpecificOutput for events we don't know how to translate.
        assert!(v.get("hookSpecificOutput").is_none());
    }

    #[test]
    fn stop_passes_through_compatible_fields_only() {
        let out = translate(
            "Stop",
            r#"{"continue":true,"decision":"block","reason":"transaction open","unknownField":"drop"}"#,
        );
        let v = parse(&out);
        assert_eq!(v["decision"], json!("block"));
        assert_eq!(v["reason"], json!("transaction open"));
        assert!(v.get("unknownField").is_none());
    }
}
