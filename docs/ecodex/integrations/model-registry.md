# Model Registry (L3)

ecodex ships a **curated model registry** so the model picker is useful out of
the box — pre-filled with coding/agentic models worth running, not a 300-entry
dump of every slug a provider exposes. You can then **discover** more models
from your own configured providers with `ecodex models refresh`.

The guiding principle is *less is more*: a good picker is short and relevant.
The registry exists to make the picker **better**, never **longer**.

## How a model is resolved

When ecodex needs `ModelInfo` for a slug, `model_info_from_slug` resolves it in
three layers, most-specific first:

1. **Exact-slug curated entry** — the bundled seed (`models.curated.json`) ∪
   your `$CODEX_HOME/models.user.json`. Carries our curated metadata.
2. **Family-prefix recognition** — `recognize_open_weights_family` (qwen, llama,
   mistral, gemma, deepseek, gpt-oss, kimi, glm, phi, …) with conservative
   context defaults. Legacy fallback for un-seeded slugs.
3. **Generic fallback** — `build_fallback_model_info`, sensible defaults +
   the "fallback model metadata" warning.

A curated entry suppresses the fallback warning and supplies precise context,
tool-use, reasoning, route, and jurisdiction metadata.

## The seed is *lean*

Curated entries carry only the **curated/epistemic** fields. The heavy
**codex-runtime** fields (`base_instructions`, `shell_type`, `truncation_policy`,
…) are **inherited** from the runtime template at load time — never
hand-authored per model. A seed entry overrides only the fields it sets
(default-inherit, explicit-wins).

```jsonc
{
  "slug": "moonshotai/kimi-k2.6",
  "display_name": "Kimi K2.6 (Moonshot)",
  "context_window": 262144,
  "supports_tools": true,
  "reasoning": { "supported": true },
  "routes": ["openrouter", "direct"],
  "jurisdiction": { "country": "CN", "eu_data_residency": false },
  "calibration_tier": "unmeasured",
  "last_verified": "2026-06-02"
}
```

### Fields

| Field | Meaning |
|---|---|
| `slug` | Exact model id as the provider/OpenRouter names it |
| `context_window` | Tokens; overrides the conservative family default |
| `supports_tools` | Function/tool calling (a hard requirement for the agent loop) |
| `reasoning.supported` | Has a thinking / high-reasoning mode. **Not always true** — e.g. Qwen3-Coder is `tools=yes / thinking=no` |
| `routes` | `local` \| `openrouter` \| `direct` — how you can actually reach it |
| `jurisdiction` | `country` + `eu_data_residency` — for data-sovereignty selection (e.g. EU users who cannot route code to US/CN providers) |
| `calibration_tier` | **Always `unmeasured` on seed.** Populated from grounded usage trajectories — never asserted upfront |
| `last_verified` | Date the slug + metadata were confirmed. Slugs churn monthly — re-verify and bump |

## `ecodex models`

```
ecodex models list
ecodex models refresh [--provider <id>]... [--dry-run] [--no-filter]
```

- **`list`** — print the resolved registry (bundled seed ∪ user overlay).
- **`refresh`** — probe each configured `[model_providers.*]` `/v1/models`
  endpoint, keep the coding/agentic models, synthesize lean entries, and write
  `$CODEX_HOME/models.user.json`. Restart ecodex to pick it up. Providers that
  are unreachable or don't expose `/v1/models` are skipped with a note, never
  fatal.
  - `--provider <id>` — probe only that provider (repeatable).
  - `--dry-run` — show what would be written without touching the file.
  - `--no-filter` — keep **all** discovered slugs (escape hatch; noisy).

### Discovery filter: curated families only

A provider's `/v1/models` typically lists hundreds of models — embeddings,
audio, vision, base checkpoints. `refresh` keeps a discovered slug **only** if:

1. it's already an exact curated-seed entry, **or**
2. its family is in the curated-families allowlist
   (`kimi, qwen, deepseek, gpt-oss, glm, minimax, mistral, mixtral, devstral,
   codestral`) **and** it carries no non-coding marker
   (`ocr, vision, -vl, embed, rerank, audio, tts, image, -base, guard, …`).

This drops general-chat families (gemma/llama/phi) and non-coding variants
(`glm-ocr`, `qwen-vl`, `qwen-embedding`) so the registry stays curated. To
widen, edit `CURATED_FAMILIES` in `models-manager/src/curated_seed.rs`.

## Maintaining the seed

`models.curated.json` is hand-maintained. Slugs and flagships move
month-to-month, so each entry carries `last_verified`. When refreshing:

- Verify the exact slug against the provider / OpenRouter model page.
- Confirm `supports_tools` and `reasoning.supported` from the model card.
- Bump `last_verified`.
- Prefer capability + availability evidence over popularity claims (adoption
  leaderboards are noisy and hard to verify).
