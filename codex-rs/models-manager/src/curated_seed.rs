//! ecodex L3 curated model seed (layer 1 of model resolution).
//!
//! The bundled `models.curated.json` holds LEAN per-slug entries — only the
//! curated/epistemic fields (context, tools, reasoning, route, jurisdiction,
//! calibration_tier). Codex-runtime fields (base_instructions, shell_type,
//! truncation_policy, ...) are NOT in the seed; they are inherited from
//! [`crate::model_info::build_fallback_model_info`] at enrichment time.
//!
//! Resolution precedence (see `model_info::model_info_from_slug`):
//!   1. exact-slug curated entry (seed  ∪  ~/.codex/models.user.json)  ← this module
//!   2. `recognize_open_weights_family` prefix table (legacy fallback)
//!   3. `build_fallback_model_info` generic default
//!
//! `models.user.json` (written by `ecodex models refresh`) uses the SAME lean
//! schema and overlays the bundled seed (user entries win on slug collision).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;

use codex_protocol::openai_models::ModelInfo;
use serde::Deserialize;

/// Lean curated entry — the only fields a human (or `models refresh`) authors.
/// Everything else on `ModelInfo` is inherited from the runtime template.
#[derive(Debug, Clone, Deserialize)]
pub struct CuratedEntry {
    pub slug: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub context_window: Option<i64>,
    #[serde(default)]
    pub supports_tools: Option<bool>,
    #[serde(default)]
    pub reasoning: Option<ReasoningMeta>,
    #[serde(default)]
    pub routes: Vec<String>,
    #[serde(default)]
    pub jurisdiction: Option<Jurisdiction>,
    /// Always `unmeasured` on seed — populated from grounded usage, never asserted.
    #[serde(default)]
    pub calibration_tier: Option<String>,
    #[serde(default)]
    pub last_verified: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReasoningMeta {
    #[serde(default)]
    pub supported: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Jurisdiction {
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub eu_data_residency: bool,
}

#[derive(Debug, Deserialize)]
struct CuratedSeedFile {
    #[serde(default)]
    seed: Vec<CuratedEntry>,
}

/// Bundled seed, parsed once. A malformed bundled file is a build/release bug,
/// so we fail loud in debug and fall back to empty in release.
static BUNDLED_SEED: LazyLock<Vec<CuratedEntry>> = LazyLock::new(|| {
    match serde_json::from_str::<CuratedSeedFile>(include_str!("../models.curated.json")) {
        Ok(f) => f.seed,
        Err(e) => {
            debug_assert!(false, "bundled models.curated.json is malformed: {e}");
            tracing::error!("bundled models.curated.json failed to parse: {e}");
            Vec::new()
        }
    }
});

/// Merged curated table: bundled seed overlaid by the user's `models.user.json`
/// (user entries win on slug collision). Keyed by exact slug.
static CURATED_TABLE: LazyLock<HashMap<String, CuratedEntry>> = LazyLock::new(|| {
    let mut table: HashMap<String, CuratedEntry> = HashMap::new();
    for e in BUNDLED_SEED.iter().cloned() {
        table.insert(e.slug.clone(), e);
    }
    for e in load_user_models() {
        table.insert(e.slug.clone(), e); // user overlay wins
    }
    table
});

/// `~/.codex/models.user.json` — written by `ecodex models refresh`. Absent or
/// malformed is non-fatal: discovery is optional, the bundled seed still works.
fn load_user_models() -> Vec<CuratedEntry> {
    let Some(path) = user_models_path() else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    match serde_json::from_str::<CuratedSeedFile>(&text) {
        Ok(f) => f.seed,
        Err(e) => {
            tracing::warn!("{} failed to parse, ignoring: {e}", path.display());
            Vec::new()
        }
    }
}

/// `$CODEX_HOME/models.user.json`, honoring the `CODEX_HOME` override.
pub fn user_models_path() -> Option<PathBuf> {
    let home = match std::env::var_os("CODEX_HOME") {
        Some(h) if !h.is_empty() => PathBuf::from(h),
        _ => dirs_home()?.join(".codex"),
    };
    Some(home.join("models.user.json"))
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Look up an exact-slug curated entry (layer 1). `None` → fall through to the
/// family-prefix table.
pub fn lookup(slug: &str) -> Option<CuratedEntry> {
    CURATED_TABLE.get(slug).cloned()
}

/// Enrich a runtime-default `ModelInfo` template with curated overlay fields.
/// Default-inherit, explicit-wins: only fields the curated entry actually sets
/// override the template. `base_instructions`, `shell_type`, `truncation_policy`,
/// etc. always come from the template.
pub fn enrich(mut template: ModelInfo, e: &CuratedEntry) -> ModelInfo {
    template.slug = e.slug.clone();
    if let Some(name) = &e.display_name {
        template.display_name = name.clone();
    }
    if e.description.is_some() {
        template.description = e.description.clone();
    }
    if let Some(ctx) = e.context_window {
        template.context_window = Some(ctx);
        template.max_context_window = Some(ctx);
    }
    if let Some(tools) = e.supports_tools {
        template.supports_parallel_tool_calls = tools;
    }
    // A curated entry is, by definition, a recognized model — suppress the
    // "fallback model metadata" warning the way the family table does.
    template.used_fallback_model_metadata = false;
    template
}

#[cfg(test)]
#[path = "curated_seed_tests.rs"]
mod tests;
