//! ecodex's curated `/model` picks, derived from the one curated catalog.
//!
//! The entries live in `models-manager/models.curated.json`: the seed that
//! also supplies model metadata (context window, tools, routes). An entry
//! appears in the picker when it carries a `picker` block (provider,
//! category, order), so a slug can no longer be offered without being seeded,
//! which is how the "metadata not found" fallback used to creep in. New
//! entries go into the seed by community PR with rationale.
//!
//! The picker's job here is routing: a curated entry names the provider the
//! session must switch to, and bare OpenAI-family ids route to the built-in
//! `openai` provider.

use codex_models_manager::curated_seed::CuratedEntry;
use codex_models_manager::curated_seed::picker_entries;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelAvailabilityNux;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ReasoningEffort;

/// Look up the provider for a curated model slug. Returns None if the
/// slug isn't in the picker — used by the picker to decide whether
/// to emit a `model_provider` override alongside the `model` override.
pub(crate) fn provider_for_slug(slug: &str) -> Option<String> {
    picker_entries()
        .into_iter()
        .find(|entry| entry.slug == slug)
        .and_then(|entry| entry.picker.map(|picker| picker.provider))
}

/// Resolve the `model_providers.<id>` a selected model must route to, so the
/// picker switches PROVIDER — not just the model name. Curated entries carry an
/// explicit provider; bare OpenAI-family ids (gpt-5.6-sol/terra/luna, gpt-5.5,
/// gpt-5.4, any `gpt-*` / `chatgpt-*` / `o1|o3|o4*` without a router prefix)
/// route to the built-in `openai` provider — otherwise selecting one leaves the
/// active custom provider (e.g. deepseek) in place and the request 404s.
/// `None` means "keep the current provider" (model isn't provider-specific).
pub(crate) fn provider_for_model(model: &str) -> Option<String> {
    if let Some(provider) = provider_for_slug(model) {
        return Some(provider);
    }
    codex_model_provider_info::openai_direct_provider(model).map(str::to_string)
}

/// The provider a TUI start should use when nothing chose one explicitly.
///
/// `exec -m` and the `/model` picker already send a bare OpenAI-family model
/// (`gpt-*`, `chatgpt-*`, `o1|o3|o4*`, no router `/`) to the built-in `openai`
/// provider. A config can still pair such a model with another provider,
/// because the model-migration prompt used to persist the model alone, and a
/// fresh start then took the pair as written: a GPT model sent to the Mistral
/// translator. Returns `Some("openai")` only when no provider was given
/// explicitly, the model is OpenAI-family and the configured provider is not
/// already `openai`; every other case keeps the configured provider.
pub(crate) fn startup_provider_override(
    explicit_provider: Option<&str>,
    model: Option<&str>,
    configured_provider: &str,
    configured_base_url: Option<&str>,
) -> Option<&'static str> {
    if explicit_provider.is_some() {
        return None;
    }
    let openai = codex_model_provider_info::openai_direct_provider(model?)?;
    if configured_provider == openai {
        return None;
    }
    // Only a chat-only bridge cannot serve an OpenAI-family model, and the
    // migration prompt only ever left such a pair behind. Bedrock Mantle,
    // Azure, or a local server running `gpt-oss` pair a GPT model with their
    // own provider on purpose, so a configured provider is kept unless it is
    // the ecodex translator. `None` means the provider is not resolved yet
    // (`-m gpt-…` before the config loads), where the model alone decides.
    match configured_base_url {
        None => Some(openai),
        Some(base_url) => is_translator_endpoint(base_url).then_some(openai),
    }
}

/// The ecodex translator's default listen address, as the shipped
/// `config.toml` and `translator-upstreams.toml` write it.
fn is_translator_endpoint(base_url: &str) -> bool {
    let host = base_url
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    host.starts_with("localhost:18080") || host.starts_with("127.0.0.1:18080")
}

/// Convert a curated entry to a `ModelPreset` so it can merge into the
/// picker alongside the upstream registry models.
fn to_preset(entry: &CuratedEntry) -> ModelPreset {
    let label = entry
        .picker
        .as_ref()
        .map(|picker| picker.category.label())
        .unwrap_or_default();
    let display_name = entry.display_name.as_deref().unwrap_or(&entry.slug);
    ModelPreset {
        id: entry.slug.clone(),
        model: entry.slug.clone(),
        display_name: format!("{label} — {display_name}"),
        description: entry.description.clone().unwrap_or_default(),
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
        model_specialty: None,
        available_access_programs: None,
    }
}

/// All curated entries as ModelPreset — ready to merge with the
/// upstream registry's models in `ModelCatalog::new`.
pub(crate) fn curated_presets() -> Vec<ModelPreset> {
    picker_entries().iter().map(to_preset).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn provider_for_slug_resolves_curated_entries() {
        assert_eq!(
            [
                provider_for_slug("kimi-k3"),
                provider_for_slug("qwen3-coder:latest"),
                provider_for_slug("openrouter/auto"),
                provider_for_slug("devstral-latest"),
                provider_for_slug("not-in-curated"),
            ],
            [
                Some("kimi".to_string()),
                Some("ollama".to_string()),
                Some("openrouter".to_string()),
                Some("mistral".to_string()),
                None,
            ]
        );
    }

    #[test]
    fn startup_provider_override_routes_only_unrouted_openai_models() {
        const TRANSLATOR: Option<&str> = Some("http://localhost:18080/v1");
        // config pairs a GPT model with the translator (the migration-prompt case)
        assert_eq!(
            startup_provider_override(
                /*explicit_provider*/ None,
                Some("gpt-6-sol"),
                "mistral",
                TRANSLATOR
            ),
            Some("openai")
        );
        // `-m gpt-…` before the config is loaded: the model alone decides
        assert_eq!(
            startup_provider_override(
                /*explicit_provider*/ None,
                Some("gpt-6-sol"),
                "",
                /*configured_base_url*/ None
            ),
            Some("openai")
        );
        // already on openai: nothing to change
        assert_eq!(
            startup_provider_override(
                /*explicit_provider*/ None,
                Some("gpt-6-sol"),
                "openai",
                /*configured_base_url*/ Some("")
            ),
            None
        );
        // an explicit provider (--oss, -c model_provider=) always wins
        assert_eq!(
            startup_provider_override(Some("mistral"), Some("gpt-6-sol"), "mistral", TRANSLATOR),
            None
        );
        // a provider that serves OpenAI models itself keeps the pair as written:
        // Bedrock Mantle (GovCloud), Azure, a local server running gpt-oss
        assert_eq!(
            [
                startup_provider_override(
                    /*explicit_provider*/ None,
                    Some("gpt-5.5"),
                    "amazon-bedrock",
                    Some("https://bedrock-mantle.us-gov-west-1.api.aws/openai/v1")
                ),
                startup_provider_override(
                    /*explicit_provider*/ None,
                    Some("gpt-5.5"),
                    "azure",
                    Some("https://example.openai.azure.com/openai")
                ),
                startup_provider_override(
                    /*explicit_provider*/ None,
                    Some("gpt-oss:20b"),
                    "ollama",
                    Some("http://localhost:11434/v1")
                ),
            ],
            [None, None, None]
        );
        // non-OpenAI models and router slugs keep their provider
        assert_eq!(
            [
                startup_provider_override(
                    /*explicit_provider*/ None,
                    Some("devstral-latest"),
                    "mistral",
                    TRANSLATOR
                ),
                startup_provider_override(
                    /*explicit_provider*/ None,
                    Some("openai/gpt-5.2-codex"),
                    "openrouter",
                    Some("https://openrouter.ai/api/v1")
                ),
                startup_provider_override(
                    /*explicit_provider*/ None, /*model*/ None, "mistral", TRANSLATOR
                ),
            ],
            [None, None, None]
        );
    }

    #[test]
    fn provider_for_model_routes_openai_family_and_curated() {
        assert_eq!(
            [
                // bare OpenAI-family upstream presets -> openai direct (the gpt-5.6 fix)
                provider_for_model("gpt-5.6-sol"),
                provider_for_model("gpt-5.6-terra"),
                provider_for_model("gpt-5.6-luna"),
                provider_for_model("gpt-5.5"),
                provider_for_model("gpt-5.4"),
                // curated entries keep their explicit provider
                provider_for_model("kimi-k3"),
                provider_for_model("devstral-latest"),
                // router-prefixed OpenAI slug routes to the router, NOT openai-direct
                provider_for_model("openai/gpt-5.2-codex"),
                // non-openai, non-curated -> keep current provider
                provider_for_model("deepseek-chat"),
            ],
            [
                Some("openai".to_string()),
                Some("openai".to_string()),
                Some("openai".to_string()),
                Some("openai".to_string()),
                Some("openai".to_string()),
                Some("kimi".to_string()),
                Some("mistral".to_string()),
                Some("openrouter".to_string()),
                None,
            ]
        );
    }

    #[test]
    fn curated_presets_follow_picker_order_and_are_unique() {
        let presets = curated_presets();
        let ids: Vec<&str> = presets.iter().map(|p| p.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "curated preset ids must be unique");
        assert_eq!(ids.first().copied(), Some("kimi-k3"));
        assert!(presets.iter().all(|p| p.show_in_picker));
    }
}
