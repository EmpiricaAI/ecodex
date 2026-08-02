//! Background runtime that invokes plugin-contributed statusline commands.
//!
//! For each [`PluginStatuslineSource`] discovered at session start
//! (Tx6(b)/2 → /3a), this module spawns a long-running tokio task that
//! re-invokes the command on a fixed interval and forwards the captured
//! stdout to the TUI render loop via
//! [`AppEvent::PluginStatuslineOutputUpdated`].
//!
//! ## Lifecycle
//!
//! - One `tokio::spawn` per source. The task owns its own interval loop;
//!   the runtime tracks each task's [`JoinHandle`] so they can be
//!   aborted when [`PluginStatuslineRuntime::set_sources`] replaces the
//!   source set (e.g. plugin reload, feature toggle).
//! - Each tick spawns a *fresh* subprocess. There is no overlap: the
//!   tick `await`s the subprocess (or its timeout) before scheduling
//!   the next sleep. So even if a plugin's command takes longer than
//!   the tick interval, we never accumulate concurrent invocations
//!   for that plugin (back-pressure built in).
//! - On any subprocess failure (spawn error, non-zero exit, timeout)
//!   the runtime emits an empty `output` so the renderer falls back to
//!   "no plugin line" rather than displaying stale text.
//!
//! ## Environment passed to plugin scripts
//!
//! Same contract as the hook subprocess invocation in the empirica
//! plugin, so vendored asset lookups work identically:
//!
//! - `PLUGIN_ROOT` + `CLAUDE_PLUGIN_ROOT` (CC compat) → plugin install dir
//! - `PLUGIN_DATA` + `CLAUDE_PLUGIN_DATA` → plugin data dir
//!
//! ## Timeouts
//!
//! Each subprocess run is wrapped with [`tokio::time::timeout`]. On
//! timeout the child is `kill`ed (best-effort) and an empty output is
//! reported. The default cap is intentionally tight so a hung plugin
//! never blocks the TUI footer for more than a render frame or two.

use std::collections::HashMap;
use std::process::Stdio;
use std::time::Duration;

use codex_plugin::PluginId;
use codex_plugin::PluginStatuslineSource;
use tokio::task::JoinHandle;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

/// How long to wait between successive invocations of a single plugin's
/// statusline command. Same cadence empirica's chat statusline + the CC
/// plugin's statusline_empirica.py settled on.
const TICK_INTERVAL: Duration = Duration::from_millis(1_500);

/// Hard cap on a single subprocess run. A plugin that hangs longer than
/// this is reported as empty output; a fresh attempt fires on the next tick.
const SUBPROCESS_TIMEOUT: Duration = Duration::from_secs(2);

/// Owns one background task per registered plugin statusline command.
/// Constructed once per ChatWidget; [`set_sources`] swaps the active
/// source set (aborts old tasks, spawns new ones).
pub(crate) struct PluginStatuslineRuntime {
    app_event_tx: AppEventSender,
    tasks: HashMap<PluginId, JoinHandle<()>>,
}

impl PluginStatuslineRuntime {
    pub(crate) fn new(app_event_tx: AppEventSender) -> Self {
        Self {
            app_event_tx,
            tasks: HashMap::new(),
        }
    }

    /// Replace the active source set. Tasks for sources no longer present
    /// are aborted; tasks for new sources are spawned. Tasks for sources
    /// that were already running (matched by plugin_id) are left alone.
    pub(crate) fn set_sources(&mut self, sources: Vec<PluginStatuslineSource>) {
        let new_ids: HashMap<PluginId, PluginStatuslineSource> = sources
            .into_iter()
            .map(|src| (src.plugin_id.clone(), src))
            .collect();

        // Abort tasks no longer needed.
        let stale_ids: Vec<PluginId> = self
            .tasks
            .keys()
            .filter(|id| !new_ids.contains_key(id))
            .cloned()
            .collect();
        for id in stale_ids {
            if let Some(handle) = self.tasks.remove(&id) {
                handle.abort();
            }
        }

        // Spawn tasks for newly-added sources.
        for (id, source) in new_ids {
            self.tasks.entry(id).or_insert_with(|| {
                let tx = self.app_event_tx.clone();
                tokio::spawn(run_plugin_statusline_loop(source, tx))
            });
        }
    }

    /// Abort all running tasks. Called on ChatWidget drop.
    pub(crate) fn abort_all(&mut self) {
        for (_, handle) in self.tasks.drain() {
            handle.abort();
        }
    }
}

impl Drop for PluginStatuslineRuntime {
    fn drop(&mut self) {
        self.abort_all();
    }
}

/// Per-source loop. Runs forever; killed via `JoinHandle::abort()`
/// when the runtime swaps source sets or ChatWidget is dropped.
async fn run_plugin_statusline_loop(source: PluginStatuslineSource, tx: AppEventSender) {
    let plugin_root = source.plugin_root.as_path().to_string_lossy().to_string();
    let plugin_data_root = source
        .plugin_data_root
        .as_path()
        .to_string_lossy()
        .to_string();
    let command = source.command.as_path().to_path_buf();
    let plugin_id = source.plugin_id.clone();

    // Fire once immediately so the footer populates before the first tick
    // interval elapses (avoids a 1.5s blank gap on session start).
    let output = invoke_once(&command, &plugin_root, &plugin_data_root).await;
    tx.send(AppEvent::PluginStatuslineOutputUpdated {
        plugin_id: plugin_id.clone(),
        output,
    });

    loop {
        tokio::time::sleep(TICK_INTERVAL).await;
        let output = invoke_once(&command, &plugin_root, &plugin_data_root).await;
        tx.send(AppEvent::PluginStatuslineOutputUpdated {
            plugin_id: plugin_id.clone(),
            output,
        });
    }
}

/// Spawn the plugin command, write the empirica-session JSON context to
/// its stdin, capture stdout up to the timeout. Returns an empty Vec on
/// any failure (spawn error, non-zero exit, timeout) so the renderer can
/// simply skip empty cells.
///
/// **ecodex T81 Tx-W fix**: previously this used `Stdio::null()`, which
/// meant the bundled `statusline_empirica.py` saw no input on stdin and
/// always rendered `[ecodex:inactive]` — every empirica session lookup
/// path requires a `session_id` (or at least a `cwd`) on stdin to
/// resolve. The fix is to discover the active empirica_session_id from
/// `~/.empirica/instance_projects/tmux_<TMUX_PANE>.json` (or by cwd
/// match across all instance_projects entries) and pipe a small JSON
/// context to the script. The doctor's
/// `check_ecodex_statusline_runtime_stdin` regression-tests this.
async fn invoke_once(
    command: &std::path::Path,
    plugin_root: &str,
    plugin_data_root: &str,
) -> Vec<u8> {
    let stdin_payload = build_statusline_stdin_payload();

    let spawn_result = tokio::process::Command::new(command)
        .env("PLUGIN_ROOT", plugin_root)
        .env("CLAUDE_PLUGIN_ROOT", plugin_root)
        .env("PLUGIN_DATA", plugin_data_root)
        .env("CLAUDE_PLUGIN_DATA", plugin_data_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn();

    let mut child = match spawn_result {
        Ok(child) => child,
        Err(_) => return Vec::new(),
    };

    // Feed the JSON context, then close stdin so the script's stdin.read()
    // returns. We swallow the write error: the child still has the env
    // vars (PLUGIN_ROOT et al.), so even if the pipe write fails the
    // script can still produce a degraded result on its own.
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        let _ = stdin.write_all(stdin_payload.as_bytes()).await;
        // Dropping `stdin` here closes the pipe — the script's blocking
        // `sys.stdin.read()` unblocks with the bytes it received.
        drop(stdin);
    }

    let wait_result = child.wait_with_output();
    match tokio::time::timeout(SUBPROCESS_TIMEOUT, wait_result).await {
        Ok(Ok(output)) if output.status.success() => output.stdout,
        // Either the subprocess produced an error exit code or we hit
        // an io error while waiting. Either way: empty output.
        Ok(_) => Vec::new(),
        // Timeout: the inner future is dropped, kill_on_drop fires.
        Err(_) => Vec::new(),
    }
}

/// Build the JSON payload piped to plugin statusline scripts.
///
/// Resolution strategy for `session_id`:
///   1. `~/.empirica/instance_projects/tmux_<TMUX_PANE>.json` — direct
///      pane bind written by the empirica session-init hook
///   2. Any `~/.empirica/instance_projects/*.json` whose `project_path`
///      matches the current cwd — fallback when TMUX_PANE isn't set
///      (e.g. running outside tmux)
///   3. Empty payload — script renders `[ecodex:inactive]`, which is the
///      correct UX signal that no session is bound to this shell
fn build_statusline_stdin_payload() -> String {
    let session_id = resolve_empirica_session_id_for_current_shell();
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|p| p.to_str().map(str::to_string));

    let mut obj = serde_json::Map::new();
    if let Some(sid) = session_id {
        obj.insert("session_id".into(), serde_json::Value::String(sid));
    }
    if let Some(c) = cwd {
        obj.insert("cwd".into(), serde_json::Value::String(c));
    }
    if obj.is_empty() {
        // Still produce valid JSON ({}) so scripts that strict-parse stdin
        // don't error.
        return "{}".to_string();
    }
    serde_json::Value::Object(obj).to_string()
}

fn resolve_empirica_session_id_for_current_shell() -> Option<String> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from)?;
    let instance_dir = home.join(".empirica").join("instance_projects");

    // Mirrors empirica/plugins/claude-code-integration/lib/project_resolver.py
    // ::get_instance_id() priority list. Order:
    //   1. EMPIRICA_INSTANCE_ID (explicit override / codex thread_id, set by
    //      Tx-Z's plugin propagation). Stored as `<id>.json` literally.
    //   2. TMUX_PANE → tmux_<num>.json
    //   3. TERM_SESSION_ID → term_<sanitized>.json (macOS Terminal.app)
    //   4. WINDOWID → wid_<num>.json (X11)
    //   5. cwd-prefix match across all instance files (last resort, ambiguous
    //      for multi-instance same-cwd; documented gap pending Tx-Z's plugin
    //      side landing on every install).
    if let Ok(explicit) = std::env::var("EMPIRICA_INSTANCE_ID")
        && !explicit.is_empty()
    {
        let path = instance_dir.join(format!("{explicit}.json"));
        if let Some(sid) = read_session_id_from_instance_file(&path) {
            return Some(sid);
        }
    }
    if let Ok(pane) = std::env::var("TMUX_PANE") {
        let pane_num = pane.trim_start_matches('%');
        let path = instance_dir.join(format!("tmux_{pane_num}.json"));
        if let Some(sid) = read_session_id_from_instance_file(&path) {
            return Some(sid);
        }
    }
    if let Ok(term) = std::env::var("TERM_SESSION_ID") {
        let safe = term.replace('/', "_");
        let path = instance_dir.join(format!("term_{safe}.json"));
        if let Some(sid) = read_session_id_from_instance_file(&path) {
            return Some(sid);
        }
    }
    if let Ok(wid) = std::env::var("WINDOWID") {
        let path = instance_dir.join(format!("wid_{wid}.json"));
        if let Some(sid) = read_session_id_from_instance_file(&path) {
            return Some(sid);
        }
    }

    // Cwd-prefix match across all instance entries — picks the most recently
    // written file whose project_path is a prefix of current cwd. Fragile
    // for multi-instance same-cwd; only reached when none of the explicit
    // identity keys above resolved.
    let cwd = std::env::current_dir().ok()?;
    let cwd_str = cwd.to_str()?;
    let entries = std::fs::read_dir(&instance_dir).ok()?;
    let mut best: Option<(std::time::SystemTime, String)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
            continue;
        };
        let project_path = json.get("project_path").and_then(|v| v.as_str());
        let session_id = json
            .get("empirica_session_id")
            .and_then(|v| v.as_str())
            .or_else(|| json.get("session_id").and_then(|v| v.as_str()));
        let (Some(pp), Some(sid)) = (project_path, session_id) else {
            continue;
        };
        if cwd_str.starts_with(pp) {
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            match &best {
                None => best = Some((mtime, sid.to_string())),
                Some((bt, _)) if &mtime > bt => best = Some((mtime, sid.to_string())),
                _ => {}
            }
        }
    }
    best.map(|(_, sid)| sid)
}

fn read_session_id_from_instance_file(path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("empirica_session_id")
        .or_else(|| json.get("session_id"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}
