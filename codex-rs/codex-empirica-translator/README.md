# codex-empirica-translator

Localhost HTTP proxy that translates between codex's Responses API client and
chat-completions providers (DeepSeek, Qwen, GLM, Kimi, LMStudio, vLLM, and
any other OpenAI-compatible chat-completions endpoint).

Restores the open-weights provider story upstream removed in
[openai/codex#10157](https://github.com/openai/codex/pull/10157) (commit
`d2394a2494`, "chore: nuke chat/completions API", Feb 3 2026).

## Why this exists

Upstream codex removed `wire_api = "chat"` support and now actively rejects
it at config-deserialize time. All curated open-weights providers ecodex
ships speak chat completions, not Responses API. Without a translator,
ecodex's open-weights value proposition is dead in the water.

## Architecture

```
┌─────────┐     ┌──────────────────────┐      ┌──────────────────┐
│ ecodex  │ ──▶ │ codex-empirica-      │ ──▶  │ Provider         │
│ (codex  │ POST│ translator           │ POST │ (DeepSeek, Qwen, │
│  client)│  /v1│ (localhost:18080)    │  /v1 │  GLM, Kimi, etc) │
│         │     │                      │      │                  │
│         │ ◀── │ Responses-format SSE │ ◀──  │ chat-completions │
└─────────┘ SSE └──────────────────────┘  SSE │ SSE              │
                                              └──────────────────┘
```

ecodex's `config.toml` points the provider's `base_url` at
`http://localhost:18080/v1`. Translator receives the Responses-format
request, converts to chat-completions wire format, forwards to the real
provider, parses the chat-completions SSE stream, and re-emits as
Responses-format SSE back to ecodex.

Mirrors the architectural pattern of `codex-rs/responses-api-proxy/`
(security-isolation HTTP proxy that codex devs already accept internally).

## Resurrection lineage

The data plane (chat-completions request building + SSE parsing) is
resurrected verbatim from openai/codex git history:

| File | Source | Lines |
|------|--------|-------|
| `src/chat_request_resurrected.rs` | `git show d2394a2494^:codex-rs/codex-api/src/requests/chat.rs` | 494 |
| `src/chat_sse_resurrected.rs` | `git show d2394a2494^:codex-rs/codex-api/src/sse/chat.rs` | 717 |
| `tests_chat_completions_payload.rs.resurrected` | `git show d2394a2494^:codex-rs/core/tests/chat_completions_payload.rs` | 338 |
| `tests_chat_completions_sse.rs.resurrected` | `git show d2394a2494^:codex-rs/core/tests/chat_completions_sse.rs` | 466 |

All Apache-2.0, originally written by OpenAI engineers. Total: ~2,000 lines
of battle-tested translation logic recovered from `git`'s memory.

## Status

**Phase 1 (this commit):** crate scaffold + vendored resurrected code as
reference. Crate builds (lib + bin stubs only). The resurrected `.rs`
files are NOT wired into the module tree — they reference internal
codex-api helpers (`crate::error::ApiError`, `crate::provider::Provider`,
`crate::common::ResponseEvent`, `crate::telemetry::SseTelemetry`,
`crate::requests::headers::*`) that need to be replaced with codex-api's
public surface or vendored locally.

**Phase 2 (next session):**
- Rewrite imports in resurrected files to use `codex_api::{ResponseEvent,
  ResponseStream, build_conversation_headers}` (already pub) and vendor
  the few internal helpers that aren't pub
- Add `tiny_http` HTTP server in `src/main.rs`
- Implement `/v1/responses` POST handler that:
  1. Parses incoming Responses-format request body (`ResponsesApiRequest`)
  2. Maps to `ChatRequestBuilder` inputs (model, instructions, input chain)
  3. Dispatches via resurrected client to upstream provider's
     `/v1/chat/completions`
  4. Pipes resulting chat-completions SSE through resurrected
     `spawn_chat_stream` → `ResponseEvent` stream
  5. Re-encodes `ResponseEvent` stream as Responses-format SSE
- Wire test harness using resurrected test files

**Phase 3:**
- Lifecycle management (`ecodex` auto-spawns translator on startup,
  optional `--no-translator` flag)
- Cockpit integration hook (translator HTTP layer is the natural intercept
  point for multi-instance request observation)
- Cross-platform install (currently Linux/macOS via `~/.local/bin`,
  Windows TBD)

## License

Apache-2.0 (matches codex upstream — same license as the resurrected code).
