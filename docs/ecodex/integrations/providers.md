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
| **empirica-server** (David's local Empirica server) | `empirica-local` | `http://empirica-server:<port>/v1` | OpenAI-chat | TBD (likely none on private network) |

All are OpenAI-compatible chat completions endpoints, which codex's existing provider abstraction handles natively. No custom adapters required.

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

Drop into `~/.codex/config.toml` (or have ecodex's installer merge on first run):

```toml
# ─── Curated open-weights providers (ecodex defaults) ────────────────

[model_providers.deepseek]
name = "DeepSeek"
base_url = "https://api.deepseek.com/v1"
env_key = "DEEPSEEK_API_KEY"
env_key_instructions = "Get an API key at https://platform.deepseek.com/api_keys, then export DEEPSEEK_API_KEY in your shell."
wire_api = "responses"

[model_providers.qwen]
name = "Qwen (Alibaba Cloud)"
base_url = "https://dashscope-intl.aliyuncs.com/compatible-mode/v1"
env_key = "DASHSCOPE_API_KEY"
env_key_instructions = "Get an API key at https://dashscope.console.aliyun.com/apiKey, then export DASHSCOPE_API_KEY in your shell. Use https://dashscope.aliyuncs.com/compatible-mode/v1 for the China-mainland endpoint."
wire_api = "responses"

[model_providers.glm]
name = "GLM (Zhipu AI)"
base_url = "https://open.bigmodel.cn/api/paas/v4"
env_key = "ZHIPU_API_KEY"
env_key_instructions = "Get an API key at https://bigmodel.cn/usercenter/proj-mgmt/apikeys, then export ZHIPU_API_KEY in your shell."
wire_api = "responses"

[model_providers.kimi]
name = "Kimi (Moonshot AI)"
base_url = "https://api.moonshot.cn/v1"
env_key = "MOONSHOT_API_KEY"
env_key_instructions = "Get an API key at https://platform.moonshot.cn/console/api-keys, then export MOONSHOT_API_KEY in your shell."
wire_api = "responses"

# ─── Local LLM hosts ─────────────────────────────────────────────────
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
base_url = "http://empirica-server:8000/v1"  # adjust port to match your install
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

# Try a one-shot exec against each:
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
