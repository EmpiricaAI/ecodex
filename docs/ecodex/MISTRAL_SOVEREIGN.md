# EU-sovereign models in ecodex: Mistral (Devstral / Codestral)

Run ecodex on **Mistral AI** models for two reasons that frontier US and Chinese
vendors can't offer together:

- **Data sovereignty.** Mistral is domiciled and hosted in the EU (Paris), so your
  code, prompts and context stay in the EU. That matters for teams that legally or
  contractually cannot route to US or Chinese providers: GDPR, the public sector,
  regulated industries. ecodex tags these models `jurisdiction = FR` and
  `eu_data_residency = true` in its curated registry.
- **Cost.** Mistral's coding models (Devstral, Codestral) cost far less per token
  than frontier flagships, and Devstral is a capable agentic-coding model:
  multi-step work, tool use and the full empirica frame all work end to end in
  ecodex. It is the strongest non-OpenAI model we have run in ecodex. The
  flagship is **Devstral 2** (`devstral-2512`, alias `devstral-medium-latest`,
  123B, 256K context); **Devstral Small 2** (`devstral-small-2512`, 24B) is the
  cheaper sibling you can self-host.

The models are also **open-weights**: once you are set up on the hosted API you
can move to your own EU hardware without changing anything in ecodex (see
[Full air-gap](#full-air-gap-self-hosted-open-weights)).

---

## Why a translator

ecodex speaks only the OpenAI **Responses** API; Mistral's API is **Chat
Completions**. `codex-empirica-translator`, which every ecodex install includes,
bridges the two. It serves a Responses endpoint on `127.0.0.1:18080` and
translates to and from the provider's chat protocol.

```
ecodex ──Responses──▶ translator :18080 ──chat──▶ api.mistral.ai/v1 ──▶ back
```

ecodex does not start the translator for you: you run it once per machine
session. Local servers that already speak Responses (Ollama, a llama.cpp server)
don't need it.

---

## Setup

### 1. Get a Mistral API key

Create one in the [Mistral console](https://console.mistral.ai/) (La Plateforme,
Mistral's pay-as-you-go developer API).

- **It is an API key, not a subscription.** A Le Chat subscription (Pro or Team)
  does not include API access, and there is no "sign in with your subscription"
  path into ecodex. Mistral is always metered per token, and rate limits rise
  with cumulative spend.
- **Fund the account for real work.** The free tier throttles agentic use hard
  enough to drop streams mid-turn; a funded key clears it. Devstral's
  requests-per-second ceiling is also fairly low under agentic bursts (see
  [`integrations/model-notes.md`](integrations/model-notes.md)).

### 2. Store the key

Put it in the empirica key store, `~/.empirica/credentials.yaml`. The translator
reads it from there and never prints it:

```yaml
mistral:
  api_key: <your-mistral-key>
```

Exporting `MISTRAL_API_KEY` works too and takes precedence. Never put the key in
a config file.

### 3. Start the translator

```sh
codex-empirica-translator                                     # foreground
nohup codex-empirica-translator >~/.codex/translator.log 2>&1 &   # background
```

With no flags it reads its routes from `~/.codex/translator-upstreams.toml` and
listens on `127.0.0.1:18080`. ecodex writes that file the first time you start a
session if you don't have one, with routes for `devstral-*`, `codestral-*` and
`mistral-*` to `https://api.mistral.ai/v1`. Use `--bind 0.0.0.0:18080` to share
the translator on a LAN.

Check it: `curl -s 127.0.0.1:18080/healthz` lists the three Mistral routes.

### 4. Choose a Mistral model in ecodex

The default config already has a `mistral` provider pointing at the translator,
so run `ecodex` and pick **Devstral** with `/model`. To make it the default, set
in `~/.codex/config.toml`:

```toml
model          = "devstral-latest"
model_provider = "mistral"
```

If your config predates ecodex's curated defaults and has no `mistral` provider,
add it. `base_url` targets the translator, and `wire_api` is `"responses"`
because ecodex speaks Responses to the translator:

```toml
[model_providers.mistral]
name     = "Mistral (EU — Paris) via translator :18080"
base_url = "http://localhost:18080/v1"
wire_api = "responses"
```

Run a turn; the translator's log shows the request going to `api.mistral.ai`.

---

## The EU model family

All are EU-hosted and in ecodex's curated registry:

| Model | Role | Context | Use for |
|---|---|---|---|
| **`devstral-latest`** | Agentic-coding flagship | 256K | Default for ecodex work: multi-step, tool use |
| `devstral-2512` | Pinned Devstral snapshot (alias `devstral-medium-latest`) | 256K | Reproducible, version-pinned runs |
| `devstral-small-2512` | Smaller Devstral | 256K | Lighter, cheaper coding turns |
| `codestral-latest` | Code completion | 256K | Fast completion and fill-in-the-middle |
| `mistral-large-latest` | General reasoning | large | Non-coding tasks |

There is no `devstral-2-latest` on the Mistral API; use the ids above.

---

## Context window matters

ecodex is a deep harness by design: the empirica frame (system prompt, hooks,
tools, skills catalogue) takes roughly 18K tokens before your own context. Real
work wants **200K tokens or more**, so short-context models struggle. Devstral's
256K window holds the frame plus a substantial working context, which is one
reason it is the recommended EU default.

---

## Full air-gap (self-hosted open weights)

Devstral and Codestral are open-weights. For a complete air-gap, serve them on
your own EU hardware behind an OpenAI-compatible endpoint:

- If your server speaks **Responses**, point the provider's `base_url` straight
  at it. No translator needed.
- If it speaks **chat** (most OpenAI-compatible servers do), keep the translator
  and change the routes' `base_url` in `translator-upstreams.toml` to your
  server. Nothing else in ecodex changes.

Same models, same config, no traffic leaving your network.

---

## Troubleshooting

- **The translator says `api_key_env MISTRAL_API_KEY is unset`.** The key is not
  in the environment or under `mistral.api_key` in `~/.empirica/credentials.yaml`.
- **The translator says it has no upstreams.** `~/.codex/translator-upstreams.toml`
  is missing: start an ecodex session once to create it, or pass
  `--upstreams-config <path>`.
- **`connection refused` or no response.** The translator isn't running, or the
  provider's `base_url` points at `api.mistral.ai` instead of `localhost:18080`.
- **Turns disconnect mid-stream.** Free-tier throttling; use a funded key.
- **`Model metadata not found`.** The model id isn't in the curated registry; use
  one from the table above.
- **The config fails to load.** The `mistral` provider must use
  `wire_api = "responses"` (it talks to the translator), not `"chat"`.
