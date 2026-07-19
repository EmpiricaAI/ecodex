//! ecodex-curated model picker entries — the recommended models for
//! epistemic-discipline work that surface at the top of `/model`.
//!
//! ecodex's audience is open-weights operators + thoughtful integrators of
//! frontier models. The default codex picker shows the OpenAI/Codex family
//! (because that's what the upstream `models.json` registry exposes with
//! `visibility=List`). For our audience, the picker should surface a
//! curated set of models that have demonstrated strength on epistemic
//! tasks (calibrated reasoning, tool calling reliability, instruction
//! following at long context, honesty about uncertainty).
//!
//! ## What "epistemic-strong" means here
//!
//! - **Reasoning trace quality**: model produces inspectable reasoning
//!   that actually informs its conclusions, not post-hoc justification.
//! - **Tool calling reliability**: passes through agent-loop scaffolds
//!   without hallucinating tool names, malforming JSON args, or losing
//!   control flow on multi-tool turns.
//! - **Long-context faithfulness**: stays grounded in the conversation
//!   even past 50K tokens (the regime where shorter-context models start
//!   to drift / re-fabricate).
//! - **Calibrated uncertainty**: when asked, names what it doesn't know
//!   instead of confabulating, and updates beliefs proportional to new
//!   evidence (the EPP-friendly pattern).
//!
//! Models that meet most of these are curated here. Models that don't
//! reach the bar — or that we haven't validated — are not listed.
//! Community PRs that add a model should include the rationale + a
//! benchmark/transcript link in the docs.
//!
//! ## Provider routing
//!
//! Each curated entry declares a `provider` (the `model_providers.<id>`
//! key from `~/.codex/config.toml`). When the user selects a curated
//! entry, the TUI emits an `OverrideTurnContext` op with both `model`
//! and `model_provider` set so routing follows the picker's intent.
//!
//! Note: provider switching currently requires session restart for the
//! new ModelClient to take effect. The picker emits the override and
//! shows a "restart ecodex to apply" notice. Mid-session ModelClient
//! hot-swap is a future enhancement.
//!
//! ## Adding a model
//!
//! 1. Add an entry below with `slug`, `display_name`, `description`
//!    (short, what's epistemically strong about it), `provider` (the
//!    `[model_providers.<id>]` block users would configure), and
//!    `category`.
//! 2. Add the corresponding `[model_providers.<id>]` block to
//!    `ecodex/config.toml.default` so users get a working starting
//!    point on install.
//! 3. Document the rationale + any link to a benchmark or transcript
//!    that demonstrates the epistemic strength.

use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelAvailabilityNux;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ReasoningEffort;

/// One curated entry — slug + provider routing + category for grouping
/// in the picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EcodexCuratedModel {
    /// Model slug as configured for the provider (e.g. "kimi-for-coding"
    /// for Kimi, "qwen3-coder:latest" for Ollama).
    pub slug: &'static str,
    /// Display name shown in the picker.
    pub display_name: &'static str,
    /// Short description — what makes this epistemically strong.
    pub description: &'static str,
    /// `model_providers.<id>` key from config.toml. Picker emits this
    /// as the `model_provider` override when the user selects this entry.
    pub provider: &'static str,
    /// Category for grouping — surfaces as section headers in the picker.
    pub category: EcodexModelCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EcodexModelCategory {
    /// Cloud-hosted, coding-tuned, frontier capability.
    CloudCoding,
    /// Cloud-hosted, reasoning-tuned, frontier capability.
    CloudReasoning,
    /// Local open-weights served via Ollama / vLLM / empirica-server.
    LocalOpenWeights,
    /// Cloud router (OpenRouter etc.) — single key, many models behind.
    CloudRouter,
}

impl EcodexModelCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::CloudCoding => "Cloud — coding-strong",
            Self::CloudReasoning => "Cloud — reasoning-strong",
            Self::LocalOpenWeights => "Local — open-weights",
            Self::CloudRouter => "Cloud — router (catchall)",
        }
    }
}

/// The curated list. Keep this tight — every entry should justify its
/// place per the criteria above. New entries by community PR with
/// rationale.
pub(crate) fn curated_models() -> Vec<EcodexCuratedModel> {
    vec![
        // ── CloudCoding ──
        EcodexCuratedModel {
            slug: "kimi-for-coding",
            display_name: "Kimi K2.6 (Moonshot)",
            description: "256K MoE, agent-tuned, strong tool-calling reliability. Routed via local translator (anthropic protocol). Subscription gated.",
            provider: "kimi",
            category: EcodexModelCategory::CloudCoding,
        },
        EcodexCuratedModel {
            slug: "claude-sonnet-4-6",
            display_name: "Claude Sonnet 4.6 (Anthropic direct)",
            description: "Frontier agent SOTA, calibrated reasoning, strong honesty about uncertainty. Pay-as-you-go via api.anthropic.com.",
            provider: "anthropic",
            category: EcodexModelCategory::CloudCoding,
        },
        EcodexCuratedModel {
            slug: "devstral-2-latest",
            display_name: "Devstral 2 (Mistral — EU sovereign)",
            description: "EU-hosted agentic-coding flagship (Mistral AI, Paris). Multi-file edits, dependency tracking. The data-sovereignty pick: code stays in the EU — for users who cannot route to US/CN providers. Also open-weights (self-hostable for full air-gap).",
            provider: "mistral",
            category: EcodexModelCategory::CloudCoding,
        },
        // ── CloudReasoning ──
        EcodexCuratedModel {
            slug: "deepseek-reasoner",
            display_name: "DeepSeek R1 / V3 (DeepSeek)",
            description: "Strong reasoning trace, 128K context, very competitive pricing. OpenAI-compat chat completions via translator.",
            provider: "deepseek",
            category: EcodexModelCategory::CloudReasoning,
        },
        // ── LocalOpenWeights (via empirica-server / Ollama) ──
        EcodexCuratedModel {
            slug: "qwen3-coder:latest",
            display_name: "Qwen3-Coder 30B-A3B (local)",
            description: "256K native context, MoE arch (~3B active), purpose-built for coding agents. Excellent for codebase work that exceeds cloud context budgets.",
            provider: "empirica-local",
            category: EcodexModelCategory::LocalOpenWeights,
        },
        EcodexCuratedModel {
            slug: "deepseek-r1:32b",
            display_name: "DeepSeek-R1 32B (local)",
            description: "Distilled reasoning model, 128K context, strong on chain-of-thought tasks. Local inference via Ollama.",
            provider: "empirica-local",
            category: EcodexModelCategory::LocalOpenWeights,
        },
        EcodexCuratedModel {
            slug: "llama3.1:70b",
            display_name: "Llama 3.1 70B (local)",
            description: "128K context, generalist baseline. Local inference; weaker at agent-loop coding than qwen3-coder, included as a control.",
            provider: "empirica-local",
            category: EcodexModelCategory::LocalOpenWeights,
        },
        // ── CloudRouter ──
        // OpenRouter is wired as a single-key gateway to many providers. The
        // entries below are OpenRouter model IDs (vendor/model). Slug catalog
        // verified live against https://openrouter.ai/api/v1/models.
        // Selection criterion: frontier capability NOT already reachable via
        // a direct provider in the curated set above (no duplication of
        // claude-sonnet, kimi, deepseek which have direct entries).
        EcodexCuratedModel {
            slug: "openrouter/auto",
            display_name: "OpenRouter — auto (best-of)",
            description: "OpenRouter's auto-routing — sends each request to whichever frontier model wins for the prompt. Single key, many models. Pay-as-you-go, mark-up over direct provider rates.",
            provider: "openrouter",
            category: EcodexModelCategory::CloudRouter,
        },
        EcodexCuratedModel {
            slug: "anthropic/claude-opus-4.7",
            display_name: "Claude Opus 4.7 (via OpenRouter)",
            description: "Anthropic's frontier Opus tier — strongest reasoning + agent calibration. 1M context. Reach Opus without separate Anthropic API key/billing.",
            provider: "openrouter",
            category: EcodexModelCategory::CloudRouter,
        },
        EcodexCuratedModel {
            slug: "openai/gpt-5.2-codex",
            display_name: "GPT-5.2 Codex (via OpenRouter)",
            description: "OpenAI's codex-tuned tier — purpose-built for coding agents, 400K context. Reach GPT-5 family without OpenAI direct provider setup.",
            provider: "openrouter",
            category: EcodexModelCategory::CloudRouter,
        },
        EcodexCuratedModel {
            slug: "x-ai/grok-code-fast-1",
            display_name: "Grok Code Fast 1 (via OpenRouter)",
            description: "xAI's fast coding tier — high throughput, 256K context, competitive on tool-use. The cheap-and-fast option when latency dominates quality.",
            provider: "openrouter",
            category: EcodexModelCategory::CloudRouter,
        },
        EcodexCuratedModel {
            slug: "google/gemini-2.5-pro",
            display_name: "Gemini 2.5 Pro (via OpenRouter)",
            description: "Google's flagship general model, 1M context. Strong on long-document analysis. Reach Gemini without separate Google Cloud setup.",
            provider: "openrouter",
            category: EcodexModelCategory::CloudRouter,
        },
    ]
}

/// Look up the provider for a curated model slug. Returns None if the
/// slug isn't in the curated list — used by the picker to decide whether
/// to emit a `model_provider` override alongside the `model` override.
pub(crate) fn provider_for_slug(slug: &str) -> Option<&'static str> {
    curated_models()
        .into_iter()
        .find(|m| m.slug == slug)
        .map(|m| m.provider)
}

/// Convert a curated entry to a `ModelPreset` so it can merge into the
/// picker alongside the upstream registry models.
pub(crate) fn to_preset(entry: &EcodexCuratedModel) -> ModelPreset {
    ModelPreset {
        id: entry.slug.to_string(),
        model: entry.slug.to_string(),
        display_name: format!("{} — {}", entry.category.label(), entry.display_name),
        description: entry.description.to_string(),
        default_reasoning_effort: ReasoningEffort::Medium,
        supported_reasoning_efforts: Vec::new(),
        supports_personality: false,
        additional_speed_tiers: Vec::new(),
        service_tiers: Vec::new(),
        default_service_tier: None,
        is_default: false,
        upgrade: None,
        show_in_picker: true,
        availability_nux: None as Option<ModelAvailabilityNux>,
        supported_in_api: true,
        input_modalities: vec![InputModality::Text],
        multi_agent_version: None,
    }
}

/// All curated entries as ModelPreset — ready to merge with the
/// upstream registry's models in `ModelCatalog::new`.
pub(crate) fn curated_presets() -> Vec<ModelPreset> {
    curated_models().iter().map(to_preset).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_for_slug_resolves_curated_entries() {
        assert_eq!(provider_for_slug("kimi-for-coding"), Some("kimi"));
        assert_eq!(
            provider_for_slug("qwen3-coder:latest"),
            Some("empirica-local")
        );
        assert_eq!(provider_for_slug("openrouter/auto"), Some("openrouter"));
        assert_eq!(provider_for_slug("devstral-2-latest"), Some("mistral"));
        assert_eq!(provider_for_slug("not-in-curated"), None);
    }

    #[test]
    fn curated_presets_are_unique_by_id() {
        let presets = curated_presets();
        let mut ids: Vec<_> = presets.iter().map(|p| p.id.as_str()).collect();
        ids.sort_unstable();
        let original_len = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), original_len, "curated preset ids must be unique");
    }

    #[test]
    fn curated_presets_show_in_picker() {
        for preset in curated_presets() {
            assert!(
                preset.show_in_picker,
                "curated preset {} must show in picker",
                preset.id
            );
        }
    }
}
