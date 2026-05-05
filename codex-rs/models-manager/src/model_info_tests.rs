use super::*;
use crate::ModelsManagerConfig;
use pretty_assertions::assert_eq;

#[test]
fn reasoning_summaries_override_true_enables_support() {
    let model = model_info_from_slug("unknown-model");
    let config = ModelsManagerConfig {
        model_supports_reasoning_summaries: Some(true),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &config);
    let mut expected = model;
    expected.supports_reasoning_summaries = true;

    assert_eq!(updated, expected);
}

#[test]
fn reasoning_summaries_override_false_does_not_disable_support() {
    let mut model = model_info_from_slug("unknown-model");
    model.supports_reasoning_summaries = true;
    let config = ModelsManagerConfig {
        model_supports_reasoning_summaries: Some(false),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &config);

    assert_eq!(updated, model);
}

#[test]
fn reasoning_summaries_override_false_is_noop_when_model_is_false() {
    let model = model_info_from_slug("unknown-model");
    let config = ModelsManagerConfig {
        model_supports_reasoning_summaries: Some(false),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &config);

    assert_eq!(updated, model);
}

#[test]
fn model_context_window_override_clamps_to_max_context_window() {
    let mut model = model_info_from_slug("unknown-model");
    model.context_window = Some(273_000);
    model.max_context_window = Some(400_000);
    let config = ModelsManagerConfig {
        model_context_window: Some(500_000),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &config);
    let mut expected = model;
    expected.context_window = Some(400_000);

    assert_eq!(updated, expected);
}

#[test]
fn model_context_window_uses_model_value_without_override() {
    let mut model = model_info_from_slug("unknown-model");
    model.context_window = Some(273_000);
    model.max_context_window = Some(400_000);
    let config = ModelsManagerConfig::default();

    let updated = with_config_overrides(model.clone(), &config);

    assert_eq!(updated, model);
}

#[test]
fn open_weights_recognizer_distinguishes_qwen_coder_from_generic_qwen() {
    // Qwen Coder family has 256K native context (purpose-built for agent
    // workflows on long codebases). Generic Qwen3 / Qwen distills only get
    // 32K. Without specific recognition, qwen3-coder:latest would have been
    // assigned the 32K default, blowing the 2% skill metadata budget on
    // hosts running it via Ollama.
    for slug in [
        "qwen3-coder:latest",
        "qwen3-coder",
        "qwen-coder",
        "qwen2.5-coder",
        "qwen3-coder-30b-a3b",
        "qwencoder",
        "ollama/qwen3-coder:30b-q4",
    ] {
        let model = model_info_from_slug(slug);
        assert_eq!(
            model.context_window,
            Some(262_144),
            "expected 256K context for slug `{slug}`, got {:?}",
            model.context_window
        );
        assert!(
            !model.used_fallback_model_metadata,
            "qwen-coder slug `{slug}` should be recognized (no fallback warning)"
        );
    }

    // qwen3-next gets the same 256K bucket
    assert_eq!(
        model_info_from_slug("qwen3-next:latest").context_window,
        Some(262_144)
    );

    // qwen2.5 (non-coder) gets 128K
    assert_eq!(
        model_info_from_slug("qwen2.5:14b").context_window,
        Some(131_072)
    );

    // Generic qwen / qwopus stays at 32K (the conservative fallback for
    // base models / community distills).
    for slug in ["qwen3:8b", "qwen3.5:latest", "qwopus:27b-q4"] {
        let model = model_info_from_slug(slug);
        assert_eq!(
            model.context_window,
            Some(32_768),
            "expected 32K conservative default for generic qwen slug `{slug}`"
        );
    }
}
