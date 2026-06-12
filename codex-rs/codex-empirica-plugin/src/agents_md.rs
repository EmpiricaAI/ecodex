//! AGENTS.md seeding for the empirica plugin.
//!
//! Codex's plugin manifest schema (`PluginManifestPaths` in
//! `core-plugins/src/manifest.rs`) only declares 4 contribution surfaces:
//! `skills`, `mcp_servers`, `apps`, `hooks`. There is no `agents_md` /
//! `instructions` / `system_prompt` field. Yet AGENTS.md (loaded by
//! `AgentsMdManager` in `core/src/agents_md.rs`) is codex's instructional
//! scaffolding — the surface where the model's identity, vocabulary, and
//! discipline-rules are conveyed. Without writing it, the codex agent gets
//! the empirica firewall enforcing rules it has no idea exist.
//!
//! This module fills that gap by writing `~/.codex/AGENTS.md` with the
//! bundled empirica system-prompt content, wrapped in marker comments so
//! we can update our section idempotently without clobbering anything the
//! user adds above/below.
//!
//! ## File-vs-override convention
//!
//! Codex looks up `AGENTS.override.md` BEFORE `AGENTS.md` (per
//! `agents_md.rs:65`). We deliberately write the *default* (`AGENTS.md`),
//! not the override. Power users who want to bypass plugin discipline
//! entirely create `AGENTS.override.md` — codex picks that and our content
//! is silently ignored.
//!
//! ## Idempotency
//!
//! Every SessionStart fires this. If the file already contains a marker
//! block matching the bundled content, no write happens. If markers are
//! present but content drifted (e.g. plugin upgrade), the block is
//! replaced. If markers are absent, the block is appended.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const EMPIRICA_PROMPT: &str = include_str!("../assets/empirica-system-prompt.md");
const BEGIN_MARKER: &str = "<!-- BEGIN EMPIRICA SYSTEM PROMPT v1 -->";
const END_MARKER: &str = "<!-- END EMPIRICA SYSTEM PROMPT v1 -->";

/// Default AGENTS.md filename codex looks up in `codex_home`.
const AGENTS_MD_FILENAME: &str = "AGENTS.md";

/// Resolve `$CODEX_HOME` (or `~/.codex` if unset) without spawning a process.
fn resolve_codex_home() -> Option<PathBuf> {
    if let Ok(env_home) = std::env::var("CODEX_HOME") {
        return Some(PathBuf::from(env_home));
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".codex"))
}

/// Wrap the bundled prompt in marker comments. Trailing newline so the next
/// content (or end-of-file) starts on its own line.
fn wrapped_block() -> String {
    let body = EMPIRICA_PROMPT.trim_end_matches('\n');
    format!("{BEGIN_MARKER}\n{body}\n{END_MARKER}\n")
}

/// Compute the desired final file content given existing content (if any).
fn compute_updated(existing: Option<&str>) -> String {
    let block = wrapped_block();
    match existing {
        None => block,
        Some(content) if !content.contains(BEGIN_MARKER) => append_block(content, &block),
        Some(content) => replace_block(content, &block),
    }
}

fn append_block(existing: &str, block: &str) -> String {
    let mut out = existing.to_string();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(block);
    out
}

fn replace_block(existing: &str, block: &str) -> String {
    // Find the marker pair and splice. If END_MARKER is missing or appears
    // before BEGIN_MARKER, fall back to append (the file is malformed —
    // don't risk further damage).
    let Some(begin_idx) = existing.find(BEGIN_MARKER) else {
        return append_block(existing, block);
    };
    let after_begin = begin_idx + BEGIN_MARKER.len();
    let Some(end_rel) = existing[after_begin..].find(END_MARKER) else {
        return append_block(existing, block);
    };
    let end_idx = after_begin + end_rel + END_MARKER.len();
    let mut out = String::with_capacity(existing.len());
    out.push_str(&existing[..begin_idx]);
    out.push_str(block.trim_end_matches('\n'));
    out.push_str(&existing[end_idx..]);
    // Normalize trailing newline
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Ensure `<codex_home>/AGENTS.md` contains our marker block with the bundled
/// empirica system prompt. Idempotent — only writes when content differs.
pub fn ensure_agents_md_seeded_at(codex_home: &Path) -> Result<bool> {
    fs::create_dir_all(codex_home)
        .with_context(|| format!("create codex_home directory {}", codex_home.display()))?;
    let path = codex_home.join(AGENTS_MD_FILENAME);
    let existing = fs::read_to_string(&path).ok();
    let updated = compute_updated(existing.as_deref());
    if existing.as_deref() == Some(&updated) {
        return Ok(false);
    }
    fs::write(&path, &updated).with_context(|| format!("write AGENTS.md at {}", path.display()))?;
    Ok(true)
}

/// Convenience wrapper resolving `$CODEX_HOME` (or `~/.codex`) before
/// delegating to [`ensure_agents_md_seeded_at`]. Returns `Ok(false)` if
/// codex_home cannot be resolved (no HOME env) — the SessionStart hook
/// fail-opens rather than blocking session boot.
pub fn ensure_agents_md_seeded() -> Result<bool> {
    let Some(codex_home) = resolve_codex_home() else {
        return Ok(false);
    };
    ensure_agents_md_seeded_at(&codex_home)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn creates_file_when_absent() {
        let tmp = TempDir::new().unwrap();
        let wrote = ensure_agents_md_seeded_at(tmp.path()).unwrap();
        assert!(wrote);
        let content = fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap();
        assert!(content.contains(BEGIN_MARKER));
        assert!(content.contains(END_MARKER));
        assert!(content.contains("Empirica Discipline"));
    }

    #[test]
    fn idempotent_when_content_matches() {
        let tmp = TempDir::new().unwrap();
        assert!(ensure_agents_md_seeded_at(tmp.path()).unwrap());
        // Second call must not rewrite.
        assert!(!ensure_agents_md_seeded_at(tmp.path()).unwrap());
    }

    #[test]
    fn appends_block_when_user_file_lacks_markers() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("AGENTS.md");
        fs::write(&path, "# My personal AGENTS.md\n\nSome rules here.\n").unwrap();
        let wrote = ensure_agents_md_seeded_at(tmp.path()).unwrap();
        assert!(wrote);
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("# My personal AGENTS.md"));
        assert!(content.contains("Some rules here."));
        assert!(content.contains(BEGIN_MARKER));
        assert!(content.contains(END_MARKER));
    }

    #[test]
    fn replaces_block_when_markers_present() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("AGENTS.md");
        let stale = format!(
            "Pre-amble\n\n{BEGIN_MARKER}\nstale empirica content\n{END_MARKER}\n\nPost-amble\n"
        );
        fs::write(&path, &stale).unwrap();
        let wrote = ensure_agents_md_seeded_at(tmp.path()).unwrap();
        assert!(wrote);
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("Pre-amble"));
        assert!(content.contains("Post-amble"));
        assert!(!content.contains("stale empirica content"));
        assert!(content.contains("Empirica Discipline"));
    }

    #[test]
    fn preserves_user_content_around_block_on_rewrite() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("AGENTS.md");
        // Seed once
        ensure_agents_md_seeded_at(tmp.path()).unwrap();
        // User adds content before and after our block
        let with_user = {
            let body = fs::read_to_string(&path).unwrap();
            format!("# User intro\n\n{body}\n# User outro\n")
        };
        fs::write(&path, &with_user).unwrap();
        // Re-seed (no content drift, but file now has user additions outside markers)
        ensure_agents_md_seeded_at(tmp.path()).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# User intro"));
        assert!(content.contains("# User outro"));
        assert!(content.contains(BEGIN_MARKER));
    }

    #[test]
    fn falls_back_to_append_when_markers_malformed() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("AGENTS.md");
        // BEGIN without END
        let malformed = format!("text {BEGIN_MARKER} dangling\n");
        fs::write(&path, &malformed).unwrap();
        ensure_agents_md_seeded_at(tmp.path()).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        // Original (malformed) preserved; full block appended after.
        assert!(content.contains("dangling"));
        // Now contains the END marker too (from the appended block).
        assert!(content.contains(END_MARKER));
    }
}
