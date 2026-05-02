//! ecodex chat-completions ↔ Responses API translator.
//!
//! Phase 1 (current): crate scaffold + vendored resurrected code from
//! upstream commit d2394a2494^ (the parent of "chore: nuke chat/completions
//! API"). The resurrected sources live alongside this lib as
//! `chat_request_resurrected.rs` and `chat_sse_resurrected.rs` — they are
//! NOT yet wired into the module tree because their imports point at
//! codex-api internal helpers (crate::error, crate::provider, crate::common,
//! crate::telemetry, crate::requests::headers) that need to be replaced
//! with codex-api's public surface or vendored.
//!
//! Phase 2 (next session): rewrite imports, add tiny_http HTTP server in
//! `src/main.rs` mirroring the responses-api-proxy pattern, expose
//! `/v1/responses` endpoint, dispatch through resurrected chat client,
//! stream Responses-format SSE back.
//!
//! See README.md for the full plan and the resurrection lineage.
