//! ecodex chat-completions ↔ Responses API translator, organized around a
//! Canonical Intermediate Format (CIF).
//!
//! ```text
//! codex → /v1/responses → [responses adapter parses to CIF]
//!                       → [chat adapter encodes from CIF]
//!                       → upstream provider /v1/chat/completions
//!                       ← [chat adapter parses chunks to CIF events]
//!                       ← [responses adapter encodes events as SSE]
//!                       → codex
//! ```
//!
//! Phase 3a: the CIF + responses + chat adapters land. Phase 3b adds an
//! `anthropic` adapter (proves the design holds at N=3). Phase 4 adds an
//! event tap so Empirica subscribers consume the CIF stream externally
//! (per David's "thin translator + external subscribers" architecture).

/// Per-protocol request/response adapters: `chat` (chat-completions),
/// `anthropic` (Messages API), and `responses` (OpenAI Responses).
/// Each adapter exposes `parse_chunk`, `encode_request`, and stream-state
/// helpers shaped to a common contract so server.rs can stay protocol-agnostic.
pub mod adapters;

/// Canonical Intermediate Format — the protocol-agnostic representation
/// every adapter parses to and encodes from. Keeps fan-out to N adapters
/// linear in cost: each one only knows CIF + its own wire shape.
pub mod cif;

/// HTTP server entry point — accepts codex's Responses-shape requests
/// on `/v1/responses`, dispatches to the configured upstream protocol
/// via the adapter chain, streams SSE back. Also exposes `/healthz`.
pub mod server;

/// Event tap — emits CIF lifecycle events (RequestStarted, ChunkParsed,
/// ToolCallEmitted, ...) to subscribers (default: NoopEmitter; opt-in
/// JsonlFileEmitter). Lets Empirica observe translation without
/// modifying the translator inner loop.
pub mod tap;

pub use server::{ServerConfig, UpstreamProtocol, run};
pub use tap::{EventEmitter, JsonlFileEmitter, NoopEmitter, TapEvent};
