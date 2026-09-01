pub(crate) mod common;
pub mod compact;
pub mod interrupt;
pub mod permission_request;
pub mod post_tool_use;
pub mod pre_tool_use;
pub mod session_end;
pub mod session_start;
pub mod stop;
pub mod user_prompt_submit;
// ecodex additions — only the 2 events upstream lacks. SessionEnd (plus
// PreCompact/PostCompact/SubagentStart/SubagentStop) converged with upstream.
pub mod post_tool_use_failure;
pub mod task_completed;
