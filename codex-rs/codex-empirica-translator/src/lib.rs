//! ecodex chat-completions ↔ Responses API translator.
//!
//! Localhost HTTP proxy that lets ecodex talk to chat-completions providers
//! (DeepSeek, Qwen, GLM, Kimi, LMStudio, vLLM, etc.) after upstream codex
//! removed `wire_api = "chat"` support in commit d2394a2494.
//!
//! Field-mapping reference: `vendored/chat_request.rs` and
//! `vendored/chat_sse.rs` (resurrected from d2394a2494^).

pub mod server;
pub mod translate;

pub use server::{run, ServerConfig};
