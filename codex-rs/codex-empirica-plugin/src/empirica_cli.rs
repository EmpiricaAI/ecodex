//! Subprocess wrapper for invoking Empirica's Python hook scripts.
//!
//! This module is the single replacement target for future optimization:
//!   * v1   — invokes `python3 <hooks_dir>/<script>` with stdin/stdout pipes
//!   * v1.1 — could be replaced with PyO3 in-process call
//!   * v2   — could be replaced with a long-running sidecar daemon over UDS
//!
//! Hook handlers should not import process / subprocess primitives directly.
//! Routing through this module keeps the swap point in one place.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

/// Default location of Empirica hook scripts when `EMPIRICA_HOOKS_DIR` is unset.
const DEFAULT_HOOKS_DIR: &str = "~/.claude/plugins/local/empirica/hooks";

/// Result of running an Empirica hook script via subprocess.
pub struct HookOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Invoke `python3 <hooks_dir>/<script>` with `input_json` on stdin.
pub fn run_hook_script(script: &str, input_json: &str) -> Result<HookOutput> {
    let script_path = resolve_hooks_dir().join(script);

    let mut child = Command::new("python3")
        .arg(&script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawn python3 {}", script_path.display()))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input_json.as_bytes())
            .context("write hook input to subprocess stdin")?;
    }

    let output = child
        .wait_with_output()
        .context("wait for empirica hook subprocess")?;

    Ok(HookOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}

fn resolve_hooks_dir() -> PathBuf {
    let raw = std::env::var("EMPIRICA_HOOKS_DIR").unwrap_or_else(|_| DEFAULT_HOOKS_DIR.to_string());
    expand_tilde(&raw)
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        let mut buf = PathBuf::from(home);
        buf.push(rest);
        return buf;
    }
    PathBuf::from(path)
}
