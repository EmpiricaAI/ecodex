mod config_rules;
mod engine;
pub(crate) mod events;
mod legacy_notify;
mod output_spill;
mod registry;
mod schema;
mod types;

pub use engine::HookListEntry;
/// Hook event names as they appear in hooks JSON and config files.
///
/// The first 6 are codex-stock; the remaining 7 are ecodex divergences
/// mirroring CC's lifecycle surface. Dispatch sites for the ecodex
/// additions are wired in incremental PRs (see goal f0004294) —
/// declaring them in hooks.json is valid today but they won't fire
/// until the corresponding lifecycle-point patch lands.
pub const HOOK_EVENT_NAMES: [&str; 13] = [
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "SessionStart",
    "UserPromptSubmit",
    "Stop",
    // ecodex additions:
    "PreCompact",
    "PostCompact",
    "SessionEnd",
    "SubagentStart",
    "SubagentStop",
    "TaskCompleted",
    "PostToolUseFailure",
];

/// Hook event names whose matcher fields are meaningful during dispatch.
///
/// Other events can appear in hooks JSON, but Codex ignores their matcher
/// fields because those events do not dispatch against a tool or session-start
/// source.
pub const HOOK_EVENT_NAMES_WITH_MATCHERS: [&str; 5] = [
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "SessionStart",
    // ecodex addition — PostToolUseFailure matches on tool_name like its
    // success twin.
    "PostToolUseFailure",
];
pub use events::permission_request::PermissionRequestDecision;
pub use events::permission_request::PermissionRequestOutcome;
pub use events::permission_request::PermissionRequestRequest;
pub use events::post_tool_use::PostToolUseOutcome;
pub use events::post_tool_use::PostToolUseRequest;
pub use events::pre_tool_use::PreToolUseOutcome;
pub use events::pre_tool_use::PreToolUseRequest;
pub use events::session_start::SessionStartOutcome;
pub use events::session_start::SessionStartRequest;
pub use events::session_start::SessionStartSource;
pub use events::stop::StopOutcome;
pub use events::stop::StopRequest;
// ecodex addition (goal f0004294)
pub use events::task_completed::TaskCompletedOutcome;
pub use events::task_completed::TaskCompletedRequest;
pub use events::user_prompt_submit::UserPromptSubmitOutcome;
pub use events::user_prompt_submit::UserPromptSubmitRequest;
pub use legacy_notify::legacy_notify_json;
pub use legacy_notify::notify_hook;
pub use registry::HookListOutcome;
pub use registry::Hooks;
pub use registry::HooksConfig;
pub use registry::command_from_argv;
pub use registry::list_hooks;
pub use schema::write_schema_fixtures;
pub use types::Hook;
pub use types::HookEvent;
pub use types::HookEventAfterAgent;
pub use types::HookEventAfterToolUse;
pub use types::HookPayload;
pub use types::HookResponse;
pub use types::HookResult;
pub use types::HookToolInput;
pub use types::HookToolInputLocalShell;
pub use types::HookToolKind;
