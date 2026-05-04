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

/// Spawn the plugin command, capture stdout up to the timeout. Returns
/// an empty Vec on any failure (spawn error, non-zero exit, timeout) so
/// the renderer can simply skip empty cells.
async fn invoke_once(
    command: &std::path::Path,
    plugin_root: &str,
    plugin_data_root: &str,
) -> Vec<u8> {
    let spawn_result = tokio::process::Command::new(command)
        .env("PLUGIN_ROOT", plugin_root)
        .env("CLAUDE_PLUGIN_ROOT", plugin_root)
        .env("PLUGIN_DATA", plugin_data_root)
        .env("CLAUDE_PLUGIN_DATA", plugin_data_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .output();

    match tokio::time::timeout(SUBPROCESS_TIMEOUT, spawn_result).await {
        Ok(Ok(output)) if output.status.success() => output.stdout,
        // Either the subprocess produced an error exit code or we hit
        // an io error while spawning. Either way: empty output.
        Ok(_) => Vec::new(),
        // Timeout: the inner future is dropped, kill_on_drop fires.
        Err(_) => Vec::new(),
    }
}
