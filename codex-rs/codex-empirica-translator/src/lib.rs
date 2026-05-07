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

pub mod adapters;
pub mod cif;
pub mod server;
pub mod tap;

pub use server::{ServerConfig, UpstreamProtocol, run};
pub use tap::{EventEmitter, JsonlFileEmitter, NoopEmitter, TapEvent};
