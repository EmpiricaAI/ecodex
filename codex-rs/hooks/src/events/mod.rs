pub(crate) mod common;
pub mod compact;
pub mod permission_request;
pub mod post_tool_use;
pub mod pre_tool_use;
pub mod session_start;
pub mod stop;
pub mod user_prompt_submit;
// ecodex additions — only the 3 events upstream lacks. PreCompact/PostCompact/
// SubagentStart/SubagentStop converged with upstream (compact / session_start / stop).
pub mod post_tool_use_failure;
pub mod session_end;
pub mod task_completed;
