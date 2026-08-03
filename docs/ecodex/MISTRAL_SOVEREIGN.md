# EU-sovereign models in ecodex: Mistral (Devstral / Codestral)

Run ecodex against **Mistral AI** models for two reasons frontier US/CN vendors
can't offer together:

- **Data sovereignty.** Mistral is EU-domiciled (Paris) and EU-hosted — your
  code, prompts, and context never leave the EU. This is the answer for teams
  that legally or contractually cannot route to US/CN providers (GDPR, public
  sector, regulated industries). ecodex tags these models `jurisdiction = FR`,
  `eu_data_residency = true` in its curated registry.
- **Cost.** Mistral's coding models (Devstral, Codestral) are materially cheaper
  per token than frontier flagships, while Devstral is a genuinely capable
  agentic-coding model — verified end-to-end in ecodex (autonomous multi-step
  work, tool use, the full deep empirica frame). In practice **Devstral is our
  workhorse — the best non-OpenAI model we've run in ecodex-lab**, second only to
  the OpenAI frontier family. Current flagship: **Devstral 2** (`devstral-2512`,
  alias `devstral-medium-latest`, 123B, 256K context); the 24B **Devstral Small 2**
  (`devstral-small-2512`) is the cheap/self-hostable sibling.

Bonus: the same models are **open-weights**, so once you're set up on the hosted
API you can later self-host them on EU hardware (Ollama / vLLM) for a full
air-gap with zero config changes on the ecodex side (see [Full air-gap](#full-air-gap-self-hosted-open-weights)).

---

## Why the translator (read this first)

codex — and therefore ecodex — speaks **only** the OpenAI *Responses* API on the
wire (upstream removed the `chat` wire protocol). **Mistral's API is Chat
Completions.** They don't talk directly.

ecodex bridges them with the **`codex-empirica-translator`** (shipped on
crates.io, part of every ecodex release). It presents a *Responses* endpoint on
`127.0.0.1:18080` and translates to/from the provider's *chat* protocol via a
provider-neutral canonical intermediate format (CIF).

```
ecodex ──Responses──▶ translator :18080 ──chat──▶ api.mistral.ai/v1  ──▶ back
```

The translator is **not auto-spawned** — you start it once and point the
provider at it. (Local providers that already speak Responses — Ollama, a
llama.cpp server — do **not** need the translator.)

---

## Setup

### 1. Get a Mistral API key

From the [Mistral console](https://console.mistral.ai/) — this is **La Plateforme**,
Mistral's pay-as-you-go developer API. Two things to be clear about:

- **It's an API key, not a subscription.** A **Le Chat** consumer subscription
  (Pro / Team) does *not* grant API access, and there is **no OAuth / "sign in
  with your subscription" path** into ecodex the way ChatGPT has. With Mistral you
  are always on the metered API-key path — billing is per-token, and rate limits
  rise with cumulative spend across tiers.
- **Fund the account for real work.** The free/experiment tier throttles
  mid-stream under agentic load (recurring stream disconnects); a funded key
  clears it (clean ~14s turns in testing). Mind Devstral's relatively low
  requests-per-second ceiling under agentic bursts — see the cross-model
  [`model-notes.md`](integrations/model-notes.md).

### 2. Store the key (never inline it)

Put it in the canonical key store, `~/.empirica/credentials.yaml` — the
translator reads it from there and never prints it:

```yaml
mistral:
  api_key: <your-mistral-key>
```

### 3. Add a translator route

In `~/.codex/translator-upstreams.toml`, map the Mistral model globs to the real
API. `protocol = "chat"` selects the Chat-Completions adapter; `api_key_env`
names the env var the launcher exports from your credentials:

```toml
[[upstream]]
model_match = "devstral-*"
base_url    = "https://api.mistral.ai/v1"
protocol    = "chat"
api_key_env = "MISTRAL_API_KEY"

[[upstream]]
model_match = "codestral-*"
base_url    = "https://api.mistral.ai/v1"
protocol    = "chat"
api_key_env = "MISTRAL_API_KEY"

[[upstream]]
model_match = "mistral-*"
base_url    = "https://api.mistral.ai/v1"
protocol    = "chat"
api_key_env = "MISTRAL_API_KEY"
```

### 4. Start the translator

```sh
# foreground (binds 127.0.0.1:18080), or:
nohup ecodex-translator >~/.codex/translator.log 2>&1 &
```

`ecodex-translator` (`scripts/ecodex-translator.sh`) sources the provider keys
from `credentials.yaml`, reads the routes from `translator-upstreams.toml`, and
binds `127.0.0.1:18080`. Use `--bind 0.0.0.0:18080` to share it on a LAN.

### 5. Point ecodex's provider at the translator

In `~/.codex/config.toml` — note `base_url` targets the **translator**, and
`wire_api = "responses"` (ecodex speaks Responses to `:18080`; the translator
does the chat conversion downstream):

```toml
model          = "devstral-latest"
model_provider = "mistral"

[model_providers.mistral]
name     = "Mistral (EU — Paris) via translator :18080"
base_url = "http://localhost:18080/v1"
wire_api = "responses"
```

### 6. Launch

```sh
ecodex
```

Confirm routing in `~/.codex/translator.log` (you'll see the chat request go to
`api.mistral.ai`) and run a turn.

---

## The EU model family

All are EU-hosted and carried in ecodex's curated registry (`jurisdiction = FR`,
`eu_data_residency = true`):

| Model | Role | Context | Use for |
|---|---|---|---|
| **`devstral-latest`** | Agentic-coding flagship | 256K | Default for ecodex work — multi-step, tool use |
| `devstral-2512` | Pinned Devstral snapshot | 256K | Reproducible / version-pinned runs |
| `devstral-medium-latest` | Smaller Devstral | 256K | Lighter/cheaper coding turns |
| `codestral-latest` | Code-completion tier | 256K | Fast completion / FIM |
| `mistral-large-latest` | General reasoning | large | Non-coding general tasks |

> **Model-id note:** use `devstral-latest` (and the snapshot ids above). There is
> **no** `devstral-2-latest` on the Mistral API — an older ecodex scaffold
> comment used that name; it's wrong.

---

## Context window matters

ecodex is a *deep* harness by design: the empirica epistemic frame (pinned
skills + constitution + hooks + tooling) is ~31K tokens on top of your own
context. This is intentional — it's what makes ecodex more than a generic LLM
wrapper. **Real use wants ≥200K context**, so short-context models struggle.

Devstral's **256K** window holds the full deep frame plus a substantial working
context comfortably — one reason it's the recommended EU default.

---

## Full air-gap (self-hosted open weights)

Devstral and Codestral are open-weights. For a complete air-gap, serve them on
your own EU hardware with an OpenAI-compatible endpoint:

- If your server speaks **Responses** natively, point `base_url` straight at it —
  **no translator needed**.
- If it speaks **chat** (most OpenAI-compatible servers), keep the translator and
  just change the route's `base_url` in `translator-upstreams.toml` to your
  server. Nothing else in ecodex changes.

Same models, same config surface, zero external network egress.

---

## Troubleshooting

- **Turns disconnect mid-stream** → free-tier throttling. Use a paid key.
- **`connection refused` / no response** → the translator isn't running, or the
  provider `base_url` points at `api.mistral.ai` instead of `localhost:18080`.
- **`Model metadata not found` / fallback** → the model id isn't in the curated
  registry; use one from the table above.
- **Config load hard-errors** → the provider block must be `wire_api = "responses"`
  (it points at the translator), not `"chat"`.
