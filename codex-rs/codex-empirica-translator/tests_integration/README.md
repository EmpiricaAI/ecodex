# tests_integration

End-to-end integration tests for `codex-empirica-translator`. Unlike the
unit tests (which exercise one adapter or function in isolation), these
spin up real processes — translator binary + a mock chat-completions
server — and verify the full flow over HTTP.

## Files

| File | Purpose |
|---|---|
| `mock_chat_completions_server.py` | Tiny stdlib HTTP server that mimics an OpenAI-compatible `/v1/chat/completions` endpoint. Validates inbound request shape, emits a canonical 4-chunk SSE stream + `[DONE]`. Exposes `GET /__last_request` for test introspection. |
| `smoke_test.sh` | Reproducible end-to-end smoke. Spins up mock + translator, sends a Responses-format request, asserts: response stream contains the expected events, assembled text is correct, upstream saw the right request, event tap captured the lifecycle. |

## Usage

```bash
# Build the translator binary first
cd codex-rs && cargo build --release -p codex-empirica-translator

# Run the smoke test
codex-empirica-translator/tests_integration/smoke_test.sh
```

Exit 0 on pass, non-zero on fail. Designed to be CI-friendly.

## Why a mock instead of a real provider

- **Deterministic** — same canonical SSE every run, no provider drift
- **No credentials needed** — works in CI without secrets
- **No network** — works offline
- **Tests our code, not theirs** — failures point at the translator, not at provider variability
- **Reusable** — same mock can drive future tests (multi-turn, tool calls, errors)

When we want to test against a real provider, that's a separate `live_test.sh`
that takes a `--provider <name> --api-key-env <var>` and runs the same
contract checks against the live endpoint. Coming in a follow-up.
