# Model notes — what works in ecodex, and how to wire it

Practical, field-tested notes on the models we've run in ecodex (and ecodex-lab,
our headless worker harness). This is a living doc — verdicts come from our own
usage, not vendor benchmarks.

## The deciding question is *auth*, not price

Whether you pay per-token or hold a subscription matters less than **how the
provider lets you authenticate into ecodex**:

| Auth mode | What it means for you | ecodex wiring |
|---|---|---|
| **OAuth / subscription login** | Your existing subscription "just works" — sign in, no API key, no metered key to manage | codex device-auth flow (`~/.codex/auth.json`); set `requires_openai_auth = true` on the provider |
| **API key (pay-as-you-go)** | You hold a metered key; billing is per-token | `env_key = "PROVIDER_API_KEY"` on the provider in `~/.codex/config.toml` |

**If a provider's subscription is usable via OAuth, subscriptions are the better
deal** (predictable cost, no key rotation). If the provider gates its API behind
a separate pay-as-you-go key that a consumer subscription does *not* unlock, you
are on the API-key path whether you like it or not — that's a per-provider fact,
not a preference.

## Provider / model matrix

| Model (provider) | Auth into ecodex | Sub usable? | Wiring | ecodex verdict |
|---|---|---|---|---|
| **gpt-5.6 family** (OpenAI) | **OAuth** — ChatGPT subscription (device auth) | ✅ yes | `openai` provider, `requires_openai_auth=true`. Version tracks the codex base so per-model gates pass | **Best.** The reference bar; everything else is measured against it |
| **Devstral 2 / Devstral** (Mistral) | **API key only** (La Plateforme) | ❌ no OAuth path | Via the translator (`base_url=http://localhost:18080/v1`), `env_key=MISTRAL_API_KEY`. See [`MISTRAL_SOVEREIGN.md`](../MISTRAL_SOVEREIGN.md) | **Our workhorse — best after the OpenAI models in ecodex-lab.** Strong agentic, multi-file coding; EU-sovereign |
| **GLM-5.2** (Zhipu) | API key (provider-direct) or OpenRouter | provider-dependent | provider-direct key preferred over OpenRouter | Promising |
| **Kimi K-3** (Moonshot) | API key (provider-direct) or OpenRouter | provider-dependent | provider-direct key preferred | Promising |
| **Deepseek-v4-flash** (DeepSeek) | API key (provider-direct) or OpenRouter | provider-dependent | provider-direct key preferred | Promising |
| **Minimax-M3** (MiniMax) | API key (provider-direct) or OpenRouter | provider-dependent | provider-direct key preferred | Promising |

> The four "promising" models above have all run in ecodex-lab and produced
> useful work. They sit below Devstral for us today; as we accumulate grounded
> per-model calibration the verdicts will sharpen. (Auth specifics — whether each
> offers an OAuth/subscription login into ecodex vs API-key-only — are still being
> confirmed per provider; treat the "Sub usable?" column as provisional.)

## The OpenRouter caching caveat

OpenRouter is a convenient single-endpoint aggregator, but for these models
**prompt/token caching through OpenRouter is unreliable and expensive without
manual tuning** — the cache-control contract is opaque or absent for several
providers, so repeated long-context turns (exactly what an agent loop does) don't
get the cache discount you'd expect, and cost balloons.

**Consequence:** for anything you run heavily, prefer **provider-direct** — either
a direct API key on the provider's own endpoint, or (where available) an
OAuth/subscription login. A provider-direct subscription or key gives predictable
caching and rate-limit behavior; OpenRouter is best kept for low-volume
evaluation and breadth, not the daily driver.

## Mistral, specifically

The full lineup + the pay-as-you-go-vs-subscription split is in
[`MISTRAL_SOVEREIGN.md`](../MISTRAL_SOVEREIGN.md). The one thing to internalize
here: **a Le Chat (consumer) subscription does not grant API access** — Mistral's
API is La Plateforme, pay-as-you-go with spend-driven rate-limit tiers, and there
is no OAuth-subscription bridge into ecodex. So Mistral is always the API-key
path. Devstral is open-weight (Modified MIT / Apache-2.0 for Small), so
self-hosting the weights is the third option — zero per-token cost and no rate
limits, at the price of running the GPU.

---

*Adding a model?* Record: the exact provider API slug, the auth mode (OAuth vs
API key vs self-host), context window, and a one-line grounded verdict from
actual ecodex use — not a vendor benchmark.
