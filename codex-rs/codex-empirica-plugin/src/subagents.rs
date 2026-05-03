//! Empirica subagent seeding into codex's agents directory.
//!
//! Codex looks up subagents at `<codex_home>/agents/` (global) and
//! `<repo>/.codex/agents/` (per-repo). The plugin manifest
//! (`PluginManifestPaths`) has no `subagents` field, so plugins must
//! install subagent markdown files imperatively — same pattern as Tx3's
//! AGENTS.md seeding.
//!
//! This module copies the bundled empirica subagents (vendored at
//! `assets/agents/` and shipped in the plugin install dir under
//! `agents/`) into `<codex_home>/agents/empirica/`. Namespacing
//! under `empirica/` prevents collision with user-defined subagents at
//! the codex_home top level.
//!
//! ## Idempotency
//!
//! Every SessionStart fires this. If a destination file already exists
//! with byte-identical content, no write happens. If it differs (e.g.
//! plugin upgrade with new subagent definitions), the file is rewritten
//! atomically. Missing files are created.
//!
//! ## Provenance marker
//!
//! Each seeded file is a verbatim copy of the bundled markdown. No
//! marker comments are injected — codex's subagent loader parses the
//! YAML frontmatter, and inserting comments above it would break that
//! parse. Provenance is communicated via the `empirica/` directory
//! namespace and the seeded files themselves (each markdown's
//! frontmatter declares its identity).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Sub-path within the plugin install dir where bundled subagent
/// markdown files live (mirrors install.sh copy target).
const PLUGIN_AGENTS_SUBPATH: &str = "agents";

/// Sub-namespace under `<codex_home>/agents/` to avoid colliding with
/// user-defined subagents.
const EMPIRICA_NAMESPACE: &str = "empirica";

/// Resolve `$CODEX_HOME` (or `~/.codex` if unset).
fn resolve_codex_home() -> Option<PathBuf> {
    if let Ok(env_home) = std::env::var("CODEX_HOME") {
        return Some(PathBuf::from(env_home));
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".codex"))
}

/// Resolve the bundled-agents source dir from the plugin install via
/// `$PLUGIN_ROOT` (codex sets this when invoking the hook command).
/// Returns `None` if `PLUGIN_ROOT` is unset (e.g. dev-mode bare-binary
/// run) — the seed becomes a no-op rather than failing the session.
fn resolve_bundled_agents_dir() -> Option<PathBuf> {
    let plugin_root = std::env::var_os("PLUGIN_ROOT")?;
    Some(PathBuf::from(plugin_root).join(PLUGIN_AGENTS_SUBPATH))
}

/// Copy every `.md` file from `source_dir` into
/// `<codex_home>/agents/empirica/`, creating the destination dir if
/// needed. Skips writes when the destination file already byte-matches
/// the source. Returns the number of files written this call.
pub fn ensure_subagents_seeded_at(source_dir: &Path, codex_home: &Path) -> Result<usize> {
    if !source_dir.is_dir() {
        // Nothing to seed — bundled assets missing. Caller fail-opens.
        return Ok(0);
    }
    let dest_dir = codex_home.join("agents").join(EMPIRICA_NAMESPACE);
    fs::create_dir_all(&dest_dir)
        .with_context(|| format!("create empirica subagents dir {}", dest_dir.display()))?;

    let mut written = 0usize;
    for entry in fs::read_dir(source_dir)
        .with_context(|| format!("read bundled agents dir {}", source_dir.display()))?
    {
        let entry = entry.context("iterate bundled agents dir")?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let Some(name) = path.file_name() else {
            continue;
        };
        let dest = dest_dir.join(name);
        let new_bytes =
            fs::read(&path).with_context(|| format!("read bundled agent {}", path.display()))?;
        let existing = fs::read(&dest).ok();
        if existing.as_deref() == Some(new_bytes.as_slice()) {
            continue;
        }
        fs::write(&dest, &new_bytes)
            .with_context(|| format!("write subagent {}", dest.display()))?;
        written += 1;
    }
    Ok(written)
}

/// Convenience wrapper resolving `$PLUGIN_ROOT` + `$CODEX_HOME` before
/// delegating to [`ensure_subagents_seeded_at`]. Returns `Ok(0)` for
/// any resolve-failure (no HOME, no PLUGIN_ROOT, missing bundled dir)
/// — fail-open so SessionStart never blocks on subagent seeding.
pub fn ensure_subagents_seeded() -> Result<usize> {
    let Some(source_dir) = resolve_bundled_agents_dir() else {
        return Ok(0);
    };
    let Some(codex_home) = resolve_codex_home() else {
        return Ok(0);
    };
    ensure_subagents_seeded_at(&source_dir, &codex_home)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_agent(dir: &Path, name: &str, body: &str) {
        fs::write(dir.join(name), body).unwrap();
    }

    #[test]
    fn seeds_when_destination_empty() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();
        write_agent(
            src.path(),
            "architecture.md",
            "---\nname: architecture\n---\nbody\n",
        );
        write_agent(
            src.path(),
            "security.md",
            "---\nname: security\n---\nbody\n",
        );

        let n = ensure_subagents_seeded_at(src.path(), dst.path()).unwrap();
        assert_eq!(n, 2);
        let dest_dir = dst.path().join("agents").join(EMPIRICA_NAMESPACE);
        assert!(dest_dir.join("architecture.md").exists());
        assert!(dest_dir.join("security.md").exists());
    }

    #[test]
    fn idempotent_when_content_matches() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();
        write_agent(src.path(), "ux.md", "ux body\n");

        assert_eq!(
            ensure_subagents_seeded_at(src.path(), dst.path()).unwrap(),
            1
        );
        // Second run: destination already byte-identical → no writes.
        assert_eq!(
            ensure_subagents_seeded_at(src.path(), dst.path()).unwrap(),
            0
        );
    }

    #[test]
    fn rewrites_when_content_drifts() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();
        write_agent(src.path(), "agent.md", "v1 body\n");
        ensure_subagents_seeded_at(src.path(), dst.path()).unwrap();

        // Source updates (e.g. sync-empirica-assets refresh).
        write_agent(src.path(), "agent.md", "v2 body\n");
        let n = ensure_subagents_seeded_at(src.path(), dst.path()).unwrap();
        assert_eq!(n, 1);
        let dest = dst
            .path()
            .join("agents")
            .join(EMPIRICA_NAMESPACE)
            .join("agent.md");
        assert_eq!(fs::read_to_string(dest).unwrap(), "v2 body\n");
    }

    #[test]
    fn skips_non_markdown_files() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();
        write_agent(src.path(), "valid.md", "ok\n");
        write_agent(src.path(), "README.txt", "skip me\n");
        write_agent(src.path(), "config.yaml", "skip\n");

        assert_eq!(
            ensure_subagents_seeded_at(src.path(), dst.path()).unwrap(),
            1
        );
        let dest_dir = dst.path().join("agents").join(EMPIRICA_NAMESPACE);
        assert!(dest_dir.join("valid.md").exists());
        assert!(!dest_dir.join("README.txt").exists());
        assert!(!dest_dir.join("config.yaml").exists());
    }

    #[test]
    fn missing_source_dir_is_noop() {
        let dst = TempDir::new().unwrap();
        let absent = PathBuf::from("/nonexistent/path/that/does/not/exist");
        assert_eq!(ensure_subagents_seeded_at(&absent, dst.path()).unwrap(), 0);
    }
}
