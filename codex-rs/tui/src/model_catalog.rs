//! TUI model and collaboration inventories; refreshing models preserves the server mode catalog.

use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::openai_models::ModelPreset;
use std::convert::Infallible;

pub(crate) const LUNA_RESERVE_MODEL: &str = "gpt-reserve";
pub(crate) const LUNA_MODEL: &str = "gpt-6-luna";

pub(crate) fn model_display_name(model: &str) -> &str {
    if model.eq_ignore_ascii_case(LUNA_RESERVE_MODEL) {
        "Luna Reserve"
    } else {
        model
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ModelCatalog {
    pub(crate) models: Vec<ModelPreset>,
    pub(crate) collaboration_modes: Vec<CollaborationModeMask>,
}

impl ModelCatalog {
    /// Construct a catalog from upstream registry presets, prepended with
    /// ecodex's curated picks. Upstream entries with the same `id` win
    /// (so user-extended `models.json` overrides our curated description).
    pub(crate) fn new(models: Vec<ModelPreset>) -> Self {
        // ecodex: fold curated presets in ahead of upstream's, de-duped by id.
        let curated = crate::ecodex_curated_models::curated_presets();
        let upstream_ids: std::collections::HashSet<String> =
            models.iter().map(|m| m.id.clone()).collect();
        let mut merged: Vec<ModelPreset> = curated
            .into_iter()
            .filter(|c| !upstream_ids.contains(&c.id))
            .collect();
        merged.extend(models);
        Self {
            models: merged,
            collaboration_modes: Vec::new(),
        }
    }

    pub(crate) fn with_collaboration_modes(mut self, modes: Vec<CollaborationModeMask>) -> Self {
        self.collaboration_modes = modes;
        self
    }

    pub(crate) fn try_list_models(&self) -> Result<Vec<ModelPreset>, Infallible> {
        Ok(self.models.clone())
    }

    pub(crate) fn display_name<'a>(&'a self, model: &'a str) -> &'a str {
        self.models
            .iter()
            .find(|preset| preset.model == model)
            .map(|preset| preset.display_name.as_str())
            .unwrap_or_else(|| model_display_name(model))
    }
}
