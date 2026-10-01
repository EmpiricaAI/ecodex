//! Installs the empirica plugin that is compiled into ecodex.
//!
//! [`provision`] runs on every ecodex start and costs one marker read plus one
//! config read when nothing has changed. It does three things.
//!
//! - **Plugin files.** The bundled plugin is written to
//!   `$CODEX_HOME/plugins/cache/empiricaAI/empirica/<version>/` whenever the
//!   installed copy does not match the bundle, as recorded by a fingerprint
//!   marker. Every ecodex upgrade therefore refreshes the plugin.
//! - **Config.** A missing `config.toml` is created from the bundled default
//!   (curated providers, plugin enabled). A missing Hugging Face profile and a
//!   missing translator route file (Mistral) are added. An existing config gains the plugin entry when it has none. An entry
//!   the user wrote, including `enabled = false`, is left alone.
//! - **Migration.** A `[plugins."empirica@nubaeon"]` entry from before the
//!   marketplace rename becomes `[plugins."empirica@empiricaAI"]` with its
//!   settings intact. The old cache directory is not deleted: sessions started
//!   by an older ecodex may still be running hook scripts from it.

use include_dir::Dir;
use include_dir::include_dir;
use sha2::Digest;
use sha2::Sha256;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use toml_edit::DocumentMut;
use toml_edit::Item;
use toml_edit::Table;
use toml_edit::value;

const MARKETPLACE: &str = "empiricaAI";
const PLUGIN_NAME: &str = "empirica";
const PLUGIN_KEY: &str = "empirica@empiricaAI";
/// The plugin's key before it moved to the empiricaAI marketplace.
const LEGACY_PLUGIN_KEY: &str = "empirica@nubaeon";
/// Fingerprint of the bundle last written into the plugin directory.
const BUNDLE_MARKER: &str = ".ecodex-bundle";

const MANIFEST: &str = include_str!("../manifest.json");
const HOOKS: &str = include_str!("../hooks.json");
const MCP_SERVERS: &str = include_str!("../mcp_servers.json");
const DEFAULT_CONFIG: &str = include_str!("../assets/config/config.toml.default");
const HUGGINGFACE_PROFILE: &str = include_str!("../assets/config/huggingface.config.toml");
const TRANSLATOR_UPSTREAMS: &str = include_str!("../assets/config/translator-upstreams.toml");
static SKILLS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/skills");
static HOOK_SCRIPTS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/assets/hooks_scripts");
static AGENTS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/assets/agents");

/// What one [`provision`] call changed. All `false` means everything was
/// already in place.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ProvisionReport {
    /// The bundled plugin was written into the plugin cache.
    pub plugin_written: bool,
    /// `config.toml` did not exist and was created from the bundled default.
    pub config_created: bool,
    /// An existing `config.toml` gained the plugin entry.
    pub plugin_enabled: bool,
    /// An existing `config.toml` had its `empirica@nubaeon` entry renamed.
    pub legacy_entry_migrated: bool,
}

/// Installs or refreshes the bundled empirica plugin under `codex_home` and
/// makes sure the config enables it. Safe to call on every start and from
/// several processes at once.
pub fn provision(codex_home: &Path) -> io::Result<ProvisionReport> {
    let marketplace_dir = codex_home.join("plugins").join("cache").join(MARKETPLACE);
    fs::create_dir_all(&marketplace_dir)?;
    // The cockpit starts a whole layout of ecodex instances at once.
    let lock = fs::File::create(marketplace_dir.join(".provision.lock"))?;
    lock.lock()?;

    let mut report = ProvisionReport {
        plugin_written: install_plugin(&marketplace_dir.join(PLUGIN_NAME))?,
        ..ProvisionReport::default()
    };
    ensure_config(codex_home, &mut report)?;
    Ok(report)
}

fn install_plugin(plugin_root: &Path) -> io::Result<bool> {
    let files = bundle_files();
    let fingerprint = fingerprint(&files);
    let plugin_dir = plugin_root.join(manifest_version()?);
    let marker = plugin_dir.join(BUNDLE_MARKER);
    if fs::read_to_string(&marker).is_ok_and(|installed| installed.trim() == fingerprint) {
        return Ok(false);
    }

    // Replace file by file rather than removing the directory, so a running
    // session never finds its hook scripts missing.
    for (relative, contents) in &files {
        write_atomically(&plugin_dir.join(relative), contents)?;
    }
    let expected: HashSet<&Path> = files
        .iter()
        .map(|(relative, _)| relative.as_path())
        .collect();
    prune_stale_files(&plugin_dir, &plugin_dir, &expected)?;
    write_atomically(&marker, fingerprint.as_bytes())?;
    Ok(true)
}

/// Every file of the installed plugin, relative to the plugin directory, in a
/// stable order.
fn bundle_files() -> Vec<(PathBuf, &'static [u8])> {
    let mut files = vec![
        (
            PathBuf::from(".codex-plugin/plugin.json"),
            MANIFEST.as_bytes(),
        ),
        (PathBuf::from("hooks.json"), HOOKS.as_bytes()),
        (PathBuf::from("mcp_servers.json"), MCP_SERVERS.as_bytes()),
    ];
    for (prefix, dir) in [
        ("skills", &SKILLS),
        ("hooks_scripts", &HOOK_SCRIPTS),
        ("agents", &AGENTS),
    ] {
        collect_dir(dir, Path::new(prefix), &mut files);
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

fn collect_dir(
    dir: &'static Dir<'static>,
    prefix: &Path,
    files: &mut Vec<(PathBuf, &'static [u8])>,
) {
    for file in dir.files() {
        if is_shipped(file.path()) {
            files.push((prefix.join(file.path()), file.contents()));
        }
    }
    for subdir in dir.dirs() {
        collect_dir(subdir, prefix, files);
    }
}

/// Leftovers from running hooks inside the source tree (Python caches, dot
/// directories) are embedded by a local build but are not part of the plugin.
fn is_shipped(path: &Path) -> bool {
    path.extension().is_none_or(|extension| extension != "pyc")
        && path.components().all(|component| match component {
            Component::Normal(name) => {
                let name = name.to_string_lossy();
                !name.starts_with('.') && name != "__pycache__"
            }
            Component::Prefix(_)
            | Component::RootDir
            | Component::CurDir
            | Component::ParentDir => true,
        })
}

fn fingerprint(files: &[(PathBuf, &[u8])]) -> String {
    let mut hasher = Sha256::new();
    for (relative, contents) in files {
        hasher.update(relative.to_string_lossy().as_bytes());
        hasher.update(contents.len().to_le_bytes());
        hasher.update(contents);
    }
    format!("{:x}", hasher.finalize())
}

fn manifest_version() -> io::Result<String> {
    let manifest: serde_json::Value = serde_json::from_str(MANIFEST).map_err(io::Error::other)?;
    manifest
        .get("version")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| io::Error::other("bundled plugin manifest has no version"))
}

fn write_atomically(path: &Path, contents: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(format!(".tmp-{}", std::process::id()));
    let temporary = PathBuf::from(temporary);
    fs::write(&temporary, contents)?;
    #[cfg(unix)]
    if path
        .extension()
        .is_some_and(|extension| extension == "py" || extension == "sh")
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o755))?;
    }
    fs::rename(&temporary, path)
}

fn prune_stale_files(plugin_dir: &Path, dir: &Path, expected: &HashSet<&Path>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let relative = path.strip_prefix(plugin_dir).map_err(io::Error::other)?;
        if !is_shipped(relative) {
            continue;
        }
        if path.is_dir() {
            prune_stale_files(plugin_dir, &path, expected)?;
        } else if !expected.contains(relative) {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

fn ensure_config(codex_home: &Path, report: &mut ProvisionReport) -> io::Result<()> {
    for (name, contents) in [
        ("huggingface.config.toml", HUGGINGFACE_PROFILE),
        ("translator-upstreams.toml", TRANSLATOR_UPSTREAMS),
    ] {
        let path = codex_home.join(name);
        if !path.exists() {
            write_atomically(&path, contents.as_bytes())?;
        }
    }

    let config_path = codex_home.join("config.toml");
    let text = match fs::read_to_string(&config_path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            write_atomically(&config_path, DEFAULT_CONFIG.as_bytes())?;
            report.config_created = true;
            return Ok(());
        }
        Err(err) => return Err(err),
    };
    // codex reports a config that does not parse when it loads it.
    let Ok(mut config) = text.parse::<DocumentMut>() else {
        return Ok(());
    };
    let Some(plugins) = plugins_table(&mut config) else {
        return Ok(());
    };
    if let Some(legacy) = plugins.remove(LEGACY_PLUGIN_KEY) {
        if !plugins.contains_key(PLUGIN_KEY) {
            plugins.insert(PLUGIN_KEY, legacy);
        }
        report.legacy_entry_migrated = true;
    }
    if !plugins.contains_key(PLUGIN_KEY) {
        let mut entry = Table::new();
        entry.insert("enabled", value(true));
        plugins.insert(PLUGIN_KEY, Item::Table(entry));
        report.plugin_enabled = true;
    }
    if report.legacy_entry_migrated || report.plugin_enabled {
        write_atomically(&config_path, config.to_string().as_bytes())?;
    }
    Ok(())
}

/// The `[plugins]` table, created implicitly when the config has none. `None`
/// when `plugins` holds something other than a table; codex rejects that itself.
fn plugins_table(config: &mut DocumentMut) -> Option<&mut Table> {
    config
        .as_table_mut()
        .entry("plugins")
        .or_insert_with(|| {
            let mut table = Table::new();
            table.set_implicit(true);
            Item::Table(table)
        })
        .as_table_mut()
}

#[cfg(test)]
#[path = "provision_tests.rs"]
mod tests;
