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
| **GLM-5.2** (Zhipu / Z.ai) | API key | **Plan-priced key** — GLM Coding Plan (from ~$10/mo) is billed as a subscription but still hands you an API key. No OAuth | `env_key`, base_url `https://api.z.ai/api/paas/v4` | Promising |
| **Kimi K-3** (Moonshot) | API key | No — metered pay-as-you-go only for third-party clients | `env_key`, base_url `https://api.moonshot.ai/v1` | Promising (1M ctx, open-weight) |
| **Deepseek-v4-flash** (DeepSeek) | API key | No — metered only; no plan, no OAuth | `env_key`, base_url `https://api.deepseek.com` | Promising (1M ctx) |
| **Minimax-M3** (MiniMax) | API key | **Plan-priced key** — Coding Plan (`sk-cp-…`, quota-based) or metered (`sk-api-…`). No OAuth | `env_key`, base_url `https://api.minimax.io/v1` | Promising (1M ctx) |

> The four "promising" models have all run in ecodex-lab and produced useful work;
> they sit below Devstral for us today and the verdicts will sharpen as we
> accumulate grounded per-model calibration. **Auth reality (verified 2026-08):**
> none offers a ChatGPT-style OAuth "sign in with your subscription" that a generic
> OpenAI-compatible client can consume — all four are API-key (Bearer). The two
> "subscription" options (Z.ai's GLM Coding Plan, MiniMax's Coding Plan) are
> **plan-priced API keys**, so ecodex wiring is identical to a metered key, just
> billed as a plan. Practical upshot of the OAuth rule: **only OpenAI's
> ChatGPT-subscription actually rides in via OAuth today**; everything else is a key.

### Provider quick-reference (slug · context · caching · links)

| Provider | Model slug | Context | Open-weight? | Native prompt caching | API console | Pricing / plan |
|---|---|---|---|---|---|---|
| **Zhipu / Z.ai** | `glm-5.2` | ⚠️ unverified (GLM-4.6 = 200K) | ⚠️ unverified | Yes (cached-input pricing) | [z.ai/model-api](https://z.ai/model-api) | [docs.z.ai pricing](https://docs.z.ai/guides/overview/pricing) |
| **Moonshot / Kimi** | `kimi-k3` | 1,048,576 (1M) | Yes (Modified MIT) | Yes (automatic context caching) | [platform.moonshot.ai](https://platform.moonshot.ai) | [Kimi pricing](https://platform.moonshot.ai/docs/pricing/chat) |
| **DeepSeek** | `deepseek-v4-flash` | 1M (384K max out) | ⚠️ unverified (V3 was MIT) | Yes (disk-based auto context caching) | [platform.deepseek.com](https://platform.deepseek.com) | [DeepSeek pricing](https://api-docs.deepseek.com/quick_start/pricing) |
| **MiniMax** | `MiniMax-M3` | 1,000,000 (1M) | ⚠️ unverified (M2 was MIT) | ⚠️ not documented | [platform.minimax.io](https://platform.minimax.io) | [MiniMax token/plan](https://platform.minimax.io/docs/token-plan/quickstart) |

Note the DeepSeek slug shift: the `deepseek-chat` / `deepseek-reasoner` aliases were
retired (~July 2026); current explicit slugs are `deepseek-v4-flash` / `deepseek-v4-pro`.
Every provider above offers **native prompt caching on its own API** (MiniMax
unconfirmed) — which is precisely why **provider-direct beats OpenRouter** for these:
you get a documented cache contract instead of OpenRouter's opaque one.

> **Unverified — do not treat as fact without re-checking:** GLM-5.2 context window;
> open-weight status of `glm-5.2` / `deepseek-v4-flash` / `MiniMax-M3` (predecessors
> were MIT, these flagships unconfirmed); MiniMax native caching.

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
