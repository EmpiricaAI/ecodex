// ecodex addition: Monitor primitive — sub-second wake on background
// subprocess output matching a regex pattern. Mirrors Claude Code's
// Monitor tool semantics so cross-AI mesh (cortex_propose / inbox-poll
// via ntfy held connection) works in non-Claude models running in ecodex.
//
// Architecture:
//   1. Tool handler receives {action:"arm", command, pattern, persistent}
//   2. `spawn_monitor` starts the subprocess via tokio::Command, reads stdout
//      line-by-line, compiles the pattern once, and on each match calls
//      Session::inject_response_items with a synthetic <task-notification>
//      message body. The agent picks it up on the next turn.
//   3. Handle is stored in a per-session registry keyed by uuid string.
//   4. Disarm via {action:"kill", monitor_id} or session shutdown cleanup.
//
// Differs from CC's Monitor: this bundles spawn+watch (one tool call instead
// of two). Simpler API for v0; persistent=true keeps the watch armed across
// multiple matches.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseInputItem;
use regex_lite::Regex;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::warn;
use uuid::Uuid;

use crate::session::session::Session;

/// What stream(s) of the watched subprocess should be matched.
#[derive(Debug, Clone, Copy, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MonitorStream {
    #[default]
    Stdout,
    Stderr,
    Both,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ArmMonitorOptions {
    /// Argv to spawn. First element is the program; rest are args.
    pub command: Vec<String>,
    /// Regex pattern (regex_lite syntax, no look-around).
    pub pattern: String,
    /// When true, the monitor stays armed after a match and continues
    /// emitting wakes. When false, it disarms itself after the first match.
    #[serde(default = "default_persistent")]
    pub persistent: bool,
    /// Which subprocess stream to watch.
    #[serde(default)]
    pub stream: MonitorStream,
    /// Optional cwd for the spawned command.
    #[serde(default)]
    pub cwd: Option<String>,
}

fn default_persistent() -> bool {
    false
}

#[derive(Debug)]
pub struct MonitorEntry {
    /// Tokio task handle for the background watcher. Aborting cancels both
    /// the watcher loop and the spawned subprocess (via child drop on
    /// task exit).
    pub handle: JoinHandle<()>,
    /// Pattern + cmd info preserved for `list`-style introspection.
    pub command: Vec<String>,
    pub pattern: String,
    pub persistent: bool,
}

/// Per-session registry of armed monitors. Held on Session::services so the
/// shutdown path can abort all entries.
#[derive(Default)]
pub struct MonitorRegistry {
    /// Exposed crate-internal for the `monitor` tool handler's `list` path,
    /// which iterates entries to surface command + pattern metadata.
    pub(crate) inner: Mutex<HashMap<String, MonitorEntry>>,
}

impl MonitorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Abort every armed monitor. Called on session shutdown.
    pub async fn abort_all(&self) {
        let mut guard = self.inner.lock().await;
        for (_id, entry) in guard.drain() {
            entry.handle.abort();
        }
    }

    /// Number of currently armed monitors (introspection / metrics).
    #[allow(dead_code)]
    pub async fn len(&self) -> usize {
        self.inner.lock().await.len()
    }

    /// Disarm a single monitor by id. Returns true if the id matched.
    pub async fn kill(&self, monitor_id: &str) -> bool {
        let mut guard = self.inner.lock().await;
        match guard.remove(monitor_id) {
            Some(entry) => {
                entry.handle.abort();
                true
            }
            None => false,
        }
    }
}

/// Spawn a background watcher and register it. Returns the monitor id.
///
/// The watcher reads the chosen stream line-by-line, applies the regex,
/// and on each match calls `Session::inject_response_items` with a
/// system-flavored user-role message describing the wake event.
pub async fn spawn_monitor(
    session: Arc<Session>,
    options: ArmMonitorOptions,
) -> Result<String, MonitorArmError> {
    if options.command.is_empty() {
        return Err(MonitorArmError::EmptyCommand);
    }
    let pattern = Regex::new(&options.pattern)
        .map_err(|err| MonitorArmError::InvalidPattern(err.to_string()))?;

    let monitor_id = Uuid::new_v4().to_string();
    let registry = session.services.monitor_registry.clone();

    let cmd_display = options.command.join(" ");
    let mut cmd = Command::new(&options.command[0]);
    cmd.args(&options.command[1..]);
    if let Some(cwd) = options.cwd.as_deref() {
        cmd.current_dir(cwd);
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);

    let mut child = cmd
        .spawn()
        .map_err(|err| MonitorArmError::SpawnFailed(err.to_string()))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let persistent = options.persistent;
    let stream_kind = options.stream;
    let id_for_task = monitor_id.clone();
    let cmd_display_for_task = cmd_display.clone();
    let session_for_task = session.clone();

    let handle = tokio::spawn(async move {
        let mut stdout_lines = stdout.map(|s| BufReader::new(s).lines());
        let mut stderr_lines = stderr.map(|s| BufReader::new(s).lines());

        loop {
            let line_result = match stream_kind {
                MonitorStream::Stdout => match stdout_lines.as_mut() {
                    Some(reader) => reader.next_line().await,
                    None => break,
                },
                MonitorStream::Stderr => match stderr_lines.as_mut() {
                    Some(reader) => reader.next_line().await,
                    None => break,
                },
                MonitorStream::Both => {
                    tokio::select! {
                        line = async {
                            match stdout_lines.as_mut() {
                                Some(r) => r.next_line().await,
                                None => Ok(None),
                            }
                        } => line,
                        line = async {
                            match stderr_lines.as_mut() {
                                Some(r) => r.next_line().await,
                                None => Ok(None),
                            }
                        } => line,
                    }
                }
            };

            let line = match line_result {
                Ok(Some(line)) => line,
                Ok(None) => break,
                Err(err) => {
                    warn!(monitor_id = %id_for_task, error = %err, "monitor stream read error");
                    break;
                }
            };

            if !pattern.is_match(&line) {
                continue;
            }

            let notification = format!(
                "<task-notification>\n\
                 <monitor-id>{id_for_task}</monitor-id>\n\
                 <command>{cmd_display_for_task}</command>\n\
                 <matched-line>{line}</matched-line>\n\
                 </task-notification>"
            );
            let item = ResponseInputItem::Message {
                role: "user".to_string(),
                content: vec![ContentItem::InputText { text: notification }],
                phase: None,
            };
            if let Err(returned) = session_for_task
                .inject_response_items(vec![item])
                .await
            {
                session_for_task
                    .queue_response_items_for_next_turn(returned)
                    .await;
            }

            if !persistent {
                break;
            }
        }

        // Ensure the registry entry is dropped when the watcher exits naturally
        // (child process exited or stream closed) so `list`/`len` stay accurate.
        let mut guard = registry.inner.lock().await;
        guard.remove(&id_for_task);
        let _ = child.start_kill();
    });

    let entry = MonitorEntry {
        handle,
        command: options.command,
        pattern: options.pattern,
        persistent: options.persistent,
    };
    session
        .services
        .monitor_registry
        .inner
        .lock()
        .await
        .insert(monitor_id.clone(), entry);
    Ok(monitor_id)
}

#[derive(Debug, thiserror::Error)]
pub enum MonitorArmError {
    #[error("monitor command must have at least one element")]
    EmptyCommand,
    #[error("invalid regex pattern: {0}")]
    InvalidPattern(String),
    #[error("failed to spawn monitor subprocess: {0}")]
    SpawnFailed(String),
}
