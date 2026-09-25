# Model notes — what works in ecodex, and how to wire it

Practical, field-tested notes on the models we've run in ecodex (and ecodex-lab, our
headless worker harness). This is a living doc — verdicts come from our own usage, not
vendor benchmarks.

## The deciding question is *auth*, then *caching* — not list price

Whether you pay per token or hold a subscription matters less than **how the provider
lets you authenticate into ecodex**:

| Auth mode | What it means for you | ecodex wiring |
|---|---|---|
| **OAuth / subscription sign-in** | Your existing subscription "just works" — sign in, no API key, no metered key to manage | codex device-auth flow (`~/.codex/auth.json`); `requires_openai_auth = true` on the provider |
| **API key (pay-as-you-go or plan-priced)** | You hold a key; billing is per token, or against a plan's quota | `env_key = "PROVIDER_API_KEY"` on the provider in `~/.codex/config.toml` |

**Where a subscription can sign in, it is usually the better deal** (predictable cost,
no key rotation). Where the provider gates its API behind a key that a consumer
subscription does *not* unlock, you are on the key path whether you like it or not —
that is a per-provider fact, not a preference.

The second question is **caching**. An agent loop resends a long, mostly unchanged
context every turn, so in our own usage the large majority of input tokens are cache
reads. A provider's *cache-read* price, and whether caching actually engages through the
route you use, decides the bill far more than its headline input price. Check that the
cache is hitting before you commit to a provider at volume.

## Provider / model matrix

| Model (provider) | Auth into ecodex | Subscription usable? | Wiring | ecodex verdict |
|---|---|---|---|---|
| **GPT-5.6 / GPT-6 families** (OpenAI) | **OAuth** — ChatGPT subscription (device auth), or API key | ✅ yes | `openai` provider, `requires_openai_auth = true`. The client version tracks the codex base so per-model gates pass | **Best** (GPT-5.6, measured). The reference bar everything else is measured against; GPT-6 models ship in the bundled catalog but are not yet measured in ecodex |
| **Devstral 2 / Devstral** (Mistral) | **API key only** (La Plateforme) | ❌ no OAuth path | Via the translator (`base_url = http://localhost:18080/v1`), `env_key = MISTRAL_API_KEY`. See [`MISTRAL_SOVEREIGN.md`](../MISTRAL_SOVEREIGN.md) | **Our workhorse — best after the OpenAI models in ecodex-lab.** Strong agentic, multi-file coding; EU-sovereign |
| **Claude** (Anthropic) | **API key only** (`x-api-key`) | ❌ no — Anthropic restricts Free/Pro/Max subscription sign-in to its own products; other tools may only use a subscription through Anthropic's Agent SDK, which ecodex does not route through | Via the translator: an upstream with `protocol = "anthropic"` and `api_key_env` naming your key | Metered API pricing. For subscription-billed Claude, use a Claude Code seat alongside ecodex |
| **GLM-5.2** (Zhipu / Z.ai) | API key | **Plan-priced key** — the GLM Coding Plan is billed as a subscription but still hands you an API key; no OAuth | `env_key`, base_url `https://api.z.ai/api/paas/v4` | Promising |
| **Kimi K3** (Moonshot) | API key | No — metered pay-as-you-go for third-party clients | `env_key`, base_url `https://api.moonshot.ai/v1` | Promising (1M context, open-weight) |
| **DeepSeek V4** (DeepSeek) | API key | No — metered only; no plan, no OAuth | `env_key`, base_url `https://api.deepseek.com` | Promising (1M context; very low cache-read price) |
| **MiniMax-M3** (MiniMax) | API key | **Plan-priced key** — Token/Coding Plan (`sk-cp-…`, quota-based) or metered (`sk-api-…`); no OAuth | `env_key`, base_url `https://api.minimax.io/v1` | Promising (1M context) |

> The "promising" models have all run in ecodex-lab and produced useful work; they sit
> below Devstral for us today, and the verdicts will sharpen as we accumulate grounded
> per-model calibration.
>
> **Auth reality:** among the non-OpenAI providers, none offers a sign-in with a
> subscription that a generic OpenAI-compatible client can consume — all are API keys
> (Bearer, or `x-api-key` for Anthropic). The two "subscription" options (Z.ai's GLM
> Coding Plan, MiniMax's plan) are **plan-priced API keys**: the ecodex wiring is the
> same as a metered key, just billed against a plan. Claude is no exception — a Claude
> subscription cannot be used from ecodex. So **only OpenAI's ChatGPT subscription signs
> in via OAuth today**; everything else is a key.

### Provider quick reference (slug · context · caching · links)

| Provider | Model slug | Context | Open-weight? | Native prompt caching | API console | Pricing / plan |
|---|---|---|---|---|---|---|
| **Zhipu / Z.ai** | `glm-5.2` | ⚠️ unverified (GLM-4.6 = 200K) | ⚠️ unverified | Yes (cached-input pricing) | [z.ai/model-api](https://z.ai/model-api) | [docs.z.ai pricing](https://docs.z.ai/guides/overview/pricing) |
| **Moonshot / Kimi** | `kimi-k3` | 1,048,576 (1M) | Yes (Modified MIT) | Yes (automatic context caching) | [platform.moonshot.ai](https://platform.moonshot.ai) | [Kimi pricing](https://platform.moonshot.ai/docs/pricing/chat) |
| **DeepSeek** | `deepseek-v4-flash` / `deepseek-v4-pro` | 1M (384K max out) | ⚠️ unverified (V3 was MIT) | Yes (disk-based automatic context caching) | [platform.deepseek.com](https://platform.deepseek.com) | [DeepSeek pricing](https://api-docs.deepseek.com/quick_start/pricing) |
| **MiniMax** | `MiniMax-M3` | 1,000,000 (1M) | ⚠️ unverified (M2 was MIT) | Yes (cache-read pricing) | [platform.minimax.io](https://platform.minimax.io) | [MiniMax token plan](https://platform.minimax.io/docs/token-plan/quickstart) |
| **Anthropic** | `claude-*` (current families) | per model | No | Yes (explicit cache-control) | [console.anthropic.com](https://console.anthropic.com) | [Anthropic pricing](https://www.anthropic.com/pricing) |

DeepSeek's `deepseek-chat` / `deepseek-reasoner` aliases were retired; use the explicit
`deepseek-v4-flash` / `deepseek-v4-pro` slugs. DeepSeek also doubles its rates during
peak hours, so the same workload costs different amounts depending on when it runs.

Every provider above offers **native prompt caching on its own API** — which is exactly
why **provider-direct beats OpenRouter** for these: you get a documented cache contract
instead of an opaque one.

> **Unverified — re-check before treating as fact:** GLM-5.2's context window; the
> open-weight status of `glm-5.2`, `deepseek-v4-flash` and `MiniMax-M3` (their
> predecessors were MIT).

## The OpenRouter caching caveat

OpenRouter is a convenient single-endpoint aggregator, but for these models
**prompt caching through OpenRouter is unreliable and expensive without manual tuning** —
the cache-control contract is opaque or absent for several providers, so repeated
long-context turns (exactly what an agent loop does) don't get the discount you'd
expect, and cost balloons.

**Consequence:** for anything you run heavily, prefer **provider-direct** — a direct key
on the provider's own endpoint, or a subscription sign-in where one exists. Keep
OpenRouter for low-volume evaluation and breadth, not the daily driver.

The same caution applies to ecodex's own translator: it converts between wire formats,
and a conversion that changes the request prefix from turn to turn defeats the
provider's cache. When you wire a new provider through the translator, confirm on the
provider's usage dashboard that cache reads are actually being billed.

## Mistral, specifically

The full lineup and the pay-as-you-go-vs-subscription split are in
[`MISTRAL_SOVEREIGN.md`](../MISTRAL_SOVEREIGN.md). The one thing to internalise here:
**a Le Chat (consumer) subscription does not grant API access** — Mistral's API is La
Plateforme, pay-as-you-go with spend-driven rate-limit tiers, and there is no
subscription bridge into ecodex. So Mistral is always the key path. Devstral is
open-weight (Modified MIT, Apache-2.0 for Devstral Small), so self-hosting the weights is
the third option — no per-token cost and no rate limits, at the price of running the GPU.

---

*Adding a model?* Record the exact provider API slug, the auth mode (OAuth, API key,
plan-priced key or self-host), the context window, whether caching engages through your
route, and a one-line grounded verdict from actual ecodex use — not a vendor benchmark.
