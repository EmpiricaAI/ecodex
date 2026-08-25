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
fn picker_curated_slugs_are_seeded_not_fallback() {
    // Regression guard for the /model "metadata not found" + GLM-5.2 context-loss
    // bug. These slugs appear in the TUI picker (ecodex_curated_models.rs) but
    // were absent from the seed → fell through to fallback metadata (32K for the
    // glm family-prefix). Each MUST resolve via Layer 1 with the correct context
    // window and used_fallback_model_metadata=false. If a row is deleted or its
    // context drifts, this fails — don't paper over by editing the assertion,
    // re-verify the model's real window first.
    let cases: &[(&str, i64)] = &[
        // The load-bearing one: glm-5.2's real window is 1M, NOT the 32K the
        // family-prefix table would hand it. Getting this wrong makes ecodex
        // auto-compact ~30x too aggressively.
        ("z-ai/glm-5.2", 1_000_000),
        ("openrouter/auto", 200_000), // router — conservative, no fixed window
        ("anthropic/claude-opus-4.7", 1_000_000),
        ("openai/gpt-5.2-codex", 400_000),
        ("x-ai/grok-code-fast-1", 256_000),
        ("google/gemini-2.5-pro", 1_000_000),
    ];
    for (slug, expected_ctx) in cases {
        let info = model_info_from_slug(slug);
        assert!(
            !info.used_fallback_model_metadata,
            "{slug} must be seeded (Layer 1), not fallback — picker shows it, so the seed must carry it. Otherwise /model emits 'metadata not found'."
        );
        assert_eq!(
            info.context_window,
            Some(*expected_ctx),
            "{slug} context_window drifted from the seeded value"
        );
    }
}

#[test]
fn curated_entry_inherits_runtime_template_fields() {
    // base_instructions is NEVER in the lean seed — it must be inherited from
    // build_fallback_model_info (BASE_INSTRUCTIONS const), i.e. non-empty.
    let info = model_info_from_slug("moonshotai/kimi-k2.6");
    assert!(
        !info.get_model_instructions(None).is_empty(),
        "curated entry must inherit base_instructions from the runtime template"
    );
    assert_eq!(info.context_window, Some(262_144));
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
fn discovery_filter_keeps_curated_families_and_drops_noise() {
    use crate::curated_seed::discovery_keeps;
    // curated families kept
    assert!(discovery_keeps("deepseek/deepseek-r1"));
    assert!(discovery_keeps("qwen/qwen3-coder-480b"));
    assert!(discovery_keeps("moonshotai/kimi-k2.7"));
    assert!(discovery_keeps("mistralai/devstral-2512"));
    // exact seed entry always kept
    assert!(discovery_keeps("moonshotai/kimi-k2.6"));
    // non-curated general-chat families dropped
    assert!(!discovery_keeps("google/gemma-2-27b-it"));
    assert!(!discovery_keeps("meta-llama/llama-3.3-70b"));
    assert!(!discovery_keeps("microsoft/phi-4"));
    // non-coding variants within a curated family dropped
    assert!(!discovery_keeps("glm-ocr:latest"));
    assert!(!discovery_keeps("qwen/qwen2.5-vl-72b"));
    assert!(!discovery_keeps("qwen/qwen3-embedding"));
}

#[test]
fn discovery_drops_unstable_and_dated_variants() {
    use crate::curated_seed::discovery_keeps;
    // unstable/derivative variants dropped
    assert!(!discovery_keeps("deepseek/deepseek-v3.2-exp"));
    assert!(!discovery_keeps("deepseek/deepseek-r1-distill-qwen-32b"));
    assert!(!discovery_keeps("deepseek/deepseek-v3.1-terminus"));
    assert!(!discovery_keeps("qwen/qwen3-coder-preview"));
    // dated snapshot pins dropped in favor of rolling alias
    assert!(!discovery_keeps("mistralai/mistral-small-24b-instruct-2501"));
    assert!(!discovery_keeps("deepseek/deepseek-chat-v3-0324"));
    // stable flagship tiers KEPT
    assert!(discovery_keeps("deepseek/deepseek-v4-pro"));
    assert!(discovery_keeps("deepseek/deepseek-v4-flash"));
    assert!(discovery_keeps("qwen/qwen3-coder-480b"));
    // exact SEED entry with a dated suffix is ALWAYS kept (seed wins first)
    assert!(discovery_keeps("mistralai/devstral-2512"));
}

#[test]
fn collapse_keeps_latest_per_line_and_splits_distinct_lines() {
    use crate::curated_seed::collapse_to_latest_per_line;
    let slugs: Vec<String> = [
        "minimax/minimax-m1",
        "minimax/minimax-m2",
        "minimax/minimax-m2.1",
        "minimax/minimax-m2.5",
        "minimax/minimax-m3",
        // DeepSeek reasoning (r1) vs chat (v3) MUST stay separate lines:
        "deepseek/deepseek-r1",
        "deepseek/deepseek-r1-0528",
        "deepseek/deepseek-v3.1",
        "deepseek/deepseek-v3.2",
    ]
    .iter()
    .map(std::string::ToString::to_string)
    .collect();

    let out = collapse_to_latest_per_line(&slugs);
    // minimax line collapses to the single newest (m3)
    assert!(out.contains(&"minimax/minimax-m3".to_string()));
    assert!(!out.contains(&"minimax/minimax-m1".to_string()));
    assert!(!out.contains(&"minimax/minimax-m2.5".to_string()));
    // deepseek r1 and v3 are DIFFERENT lines — both survive (latest of each)
    let has_r1 = out.iter().any(|s| s.contains("deepseek-r1"));
    let has_v3 = out.iter().any(|s| s.contains("deepseek-v3"));
    assert!(has_r1, "deepseek-r1 (reasoning) line must survive: {out:?}");
    assert!(has_v3, "deepseek-v3 (chat) line must survive: {out:?}");
    // v3.2 beats v3.1
    assert!(out.iter().any(|s| s.contains("v3.2")));
    assert!(!out.iter().any(|s| s.contains("v3.1")));
}

#[test]
fn collapse_keeps_unversioned_slugs_as_own_lines() {
    use crate::curated_seed::collapse_to_latest_per_line;
    let slugs: Vec<String> = ["qwen/qwen3-coder", "moonshotai/kimi-k2"]
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    let out = collapse_to_latest_per_line(&slugs);
    assert_eq!(out.len(), 2, "distinct unversioned slugs both kept: {out:?}");
}

#[test]
fn unseeded_slug_still_falls_through_to_family_or_fallback() {
    // A slug not in the curated seed must not panic and must still resolve
    // (via family table or generic fallback).
    let info = model_info_from_slug("some-unknown-model-xyz:7b");
    assert_eq!(info.slug, "some-unknown-model-xyz:7b");
}
