use codex_protocol::openai_models::ModelPreset;
use std::convert::Infallible;

#[derive(Debug, Clone)]
pub(crate) struct ModelCatalog {
    models: Vec<ModelPreset>,
}

impl ModelCatalog {
    /// Construct a catalog from upstream registry presets, prepended with
    /// ecodex's curated picks. Upstream entries with the same `id` win
    /// (so user-extended `models.json` overrides our curated description).
    pub(crate) fn new(models: Vec<ModelPreset>) -> Self {
        let curated = crate::ecodex_curated_models::curated_presets();
        let upstream_ids: std::collections::HashSet<String> =
            models.iter().map(|m| m.id.clone()).collect();
        let mut merged: Vec<ModelPreset> = curated
            .into_iter()
            .filter(|c| !upstream_ids.contains(&c.id))
            .collect();
        merged.extend(models);
        Self { models: merged }
    }

    pub(crate) fn try_list_models(&self) -> Result<Vec<ModelPreset>, Infallible> {
        Ok(self.models.clone())
    }
}
