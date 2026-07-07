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

use serde_json::{Map, Value, json};

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

/// Like `pluck_context`, but also accepts the codex-native nested shape
/// `hookSpecificOutput.additionalContext`. `session-init.py` emits the final
/// codex shape directly (the session_id + a ready-to-fill PREFLIGHT template
/// under `hookSpecificOutput.additionalContext`), so the flat-only
/// `pluck_context` silently DROPPED it — a fresh model then started with no
/// session_id and no PREFLIGHT template and could not bootstrap into a
/// transaction. Flat `context` still wins when present (CC-shape hooks like
/// tool-router.py). Scoped to SessionStart on purpose: codex does NOT accept
/// `additionalContext` on PreToolUse, so `translate_pre_tool_use` must keep
/// using the flat-only `pluck_context`.
fn pluck_context_or_additional(cc: &Value) -> Option<String> {
    pluck_context(cc).or_else(|| {
        cc.get("hookSpecificOutput")
            .and_then(|h| h.get("additionalContext"))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    })
}

fn translate_session_start(cc: &Value) -> Value {
    let mut out = Map::new();
    out.insert("continue".into(), json!(pluck_continue(cc)));
    let mut hso = Map::new();
    hso.insert("hookEventName".into(), json!("SessionStart"));
    if let Some(ctx) = pluck_context_or_additional(cc) {
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
    // Permission decision. The empirica sentinel emits the codex-native shape
    // DIRECTLY — `hookSpecificOutput.permissionDecision` = "allow"|"deny"|"ask"
    // — so resolve that first. (This was the firewall-gating bug: translate
    // previously read only the legacy top-level `decision`, so the sentinel's
    // nested deny was dropped and praxic tools ran despite a block.
    // Regression-tested below.) Fall back to the legacy CC-flat top-level
    // `decision`, mapped to codex's allow/deny vocabulary — codex's
    // PreToolUsePermissionDecisionWire rejects "block"/"approve".
    let raw_decision = cc
        .get("hookSpecificOutput")
        .and_then(|h| h.get("permissionDecision"))
        .and_then(Value::as_str)
        .filter(|d| matches!(*d, "allow" | "deny" | "ask"))
        .or_else(|| {
            cc.get("decision")
                .and_then(Value::as_str)
                .and_then(|d| match d {
                    "allow" | "approve" => Some("allow"),
                    "deny" | "block" => Some("deny"),
                    "ask" => Some("ask"),
                    _ => None,
                })
        });
    // FAIL-CLOSED: codex has NO PreToolUse "ask" path — it treats
    // permissionDecision=ask as unsupported and FAILS OPEN (the tool runs).
    // empirica emits "ask" for advisory cases (e.g. the carry-over-INVESTIGATE
    // nudge at sentinel-gate.py:_check_prior_investigate). An advisory is only
    // safe when a practitioner will read+heed it; in a harness that may run an
    // arbitrary non-Claude model with no human to adjudicate, that precondition
    // fails — and codex can't even surface the nudge (it drops additionalContext
    // on PreToolUse). So normalize ask→deny here at the codex boundary: the
    // context-appropriate translation of an advisory whose heed-precondition is
    // absent is the floor, DENY. CC keeps "ask" unchanged for its interactive
    // human-override path. Ratified with empirica-autonomy 2026-06-24
    // (findings ab55ca46, a9391d3e; decision 18a98f41).
    // codex's PreToolUse contract treats only `permissionDecision:deny` (with a
    // non-empty reason) as a real decision. A bare `permissionDecision:allow`
    // is "unsupported" — codex emits a `hook (failed)` line and FAILS OPEN
    // (allow is valid ONLY paired with `updatedInput`, which the sentinel never
    // emits). So the old code's `allow` passthrough spammed an error on every
    // noetic-allowed tool call. `ask` has no codex path and fail-closes to
    // deny. Therefore: emit permissionDecision + reason ONLY for deny; a
    // sentinel `allow` (or nothing) becomes a clean OMIT, which codex reads as
    // "proceed" with no error line.
    let ask_normalized = raw_decision == Some("ask");
    let emit_deny = matches!(raw_decision, Some("deny") | Some("ask"));
    if emit_deny {
        hso.insert("permissionDecision".into(), json!("deny"));
        // Reason: prefer the nested codex-native `permissionDecisionReason`,
        // fall back to legacy top-level `stopReason` / `reason`. codex FAILS
        // OPEN on a deny carrying an empty/missing reason, so guarantee a
        // non-empty one (the gate's own when present, else a clear fallback).
        let reason = cc
            .get("hookSpecificOutput")
            .and_then(|h| h.get("permissionDecisionReason"))
            .and_then(Value::as_str)
            .or_else(|| cc.get("stopReason").and_then(Value::as_str))
            .or_else(|| cc.get("reason").and_then(Value::as_str))
            .filter(|r| !r.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| {
                if ask_normalized {
                    "ecodex: sentinel returned ASK (advisory); fail-closed to DENY — \
                     no human-adjudication path in this harness. Run CHECK with proceed \
                     before praxic actions."
                        .to_string()
                } else {
                    "ecodex firewall: action denied by gate.".to_string()
                }
            });
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
        assert_eq!(
            v["hookSpecificOutput"]["hookEventName"],
            json!("SessionStart")
        );
        assert_eq!(v["hookSpecificOutput"]["additionalContext"], json!("hello"));
    }

    #[test]
    fn session_start_reads_nested_additional_context() {
        // session-init.py emits the codex-native shape DIRECTLY:
        // hookSpecificOutput.additionalContext carrying the session_id + a
        // ready-to-fill PREFLIGHT template. The flat-only pluck_context dropped
        // it, so a fresh model never learned its session_id and could not
        // bootstrap. This is the v0.2.4 onboarding-unblocker fix.
        let out = translate(
            "SessionStart",
            r#"{"ok":true,"session_id":"785ec3ba","hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"Session ID: 785ec3ba\nRun PREFLIGHT."}}"#,
        );
        let v = parse(&out);
        assert_eq!(
            v["hookSpecificOutput"]["additionalContext"],
            json!("Session ID: 785ec3ba\nRun PREFLIGHT."),
            "SessionStart must surface session-init's nested additionalContext to the model"
        );
    }

    #[test]
    fn user_prompt_submit_wraps_context_and_carries_block_decision() {
        let out = translate(
            "UserPromptSubmit",
            r#"{"continue":true,"context":"<x>","decision":"block","reason":"why"}"#,
        );
        let v = parse(&out);
        assert_eq!(
            v["hookSpecificOutput"]["hookEventName"],
            json!("UserPromptSubmit")
        );
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
        assert_eq!(
            v["hookSpecificOutput"]["hookEventName"],
            json!("PreToolUse")
        );
        // Legacy CC "block" maps to codex's "deny" (codex rejects "block").
        assert_eq!(v["hookSpecificOutput"]["permissionDecision"], json!("deny"));
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
    fn pre_tool_use_approve_omits_permission_decision() {
        // Legacy CC "approve"/"allow" must NOT emit permissionDecision:allow —
        // codex rejects a bare allow as "unsupported" and fails it open with a
        // noisy `hook (failed)` line. The correct allow shape is to OMIT
        // permissionDecision entirely so codex cleanly proceeds.
        let out = translate(
            "PreToolUse",
            r#"{"continue":true,"decision":"approve","stopReason":"sentinel allowed"}"#,
        );
        let v = parse(&out);
        assert!(
            v["hookSpecificOutput"].get("permissionDecision").is_none(),
            "allow must omit permissionDecision (codex rejects bare allow)"
        );
        // No reason emitted either — there is no decision to justify.
        assert!(
            v["hookSpecificOutput"]
                .get("permissionDecisionReason")
                .is_none()
        );
    }

    #[test]
    fn pre_tool_use_carries_nested_permission_decision_deny() {
        // The empirica sentinel emits the codex-native shape directly:
        // hookSpecificOutput.permissionDecision = "deny". This MUST survive
        // translation or the firewall silently fails to gate — the regression
        // this guards against.
        let out = translate(
            "PreToolUse",
            r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"praxic before CHECK"}}"#,
        );
        let v = parse(&out);
        assert_eq!(v["hookSpecificOutput"]["permissionDecision"], json!("deny"));
        assert_eq!(
            v["hookSpecificOutput"]["permissionDecisionReason"],
            json!("praxic before CHECK")
        );
    }

    #[test]
    fn pre_tool_use_nested_allow_omits_permission_decision_and_drops_suppress() {
        // The sentinel's nested allow (noetic-allowed tools) must translate to a
        // clean proceed: NO permissionDecision (codex rejects bare allow as
        // "unsupported permissionDecision:allow") and NO suppressOutput (codex
        // rejects it on PreToolUse). This is the v0.2.4 fix for the
        // `hook (failed)` spam on every allowed tool call.
        let out = translate(
            "PreToolUse",
            r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow"},"suppressOutput":true}"#,
        );
        let v = parse(&out);
        assert!(
            v["hookSpecificOutput"].get("permissionDecision").is_none(),
            "nested allow must omit permissionDecision so codex does not log it as unsupported"
        );
        assert!(v.get("suppressOutput").is_none());
    }

    #[test]
    fn pre_tool_use_normalizes_ask_to_deny_nested() {
        // codex has no PreToolUse "ask" path — it fails ask OPEN. empirica's
        // advisory ask (carry-over INVESTIGATE nudge) MUST become deny at the
        // codex boundary, with the gate's own reason preserved.
        let out = translate(
            "PreToolUse",
            r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask","permissionDecisionReason":"Previous CHECK returned INVESTIGATE. Consider running CHECK with proceed before praxic actions."}}"#,
        );
        let v = parse(&out);
        assert_eq!(
            v["hookSpecificOutput"]["permissionDecision"],
            json!("deny"),
            "ask must normalize to deny or codex fails it open"
        );
        // Original advisory reason preserved (it guides the model to re-CHECK).
        assert_eq!(
            v["hookSpecificOutput"]["permissionDecisionReason"],
            json!(
                "Previous CHECK returned INVESTIGATE. Consider running CHECK with proceed before praxic actions."
            )
        );
    }

    #[test]
    fn pre_tool_use_normalizes_ask_to_deny_legacy() {
        // Legacy CC-flat decision=ask also normalizes to deny.
        let out = translate(
            "PreToolUse",
            r#"{"continue":true,"decision":"ask","reason":"need confirmation"}"#,
        );
        let v = parse(&out);
        assert_eq!(v["hookSpecificOutput"]["permissionDecision"], json!("deny"));
        assert_eq!(
            v["hookSpecificOutput"]["permissionDecisionReason"],
            json!("need confirmation")
        );
    }

    #[test]
    fn pre_tool_use_ask_without_reason_gets_nonempty_deny_reason() {
        // A deny with empty/missing reason FAILS OPEN in codex. When the gate
        // emits ask with no reason, the normalized deny must still carry a
        // non-empty reason so codex actually blocks.
        let out = translate(
            "PreToolUse",
            r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask"}}"#,
        );
        let v = parse(&out);
        assert_eq!(v["hookSpecificOutput"]["permissionDecision"], json!("deny"));
        let reason = v["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap_or("");
        assert!(
            !reason.trim().is_empty(),
            "normalized ask→deny must carry a non-empty reason or codex fails open"
        );
    }

    #[test]
    fn pre_tool_use_deny_without_reason_gets_nonempty_reason() {
        // Defense-in-depth: a bare nested deny (no reason) must not slip through
        // as a reason-less deny (which codex would fail open).
        let out = translate(
            "PreToolUse",
            r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny"}}"#,
        );
        let v = parse(&out);
        assert_eq!(v["hookSpecificOutput"]["permissionDecision"], json!("deny"));
        let reason = v["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap_or("");
        assert!(!reason.trim().is_empty());
    }

    // ── E2E firewall guard: sentinel emission → translate → codex decision ──
    //
    // These encode codex's PreToolUse BLOCK contract (verified against
    // codex-rs/hooks/src/engine/output_parser.rs + events/pre_tool_use.rs,
    // finding a9391d3e): on an exit-0 hook, codex BLOCKS the tool call iff the
    // translated stdout carries hookSpecificOutput.permissionDecision == "deny"
    // WITH a non-empty permissionDecisionReason. Allow / ask / bare-allow /
    // deny-without-reason all let the tool RUN. `codex_would_block` mirrors that
    // predicate so these tests fail if translate ever regresses to a non-gating
    // shape — the guard for BOTH the v0.2.0 silent break (permissionDecision
    // dropped) AND the advisory-ask fail-open (ask passed through).

    /// Mirror of codex's exit-0 PreToolUse block predicate.
    fn codex_would_block(translated_stdout: &str) -> bool {
        let v: Value = serde_json::from_str(translated_stdout).unwrap_or(Value::Null);
        let hso = &v["hookSpecificOutput"];
        let decision = hso.get("permissionDecision").and_then(Value::as_str);
        let reason = hso
            .get("permissionDecisionReason")
            .and_then(Value::as_str)
            .unwrap_or("");
        decision == Some("deny") && !reason.trim().is_empty()
    }

    #[test]
    fn e2e_sentinel_deny_blocks_through_translate() {
        // The core gate (no valid CHECK) emits permissionDecision=deny. This is
        // the exact chain the v0.2.0 regression broke (deny was dropped).
        let sentinel = r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"No valid CHECK found. Run CHECK after investigation."}}"#;
        assert!(
            codex_would_block(&translate("PreToolUse", sentinel)),
            "sentinel deny must translate to a codex-blocking shape"
        );
    }

    #[test]
    fn e2e_sentinel_ask_blocks_through_translate() {
        // The advisory carry-over-INVESTIGATE nudge emits ask; codex fails ask
        // OPEN, so translate must normalize it to a blocking deny.
        let sentinel = r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask","permissionDecisionReason":"Previous CHECK returned INVESTIGATE."}}"#;
        assert!(
            codex_would_block(&translate("PreToolUse", sentinel)),
            "sentinel ask must normalize to a codex-blocking deny"
        );
    }

    #[test]
    fn e2e_sentinel_allow_does_not_block() {
        let sentinel = r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow"},"suppressOutput":true}"#;
        assert!(
            !codex_would_block(&translate("PreToolUse", sentinel)),
            "sentinel allow must NOT block"
        );
    }

    #[test]
    fn e2e_legacy_block_decision_blocks_through_translate() {
        // Older CC-flat shape (decision=block) must still reach a blocking deny.
        let sentinel = r#"{"continue":true,"decision":"block","stopReason":"praxic before CHECK","suppressOutput":true}"#;
        assert!(codex_would_block(&translate("PreToolUse", sentinel)));
    }

    #[test]
    fn empty_input_yields_minimal_continue_true() {
        let out = translate("SessionStart", "");
        let v = parse(&out);
        assert_eq!(v["continue"], json!(true));
        // hookSpecificOutput is still present so codex can route it as a SessionStart
        assert_eq!(
            v["hookSpecificOutput"]["hookEventName"],
            json!("SessionStart")
        );
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
