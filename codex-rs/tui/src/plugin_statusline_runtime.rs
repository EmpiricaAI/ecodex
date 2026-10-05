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
//! - `EMPIRICA_INSTANCE_ID` → the codex thread id, once the session is
//!   configured. The empirica session-init hook records each session as
//!   `~/.empirica/instance_projects/<thread id>.json`, so this is what lets
//!   the statusline name the right session when several run in one
//!   directory. The same id goes first in the stdin payload resolution.
//!
//! The stdin payload also carries the context-window use and the active
//! model (see [`LiveContext`]), in the shape Claude Code's statusline
//! payload uses, so one script serves both hosts.
//!
//! ## Timeouts
//!
//! Each subprocess run is wrapped with [`tokio::time::timeout`]. On
//! timeout the child is `kill`ed (best-effort) and an empty output is
//! reported. The default cap is intentionally tight so a hung plugin
//! never blocks the TUI footer for more than a render frame or two.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
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

/// What the TUI knows and a statusline script cannot find on its own: which
/// codex thread this is, how much of the model's context window is in use,
/// and which model is active. Shared with the running loops so every tick
/// carries the latest values; the script receives them on stdin in the
/// shape Claude Code's statusline payload uses (`context_window.used_percentage`,
/// `model.{id,display_name}`), so the vendored statusline renders them unchanged.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LiveContext {
    /// The codex thread id of the active session.
    pub(crate) instance_id: Option<String>,
    /// Percent of the model's context window in use, 0–100, once token
    /// usage is known for the session.
    pub(crate) context_used_percentage: Option<u8>,
    /// The active model slug.
    pub(crate) model: Option<String>,
}

/// Owns one background task per registered plugin statusline command.
/// Constructed once per ChatWidget; [`set_sources`] swaps the active
/// source set (aborts old tasks, spawns new ones).
pub(crate) struct PluginStatuslineRuntime {
    app_event_tx: AppEventSender,
    tasks: HashMap<PluginId, JoinHandle<()>>,
    /// Session facts shared with the running loops so a change reaches them
    /// on their next tick.
    live: Arc<Mutex<LiveContext>>,
}

impl PluginStatuslineRuntime {
    pub(crate) fn new(app_event_tx: AppEventSender) -> Self {
        Self {
            app_event_tx,
            tasks: HashMap::new(),
            live: Arc::new(Mutex::new(LiveContext::default())),
        }
    }

    /// Record the codex thread id the statusline should identify itself by.
    /// Set on the child's environment and tried first when resolving the
    /// empirica session, instead of a process-global variable.
    pub(crate) fn set_instance_id(&self, instance_id: Option<String>) {
        if let Ok(mut live) = self.live.lock() {
            live.instance_id = instance_id;
        }
    }

    /// Record how much of the context window is in use (0–100), or `None`
    /// while token usage is unknown; the statusline shows it on the next tick.
    pub(crate) fn set_context_usage(&self, used_percentage: Option<u8>) {
        if let Ok(mut live) = self.live.lock() {
            live.context_used_percentage = used_percentage;
        }
    }

    /// Record the active model so the statusline can name it.
    pub(crate) fn set_model(&self, model: Option<String>) {
        if let Ok(mut live) = self.live.lock() {
            live.model = model;
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
                let live = Arc::clone(&self.live);
                tokio::spawn(run_plugin_statusline_loop(source, tx, live))
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
async fn run_plugin_statusline_loop(
    source: PluginStatuslineSource,
    tx: AppEventSender,
    live: Arc<Mutex<LiveContext>>,
) {
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
    let output = invoke_once(&command, &plugin_root, &plugin_data_root, &snapshot(&live)).await;
    tx.send(AppEvent::PluginStatuslineOutputUpdated {
        plugin_id: plugin_id.clone(),
        output,
    });

    loop {
        tokio::time::sleep(TICK_INTERVAL).await;
        let output = invoke_once(&command, &plugin_root, &plugin_data_root, &snapshot(&live)).await;
        tx.send(AppEvent::PluginStatuslineOutputUpdated {
            plugin_id: plugin_id.clone(),
            output,
        });
    }
}

fn snapshot(live: &Mutex<LiveContext>) -> LiveContext {
    live.lock().map(|l| l.clone()).unwrap_or_default()
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
    command: &Path,
    plugin_root: &str,
    plugin_data_root: &str,
    live: &LiveContext,
) -> Vec<u8> {
    let stdin_payload = build_statusline_stdin_payload(live);
    let instance_id = live.instance_id.as_deref();

    let mut process = tokio::process::Command::new(command);
    process
        .env("PLUGIN_ROOT", plugin_root)
        .env("CLAUDE_PLUGIN_ROOT", plugin_root)
        .env("PLUGIN_DATA", plugin_data_root)
        .env("CLAUDE_PLUGIN_DATA", plugin_data_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    if let Some(instance_id) = instance_id {
        process.env("EMPIRICA_INSTANCE_ID", instance_id);
    }
    let spawn_result = process.spawn();

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
///   1. `~/.empirica/instance_projects/<codex thread id>.json`, the file
///      the empirica session-init hook writes for this session
///   2. The same directory keyed by the TUI's own environment: an explicit
///      `EMPIRICA_INSTANCE_ID`, `tmux_<TMUX_PANE>`, `term_<TERM_SESSION_ID>`,
///      `wid_<WINDOWID>`
///   3. Any instance file whose `project_path` is a prefix of the current
///      cwd — ambiguous when several sessions share a directory
///   4. Empty payload — script renders `[ecodex:inactive]`, which is the
///      correct UX signal that no session is bound to this shell
///
/// Alongside the session, the payload carries what the TUI knows and the
/// script cannot find on its own, in Claude Code's statusline shape:
/// `context_window.used_percentage` and `model.{id,display_name}`.
fn build_statusline_stdin_payload(live: &LiveContext) -> String {
    let cwd = std::env::current_dir().ok();
    let session_id = std::env::var_os("HOME")
        .map(PathBuf::from)
        .and_then(|home| {
            let instance_dir = home.join(".empirica").join("instance_projects");
            resolve_in_dir(
                &instance_dir,
                &instance_file_candidates(live.instance_id.as_deref()),
                cwd.as_deref(),
            )
        });
    let cwd = cwd.and_then(|p| p.to_str().map(str::to_string));
    statusline_payload_json(session_id, cwd, live)
}

/// The stdin JSON for the statusline script, from already-resolved parts.
/// Pure, so the shape is testable without a home directory or a cwd.
fn statusline_payload_json(
    session_id: Option<String>,
    cwd: Option<String>,
    live: &LiveContext,
) -> String {
    let mut obj = serde_json::Map::new();
    if let Some(sid) = session_id {
        obj.insert("session_id".into(), serde_json::Value::String(sid));
    }
    if let Some(c) = cwd {
        obj.insert("cwd".into(), serde_json::Value::String(c));
    }
    if let Some(used) = live.context_used_percentage {
        obj.insert(
            "context_window".into(),
            serde_json::json!({ "used_percentage": used }),
        );
    }
    if let Some(model) = live.model.as_deref() {
        obj.insert(
            "model".into(),
            serde_json::json!({ "id": model, "display_name": model }),
        );
    }
    if obj.is_empty() {
        // Still produce valid JSON ({}) so scripts that strict-parse stdin
        // don't error.
        return "{}".to_string();
    }
    serde_json::Value::Object(obj).to_string()
}

/// Instance-file stems to try, most specific first. Mirrors
/// empirica's `get_instance_id()` priority list after the session's own
/// thread id: an explicit `EMPIRICA_INSTANCE_ID`, then the tmux pane, the
/// Terminal.app session and the X11 window.
fn instance_file_candidates(instance_id: Option<&str>) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(id) = instance_id.filter(|id| !id.is_empty()) {
        candidates.push(id.to_string());
    }
    if let Ok(explicit) = std::env::var("EMPIRICA_INSTANCE_ID")
        && !explicit.is_empty()
    {
        candidates.push(explicit);
    }
    if let Ok(pane) = std::env::var("TMUX_PANE") {
        candidates.push(format!("tmux_{}", pane.trim_start_matches('%')));
    }
    if let Ok(term) = std::env::var("TERM_SESSION_ID") {
        candidates.push(format!("term_{}", term.replace('/', "_")));
    }
    if let Ok(wid) = std::env::var("WINDOWID") {
        candidates.push(format!("wid_{wid}"));
    }
    candidates
}

/// The first candidate whose `<stem>.json` names a session wins. Failing
/// that, the most recently written instance file whose `project_path` is a
/// prefix of `cwd`, which cannot tell apart sessions sharing a directory;
/// that is why callers put the thread id first.
fn resolve_in_dir(
    instance_dir: &Path,
    candidates: &[String],
    cwd: Option<&Path>,
) -> Option<String> {
    for stem in candidates {
        let path = instance_dir.join(format!("{stem}.json"));
        if let Some(sid) = read_session_id_from_instance_file(&path) {
            return Some(sid);
        }
    }

    let cwd_str = cwd?.to_str()?;
    let entries = std::fs::read_dir(instance_dir).ok()?;
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

fn read_session_id_from_instance_file(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("empirica_session_id")
        .or_else(|| json.get("session_id"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

#[cfg(test)]
#[path = "plugin_statusline_runtime_tests.rs"]
mod tests;
