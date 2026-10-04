# Curated Providers for ecodex

ecodex is Empirica's codex for open-weights operators, so its curated providers
lead with the open-weights clouds (DeepSeek, Qwen, GLM, Kimi, Mistral) and local
servers, without shutting out frontier models.

One constraint shapes everything below: **codex speaks only the OpenAI Responses
API.** Upstream removed `wire_api = "chat"`. A provider that serves Responses is
reached directly. A provider that speaks only Chat Completions (or Anthropic
Messages) is reached through **`codex-empirica-translator`**, which ships with
ecodex, listens on `127.0.0.1:18080`, and converts each request to the provider's
protocol and back. Nothing starts the translator for you.

## The curated set

| Provider | Provider id | Real API | How ecodex reaches it | Key |
|---|---|---|---|---|
| **Ollama** (local) | `ollama` (built-in) | `http://localhost:11434/v1` | direct | none |
| **LM Studio** (local) | `lmstudio` (built-in) | `http://localhost:1234/v1` | direct | none |
| **llama.cpp** (local) | `llamacpp` | `http://localhost:8080/v1` | direct | none |
| **vLLM** (local) | `vllm` | `http://localhost:8000/v1` | direct | optional `VLLM_API_KEY` |
| **Mistral** (EU, Paris) | `mistral` | `https://api.mistral.ai/v1` (Chat Completions) | translator | `MISTRAL_API_KEY` or `mistral.api_key` |
| **DeepSeek** | `deepseek` | `https://api.deepseek.com/v1` (Chat Completions) | translator | `DEEPSEEK_API_KEY` or `deepseek.api_key` |
| **Qwen** (Alibaba Cloud DashScope) | `qwen` | `https://dashscope-intl.aliyuncs.com/compatible-mode/v1` (Chat Completions) | translator | `DASHSCOPE_API_KEY` or `dashscope.api_key` |
| **GLM** (Zhipu AI) | `glm` | `https://open.bigmodel.cn/api/paas/v4` (Chat Completions) | translator | `ZHIPU_API_KEY` or `zhipu.api_key` |
| **Kimi** (Moonshot AI) | `kimi` | `https://api.moonshot.cn/v1` (Chat Completions) | translator | `MOONSHOT_API_KEY` or `moonshot.api_key` |
| **Anthropic** (Claude) | `anthropic` | `https://api.anthropic.com/v1` (Messages API) | translator | `ANTHROPIC_API_KEY` or `anthropic.api_key` |
| **OpenRouter** (gateway) | `openrouter` | `https://openrouter.ai/api/v1` (serves Responses) | direct | `OPENROUTER_API_KEY` |
| **Hugging Face** Inference Providers | `huggingface` | `https://router.huggingface.co/v1` (serves Responses) | direct | `HF_TOKEN` |
| **OpenAI** | `openai` (built-in) | `https://api.openai.com/v1` | direct | `OPENAI_API_KEY` or ChatGPT sign-in |

For the translated providers, the key goes to the translator, not to ecodex: an
environment variable in the shell that starts it, or the named section of
`~/.empirica/credentials.yaml` (`deepseek: {api_key: …}`). The default config
points their `base_url` at the translator, and the routes file ecodex writes
already has a route for each. All of them speak Chat Completions for certain;
whether some also serve the Responses API was not tested, and through the
translator they work either way.

**EU data sovereignty.** `mistral` is the EU-hosted cloud route: Mistral AI is
based in Paris and its API is hosted in the EU, so code stays in the EU. It is the
answer for anyone who legally or contractually cannot send code to US or Chinese
providers. Devstral (`devstral-latest`, or the pinned `devstral-2512`; agentic
coding) and Codestral (`codestral-latest`; completion) are the coding models. Both
are open-weights too, so for a full air gap you can serve them on your own EU
hardware through the local servers below. There is no `devstral-2-latest` id on the
Mistral API. [`MISTRAL_SOVEREIGN.md`](../MISTRAL_SOVEREIGN.md) is the worked setup.

**OpenAI.** ecodex leads with open-weights models but does not lock out frontier
ones. The GPT presets in the `/model` picker use the built-in `openai` provider:
choosing a bare OpenAI-family id (`gpt-*`, `chatgpt-*`, `o1`/`o3`/`o4*`, with no
`/` router prefix) switches `model_provider` to `openai`, so the request goes to
OpenAI rather than to whichever provider was active. The rule is
`openai_direct_provider` in `codex-rs/model-provider-info/src/lib.rs`, applied
by the picker (`provider_for_model`) and at startup (`startup_provider_override`
in `codex-rs/tui/src/ecodex_curated_models.rs`) when the saved model is an
OpenAI id but the saved provider is not. `openai` is a built-in id: do not
redefine it under `[model_providers]`.

### Local servers

| Server | Default base_url | Built in? | Serves |
|---|---|---|---|
| **Ollama** | `http://localhost:11434/v1` | yes, `ollama` | many models, swapped on demand |
| **LM Studio** | `http://localhost:1234/v1` | yes, `lmstudio` | the loaded model(s) |
| **llama.cpp** (`llama-server`) | `http://localhost:8080/v1` | no, `[model_providers.llamacpp]` | one model per server |
| **vLLM** (`vllm serve`) | `http://localhost:8000/v1` | no, `[model_providers.vllm]` | one model at a time |

codex finds Ollama and LM Studio on their standard ports with no config.
llama.cpp and vLLM need a `[model_providers.*]` block; the default config already
has both. All four list their models at `GET /v1/models`, so `ecodex models
refresh` can discover what each is serving.

## Using a provider through the translator

Two files are involved, both in `~/.codex/`:

1. **`translator-upstreams.toml`** maps model names to the real API. ecodex writes
   it on first start, with routes for Mistral (`devstral-*`, `codestral-*`,
   `mistral-*`), DeepSeek, Qwen, GLM, Kimi and Anthropic (`claude-*`, Messages
   API), and never overwrites your edits. A
   route whose key the translator cannot find is skipped with a warning when it
   starts; it refuses to start only when no route has a key. If your file
   predates these routes, add them (this is what ecodex ships):

   ```toml
   [[upstream]]
   name        = "deepseek"
   model_match = "deepseek-*"
   base_url    = "https://api.deepseek.com/v1"
   protocol    = "chat"                # or "anthropic"
   api_key_env = "DEEPSEEK_API_KEY"

   [[upstream]]
   name        = "qwen"
   model_match = "qwen*"
   base_url    = "https://dashscope-intl.aliyuncs.com/compatible-mode/v1"   # dashscope.aliyuncs.com in mainland China
   protocol    = "chat"
   api_key_env = "DASHSCOPE_API_KEY"

   [[upstream]]
   name        = "glm"
   model_match = "glm-*"
   base_url    = "https://open.bigmodel.cn/api/paas/v4"
   protocol    = "chat"
   api_key_env = "ZHIPU_API_KEY"

   [[upstream]]
   name        = "kimi"
   model_match = "kimi-*"
   base_url    = "https://api.moonshot.cn/v1"
   protocol    = "chat"
   api_key_env = "MOONSHOT_API_KEY"

   [[upstream]]
   name        = "moonshot"
   model_match = "moonshot-*"
   base_url    = "https://api.moonshot.cn/v1"
   protocol    = "chat"
   api_key_env = "MOONSHOT_API_KEY"

   [[upstream]]
   name        = "anthropic"
   model_match = "claude-*"
   base_url    = "https://api.anthropic.com/v1"
   protocol    = "anthropic"
   api_key_env = "ANTHROPIC_API_KEY"
   ```

   The first matching route wins, so put specific patterns before any catch-all.
   Never put a key in this file. The shipped GLM and Kimi routes use the
   mainland endpoints (`open.bigmodel.cn`, `api.moonshot.cn`); international
   accounts change `base_url` to `https://api.z.ai/api/paas/v4` and
   `https://api.moonshot.ai/v1` — same keys, same protocol.

2. **`config.toml`** points the provider at the translator instead of its API.
   The default config does this already; an older config may need it:

   ```toml
   [model_providers.deepseek]
   name = "DeepSeek via translator :18080"
   base_url = "http://localhost:18080/v1"
   wire_api = "responses"
   ```

   The translator holds the key, so the provider block needs no `env_key`.

The translator takes each key from the environment variable named in
`api_key_env`, or from the empirica key store `~/.empirica/credentials.yaml`,
where `DEEPSEEK_API_KEY` reads `deepseek: {api_key: ...}`. Then start it, with no
flags:

```sh
codex-empirica-translator
# or in the background:
nohup codex-empirica-translator >~/.codex/translator.log 2>&1 &
```

It retries a provider's rate limit (429) itself and repairs the tool-call habits
of chat models that codex would otherwise reject.

## Example models

Model ids change; `ecodex models list` shows the curated registry, and `ecodex
models refresh` discovers what your configured providers actually serve (see
[`model-registry.md`](model-registry.md)). Some useful starting points:

| Provider | Model id | Notes |
|---|---|---|
| `mistral` | `devstral-latest` | agentic coding, 256K context |
| `mistral` | `codestral-latest` | completion |
| `deepseek` | `deepseek-flash`, `deepseek-v4-pro` | current DeepSeek ids (1M context); the old `deepseek-chat` / `deepseek-reasoner` aliases are gone |
| `qwen` | `qwen3-coder-plus` | coder-tuned, strong tool use |
| `glm` | `glm-5.2` | Zhipu flagship |
| `kimi` | `kimi-k3` | Moonshot flagship; `kimi-k2.7-code-highspeed` for faster output |
| `anthropic` | `claude-sonnet-5-5`, `claude-opus-5-5` | Anthropic's speed and reasoning tiers, 1M context |
| `ollama` | `qwen3-coder:30b`, `gpt-oss:20b`, `deepseek-r1` | whatever you have pulled |
| `lmstudio`, `llamacpp`, `vllm` | the model the server is running | local |

## How the defaults reach you

The first time ecodex starts and finds no `~/.codex/config.toml`, it writes one
from its bundled default: the empirica plugin enabled, every provider block above
that needs one, and strict mode documented. If you already have a config, ecodex
only adds the `[plugins."empirica@empiricaAI"]` entry; copy any provider block you
want from `codex-rs/codex-empirica-plugin/assets/config/config.toml.default`.
`translator-upstreams.toml` is written the same way, only when missing.

The default config sets **no default model**: ecodex starts on codex's own
default until you pick a model with `/model`, pass `-m`, or set `model` and
`model_provider` at the top of `config.toml`.

## Checking a provider

```sh
ecodex models list                       # the resolved registry
ecodex models refresh --dry-run          # probe every configured provider's /v1/models
ecodex models refresh --provider vllm    # probe one provider (repeatable)
ecodex models refresh                    # write ~/.codex/models.user.json, then restart

# One-shot runs. -c picks the provider; -p would pick a profile file instead.
ecodex exec -c model_provider=mistral -m devstral-latest "say hi"     # translator running
ecodex exec -c model_provider=ollama  -m qwen3-coder:30b "say hi"     # ollama serve running
ecodex exec -c model_provider=vllm    -m <model>         "say hi"     # vllm serve running
ecodex exec -p huggingface "say hi"      # the bundled Hugging Face profile (huggingface.config.toml)
```

`-p NAME` layers `~/.codex/NAME.config.toml` over your config; ecodex ships one
such profile, `huggingface`.
