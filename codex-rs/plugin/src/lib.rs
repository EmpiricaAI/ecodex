//! Shared plugin identifiers and telemetry-facing summaries.

pub use codex_utils_plugins::mention_syntax;
pub use codex_utils_plugins::plugin_namespace_for_skill_path;

mod load_outcome;
mod plugin_id;

use codex_config::HookEventsToml;
use codex_utils_absolute_path::AbsolutePathBuf;
pub use load_outcome::EffectiveSkillRoots;
pub use load_outcome::LoadedPlugin;
pub use load_outcome::PluginLoadOutcome;
pub use load_outcome::prompt_safe_plugin_description;
pub use plugin_id::PluginId;
pub use plugin_id::PluginIdError;
pub use plugin_id::validate_plugin_segment;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppConnectorId(pub String);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginCapabilitySummary {
    pub config_name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub has_skills: bool,
    pub mcp_server_names: Vec<String>,
    pub app_connector_ids: Vec<AppConnectorId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginHookSource {
    pub plugin_id: PluginId,
    pub plugin_root: AbsolutePathBuf,
    pub plugin_data_root: AbsolutePathBuf,
    pub source_path: AbsolutePathBuf,
    pub source_relative_path: String,
    pub hooks: HookEventsToml,
}

/// A plugin-contributed statusline command. The TUI render loop
/// invokes `command` on a debounced tick and renders the captured
/// stdout in the bottom pane. `plugin_root` and `plugin_data_root`
/// are exposed via env vars (PLUGIN_ROOT / CLAUDE_PLUGIN_ROOT /
/// PLUGIN_DATA / CLAUDE_PLUGIN_DATA) so the script can locate
/// vendored assets under its own install dir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginStatuslineSource {
    pub plugin_id: PluginId,
    pub plugin_root: AbsolutePathBuf,
    pub plugin_data_root: AbsolutePathBuf,
    pub command: AbsolutePathBuf,
}

/// A plugin-contributed writable root. Declared in the plugin's manifest
/// (`writableRoots: [...]`) and merged into the active SandboxPolicy's
/// writable_roots at session start so the agent can write to filesystem
/// locations the plugin's runtime requires (e.g. `~/.empirica` for
/// Empirica's session DB / instance pointers / transaction state, which
/// live outside any project cwd by design).
///
/// One `PluginWritableRootSource` per declared root, per plugin — so
/// telemetry and audit can attribute each granted carve-out to its
/// declaring plugin without losing the per-path granularity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginWritableRootSource {
    pub plugin_id: PluginId,
    pub plugin_root: AbsolutePathBuf,
    pub root: AbsolutePathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginTelemetryMetadata {
    pub plugin_id: PluginId,
    /// Optional backend identifier for remote plugins, used when analytics
    /// should report the remote id instead of the local plugin cache id.
    pub remote_plugin_id: Option<String>,
    pub capability_summary: Option<PluginCapabilitySummary>,
}

impl PluginTelemetryMetadata {
    pub fn from_plugin_id(plugin_id: &PluginId) -> Self {
        Self {
            plugin_id: plugin_id.clone(),
            remote_plugin_id: None,
            capability_summary: None,
        }
    }
}

impl PluginCapabilitySummary {
    pub fn telemetry_metadata(&self) -> Option<PluginTelemetryMetadata> {
        PluginId::parse(&self.config_name)
            .ok()
            .map(|plugin_id| PluginTelemetryMetadata {
                plugin_id,
                remote_plugin_id: None,
                capability_summary: Some(self.clone()),
            })
    }
}
