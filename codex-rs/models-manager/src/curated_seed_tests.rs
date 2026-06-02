//! Tests for the L3 curated seed (layer 1 of model resolution).

use crate::curated_seed::lookup;
use crate::model_info::model_info_from_slug;

#[test]
fn bundled_seed_parses_and_is_nonempty() {
    // If models.curated.json were malformed, the LazyLock would silently empty
    // and every lookup below would fall through — assert it actually loaded.
    assert!(
        lookup("moonshotai/kimi-k2.6").is_some(),
        "bundled curated seed failed to load or is missing the Kimi K2.6 anchor"
    );
}

#[test]
fn curated_exact_slug_overrides_family_fallback() {
    // qwen3-coder:30b is in BOTH the curated seed (262144 ctx, non-thinking)
    // and would match the family-prefix table. Layer 1 must win.
    let info = model_info_from_slug("qwen3-coder:30b");
    assert_eq!(info.context_window, Some(262_144));
    assert!(
        !info.used_fallback_model_metadata,
        "curated entry must suppress the fallback-metadata warning"
    );
    assert_eq!(info.display_name, "Qwen3-Coder 30B-A3B (Alibaba)");
}

#[test]
fn curated_entry_inherits_runtime_template_fields() {
    // base_instructions is NEVER in the lean seed — it must be inherited from
    // build_fallback_model_info (BASE_INSTRUCTIONS const), i.e. non-empty.
    let info = model_info_from_slug("moonshotai/kimi-k2.6");
    assert!(
        !info.base_instructions.is_empty(),
        "curated entry must inherit base_instructions from the runtime template"
    );
    assert_eq!(info.context_window, Some(262_144));
    assert!(info.supports_parallel_tool_calls);
}

#[test]
fn non_thinking_curated_model_reflects_reasoning_false() {
    // Qwen3-Coder is tools=yes / thinking=NO. The curated metadata records that;
    // here we just assert the entry resolves and carries the coder display name
    // (reasoning surfacing into ModelInfo's reasoning levels is a follow-on).
    let entry = lookup("qwen3-coder:30b").expect("seeded");
    assert_eq!(entry.reasoning.as_ref().map(|r| r.supported), Some(false));
}

#[test]
fn eu_sovereignty_entry_is_tagged() {
    let entry = lookup("mistralai/devstral-small").expect("seeded");
    let j = entry.jurisdiction.expect("devstral has jurisdiction");
    assert!(j.eu_data_residency);
    assert_eq!(j.country.as_deref(), Some("FR"));
}

#[test]
fn unseeded_slug_still_falls_through_to_family_or_fallback() {
    // A slug not in the curated seed must not panic and must still resolve
    // (via family table or generic fallback).
    let info = model_info_from_slug("some-unknown-model-xyz:7b");
    assert_eq!(info.slug, "some-unknown-model-xyz:7b");
}
