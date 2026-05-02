//! Per-provider wire-format adapters. Each adapter translates between
//! provider-native JSON and the canonical intermediate format (`crate::cif`).
//!
//! Phase 3a: `responses` (codex-side, request parser + SSE encoder),
//! `chat` (provider-side, request encoder + SSE chunk parser).
//!
//! Phase 3b will add `anthropic`. Phase 4 adds the event tap that emits CIF
//! `StreamEvent`s on a Unix socket for Empirica subscribers.

pub mod anthropic;
pub mod chat;
pub mod responses;
