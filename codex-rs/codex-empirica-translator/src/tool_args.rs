//! Repairs tool-call arguments that chat models produce but codex rejects.
//!
//! Every model behind the translator is a non-OpenAI chat model, trained on
//! other tool schemas. Each repair here removes a harmless habit that would
//! otherwise make codex refuse the call outright.

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

#[cfg(test)]
#[path = "tool_args_tests.rs"]
mod tests;
