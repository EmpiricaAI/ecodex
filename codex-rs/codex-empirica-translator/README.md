# codex-empirica-translator

A small localhost HTTP server that lets codex's Responses API client talk to
providers that speak only **Chat Completions** (Mistral, DeepSeek, Qwen, GLM,
Kimi / Moonshot, and any other OpenAI-compatible endpoint) or the **Anthropic
Messages API**. ecodex points a provider's `base_url` at the translator; the
translator forwards each request to the real provider and streams the answer
back in Responses format.

Upstream codex removed `wire_api = "chat"`
([openai/codex#10157](https://github.com/openai/codex/pull/10157), commit
`d2394a2494`) and rejects it at config load. This crate restores the route
without touching codex's client.

Providers that already speak the Responses API do **not** go through it: OpenAI,
OpenRouter, Hugging Face, Ollama, LM Studio, llama.cpp and vLLM are reached
directly (see `docs/ecodex/integrations/providers.md`).

## Run

```sh
codex-empirica-translator
```

With no flags it:

- listens on `127.0.0.1:18080` (`--bind`, `ECODEX_TRANSLATOR_BIND`);
- reads its routes from `$CODEX_HOME/translator-upstreams.toml`
  (`--upstreams-config`, `ECODEX_TRANSLATOR_UPSTREAMS_CONFIG`). ecodex's first
  session writes the default file from
  `codex-empirica-plugin/assets/config/translator-upstreams.toml` and never
  overwrites it, so edits are kept;
- takes each route's key from the environment variable the route names, or from
  the empirica key store `~/.empirica/credentials.yaml` (`EMPIRICA_CREDENTIALS`
  overrides the path), where `MISTRAL_API_KEY` reads `mistral.api_key`,
  `DEEPSEEK_API_KEY` reads `deepseek.api_key`, and so on. **A key never goes in
  a config file.**

A route whose key is missing is skipped with a warning at startup, so the
default file can list every provider: add a key and restart. If no route has a
key the translator refuses to start and says which variables it looked for.

Nothing starts the translator for you. Keep it running alongside ecodex, for
example `nohup codex-empirica-translator >~/.codex/translator.log 2>&1 &`.

### Routes file

```toml
[[upstream]]
name        = "deepseek"            # key: deepseek.api_key
model_match = "deepseek-*"          # glob on the request's model; first match wins
base_url    = "https://api.deepseek.com/v1"
protocol    = "chat"                # or "anthropic"
api_key_env = "DEEPSEEK_API_KEY"    # omit for servers that need no auth
```

`protocol = "chat"` posts to `<base_url>/chat/completions` with
`Authorization: Bearer`; `protocol = "anthropic"` posts to `<base_url>/messages`
with `x-api-key` and `anthropic-version: 2023-06-01`. Put specific patterns
before any catch-all. A model that matches no route gets an error naming the
configured routes (ecodex shows it as *no upstream matches model*).

### Single-upstream mode

For a one-off provider, skip the routes file:

```sh
codex-empirica-translator \
  --upstream-base-url https://api.deepseek.com/v1 \
  --upstream-api-key-env DEEPSEEK_API_KEY \
  --upstream-protocol chat
```

These flags are ignored when a routes file is in use.

### ecodex side

The provider block in `~/.codex/config.toml` points at the translator and carries
no key:

```toml
[model_providers.mistral]
name     = "Mistral (EU — Paris) via translator :18080"
base_url = "http://localhost:18080/v1"
wire_api = "responses"
```

Then pick the model in `/model` (for example `devstral-latest`). The shipped
`config.toml.default` already has this block.

## What it does per request

```
codex → POST /v1/responses → [responses adapter → CIF]
                            → [chat | anthropic adapter → upstream request]
                            → provider
                            ← [adapter parses chunks → CIF events]
                            ← [responses adapter → Responses-format SSE]
                            → codex
```

- **CIF.** Every adapter parses to and encodes from a Canonical Intermediate
  Format (`src/cif.rs`), so adding a protocol costs one adapter, not N×N
  translations. Adapters live in `src/adapters/` (`responses`, `chat`,
  `anthropic`).
- **Tool-call repair** (`src/tool_args.rs`). Chat models are trained on other
  tool schemas and pick up habits codex rejects. Two repairs today: a shell
  call that carries `justification` without `sandbox_permissions` has the
  stray field dropped (otherwise every Devstral shell call failed), and a tool
  name outside `^[a-zA-Z0-9_-]+$` has each bad character rewritten to `_`
  (otherwise the Responses API rejected the whole history on the next request
  after a provider switch).
- **Rate limits.** An upstream 429 is retried up to four times with exponential
  backoff from 2 s, honouring `Retry-After` up to a 30 s cap, before the error
  is passed on. codex's own retry comes back too fast for per-second limits.
- **One thread per request**, so a slow stream or a backoff never stalls another
  session sharing the translator, or a health probe.
- **`GET /healthz`** returns 200 while the server is up. `empirica diagnose
  --frontend ecodex` probes it.
- **Event tap** (`--event-log <path>`, `ECODEX_TRANSLATOR_EVENT_LOG`). When set,
  one JSONL line per request lifecycle event (`request_started`,
  `stream_event`, `request_completed`, `request_errored`) and per CIF stream
  event is appended for external subscribers to tail. Off by default; a tap
  failure never fails a translation.

## Layout

| Path | Role |
|---|---|
| `src/main.rs` | CLI: flags, routes file / single-upstream construction, key store |
| `src/server.rs` | HTTP server, `/v1/responses`, `/healthz`, 429 retry |
| `src/upstreams.rs` | Routes file schema, glob router, key resolution |
| `src/cif.rs` | Canonical Intermediate Format |
| `src/adapters/` | `responses.rs`, `chat.rs`, `anthropic.rs` |
| `src/tool_args.rs` | Tool-call repairs |
| `src/tap.rs` | Event tap emitters (`NoopEmitter`, `JsonlFileEmitter`) |
| `vendored/` | Upstream's pre-removal chat-completions request builder, SSE parser and their tests (`git show d2394a2494^:…`), kept as reference and the source the `chat` adapter was derived from. Not compiled. |

## Tests

```sh
cargo nextest run -p codex-empirica-translator
```

Unit tests sit next to the code (`server_tests.rs`, `tool_args_tests.rs`, tests
in `upstreams.rs` and `tap.rs`).

## License

Apache-2.0, like codex upstream and the vendored code.
