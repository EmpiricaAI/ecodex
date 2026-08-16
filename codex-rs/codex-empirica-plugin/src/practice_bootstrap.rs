//! Host-side initialization for a fresh Empirica practice.
//!
//! Codex launches `SessionStart` hooks in the harness process at the session
//! cwd, before agent-requested commands enter the exec sandbox. That ordering
//! lets ecodex create the git transport without granting the agent write access
//! to protected `.git` metadata.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;

const AI_ID_ENV: &str = "EMPIRICA_AI_ID";

#[derive(Debug, Deserialize)]
struct SessionStartInput {
    cwd: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BootstrapCommand {
    GitInit,
    EmpiricaProjectInit,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PracticeBootstrapOutcome {
    workspace: PathBuf,
    git_initialized: bool,
    empirica_initialized: bool,
}

impl PracticeBootstrapOutcome {
    pub(crate) fn changed(&self) -> bool {
        self.git_initialized || self.empirica_initialized
    }

    pub(crate) fn workspace(&self) -> &Path {
        &self.workspace
    }
}

pub(crate) fn ensure_practice(input_json: &str) -> Result<PracticeBootstrapOutcome> {
    let process_cwd = std::env::current_dir().context("resolve SessionStart process cwd")?;
    let ai_id = std::env::var(AI_ID_ENV).ok();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    ensure_practice_with(
        input_json,
        &process_cwd,
        home.as_deref(),
        ai_id.as_deref(),
        ancestor_has_git_metadata,
        run_command,
    )
}

fn ensure_practice_with(
    input_json: &str,
    process_cwd: &Path,
    home: Option<&Path>,
    ai_id: Option<&str>,
    ancestor_has_git: impl FnOnce(&Path) -> bool,
    mut run: impl FnMut(BootstrapCommand, &Path) -> Result<()>,
) -> Result<PracticeBootstrapOutcome> {
    let input: SessionStartInput =
        serde_json::from_str(input_json).context("parse SessionStart cwd")?;
    let workspace = canonical_directory(&input.cwd, "hook-provided SessionStart cwd")?;
    let process_cwd = canonical_directory(process_cwd, "SessionStart process cwd")?;
    if workspace != process_cwd {
        bail!(
            "refusing to bootstrap {} because the SessionStart process runs at {}",
            workspace.display(),
            process_cwd.display()
        );
    }

    // Never bootstrap the user's home directory or the filesystem root: a
    // session opened there is almost certainly not "a workspace the user
    // pointed ecodex at", and silently running `git init` in $HOME is exactly
    // the kind of surprise mutation a harness must not make. Such sessions
    // keep today's behavior (no practice until the user initializes one
    // deliberately).
    if workspace.parent().is_none() {
        bail!("refusing to bootstrap the filesystem root");
    }
    if let Some(home) = home
        && !home.as_os_str().is_empty()
        && fs::canonicalize(home).is_ok_and(|home| home == workspace)
    {
        bail!(
            "refusing to bootstrap the home directory {}",
            workspace.display()
        );
    }

    let git_exists = workspace.join(".git").exists();
    let empirica_exists = workspace.join(".empirica").exists();
    if !git_exists && ancestor_has_git(&workspace) {
        bail!(
            "refusing to bootstrap nested workspace {} inside an ancestor git repository",
            workspace.display()
        );
    }

    let mut outcome = PracticeBootstrapOutcome {
        workspace: workspace.clone(),
        git_initialized: false,
        empirica_initialized: false,
    };
    if !git_exists {
        run(BootstrapCommand::GitInit, &workspace)?;
        outcome.git_initialized = true;
    }
    if !empirica_exists {
        run(BootstrapCommand::EmpiricaProjectInit, &workspace)?;
        outcome.empirica_initialized = true;
        if let Some(ai_id) = ai_id.map(str::trim).filter(|ai_id| !ai_id.is_empty()) {
            persist_ai_id(&workspace, ai_id)?;
        }
    }

    Ok(outcome)
}

fn canonical_directory(path: &Path, description: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path)
        .with_context(|| format!("canonicalize {description} {}", path.display()))?;
    if !canonical.is_dir() {
        bail!("{description} is not a directory: {}", canonical.display());
    }
    Ok(canonical)
}

fn ancestor_has_git_metadata(workspace: &Path) -> bool {
    workspace
        .parent()
        .into_iter()
        .flat_map(Path::ancestors)
        .any(|ancestor| ancestor.join(".git").exists())
}

fn run_command(command: BootstrapCommand, workspace: &Path) -> Result<()> {
    let mut process = match command {
        BootstrapCommand::GitInit => {
            let mut process = Command::new("git");
            process.args(["init", "--quiet"]);
            process
        }
        BootstrapCommand::EmpiricaProjectInit => {
            let mut process = Command::new("empirica");
            process.args(["project-init", "--non-interactive", "--output", "json"]);
            process
        }
    };
    let output = process
        .current_dir(workspace)
        .output()
        .with_context(|| format!("spawn {}", command.program_name()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "{} failed in {}: {}",
            command.program_name(),
            workspace.display(),
            stderr.trim()
        );
    }
    Ok(())
}

impl BootstrapCommand {
    fn program_name(self) -> &'static str {
        match self {
            Self::GitInit => "git init",
            Self::EmpiricaProjectInit => "empirica project-init",
        }
    }
}

fn persist_ai_id(workspace: &Path, ai_id: &str) -> Result<()> {
    let project_yaml = workspace.join(".empirica/project.yaml");
    let contents = fs::read_to_string(&project_yaml)
        .with_context(|| format!("read {}", project_yaml.display()))?;
    let quoted_ai_id = serde_json::to_string(ai_id).context("quote project.yaml ai_id")?;
    let mut replacements = 0;
    let contents = contents
        .split_inclusive('\n')
        .map(|line| {
            let (body, newline) = line.strip_suffix("\r\n").map_or_else(
                || {
                    line.strip_suffix('\n')
                        .map_or((line, ""), |body| (body, "\n"))
                },
                |body| (body, "\r\n"),
            );
            if body.starts_with("ai_id:") {
                replacements += 1;
                format!("ai_id: {quoted_ai_id}{newline}")
            } else {
                line.to_string()
            }
        })
        .collect::<String>();
    if replacements != 1 {
        bail!(
            "expected one top-level ai_id in {}, found {replacements}",
            project_yaml.display()
        );
    }
    fs::write(&project_yaml, contents)
        .with_context(|| format!("write {}", project_yaml.display()))?;
    Ok(())
}

#[cfg(test)]
#[path = "practice_bootstrap_tests.rs"]
mod tests;
