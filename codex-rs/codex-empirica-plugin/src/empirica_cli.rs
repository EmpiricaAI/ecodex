//! Subprocess wrapper for invoking Empirica's Python hook scripts.
//!
//! This module is the single replacement target for future optimization:
//!   * v1   — invokes `<empirica's python> <hooks_dir>/<script>` with stdin/stdout pipes
//!   * v1.1 — could be replaced with PyO3 in-process call
//!   * v2   — could be replaced with a long-running sidecar daemon over UDS
//!
//! Hook handlers should not import process / subprocess primitives directly.
//! Routing through this module keeps the swap point in one place.

use std::ffi::OsStr;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

/// Sub-path within the plugin install dir where the bundled hooks live.
/// Mirrors CC empirica's `hooks/` + `lib/` sibling layout because
/// `sentinel-gate.py` (and others) compute `Path(__file__).parent.parent /
/// 'lib'` to find shared modules — bundling preserves that relationship.
const PLUGIN_HOOKS_SUBPATH: &str = "hooks_scripts/hooks";

/// CC-empirica fallback location used only when neither `EMPIRICA_HOOKS_DIR`
/// nor `PLUGIN_ROOT` is set. Lets users with both ecodex AND CC empirica
/// installed still work if the plugin install somehow shipped without the
/// bundled hooks (e.g. dev-mode running the binary directly).
const CC_FALLBACK_HOOKS_DIR: &str = "~/.claude/plugins/local/empirica/hooks";

/// Interpreter used when the empirica CLI's own cannot be resolved.
const FALLBACK_PYTHON: &str = "python3";

/// How much of the `empirica` script to read when looking for its shebang.
/// A console script's shebang (pip's two-line `/bin/sh` form included) sits
/// well inside this, and an unexpected large binary named `empirica` is never
/// read whole.
const SHEBANG_READ_LIMIT: u64 = 4096;

/// Result of running an Empirica hook script via subprocess.
pub struct HookOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Invoke `<empirica's python> <hooks_dir>/<script>` with `input_json` on stdin.
///
/// Returns `Err` for *infrastructure* failures — missing script file, failure
/// to spawn the interpreter, IO errors writing stdin. Hook handlers translate
/// `Err` into fail-open (allow). Exit code 2 from a successful subprocess run
/// is a genuine "block" signal from the script and must reach the caller
/// verbatim.
pub fn run_hook_script(script: &str, input_json: &str) -> Result<HookOutput> {
    let script_path = resolve_hooks_dir().join(script);

    // Pre-check: if the script file is missing, fail open rather than letting
    // python exit 2 (which would be misread as a hook block by codex).
    if !script_path.is_file() {
        anyhow::bail!("hook script not found: {}", script_path.display());
    }

    // ecodex T81 Tx-Z: propagate codex's session_id (thread_id UUID) as
    // EMPIRICA_INSTANCE_ID for the hook subprocess. Empirica's
    // get_instance_id() priority list reads EMPIRICA_INSTANCE_ID first,
    // so this gives every empirica artifact (sentinel state, session
    // bind, statusline) the same identity as codex's session — works
    // identically in tmux, non-tmux, ssh, container, headless. Without
    // this, empirica falls back to TMUX_PANE/TERM_SESSION_ID/WINDOWID
    // (none of which codex propagates) and silently fails to write the
    // instance file in non-tmux contexts. Caller doesn't have to set
    // EMPIRICA_INSTANCE_ID — we extract from the input JSON. Empty
    // session_id (e.g. legacy non-codex caller) leaves the env unset
    // and empirica falls back to its other priority keys.
    let codex_session_id = extract_session_id_from_input(input_json).unwrap_or_default();

    let interpreter = hook_interpreter(&std::env::var_os("PATH").unwrap_or_default());
    let mut command = Command::new(&interpreter);
    command
        .arg(&script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if !codex_session_id.is_empty() {
        command.env("EMPIRICA_INSTANCE_ID", &codex_session_id);
    }

    // Harness CWD==practice vouch (Karson's #91 contract; empirica PR #246).
    // codex invokes plugin hook commands with cwd = the session's project dir
    // (the practice), and there is no multiplexer launch-dir indirection — so
    // for the codex/ecodex harness CWD is always the verified practice. This
    // lets empirica's session-boundary hooks enable their gated filesystem
    // fallback (CWD/git-root) and resolve the practice for a FRESH practitioner
    // whose instance_projects/active_work cache is still empty — the case that
    // otherwise made post-compact.py fail with "SessionStart hook (failed)".
    // Empirica keeps the fallback OFF when this is unset (tmux/multiplexer,
    // where cwd may be the launch dir), so no cross-project bleed.
    command.env("EMPIRICA_CWD_RELIABLE", "true");

    // Harness identity for harness-aware empirica hooks (ecodex proposal to
    // empirica, 2026-07-07). A value != "claude-code" tells empirica's hooks
    // they are NOT under Claude Code: session-init._auto_sync_plugin() no-ops
    // (codex bundles hooks at PLUGIN_ROOT, not the CC install path, so
    // `empirica plugin-sync` heals a path codex never reads), and the
    // practitioner-presence writes drop the "Claude Code parent" labeling for a
    // harness-generic liveness anchor. Forward-compat: empirica ignores this
    // until the harness guard lands upstream, so it is a no-op today and
    // self-activates on the next re-vendor once empirica ships the guard.
    command.env("EMPIRICA_HARNESS", "codex");

    let mut child = command
        .spawn()
        .with_context(|| format!("spawn {} {}", interpreter.display(), script_path.display()))?;

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

/// Resolve the directory containing the empirica hook Python scripts.
///
/// Priority (highest first):
/// 1. `$EMPIRICA_HOOKS_DIR` — manual override (dev / debugging / non-standard layouts)
/// 2. `$PLUGIN_ROOT/hooks_scripts/hooks` — codex sets `PLUGIN_ROOT` when invoking
///    plugin hook commands (per `codex-rs/hooks/src/engine/discovery.rs:175`).
///    This is the normal runtime path: plugin install bundled its own copy.
/// 3. `~/.claude/plugins/local/empirica/hooks` — CC-empirica fallback for
///    coexisting installs / dev-mode runs of the bare binary.
fn resolve_hooks_dir() -> PathBuf {
    if let Ok(raw) = std::env::var("EMPIRICA_HOOKS_DIR") {
        return expand_tilde(&raw);
    }
    if let Ok(plugin_root) = std::env::var("PLUGIN_ROOT") {
        return PathBuf::from(plugin_root).join(PLUGIN_HOOKS_SUBPATH);
    }
    expand_tilde(CC_FALLBACK_HOOKS_DIR)
}

/// Whether the named hook script resolves to an existing file.
///
/// The PreToolUse firewall uses this to distinguish two failure modes that
/// `run_hook_script` otherwise collapses into one `Err`:
///   * script ABSENT (not installed) → fail-OPEN: the user opted out of the
///     firewall, so don't brick the harness.
///   * script PRESENT but unrunnable (spawn/IO failure) → fail-CLOSED: a
///     broken firewall must never silently allow.
pub fn hook_script_exists(script: &str) -> bool {
    resolve_hooks_dir().join(script).is_file()
}

/// The interpreter the vendored hooks run under: the one the `empirica` CLI on
/// `path_var` runs under, else `python3`.
///
/// pipx, uv tool and Homebrew put empirica in a private venv, so the first
/// `python3` on PATH usually cannot import it, and sentinel-gate.py then lets
/// every tool call through ("Empirica Sentinel is OFF"). The CLI's console
/// script names its venv interpreter in its shebang; empirica's own Claude Code
/// setup uses that same interpreter. This is resolved on every hook run, so it
/// follows upgrades without a cached path going stale.
///
/// Falls back to `python3` when no `empirica` is on PATH, its shebang goes
/// through `env`, the interpreter it names is gone, or it has no shebang (the
/// `.exe` launchers on Windows).
fn hook_interpreter(path_var: &OsStr) -> PathBuf {
    std::env::split_paths(path_var)
        .map(|dir| dir.join("empirica"))
        .find(|candidate| candidate.is_file())
        .and_then(|script| {
            let mut head = Vec::new();
            std::fs::File::open(script)
                .and_then(|file| file.take(SHEBANG_READ_LIMIT).read_to_end(&mut head))
                .ok()?;
            shebang_interpreter(&String::from_utf8_lossy(&head))
        })
        .filter(|interpreter| interpreter.is_file())
        .unwrap_or_else(|| PathBuf::from(FALLBACK_PYTHON))
}

/// The interpreter a Python console script's shebang names, if it names one
/// directly.
///
/// Besides `#!/path/to/python`, pip and uv write a two-line `/bin/sh`
/// trampoline when the interpreter path is too long for a shebang or contains
/// spaces:
///
/// ```text
/// #!/bin/sh
/// '''exec' "/path with spaces/python" "$0" "$@"
/// ```
///
/// The path there may be double-quoted, single-quoted or bare. An `env`
/// shebang names no interpreter of its own, so it yields `None`.
fn shebang_interpreter(head: &str) -> Option<PathBuf> {
    let mut lines = head.lines();
    let shebang = lines.next()?.strip_prefix("#!")?.trim();
    if shebang == "/bin/sh" {
        let exec = lines.next()?.strip_prefix("'''exec' ")?;
        let interpreter = match exec.chars().next() {
            Some(quote @ ('"' | '\'')) => exec[1..].split(quote).next()?,
            _ => exec.split_whitespace().next()?,
        };
        return (!interpreter.is_empty()).then(|| PathBuf::from(interpreter));
    }
    let interpreter = shebang.split_whitespace().next()?;
    (!interpreter.ends_with("/env")).then(|| PathBuf::from(interpreter))
}

/// Pluck the `session_id` field out of the codex hook input JSON.
///
/// Codex's hook payload schema (per
/// `codex-rs/hooks/schema/generated/session-start.command.input.schema.json`
/// and the matching ones for PreToolUse / PostToolUse / etc.) carries
/// `session_id` at the top level — the per-session UUID codex uses as
/// `ThreadId`. We don't want a serde dependency just for this one read,
/// so use `serde_json::Value` directly. Returns `None` on parse failure
/// (caller falls back to env-unset + empirica's TMUX_PANE/TTY-key path).
fn extract_session_id_from_input(input_json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(input_json).ok()?;
    value
        .get("session_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: run a closure with a clean env (the three vars we read all
    /// unset), then restore. Avoids cross-test pollution.
    fn with_clean_env<F: FnOnce()>(f: F) {
        // cargo test runs tests in PARALLEL by default, and these tests all
        // read/write the same process-global env vars (EMPIRICA_HOOKS_DIR /
        // PLUGIN_ROOT / HOME) — so they MUST be serialized or they race
        // (explicit_override_wins failed in CI when a sibling cleared
        // PLUGIN_ROOT mid-assert). A static mutex held for the closure's
        // duration enforces one-at-a-time execution; recover from poisoning
        // so one panicking test doesn't cascade-fail the rest.
        static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let saved = [
            ("EMPIRICA_HOOKS_DIR", std::env::var_os("EMPIRICA_HOOKS_DIR")),
            ("PLUGIN_ROOT", std::env::var_os("PLUGIN_ROOT")),
            ("HOME", std::env::var_os("HOME")),
        ];
        unsafe {
            std::env::remove_var("EMPIRICA_HOOKS_DIR");
            std::env::remove_var("PLUGIN_ROOT");
        }
        f();
        unsafe {
            for (k, v) in saved {
                match v {
                    Some(val) => std::env::set_var(k, val),
                    None => std::env::remove_var(k),
                }
            }
        }
    }

    #[test]
    fn explicit_override_wins() {
        with_clean_env(|| {
            unsafe {
                std::env::set_var("EMPIRICA_HOOKS_DIR", "/tmp/manual/hooks");
                std::env::set_var("PLUGIN_ROOT", "/should/be/ignored");
            }
            assert_eq!(resolve_hooks_dir(), PathBuf::from("/tmp/manual/hooks"));
        });
    }

    #[test]
    fn plugin_root_used_when_override_absent() {
        with_clean_env(|| {
            unsafe {
                std::env::set_var("PLUGIN_ROOT", "/var/codex/plugin/install");
            }
            assert_eq!(
                resolve_hooks_dir(),
                PathBuf::from("/var/codex/plugin/install/hooks_scripts/hooks")
            );
        });
    }

    #[test]
    fn cc_fallback_when_nothing_set() {
        with_clean_env(|| {
            unsafe {
                std::env::set_var("HOME", "/home/test");
            }
            assert_eq!(
                resolve_hooks_dir(),
                PathBuf::from("/home/test/.claude/plugins/local/empirica/hooks")
            );
        });
    }

    /// A PATH dir holding an `empirica` script with `shebang`, plus a venv
    /// interpreter file the shebang can point at.
    fn fake_install(
        shebang: impl FnOnce(&std::path::Path) -> String,
    ) -> (tempfile::TempDir, PathBuf) {
        let root = tempfile::tempdir().expect("tempdir");
        let python = root.path().join("venv/bin/python");
        std::fs::create_dir_all(python.parent().expect("parent")).expect("venv dir");
        std::fs::write(&python, "").expect("python");
        let bin = root.path().join("bin");
        std::fs::create_dir_all(&bin).expect("bin dir");
        std::fs::write(bin.join("empirica"), shebang(&python)).expect("empirica script");
        (root, python)
    }

    #[test]
    fn hook_interpreter_is_the_empirica_cli_interpreter() {
        let (root, python) = fake_install(|python| {
            format!(
                "#!{}\nimport sys\nfrom empirica.cli.cli_core import main\n",
                python.display()
            )
        });
        let path_var = std::env::join_paths([root.path().join("missing"), root.path().join("bin")])
            .expect("PATH");

        pretty_assertions::assert_eq!(hook_interpreter(&path_var), python);
    }

    #[test]
    fn hook_interpreter_falls_back_to_python3() {
        let (env_root, _) = fake_install(|_| "#!/usr/bin/env python3\n".to_string());
        let (gone_root, _) = fake_install(|_| "#!/no/such/venv/bin/python\n".to_string());
        let empty = tempfile::tempdir().expect("tempdir");

        pretty_assertions::assert_eq!(
            [
                env_root.path().join("bin"),
                gone_root.path().join("bin"),
                empty.path().to_path_buf(),
            ]
            .map(|dir| hook_interpreter(dir.as_os_str())),
            [
                PathBuf::from("python3"),
                PathBuf::from("python3"),
                PathBuf::from("python3"),
            ]
        );
    }

    #[test]
    fn shebang_interpreter_reads_direct_and_sh_trampoline_forms() {
        let heads = [
            "#!/home/u/.local/share/pipx/venvs/empirica/bin/python\nimport sys\n",
            "#!/bin/sh\n'''exec' \"/opt/my tools/venv/bin/python\" \"$0\" \"$@\"\n' '''\n",
            "#!/bin/sh\n'''exec' '/home/u/.local/share/uv/tools/empirica/bin/python' \"$0\" \"$@\"\n",
            "#!/bin/sh\n'''exec' /very/long/venv/bin/python \"$0\" \"$@\"\n",
            "#!/usr/bin/env python3\n",
            "import sys\n",
        ];

        pretty_assertions::assert_eq!(
            heads.map(shebang_interpreter),
            [
                Some(PathBuf::from(
                    "/home/u/.local/share/pipx/venvs/empirica/bin/python"
                )),
                Some(PathBuf::from("/opt/my tools/venv/bin/python")),
                Some(PathBuf::from(
                    "/home/u/.local/share/uv/tools/empirica/bin/python"
                )),
                Some(PathBuf::from("/very/long/venv/bin/python")),
                None,
                None,
            ]
        );
    }

    #[test]
    fn override_path_with_tilde_expands() {
        with_clean_env(|| {
            unsafe {
                std::env::set_var("HOME", "/home/foo");
                std::env::set_var("EMPIRICA_HOOKS_DIR", "~/custom/hooks");
            }
            assert_eq!(resolve_hooks_dir(), PathBuf::from("/home/foo/custom/hooks"));
        });
    }
}
