//! Repairs tool calls that chat models produce but codex, or the Responses API
//! codex later replays them to, rejects.
//!
//! Every model behind the translator is a non-OpenAI chat model, trained on
//! other tool schemas. Each repair here removes a harmless habit that would
//! otherwise make codex refuse the call, or poison the session's history.

use serde_json::Value;
use std::borrow::Cow;

/// Normalises one tool call's JSON arguments before they reach codex.
///
/// codex's shell tools accept `justification` only to explain an escalation
/// request (`sandbox_permissions`) and reject a call that carries one without
/// the other. Chat models such as Devstral fill the optional field anyway, so
/// every shell call failed. On its own the field requests nothing, so dropping
/// it runs the command exactly as the model asked. Anything that isn't a JSON
/// object, or needs no repair, passes through untouched.
pub(crate) fn normalize_tool_arguments(arguments: &str) -> Cow<'_, str> {
    let Ok(Value::Object(mut fields)) = serde_json::from_str::<Value>(arguments) else {
        return Cow::Borrowed(arguments);
    };
    if fields.contains_key("sandbox_permissions") || fields.remove("justification").is_none() {
        return Cow::Borrowed(arguments);
    }
    Cow::Owned(Value::Object(fields).to_string())
}

/// Makes a tool call's name safe to keep in codex's history.
///
/// The Responses API accepts only names matching `^[a-zA-Z0-9_-]+$`, but a chat
/// model can call a tool by any string: Devstral once echoed codex's error
/// "unsupported call: empirica" back as a tool name. codex records the call as
/// given, and after a switch to an OpenAI model that history item failed every
/// request. Each disallowed character becomes `_`; codex still answers the call
/// as an unknown tool, exactly as it would have.
pub(crate) fn sanitize_tool_name(name: &str) -> Cow<'_, str> {
    let allowed = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    if name.chars().all(allowed) {
        return Cow::Borrowed(name);
    }
    Cow::Owned(
        name.chars()
            .map(|c| if allowed(c) { c } else { '_' })
            .collect(),
    )
}

#[cfg(test)]
#[path = "tool_args_tests.rs"]
mod tests;
