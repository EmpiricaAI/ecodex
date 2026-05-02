# Curated Provider Defaults for ecodex

ecodex's brand identity is "Empirica's branded codex for open-weights operators." The default provider set should reflect that — first-class support for the providers most likely to be used by Llama/Qwen/DeepSeek/GLM/Kimi users, plus seamless local-LLM integration.

## Curated set

| Provider | ID | Endpoint | Wire | Auth |
|---|---|---|---|---|
| **Ollama** (local) | `oss` (built-in) | `http://localhost:11434/v1` | OpenAI-chat | none |
| **LMStudio** (local) | `lmstudio` (built-in) | `http://localhost:1234/v1` | OpenAI-chat | none |
| **DeepSeek** | `deepseek` | `https://api.deepseek.com/v1` | OpenAI-chat | `DEEPSEEK_API_KEY` |
| **Qwen** (Alibaba Cloud / Dashscope) | `qwen` | `https://dashscope-intl.aliyuncs.com/compatible-mode/v1` | OpenAI-chat | `DASHSCOPE_API_KEY` |
| **GLM** (Zhipu AI) | `glm` | `https://open.bigmodel.cn/api/paas/v4` | OpenAI-chat | `ZHIPU_API_KEY` |
| **Kimi** (Moonshot AI) | `kimi` | `https://api.moonshot.cn/v1` | OpenAI-chat | `MOONSHOT_API_KEY` |
| **empirica-server** (David's local Empirica server) | `empirica-local` | `http://empirica-server:<port>/v1` | OpenAI-chat | TBD (likely none on private network) |

All are OpenAI-compatible chat completions endpoints, which codex's existing provider abstraction handles natively. No custom adapters required.

## config.toml fragment

Drop into `~/.codex/config.toml` (or have ecodex's installer merge on first run):

```toml
# ─── Curated open-weights providers (ecodex defaults) ────────────────

[model_providers.deepseek]
name = "DeepSeek"
base_url = "https://api.deepseek.com/v1"
env_key = "DEEPSEEK_API_KEY"
env_key_instructions = "Get an API key at https://platform.deepseek.com/api_keys, then export DEEPSEEK_API_KEY in your shell."
wire_api = "chat"

[model_providers.qwen]
name = "Qwen (Alibaba Cloud)"
base_url = "https://dashscope-intl.aliyuncs.com/compatible-mode/v1"
env_key = "DASHSCOPE_API_KEY"
env_key_instructions = "Get an API key at https://dashscope.console.aliyun.com/apiKey, then export DASHSCOPE_API_KEY in your shell. Use https://dashscope.aliyuncs.com/compatible-mode/v1 for the China-mainland endpoint."
wire_api = "chat"

[model_providers.glm]
name = "GLM (Zhipu AI)"
base_url = "https://open.bigmodel.cn/api/paas/v4"
env_key = "ZHIPU_API_KEY"
env_key_instructions = "Get an API key at https://bigmodel.cn/usercenter/proj-mgmt/apikeys, then export ZHIPU_API_KEY in your shell."
wire_api = "chat"

[model_providers.kimi]
name = "Kimi (Moonshot AI)"
base_url = "https://api.moonshot.cn/v1"
env_key = "MOONSHOT_API_KEY"
env_key_instructions = "Get an API key at https://platform.moonshot.cn/console/api-keys, then export MOONSHOT_API_KEY in your shell."
wire_api = "chat"

# ─── Local LLM hosts ─────────────────────────────────────────────────
# `oss` (Ollama) and `lmstudio` are built-in providers — codex auto-discovers
# them on the standard local ports. No config required unless overriding the
# port or remote address.

# Optional: empirica-server as a local LLM gateway (private network).
[model_providers.empirica-local]
name = "empirica-server (local)"
base_url = "http://empirica-server:8000/v1"  # adjust port to match your install
env_key_instructions = "If empirica-server requires auth, set EMPIRICA_LOCAL_API_KEY and uncomment the env_key line below."
# env_key = "EMPIRICA_LOCAL_API_KEY"
wire_api = "chat"
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
| `kimi` | `kimi-k2-0905-preview` | 200k | k2 variant per David's note |
| `kimi` | `moonshot-v1-128k` | 128k | Stable production |
| `oss` (Ollama) | `llama3.3:70b` / `qwen2.5-coder:32b` / etc. | varies | Whatever you've pulled |
| `lmstudio` | (whatever model you've loaded) | varies | Local |
| `empirica-local` | (whatever empirica-server is serving) | varies | Local testbed |

**Note on Kimi-k2.6:** As of this writing the latest published Moonshot model is `kimi-k2-0905-preview`. The `k2.6` David referenced may be a newer or in-rollout version — check the Moonshot console for the exact model id when configuring.

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

# List providers (if codex exposes a listing subcommand)
ecodex --list-providers                 # if available — otherwise inspect config.toml

# Try a one-shot exec against each:
DEEPSEEK_API_KEY=sk-... ecodex exec -p deepseek -m deepseek-chat "say hi"
DASHSCOPE_API_KEY=sk-... ecodex exec -p qwen -m qwen3-coder-plus "say hi"
ZHIPU_API_KEY=sk-...    ecodex exec -p glm -m glm-4.6 "say hi"
MOONSHOT_API_KEY=sk-... ecodex exec -p kimi -m kimi-k2-0905-preview "say hi"

# Local hosts:
ollama serve & ecodex exec -p oss -m llama3.3:70b "say hi"
ecodex exec -p empirica-local -m <model> "say hi"   # against empirica-server
```

These verification steps will land in a future transaction once the live binary is built and the empirica plugin is installed.
