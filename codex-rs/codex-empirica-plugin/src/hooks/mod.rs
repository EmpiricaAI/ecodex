//! Codex hook event handlers.
//!
//! Each module corresponds to one codex hook event. Handlers translate
//! codex's stdin/stdout protocol to/from the Empirica Python script that
//! implements the underlying logic.

pub mod post_tool_use;
pub mod pre_tool_use;
pub mod session_start;
pub mod stop;
pub mod user_prompt_submit;
