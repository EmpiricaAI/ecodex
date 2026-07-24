# Curated Provider Defaults for ecodex

ecodex's brand identity is "Empirica's branded codex for open-weights operators." The default provider set should reflect that — first-class support for the providers most likely to be used by Llama/Qwen/DeepSeek/GLM/Kimi users, plus seamless local-LLM integration.

## Curated set

| Provider | ID | Endpoint | Wire | Auth |
|---|---|---|---|---|
| **Ollama** (local) | `oss` (built-in) | `http://localhost:11434/v1` | OpenAI-chat | none |
| **LMStudio** (local) | `lmstudio` (built-in) | `http://localhost:1234/v1` | OpenAI-chat | none |
| **llama.cpp** (local) | `llamacpp` | `http://localhost:8080/v1` | OpenAI-chat | none |
| **vLLM** (local) | `vllm` | `http://localhost:8000/v1` | OpenAI-chat | optional `VLLM_API_KEY` |
| **DeepSeek** | `deepseek` | `https://api.deepseek.com/v1` | OpenAI-chat | `DEEPSEEK_API_KEY` |
| **Qwen** (Alibaba Cloud / Dashscope) | `qwen` | `https://dashscope-intl.aliyuncs.com/compatible-mode/v1` | OpenAI-chat | `DASHSCOPE_API_KEY` |
| **GLM** (Zhipu AI) | `glm` | `https://open.bigmodel.cn/api/paas/v4` | OpenAI-chat | `ZHIPU_API_KEY` |
| **Kimi** (Moonshot AI) | `kimi` | `https://api.moonshot.cn/v1` | OpenAI-chat | `MOONSHOT_API_KEY` |
| **Mistral** (EU — Paris) 🇪🇺 | `mistral` | `https://api.mistral.ai/v1` | OpenAI-chat | `MISTRAL_API_KEY` |
| **OpenAI** (frontier direct) 🇺🇸 | `openai` | `https://api.openai.com/v1` | **Responses (native)** | `OPENAI_API_KEY` |
| **OpenRouter** (multi-model gateway) | `openrouter` | `https://openrouter.ai/api/v1` | **Responses (native)** | `OPENROUTER_API_KEY` |
| **empirica-server** (David's local Empirica server) | `empirica-local` | `http://empirica-server:<port>/v1` | Responses (native) | TBD (likely none on private network) |

> **⚠ The cloud *chat* providers require the translator — this is NOT "no adapters."**
> Upstream codex removed `wire_api = "chat"` (commit `d2394a2494`, "chore: nuke
> chat/completions API"); ecodex speaks **only** the OpenAI *Responses* API on the
> wire. The `Endpoint` column above is each provider's real *upstream* API — for
> deepseek/qwen/glm/kimi/mistral those are Chat-Completions endpoints that ecodex
> **cannot** call directly. They route through the **`codex-empirica-translator`**,
> which presents a Responses endpoint on `127.0.0.1:18080` and converts to/from each
> provider's chat protocol via a provider-neutral canonical intermediate format (CIF).
> The translator ships with every ecodex release (crates.io) but is **not
> auto-spawned** — you start it once and point each chat provider's `base_url` at it
> (`http://localhost:18080/v1`, `wire_api = "responses"`), NOT at the real API. The
> **Responses-native** providers — `openai`, `openrouter`, and the local backends
> (Ollama, LM Studio, llama.cpp/vLLM, empirica-server) — speak Responses directly and
> need **no** translator. See [`MISTRAL_SOVEREIGN.md`](../MISTRAL_SOVEREIGN.md) for the
> canonical, worked translator setup.

**EU data-sovereignty:** `mistral` is the EU-hosted cloud route — Mistral AI is EU-domiciled (Paris), so code never leaves the EU. It's the answer for users who legally/contractually cannot route to US (DeepSeek-via-OpenRouter, OpenAI) or CN (DeepSeek/Qwen/GLM/Kimi direct) providers. Devstral 2 (`devstral-latest` — or the pinned snapshot `devstral-2512`, agentic coding) and Codestral (`codestral-latest`, completion) are the coding tiers. Both are *also* open-weights — for full air-gap, self-host them on EU hardware via the local backends below (the `[EU]`-tagged entries in `ecodex models`). Note: there is **no** `devstral-2-latest` id on the Mistral API — use `devstral-latest`.

**Frontier direct (`openai`):** ecodex leads with open-weights but does not lock out
frontier models. The GPT-5.x presets in the `/model` picker — `gpt-5.6-sol` / `-terra`
/ `-luna`, `gpt-5.5`, `gpt-5.4` — route to the `openai` provider. Selecting any bare
OpenAI-family id (`gpt-*`, `chatgpt-*`, `o1`/`o3`/`o4*` with no `/` router prefix) in
`/model` **auto-switches `model_provider` to `openai`** so the request hits
`api.openai.com` rather than whatever custom provider was active (see
`codex-rs/tui/src/ecodex_curated_models.rs` → `provider_for_model`). Requires an OpenAI
key with access to those models. These are native Responses — no translator.

### Local serving backends

The four most-used local OpenAI-compatible servers, all of which expose a
`GET /v1/models` endpoint that `ecodex models refresh` can discover from:

| Backend | Default base_url | Built-in? | Serves |
|---|---|---|---|
| **Ollama** | `http://localhost:11434/v1` | ✅ `oss` | many models, hot-swappable |
| **LM Studio** | `http://localhost:1234/v1` | ✅ `lmstudio` | the loaded model(s) |
| **llama.cpp** (`llama-server`) | `http://localhost:8080/v1` | ❌ add `[model_providers.llamacpp]` | one model per server |
| **vLLM** (`vllm serve`) | `http://localhost:8000/v1` | ❌ add `[model_providers.vllm]` | one model at a time |

Ollama and LM Studio are built in (codex auto-discovers them on the standard
ports). llama.cpp and vLLM need an explicit `[model_providers.*]` block — see
the config fragment below. After adding them, `ecodex models refresh` will
probe each and add the served coding models to your registry.

## config.toml fragment

The cloud **chat** providers (deepseek/qwen/glm/kimi/mistral) involve **two**
files, because ecodex speaks Responses and they speak Chat Completions — the
translator bridges the two (see the callout above and
[`MISTRAL_SOVEREIGN.md`](../MISTRAL_SOVEREIGN.md) for the canonical walk-through):

1. **`~/.codex/translator-upstreams.toml`** — maps model globs to the real
   upstream API + protocol + key env var. The **translator** reads this and does
   the chat conversion downstream.
2. **`~/.codex/config.toml`** — each chat provider's `base_url` points at the
   **translator** (`http://localhost:18080/v1`), `wire_api = "responses"`. The
   Responses-native providers (`openai`, `openrouter`, local backends) point
   straight at their real endpoint.

### `~/.codex/translator-upstreams.toml` — routes to the real APIs

```toml
[[upstream]]
model_match = "deepseek-*"
base_url    = "https://api.deepseek.com/v1"
protocol    = "chat"
api_key_env = "DEEPSEEK_API_KEY"   # key: https://platform.deepseek.com/api_keys

[[upstream]]
model_match = "qwen*"
base_url    = "https://dashscope-intl.aliyuncs.com/compatible-mode/v1"
protocol    = "chat"
api_key_env = "DASHSCOPE_API_KEY"  # key: https://dashscope.console.aliyun.com/apiKey
                                   # (use dashscope.aliyuncs.com for China-mainland)

[[upstream]]
model_match = "glm-*"
base_url    = "https://open.bigmodel.cn/api/paas/v4"
protocol    = "chat"
api_key_env = "ZHIPU_API_KEY"      # key: https://bigmodel.cn/usercenter/proj-mgmt/apikeys

[[upstream]]
model_match = "kimi*"
base_url    = "https://api.moonshot.cn/v1"
protocol    = "chat"
api_key_env = "MOONSHOT_API_KEY"   # key: https://platform.moonshot.cn/console/api-keys

[[upstream]]
model_match = "devstral-*"         # (also codestral-*, mistral-* — one route each)
base_url    = "https://api.mistral.ai/v1"
protocol    = "chat"
api_key_env = "MISTRAL_API_KEY"    # key: https://console.mistral.ai/api-keys
```

Store the keys named above in `~/.empirica/credentials.yaml` (the translator
sources them from there and never prints them), then start it once — it binds
`127.0.0.1:18080`:

```sh
nohup ecodex-translator >~/.codex/translator.log 2>&1 &
```

### `~/.codex/config.toml` — providers point at the translator

Drop into `~/.codex/config.toml` (or have ecodex's installer merge on first run):

```toml
# ─── Cloud chat providers (via the translator on :18080) ─────────────
# base_url targets the TRANSLATOR, not the real API — ecodex is Responses-only.
# Auth is handled by the translator (credentials.yaml), so no env_key here.

[model_providers.deepseek]
name = "DeepSeek"
base_url = "http://localhost:18080/v1"
wire_api = "responses"

[model_providers.qwen]
name = "Qwen (Alibaba Cloud)"
base_url = "http://localhost:18080/v1"
wire_api = "responses"

[model_providers.glm]
name = "GLM (Zhipu AI)"
base_url = "http://localhost:18080/v1"
wire_api = "responses"

[model_providers.kimi]
name = "Kimi (Moonshot AI)"
base_url = "http://localhost:18080/v1"
wire_api = "responses"

# EU data-sovereignty route (Mistral AI, Paris). See MISTRAL_SOVEREIGN.md.
[model_providers.mistral]
name = "Mistral (EU — Paris) via translator :18080"
base_url = "http://localhost:18080/v1"
wire_api = "responses"

# ─── Frontier direct (native Responses — NO translator) ──────────────
# openai + openrouter speak the Responses API natively, so they point straight
# at the real endpoint. Selecting a bare GPT-5.x id in /model auto-switches
# model_provider to `openai` (see ecodex_curated_models.rs::provider_for_model).

[model_providers.openai]
name = "OpenAI"
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"
env_key_instructions = "Get an API key at https://platform.openai.com/api-keys, then export OPENAI_API_KEY in your shell or add to ~/.codex/.env."
wire_api = "responses"

[model_providers.openrouter]
name = "OpenRouter"
base_url = "https://openrouter.ai/api/v1"
env_key = "OPENROUTER_API_KEY"
env_key_instructions = "Get an API key at https://openrouter.ai/keys, then export OPENROUTER_API_KEY in your shell or add to ~/.codex/.env."
wire_api = "responses"

# ─── Local LLM hosts (native Responses — NO translator) ──────────────
# `oss` (Ollama) and `lmstudio` are built-in providers — codex auto-discovers
# them on the standard local ports. No config required unless overriding the
# port or remote address. llama.cpp and vLLM are NOT built-in — add them:

[model_providers.llamacpp]
name = "llama.cpp (local)"
base_url = "http://localhost:8080/v1"   # llama-server default
wire_api = "responses"

[model_providers.vllm]
name = "vLLM (local)"
base_url = "http://localhost:8000/v1"   # vllm serve default
# env_key = "VLLM_API_KEY"              # only if started with --api-key
wire_api = "responses"

# Optional: empirica-server as a local LLM gateway (private network).
[model_providers.empirica-local]
name = "empirica-server (local)"
base_url = "http://empirica-server:11434/v1"  # Ollama on empirica-server; adjust to match your install
env_key_instructions = "If empirica-server requires auth, set EMPIRICA_LOCAL_API_KEY and uncomment the env_key line below."
# env_key = "EMPIRICA_LOCAL_API_KEY"
wire_api = "responses"
```

## Recommended models per provider

For each provider, the model strings most relevant to coding/agent workflows. Pass via `--model <id>` or set `model = "..."` in `config.toml`.

| Provider | Model id | Context | Notes |
|---|---|---|---|
| `deepseek` | `deepseek-chat` | 128k | Default chat model — strong general |
| `deepseek` | `deepseek-coder` | 128k | Code-tuned variant |
| `qwen` | `qwen3-coder-plus` | 256k | Coder-tuned, strong tool use |
| `qwen` | `qwen-max` | 32k | Flagship general |
| `glm` | `glm-4.6` | 128k | Latest Zhipu flagship |
| `glm` | `glm-4-plus` | 128k | Stable choice |
| `kimi` | `kimi-k2.6` | 262k | Current flagship (verified OpenRouter slug `moonshotai/kimi-k2.6`) |
| `oss` (Ollama) | `qwen3-coder:30b` / `gpt-oss:20b` / `deepseek-r1` | varies | Whatever you've pulled |
| `lmstudio` | (whatever model you've loaded) | varies | Local |
| `llamacpp` | (the single model `llama-server` was started with) | varies | Local |
| `vllm` | (the single model `vllm serve` was started with) | varies | Local |
| `empirica-local` | (whatever empirica-server is serving) | varies | Local testbed |

**Model registry (L3):** rather than hand-picking model ids, run
`ecodex models list` to see the curated registry and `ecodex models refresh` to
discover what your configured providers (incl. local backends) actually serve.
See `docs/ecodex/integrations/model-registry.md`.

## Recommended default

For ecodex's bundled `config.toml`:
- **`model = "deepseek-chat"`** as the default — strongest general open-weights option, generous free tier, low-friction for first-run users.
- **`model_provider = "deepseek"`** as the default provider.

User can override via `~/.codex/config.toml` or `--model` / `-c model_provider=...` flags.

> **Note:** `deepseek` is a chat provider, so this default needs the translator
> running (`ecodex-translator`) with a `deepseek-*` route in
> `translator-upstreams.toml`. For a zero-translator first run, point at a
> Responses-native provider instead (a local Ollama model, or `openai`/`openrouter`).

## Install delivery (deferred to a follow-up transaction)

Three options for shipping these to ecodex users:

| Option | Pros | Cons |
|---|---|---|
| **A. Doc-only** (this file) — user copies snippets manually | Zero install complexity | Friction on first run; users skip configuration |
| **B. Bundle a `config.toml.default`** in the ecodex install dir; on first run, copy to `~/.codex/config.toml` if absent | Low friction; clear baseline | Doesn't help users who already have a `~/.codex/config.toml` |
| **C. Bundle a `providers.toml` and have ecodex's install script merge it into `~/.codex/config.toml` on first run** (TOML merge) | Best UX; safe with existing config | Needs an install script that does TOML merge correctly |

Recommendation: **Option C for v1.1**, **Option A documented now (in this file) for v1.0** so users have a clear copy-paste path. The install-script automation is a separate transaction.

## Verification when wiring up locally

After dropping the config snippet:

```sh
# Confirm config parses
ecodex --help                          # should not error on config load

# Inspect + discover the model registry (L3):
ecodex models list                     # resolved registry (curated seed + overlay)
ecodex models refresh --dry-run        # probe every configured provider's /v1/models
ecodex models refresh --provider vllm  # probe just one (e.g. a local backend)
ecodex models refresh                  # write ~/.codex/models.user.json, then restart

# Try a one-shot exec against each. NB: the cloud CHAT providers (deepseek,
# qwen, glm, kimi, mistral) need `ecodex-translator` running first, with a
# matching route in ~/.codex/translator-upstreams.toml — the key goes in
# ~/.empirica/credentials.yaml, not on the command line. The exec lines below
# assume the translator is up. openai/openrouter/local backends need no translator.
DEEPSEEK_API_KEY=sk-... ecodex exec -p deepseek -m deepseek-chat "say hi"
DASHSCOPE_API_KEY=sk-... ecodex exec -p qwen -m qwen3-coder-plus "say hi"
ZHIPU_API_KEY=sk-...    ecodex exec -p glm -m glm-4.6 "say hi"
MOONSHOT_API_KEY=sk-... ecodex exec -p kimi -m kimi-k2.6 "say hi"

# Local backends:
ollama serve &        ecodex exec -p oss      -m qwen3-coder:30b "say hi"
llama-server -m … &   ecodex exec -p llamacpp -m <model>         "say hi"
vllm serve … &        ecodex exec -p vllm     -m <model>         "say hi"
```

See `docs/ecodex/integrations/model-registry.md` for how discovery filters and
curates what each provider serves.
