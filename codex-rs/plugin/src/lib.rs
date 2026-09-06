//! Shared plugin package models, source providers, identifiers, and telemetry summaries.

use std::collections::HashSet;

pub use codex_utils_plugins::mention_syntax;

mod bundled_hooks;
mod load_outcome;
pub mod manifest;
mod plugin_id;
mod provider;

pub use bundled_hooks::is_allowlisted_bundled_cleanup_hook;
use codex_config::HookEventsToml;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
pub use load_outcome::LoadedPlugin;
pub use load_outcome::PluginLoadOutcome;
pub use load_outcome::prompt_safe_plugin_description;
pub use plugin_id::PluginId;
pub use plugin_id::PluginIdError;
pub use plugin_id::validate_plugin_segment;
pub use provider::PluginProvider;
pub use provider::PluginResourceLocator;
pub use provider::ResolvedPlugin;
pub use provider::ResolvedPluginError;
pub use provider::ResolvedPluginLocation;
use serde_json::Map;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppConnectorId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDeclaration {
    pub name: String,
    pub connector_id: AppConnectorId,
    pub category: Option<String>,
}

pub fn app_connector_ids_from_declarations<'a>(
    app_declarations: impl IntoIterator<Item = &'a AppDeclaration>,
) -> Vec<AppConnectorId> {
    let mut connector_ids = Vec::new();
    let mut seen_connector_ids = HashSet::new();
    for app in app_declarations {
        if seen_connector_ids.insert(&app.connector_id) {
            connector_ids.push(app.connector_id.clone());
        }
    }
    connector_ids
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginCapabilitySummary {
    pub config_name: String,
    pub display_name: String,
    pub plugin_namespace: Option<String>,
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

/// Inline plugin hooks discovered in an executor environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutorPluginHookSource {
    pub plugin_id: PluginId,
    pub environment_id: String,
    /// An admitted MCP target can run outside the plugin's source environment.
    pub mcp_environment_id: Option<String>,
    /// Trusted MCP routing metadata for this cleanup target.
    pub mcp_metadata: Option<Map<String, Value>>,
    pub plugin_root: PathUri,
    pub manifest_path: PathUri,
    pub source_relative_path: String,
    pub hooks: HookEventsToml,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginTelemetryMetadata {
    /// Local plugin identifier used by Codex configuration and the plugin cache,
    /// when it has been resolved.
    pub plugin_id: Option<PluginId>,
    /// Optional backend identifier for remote plugins.
    pub remote_plugin_id: Option<String>,
    pub capability_summary: Option<PluginCapabilitySummary>,
}
