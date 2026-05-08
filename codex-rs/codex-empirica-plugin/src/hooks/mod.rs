//! Codex hook event handlers.
//!
//! Each module corresponds to one codex hook event. Handlers translate
//! codex's stdin/stdout protocol to/from the Empirica Python script that
//! implements the underlying logic.

/// `PostToolUse` event handler — captures tool result, updates phase
/// counters and edited-file tracking on the empirica side.
pub mod post_tool_use;

/// `PreToolUse` event handler — the Sentinel firewall. Gates praxic
/// tool calls on the active transaction's CHECK state and the
/// investigation-proportionality budget.
pub mod pre_tool_use;

/// `SessionStart` event handler — boots empirica state for a new (or
/// resumed) codex session: writes anchor files, loads project context,
/// seeds AGENTS.md + subagents.
pub mod session_start;

/// `Stop` event handler — transaction enforcer; refuses to let the
/// session end with an open transaction (forces POSTFLIGHT first).
pub mod stop;

/// `UserPromptSubmit` event handler — context router. Detects
/// hypothesis markers, hedges, and proportional-scope cues; injects
/// epistemic-discipline guidance and arms the proportionality budget.
pub mod user_prompt_submit;
