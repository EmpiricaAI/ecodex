use codex_config::HooksFile;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_plugins::find_plugin_manifest_path;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::fs;
use std::path::Component;
use std::path::Path;
const MAX_DEFAULT_PROMPT_COUNT: usize = 3;
const MAX_DEFAULT_PROMPT_LEN: usize = 128;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPluginManifest {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    keywords: Vec<String>,
    // Keep manifest paths as raw strings so we can validate the required `./...` syntax before
    // resolving them under the plugin root.
    #[serde(default)]
    skills: Option<String>,
    #[serde(default)]
    mcp_servers: Option<String>,
    #[serde(default)]
    apps: Option<String>,
    #[serde(default)]
    hooks: Option<RawPluginManifestHooks>,
    /// Path (relative `./...`) to an executable the plugin host invokes
    /// on the TUI render tick to contribute a single line to the bottom
    /// status bar. Stdout is interpreted as ANSI text (one or more lines
    /// — each becomes a footer row). Empirica's plugin uses this for
    /// the live epistemic-state strip; any plugin can register its own.
    #[serde(default)]
    statusline: Option<String>,
    /// Filesystem paths the plugin needs to write to that lie OUTSIDE the
    /// session cwd (which is already writable under WorkspaceWrite). Each
    /// entry is either:
    ///   * `~/...` — expanded against the user's HOME at load time
    ///   * absolute path (`/...`) — used verbatim
    /// Relative paths are rejected (plugin authors should use absolute
    /// paths or `~/`-prefixed ones; anything else is ambiguous against
    /// the agent's mutable cwd).
    ///
    /// Plugins declare these so the codex sandbox layer can grant the
    /// minimum cross-cwd write access the plugin's runtime actually
    /// needs — e.g. Empirica needs `~/.empirica` for its global state
    /// (sessions DB, instance pointers, transaction state) since its
    /// project lifecycle exists outside any single cwd by design.
    ///
    /// Empty/unset → no additional writable roots from this plugin.
    #[serde(default)]
    writable_roots: Option<Vec<String>>,
    #[serde(default)]
    interface: Option<RawPluginManifestInterface>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginManifest {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub keywords: Vec<String>,
    pub paths: PluginManifestPaths,
    pub interface: Option<PluginManifestInterface>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginManifestPaths {
    pub skills: Option<AbsolutePathBuf>,
    pub mcp_servers: Option<AbsolutePathBuf>,
    pub apps: Option<AbsolutePathBuf>,
    pub hooks: Option<PluginManifestHooks>,
    /// Optional executable the TUI invokes on a render tick to render a
    /// plugin-contributed status line. Resolved relative to the plugin
    /// root from the manifest's `statusline` field. The host runs the
    /// command in a debounced subprocess (see TUI render loop) and
    /// renders stdout below the existing footer status items.
    pub statusline: Option<AbsolutePathBuf>,
    /// Cross-cwd writable roots the plugin's runtime needs (e.g.
    /// `~/.empirica` for Empirica's global state). Resolved at manifest
    /// load time: tilde paths are expanded against the user's HOME;
    /// absolute paths are kept verbatim. The plugin host merges these
    /// into the active SandboxPolicy's writable_roots. Empty when the
    /// plugin doesn't declare any.
    pub writable_roots: Vec<AbsolutePathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginManifestHooks {
    Paths(Vec<AbsolutePathBuf>),
    Inline(Vec<HooksFile>),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginManifestInterface {
    pub display_name: Option<String>,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub developer_name: Option<String>,
    pub category: Option<String>,
    pub capabilities: Vec<String>,
    pub website_url: Option<String>,
    pub privacy_policy_url: Option<String>,
    pub terms_of_service_url: Option<String>,
    pub default_prompt: Option<Vec<String>>,
    pub brand_color: Option<String>,
    pub composer_icon: Option<AbsolutePathBuf>,
    pub logo: Option<AbsolutePathBuf>,
    pub screenshots: Vec<AbsolutePathBuf>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPluginManifestInterface {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    short_description: Option<String>,
    #[serde(default)]
    long_description: Option<String>,
    #[serde(default)]
    developer_name: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default)]
    #[serde(alias = "websiteURL")]
    website_url: Option<String>,
    #[serde(default)]
    #[serde(alias = "privacyPolicyURL")]
    privacy_policy_url: Option<String>,
    #[serde(default)]
    #[serde(alias = "termsOfServiceURL")]
    terms_of_service_url: Option<String>,
    #[serde(default)]
    default_prompt: Option<RawPluginManifestDefaultPrompt>,
    #[serde(default)]
    brand_color: Option<String>,
    #[serde(default)]
    composer_icon: Option<String>,
    #[serde(default)]
    logo: Option<String>,
    #[serde(default)]
    screenshots: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawPluginManifestDefaultPrompt {
    String(String),
    List(Vec<RawPluginManifestDefaultPromptEntry>),
    Invalid(JsonValue),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawPluginManifestDefaultPromptEntry {
    String(String),
    Invalid(JsonValue),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawPluginManifestHooks {
    Path(String),
    Paths(Vec<String>),
    Inline(HooksFile),
    InlineList(Vec<HooksFile>),
    Invalid(JsonValue),
}

pub fn load_plugin_manifest(plugin_root: &Path) -> Option<PluginManifest> {
    let manifest_path = find_plugin_manifest_path(plugin_root)?;
    let contents = fs::read_to_string(&manifest_path).ok()?;
    match serde_json::from_str::<RawPluginManifest>(&contents) {
        Ok(manifest) => {
            let RawPluginManifest {
                name: raw_name,
                version,
                description,
                keywords,
                skills,
                mcp_servers,
                apps,
                hooks,
                statusline,
                writable_roots,
                interface,
            } = manifest;
            let name = plugin_root
                .file_name()
                .and_then(|entry| entry.to_str())
                .filter(|_| raw_name.trim().is_empty())
                .unwrap_or(&raw_name)
                .to_string();
            let version = version.and_then(|version| {
                let version = version.trim();
                (!version.is_empty()).then(|| version.to_string())
            });
            let interface = interface.and_then(|interface| {
                let RawPluginManifestInterface {
                    display_name,
                    short_description,
                    long_description,
                    developer_name,
                    category,
                    capabilities,
                    website_url,
                    privacy_policy_url,
                    terms_of_service_url,
                    default_prompt,
                    brand_color,
                    composer_icon,
                    logo,
                    screenshots,
                } = interface;

                let interface = PluginManifestInterface {
                    display_name,
                    short_description,
                    long_description,
                    developer_name,
                    category,
                    capabilities,
                    website_url,
                    privacy_policy_url,
                    terms_of_service_url,
                    default_prompt: resolve_default_prompts(plugin_root, default_prompt.as_ref()),
                    brand_color,
                    composer_icon: resolve_interface_asset_path(
                        plugin_root,
                        "interface.composerIcon",
                        composer_icon.as_deref(),
                    ),
                    logo: resolve_interface_asset_path(
                        plugin_root,
                        "interface.logo",
                        logo.as_deref(),
                    ),
                    screenshots: screenshots
                        .iter()
                        .filter_map(|screenshot| {
                            resolve_interface_asset_path(
                                plugin_root,
                                "interface.screenshots",
                                Some(screenshot),
                            )
                        })
                        .collect(),
                };

                let has_fields = interface.display_name.is_some()
                    || interface.short_description.is_some()
                    || interface.long_description.is_some()
                    || interface.developer_name.is_some()
                    || interface.category.is_some()
                    || !interface.capabilities.is_empty()
                    || interface.website_url.is_some()
                    || interface.privacy_policy_url.is_some()
                    || interface.terms_of_service_url.is_some()
                    || interface.default_prompt.is_some()
                    || interface.brand_color.is_some()
                    || interface.composer_icon.is_some()
                    || interface.logo.is_some()
                    || !interface.screenshots.is_empty();

                has_fields.then_some(interface)
            });
            Some(PluginManifest {
                name,
                version,
                description,
                keywords,
                paths: PluginManifestPaths {
                    skills: resolve_manifest_path(plugin_root, "skills", skills.as_deref()),
                    mcp_servers: resolve_manifest_path(
                        plugin_root,
                        "mcpServers",
                        mcp_servers.as_deref(),
                    ),
                    apps: resolve_manifest_path(plugin_root, "apps", apps.as_deref()),
                    hooks: resolve_manifest_hooks(plugin_root, hooks),
                    statusline: resolve_manifest_path(
                        plugin_root,
                        "statusline",
                        statusline.as_deref(),
                    ),
                    writable_roots: resolve_manifest_runtime_paths(
                        "writableRoots",
                        writable_roots.as_deref(),
                    ),
                },
                interface,
            })
        }
        Err(err) => {
            tracing::warn!(
                path = %manifest_path.display(),
                "failed to parse plugin manifest: {err}"
            );
            None
        }
    }
}

fn resolve_manifest_hooks(
    plugin_root: &Path,
    hooks: Option<RawPluginManifestHooks>,
) -> Option<PluginManifestHooks> {
    match hooks? {
        RawPluginManifestHooks::Path(path) => {
            resolve_manifest_path(plugin_root, "hooks", Some(&path))
                .map(|path| PluginManifestHooks::Paths(vec![path]))
        }
        RawPluginManifestHooks::Paths(paths) => {
            let hooks = paths
                .iter()
                .filter_map(|path| resolve_manifest_path(plugin_root, "hooks", Some(path)))
                .collect::<Vec<_>>();
            (!hooks.is_empty()).then_some(PluginManifestHooks::Paths(hooks))
        }
        RawPluginManifestHooks::Inline(hooks) => Some(PluginManifestHooks::Inline(vec![hooks])),
        RawPluginManifestHooks::InlineList(hooks) => {
            (!hooks.is_empty()).then_some(PluginManifestHooks::Inline(hooks))
        }
        RawPluginManifestHooks::Invalid(value) => {
            tracing::warn!(
                "ignoring hooks: expected a string, string array, object, or object array; found {}",
                json_value_type(&value)
            );
            None
        }
    }
}

fn resolve_interface_asset_path(
    plugin_root: &Path,
    field: &'static str,
    path: Option<&str>,
) -> Option<AbsolutePathBuf> {
    resolve_manifest_path(plugin_root, field, path)
}

fn resolve_default_prompts(
    plugin_root: &Path,
    value: Option<&RawPluginManifestDefaultPrompt>,
) -> Option<Vec<String>> {
    match value? {
        RawPluginManifestDefaultPrompt::String(prompt) => {
            resolve_default_prompt_str(plugin_root, "interface.defaultPrompt", prompt)
                .map(|prompt| vec![prompt])
        }
        RawPluginManifestDefaultPrompt::List(values) => {
            let mut prompts = Vec::new();
            for (index, item) in values.iter().enumerate() {
                if prompts.len() >= MAX_DEFAULT_PROMPT_COUNT {
                    warn_invalid_default_prompt(
                        plugin_root,
                        "interface.defaultPrompt",
                        &format!("maximum of {MAX_DEFAULT_PROMPT_COUNT} prompts is supported"),
                    );
                    break;
                }

                match item {
                    RawPluginManifestDefaultPromptEntry::String(prompt) => {
                        let field = format!("interface.defaultPrompt[{index}]");
                        if let Some(prompt) =
                            resolve_default_prompt_str(plugin_root, &field, prompt)
                        {
                            prompts.push(prompt);
                        }
                    }
                    RawPluginManifestDefaultPromptEntry::Invalid(value) => {
                        let field = format!("interface.defaultPrompt[{index}]");
                        warn_invalid_default_prompt(
                            plugin_root,
                            &field,
                            &format!("expected a string, found {}", json_value_type(value)),
                        );
                    }
                }
            }

            (!prompts.is_empty()).then_some(prompts)
        }
        RawPluginManifestDefaultPrompt::Invalid(value) => {
            warn_invalid_default_prompt(
                plugin_root,
                "interface.defaultPrompt",
                &format!(
                    "expected a string or array of strings, found {}",
                    json_value_type(value)
                ),
            );
            None
        }
    }
}

fn resolve_default_prompt_str(plugin_root: &Path, field: &str, prompt: &str) -> Option<String> {
    let prompt = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    if prompt.is_empty() {
        warn_invalid_default_prompt(plugin_root, field, "prompt must not be empty");
        return None;
    }
    if prompt.chars().count() > MAX_DEFAULT_PROMPT_LEN {
        warn_invalid_default_prompt(
            plugin_root,
            field,
            &format!("prompt must be at most {MAX_DEFAULT_PROMPT_LEN} characters"),
        );
        return None;
    }
    Some(prompt)
}

fn warn_invalid_default_prompt(plugin_root: &Path, field: &str, message: &str) {
    if let Some(manifest_path) = find_plugin_manifest_path(plugin_root) {
        tracing::warn!(
            path = %manifest_path.display(),
            "ignoring {field}: {message}"
        );
    } else {
        tracing::warn!("ignoring {field}: {message}");
    }
}

fn json_value_type(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "boolean",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

/// Resolve plugin manifest paths that name *runtime* filesystem locations
/// outside the plugin install dir (e.g. writable_roots like `~/.empirica`).
/// Unlike `resolve_manifest_path` (which constrains paths to live inside
/// the plugin root via `./...`), this accepts:
///   * `~/...` — expanded against `$HOME`
///   * absolute paths (`/...`) — kept verbatim
/// Relative paths and paths containing `..` are rejected with a warning.
/// Empty/missing input returns an empty Vec.
fn resolve_manifest_runtime_paths(
    field: &'static str,
    paths: Option<&[String]>,
) -> Vec<AbsolutePathBuf> {
    let Some(paths) = paths else {
        return Vec::new();
    };
    let mut resolved = Vec::with_capacity(paths.len());
    for raw in paths {
        let raw = raw.trim();
        if raw.is_empty() {
            tracing::warn!("ignoring {field}: entry must not be empty");
            continue;
        }
        // Reject any explicit relative paths — `./` and `../` would be
        // ambiguous against the agent's mutable cwd.
        if raw.starts_with("./") || raw.starts_with("../") || raw == "." || raw == ".." {
            tracing::warn!(
                "ignoring {field} entry {raw:?}: relative paths are rejected; use ~/ or absolute"
            );
            continue;
        }
        let expanded = if let Some(rest) = raw.strip_prefix("~/") {
            let Some(home) = std::env::var_os("HOME") else {
                tracing::warn!(
                    "ignoring {field} entry {raw:?}: HOME is not set, cannot expand `~/`"
                );
                continue;
            };
            let mut buf = std::path::PathBuf::from(home);
            buf.push(rest);
            buf
        } else if raw == "~" {
            let Some(home) = std::env::var_os("HOME") else {
                tracing::warn!(
                    "ignoring {field} entry {raw:?}: HOME is not set, cannot expand `~`"
                );
                continue;
            };
            std::path::PathBuf::from(home)
        } else {
            std::path::PathBuf::from(raw)
        };
        // Reject any `..` components after expansion — keeps the path
        // contract auditable (no traversal escapes from declared roots).
        if expanded
            .components()
            .any(|c| matches!(c, Component::ParentDir))
        {
            tracing::warn!("ignoring {field} entry {raw:?}: path must not contain '..' components");
            continue;
        }
        // Reject anything that's not absolute after expansion — relative
        // entries like "relative/path" would otherwise resolve against
        // the agent's cwd via AbsolutePathBuf and silently grant scope
        // we never declared.
        if !expanded.is_absolute() {
            tracing::warn!("ignoring {field} entry {raw:?}: must be absolute or `~/`-prefixed");
            continue;
        }
        match AbsolutePathBuf::try_from(expanded) {
            Ok(abs) => resolved.push(abs),
            Err(err) => {
                tracing::warn!("ignoring {field} entry {raw:?}: {err}");
            }
        }
    }
    resolved
}

fn resolve_manifest_path(
    plugin_root: &Path,
    field: &'static str,
    path: Option<&str>,
) -> Option<AbsolutePathBuf> {
    // `plugin.json` paths are required to be relative to the plugin root and we return the
    // normalized absolute path to the rest of the system.
    let path = path?;
    if path.is_empty() {
        return None;
    }
    let Some(relative_path) = path.strip_prefix("./") else {
        tracing::warn!("ignoring {field}: path must start with `./` relative to plugin root");
        return None;
    };
    if relative_path.is_empty() {
        tracing::warn!("ignoring {field}: path must not be `./`");
        return None;
    }

    let mut normalized = std::path::PathBuf::new();
    for component in Path::new(relative_path).components() {
        match component {
            Component::Normal(component) => normalized.push(component),
            Component::ParentDir => {
                tracing::warn!("ignoring {field}: path must not contain '..'");
                return None;
            }
            _ => {
                tracing::warn!("ignoring {field}: path must stay within the plugin root");
                return None;
            }
        }
    }

    AbsolutePathBuf::try_from(plugin_root.join(normalized))
        .map_err(|err| {
            tracing::warn!("ignoring {field}: path must resolve to an absolute path: {err}");
            err
        })
        .ok()
}

#[cfg(test)]
mod tests {
    use super::MAX_DEFAULT_PROMPT_LEN;
    use super::PluginManifest;
    use super::load_plugin_manifest;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    const ALTERNATE_PLUGIN_MANIFEST_RELATIVE_PATH: &str = ".claude-plugin/plugin.json";

    fn write_manifest(plugin_root: &Path, version: Option<&str>, interface: &str) {
        fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("create manifest dir");
        let version = version
            .map(|version| format!("  \"version\": \"{version}\",\n"))
            .unwrap_or_default();
        fs::write(
            plugin_root.join(".codex-plugin/plugin.json"),
            format!(
                r#"{{
  "name": "demo-plugin",
{version}
  "interface": {interface}
}}"#
            ),
        )
        .expect("write manifest");
    }

    fn write_alternate_plugin_manifest(plugin_root: &Path, contents: &str) {
        let manifest_path = plugin_root.join(ALTERNATE_PLUGIN_MANIFEST_RELATIVE_PATH);
        fs::create_dir_all(manifest_path.parent().expect("manifest parent"))
            .expect("create manifest dir");
        fs::write(manifest_path, contents).expect("write manifest");
    }

    fn load_manifest(plugin_root: &Path) -> PluginManifest {
        load_plugin_manifest(plugin_root).expect("load plugin manifest")
    }

    #[test]
    fn plugin_interface_accepts_legacy_default_prompt_string() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        write_manifest(
            &plugin_root,
            /*version*/ None,
            r#"{
    "displayName": "Demo Plugin",
    "defaultPrompt": "  Summarize   my inbox  "
  }"#,
        );

        let manifest = load_manifest(&plugin_root);
        let interface = manifest.interface.expect("plugin interface");

        assert_eq!(
            interface.default_prompt,
            Some(vec!["Summarize my inbox".to_string()])
        );
    }

    #[test]
    fn plugin_interface_normalizes_default_prompt_array() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        let too_long = "x".repeat(MAX_DEFAULT_PROMPT_LEN + 1);
        write_manifest(
            &plugin_root,
            /*version*/ None,
            &format!(
                r#"{{
    "displayName": "Demo Plugin",
    "defaultPrompt": [
      " Summarize my inbox ",
      123,
      "{too_long}",
      "   ",
      "Draft the reply  ",
      "Find   my next action",
      "Archive old mail"
    ]
  }}"#
            ),
        );

        let manifest = load_manifest(&plugin_root);
        let interface = manifest.interface.expect("plugin interface");

        assert_eq!(
            interface.default_prompt,
            Some(vec![
                "Summarize my inbox".to_string(),
                "Draft the reply".to_string(),
                "Find my next action".to_string(),
            ])
        );
    }

    #[test]
    fn plugin_interface_ignores_invalid_default_prompt_shape() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        write_manifest(
            &plugin_root,
            /*version*/ None,
            r#"{
    "displayName": "Demo Plugin",
    "defaultPrompt": { "text": "Summarize my inbox" }
  }"#,
        );

        let manifest = load_manifest(&plugin_root);
        let interface = manifest.interface.expect("plugin interface");

        assert_eq!(interface.default_prompt, None);
    }

    #[test]
    fn plugin_manifest_reads_trimmed_version() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        write_manifest(
            &plugin_root,
            Some(" 1.2.3-beta+7 "),
            r#"{
    "displayName": "Demo Plugin"
  }"#,
        );

        let manifest = load_manifest(&plugin_root);

        assert_eq!(manifest.version, Some("1.2.3-beta+7".to_string()));
    }

    #[test]
    fn plugin_manifest_reads_keywords() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("create manifest dir");
        fs::write(
            plugin_root.join(".codex-plugin/plugin.json"),
            r#"{
  "name": "demo-plugin",
  "keywords": ["api-key", "developer tools"]
}"#,
        )
        .expect("write manifest");

        let manifest = load_manifest(&plugin_root);

        assert_eq!(
            manifest.keywords,
            vec!["api-key".to_string(), "developer tools".to_string()]
        );
    }

    #[test]
    fn plugin_manifest_uses_alternate_discoverable_path() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        write_alternate_plugin_manifest(
            &plugin_root,
            r#"{
  "name": "demo-plugin",
  "version": " 2.0.0 ",
  "interface": {
    "displayName": "Fallback Plugin"
  }
}"#,
        );

        let manifest = load_manifest(&plugin_root);

        assert_eq!(manifest.version, Some("2.0.0".to_string()));
        assert_eq!(
            manifest
                .interface
                .as_ref()
                .and_then(|interface| interface.display_name.as_deref()),
            Some("Fallback Plugin")
        );
    }

    /// Helper: write a top-level plugin manifest (statusline / hooks /
    /// mcpServers / etc are top-level fields, not nested under interface).
    fn write_full_manifest(plugin_root: &Path, contents: &str) {
        fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("create manifest dir");
        fs::write(plugin_root.join(".codex-plugin/plugin.json"), contents).expect("write manifest");
    }

    #[test]
    fn plugin_manifest_resolves_statusline_path() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        write_full_manifest(
            &plugin_root,
            r#"{
  "name": "demo-plugin",
  "statusline": "./scripts/statusline.sh"
}"#,
        );

        let manifest = load_manifest(&plugin_root);
        let statusline = manifest
            .paths
            .statusline
            .as_ref()
            .expect("statusline resolved");
        assert!(statusline.as_path().ends_with("scripts/statusline.sh"));
    }

    #[test]
    fn plugin_manifest_statusline_absent_when_unset() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        write_full_manifest(
            &plugin_root,
            r#"{
  "name": "demo-plugin"
}"#,
        );

        let manifest = load_manifest(&plugin_root);
        assert!(manifest.paths.statusline.is_none());
    }

    #[test]
    fn plugin_manifest_statusline_rejects_non_relative_path() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        // Absolute path — must be rejected (must start with `./`).
        write_full_manifest(
            &plugin_root,
            r#"{
  "name": "demo-plugin",
  "statusline": "/etc/passwd"
}"#,
        );

        let manifest = load_manifest(&plugin_root);
        assert!(manifest.paths.statusline.is_none());
    }

    /// HOME mutation must be serialized — cargo runs tests in parallel by
    /// default, and any unsynchronized HOME swap will race with sibling
    /// tests reading $HOME for tilde expansion.
    fn home_test_lock() -> &'static std::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    /// Helper: temporarily set $HOME for a closure (writable_roots tests
    /// rely on tilde expansion). Restores HOME after the closure runs.
    /// Holds `home_test_lock()` for the duration so concurrent HOME-mutating
    /// tests serialize.
    fn with_home<F: FnOnce() -> R, R>(home: &Path, f: F) -> R {
        let _guard = home_test_lock().lock().unwrap_or_else(|e| e.into_inner());
        let saved = std::env::var_os("HOME");
        // SAFETY: HOME mutation is serialized via home_test_lock() above.
        unsafe {
            std::env::set_var("HOME", home);
        }
        let result = f();
        unsafe {
            match saved {
                Some(value) => std::env::set_var("HOME", value),
                None => std::env::remove_var("HOME"),
            }
        }
        result
    }

    #[test]
    fn plugin_manifest_resolves_writable_roots_tilde_and_absolute() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        let home = tmp.path().join("home");
        fs::create_dir_all(&home).expect("create home");
        write_full_manifest(
            &plugin_root,
            r#"{
  "name": "demo-plugin",
  "writableRoots": ["~/.empirica", "/var/lib/demo-plugin"]
}"#,
        );

        let manifest = with_home(&home, || load_manifest(&plugin_root));
        let roots = manifest.paths.writable_roots;
        assert_eq!(roots.len(), 2, "expected both entries to resolve");
        assert_eq!(roots[0].as_path(), home.join(".empirica"));
        assert_eq!(roots[1].as_path(), Path::new("/var/lib/demo-plugin"));
    }

    #[test]
    fn plugin_manifest_writable_roots_rejects_relative_paths() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        let home = tmp.path().join("home");
        fs::create_dir_all(&home).expect("create home");
        write_full_manifest(
            &plugin_root,
            r#"{
  "name": "demo-plugin",
  "writableRoots": ["./scoped", "../escape", ".", "..", "relative/path"]
}"#,
        );

        let manifest = with_home(&home, || load_manifest(&plugin_root));
        // `./`, `../`, `.`, and `..` are explicitly rejected. A bare
        // `relative/path` is treated as a path string by AbsolutePathBuf
        // and rejected because it isn't absolute — confirms the contract:
        // no relative entries survive.
        assert!(
            manifest.paths.writable_roots.is_empty(),
            "expected all relative entries to be rejected, got {:?}",
            manifest.paths.writable_roots
        );
    }

    #[test]
    fn plugin_manifest_writable_roots_rejects_parent_dir_components() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        let home = tmp.path().join("home");
        fs::create_dir_all(&home).expect("create home");
        // `~/` expansion that contains `..` AFTER expansion must still
        // be rejected — we check the resolved PathBuf for ParentDir.
        write_full_manifest(
            &plugin_root,
            r#"{
  "name": "demo-plugin",
  "writableRoots": ["~/foo/../bar", "/etc/../passwd"]
}"#,
        );

        let manifest = with_home(&home, || load_manifest(&plugin_root));
        assert!(
            manifest.paths.writable_roots.is_empty(),
            "expected `..`-containing entries to be rejected, got {:?}",
            manifest.paths.writable_roots
        );
    }

    #[test]
    fn plugin_manifest_writable_roots_absent_when_unset() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        write_full_manifest(
            &plugin_root,
            r#"{
  "name": "demo-plugin"
}"#,
        );

        let manifest = load_manifest(&plugin_root);
        assert!(
            manifest.paths.writable_roots.is_empty(),
            "expected empty Vec when field is absent"
        );
    }

    #[test]
    fn plugin_manifest_writable_roots_skips_empty_strings() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        let home = tmp.path().join("home");
        fs::create_dir_all(&home).expect("create home");
        write_full_manifest(
            &plugin_root,
            r#"{
  "name": "demo-plugin",
  "writableRoots": ["", "   ", "/var/valid"]
}"#,
        );

        let manifest = with_home(&home, || load_manifest(&plugin_root));
        let roots = manifest.paths.writable_roots;
        assert_eq!(roots.len(), 1, "expected only the valid entry to remain");
        assert_eq!(roots[0].as_path(), Path::new("/var/valid"));
    }

    #[test]
    fn plugin_manifest_writable_roots_handles_bare_tilde() {
        let tmp = tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("demo-plugin");
        let home = tmp.path().join("home");
        fs::create_dir_all(&home).expect("create home");
        write_full_manifest(
            &plugin_root,
            r#"{
  "name": "demo-plugin",
  "writableRoots": ["~"]
}"#,
        );

        let manifest = with_home(&home, || load_manifest(&plugin_root));
        let roots = manifest.paths.writable_roots;
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].as_path(), home.as_path());
    }
}
