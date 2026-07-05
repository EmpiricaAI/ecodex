//! Native in-harness ntfy mesh wake-listener for ecodex.
//!
//! This holds an authenticated ntfy stream and turns each ECO-decided
//! proposal-event *doorbell* into a session wake, replacing the
//! `empirica loop listen` Python subprocess for the ecodex harness. Going
//! native drops the Python-version-drift coupling that can silently kill
//! the push path (the Python listener self-exits when its in-process
//! version differs from the installed dist).
//!
//! Per the mesh design agreed with the `cortex` + `empirica` peers
//! (2026-05-29): **the harness owns the wake-loop; Cortex owns the comms
//! tools (`cortex_*`) over MCP.** So this module is *transport only* — it
//! never calls the Cortex API natively. ntfy is the wake-ping ("doorbell");
//! the authoritative content fetch (`cortex_inbox_poll`) happens over MCP,
//! driven by the woken model.
//!
//! This file is the T1 scaffold: configuration + credential loading +
//! subscribe-URL construction + `ai_id` resolution. The held-connection
//! stream loop and session injection land in the follow-up task.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseInputItem;
use futures::StreamExt;
use serde::Deserialize;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::session::session::Session;

/// Resolved ntfy connection + auth configuration.
///
/// Mirrors the fields the empirica CLI listener resolves from
/// `~/.empirica/credentials.yaml`, with the same env-var override
/// precedence so a native ecodex listener and the reference Python
/// listener read identical credentials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NtfyListenerConfig {
    /// ntfy base URL, e.g. `https://ntfy.getempirica.com`.
    pub url: String,
    /// ntfy topic, e.g. `orchestration-events`.
    pub topic: String,
    /// ntfy access token (Bearer auth; `tk_`-prefixed in practice).
    pub token: Option<String>,
    /// Legacy basic-auth user (token is preferred).
    pub user: Option<String>,
    /// Legacy basic-auth password.
    pub password: Option<String>,
    /// This instance's bare `ai_id` (e.g. `ecodex-lab`). Used for wake-item
    /// hint text; NOT for the tag filter (see `ai_id_tag`).
    pub ai_id: String,
    /// The CANONICAL 3-form (`<org>.<tenant>.<project>`, e.g.
    /// `empirica.david.ecodex-lab`) used as the ntfy tag filter
    /// (`?tags=<canonical>`). Cortex tags every event with the canonical
    /// source_claude + target_claudes (strict-canonical, no bare-basename
    /// bridge since the 2026-06-03 rip) — a bare `?tags=<ai_id>` subscription
    /// silently misses. Resolved from `.empirica/project.yaml` `canonical_seat`.
    pub ai_id_tag: String,
}

/// Errors from resolving the listener configuration.
#[derive(Debug, thiserror::Error)]
pub(crate) enum NtfyConfigError {
    #[error("ntfy credentials file not found at {0}")]
    NotFound(PathBuf),
    #[error("failed to read ntfy credentials file {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse ntfy credentials file {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_yaml::Error,
    },
    #[error("ntfy config incomplete — missing: {0}")]
    Incomplete(String),
    #[error("could not resolve ai_id (no .empirica/project.yaml ai_id, no EMPIRICA_AI_ID, no project dir name)")]
    UnresolvedAiId,
}

/// On-disk shape of `~/.empirica/credentials.yaml` (only the `ntfy:` block
/// matters here; other keys like `cortex:` are ignored by serde).
#[derive(Debug, Default, Deserialize)]
struct CredentialsFile {
    ntfy: Option<NtfyCredsYaml>,
}

#[derive(Debug, Default, Deserialize)]
struct NtfyCredsYaml {
    url: Option<String>,
    topic: Option<String>,
    token: Option<String>,
    user: Option<String>,
    password: Option<String>,
}

/// Minimal projection of `.empirica/project.yaml` for `ai_id` + canonical
/// tag resolution.
#[derive(Debug, Default, Deserialize)]
struct ProjectFile {
    ai_id: Option<String>,
    /// The canonical 3-form seat, e.g. `empirica.david.ecodex-lab`. This is
    /// the authoritative ntfy tag (matches what cortex publishes).
    canonical_seat: Option<String>,
    /// `<org>.<tenant>` prefix, e.g. `empirica.david`. Fallback source for
    /// composing the canonical when `canonical_seat` is absent.
    mesh_id_prefix: Option<String>,
}

/// Default credentials path: `$HOME/.empirica/credentials.yaml`.
pub(crate) fn default_credentials_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(".empirica")
            .join("credentials.yaml")
    })
}

/// Resolve this instance's `ai_id`, in the same precedence the mesh skills
/// document: `.empirica/project.yaml` `ai_id:` → `EMPIRICA_AI_ID` env →
/// project directory basename with the `empirica-` prefix stripped.
pub(crate) fn resolve_ai_id(project_root: &Path) -> Result<String, NtfyConfigError> {
    // 1. .empirica/project.yaml ai_id field.
    let project_yaml = project_root.join(".empirica").join("project.yaml");
    if let Ok(text) = std::fs::read_to_string(&project_yaml)
        && let Ok(parsed) = serde_yaml::from_str::<ProjectFile>(&text)
        && let Some(ai_id) = parsed.ai_id.filter(|s| !s.trim().is_empty())
    {
        return Ok(ai_id.trim().to_string());
    }

    // 2. EMPIRICA_AI_ID env override.
    if let Some(env_id) = std::env::var("EMPIRICA_AI_ID")
        .ok()
        .filter(|s| !s.trim().is_empty())
    {
        return Ok(env_id.trim().to_string());
    }

    // 3. Project dir basename, strip `empirica-` prefix.
    if let Some(name) = project_root.file_name().and_then(|n| n.to_str()) {
        let stripped = name.strip_prefix("empirica-").unwrap_or(name);
        if !stripped.is_empty() {
            return Ok(stripped.to_string());
        }
    }

    Err(NtfyConfigError::UnresolvedAiId)
}

/// Resolve this instance's CANONICAL 3-form tag for ntfy subscription
/// (`<org>.<tenant>.<project>`). Precedence: `.empirica/project.yaml`
/// `canonical_seat` → compose `<mesh_id_prefix>.<ai_id>` if the prefix is
/// present → bare `ai_id` (best-effort; will silently miss cortex's
/// canonical-tagged events — the strict-canonical convention requires the
/// full 3-form here).
pub(crate) fn resolve_ai_id_tag(project_root: &Path, bare_ai_id: &str) -> String {
    let project_yaml = project_root.join(".empirica").join("project.yaml");
    if let Ok(text) = std::fs::read_to_string(&project_yaml)
        && let Ok(parsed) = serde_yaml::from_str::<ProjectFile>(&text)
    {
        if let Some(seat) = parsed.canonical_seat.filter(|s| !s.trim().is_empty()) {
            return seat.trim().to_string();
        }
        if let Some(prefix) = parsed.mesh_id_prefix.filter(|s| !s.trim().is_empty()) {
            return format!("{}.{bare_ai_id}", prefix.trim());
        }
    }
    bare_ai_id.to_string()
}

/// Load + validate the listener config from a credentials file, applying env
/// overrides. `ai_id` is resolved separately (see [`resolve_ai_id`]) and
/// passed in so config loading stays a pure function of its inputs.
pub(crate) fn load_config(
    credentials_path: &Path,
    ai_id: String,
) -> Result<NtfyListenerConfig, NtfyConfigError> {
    let creds = read_credentials(credentials_path)?;
    let ntfy = creds.ntfy.unwrap_or_default();

    // Env overrides mirror the empirica CLI resolver's precedence
    // (env wins over file) so both listeners read identical credentials.
    let url = env_first(&["ORCHESTRATION_NTFY_URL", "NTFY_URL"]).or(ntfy.url);
    let topic = env_first(&["ORCHESTRATION_NTFY_TOPIC"]).or(ntfy.topic);
    let token = env_first(&["ORCHESTRATION_NTFY_TOKEN", "NTFY_TOKEN"]).or(ntfy.token);
    let user = env_first(&["ORCHESTRATION_NTFY_USER"]).or(ntfy.user);
    let password = env_first(&["ORCHESTRATION_NTFY_PASS"]).or(ntfy.password);

    let mut missing = Vec::new();
    if url.as_deref().unwrap_or("").is_empty() {
        missing.push("url");
    }
    if topic.as_deref().unwrap_or("").is_empty() {
        missing.push("topic");
    }
    let has_token = token.as_deref().is_some_and(|t| !t.is_empty());
    let has_basic = user.as_deref().is_some_and(|u| !u.is_empty())
        && password.as_deref().is_some_and(|p| !p.is_empty());
    if !has_token && !has_basic {
        missing.push("token (or user+password)");
    }
    if !missing.is_empty() {
        return Err(NtfyConfigError::Incomplete(missing.join(", ")));
    }

    Ok(NtfyListenerConfig {
        url: url.unwrap_or_default(),
        topic: topic.unwrap_or_default(),
        token,
        user,
        // Defaults to the bare ai_id; `try_start_mesh_listener` overrides this
        // with the canonical 3-form (it has `project_root` to resolve it).
        ai_id_tag: ai_id.clone(),
        password,
        ai_id,
    })
}

fn read_credentials(path: &Path) -> Result<CredentialsFile, NtfyConfigError> {
    if !path.exists() {
        return Err(NtfyConfigError::NotFound(path.to_path_buf()));
    }
    let text = std::fs::read_to_string(path).map_err(|source| NtfyConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_yaml::from_str(&text).map_err(|source| NtfyConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

fn env_first(keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.trim().is_empty())
}

/// Build the ntfy JSON-stream subscribe URL with a tag filter scoped to this
/// instance's CANONICAL 3-form: `<url>/<topic>/json?tags=<canonical>`. Cortex
/// publishes events with `X-Tags: …,<source_claude>,<target_claudes…>` — all
/// canonical, strict-canonical-only (no bare-basename bridge since 2026-06-03).
/// A bare `?tags=<ai_id>` subscription silently misses every event, so the tag
/// MUST be the canonical 3-form (`ai_id_tag`). Dots are NOT percent-encoded
/// (`encode_query_value` treats `.` as safe) — ntfy matches the raw tag string.
pub(crate) fn build_subscribe_url(config: &NtfyListenerConfig) -> String {
    let base = format!(
        "{}/{}/json",
        config.url.trim_end_matches('/'),
        encode_path_segment(&config.topic),
    );
    format!("{base}?tags={}", encode_query_value(&config.ai_id_tag))
}

/// Resolve the ntfy auth header: Bearer token takes precedence over basic
/// auth (matches `_ntfy_auth_header` in the reference listener).
pub(crate) fn auth_header(config: &NtfyListenerConfig) -> Option<(String, String)> {
    if let Some(token) = config.token.as_deref().filter(|t| !t.is_empty()) {
        return Some(("Authorization".to_string(), format!("Bearer {token}")));
    }
    match (config.user.as_deref(), config.password.as_deref()) {
        (None, None) => None,
        (user, password) => {
            use base64::Engine as _;
            let raw = format!("{}:{}", user.unwrap_or(""), password.unwrap_or(""));
            let encoded = base64::engine::general_purpose::STANDARD.encode(raw.as_bytes());
            Some(("Authorization".to_string(), format!("Basic {encoded}")))
        }
    }
}

/// Percent-encode characters not allowed unescaped in a URL path segment.
/// Topics/ai_ids are slugs in practice, so this is a conservative pass that
/// leaves the common case untouched while staying correct for odd values.
fn encode_path_segment(s: &str) -> String {
    encode_with(s, |c| {
        c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~')
    })
}

fn encode_query_value(s: &str) -> String {
    encode_with(s, |c| {
        c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~')
    })
}

fn encode_with(s: &str, is_safe: impl Fn(char) -> bool) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        let c = b as char;
        if is_safe(c) {
            out.push(c);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Stream loop (T2) — held ntfy connection, doorbell → session wake, reconnect.
// ---------------------------------------------------------------------------

/// Reconnect backoff bounds. ntfy held-connections end periodically and on
/// transient network failures; we reconnect promptly, growing the delay only
/// while errors persist, and reset on a clean connection.
const RECONNECT_BASE: Duration = Duration::from_secs(2);
const RECONNECT_MAX: Duration = Duration::from_secs(60);

/// Holds the single background listener task for a session so the shutdown
/// path can abort it. Mirrors `MonitorRegistry` (one listener per session).
#[derive(Default)]
pub(crate) struct NtfyListenerRegistry {
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl NtfyListenerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Install the listener task, aborting any prior one.
    pub async fn set(&self, handle: JoinHandle<()>) {
        let mut guard = self.handle.lock().await;
        if let Some(old) = guard.take() {
            old.abort();
        }
        *guard = Some(handle);
    }

    /// Abort the listener task. Called on session shutdown.
    pub async fn abort(&self) {
        if let Some(handle) = self.handle.lock().await.take() {
            handle.abort();
        }
    }
}

/// Standard ntfy JSON-stream envelope (only the fields we use). ntfy emits
/// `open`/`keepalive`/`poll_request` housekeeping events too — only `message`
/// carries a real publication.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
struct NtfyEnvelope {
    event: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
enum NtfyStreamError {
    #[error("ntfy request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("ntfy returned non-success status {0}")]
    Status(u16),
}

/// Parse one ntfy JSON-stream line. Returns `None` for blank lines and any
/// line that isn't a well-formed ntfy envelope (defensive — never panics on
/// junk in the stream).
fn parse_ntfy_line(line: &str) -> Option<NtfyEnvelope> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    serde_json::from_str::<NtfyEnvelope>(line).ok()
}

/// Only `message` events are real publications worth a wake.
fn is_wake_event(env: &NtfyEnvelope) -> bool {
    env.event == "message"
}

/// Build the session wake for one ntfy doorbell. Per the agreed design the
/// ntfy event is a *doorbell*: this injects a poll-trigger wake so the model
/// fetches authoritative content via the `cortex_inbox_poll` MCP tool (Cortex
/// owns comms). Any message body / tags ntfy carried are surfaced as a hint.
fn build_wake_item(env: &NtfyEnvelope, ai_id: &str) -> ResponseInputItem {
    let ntfy_id = env.id.as_deref().unwrap_or("?");
    let tags = env.tags.join(",");
    let hint = match (env.title.as_deref(), env.message.as_deref()) {
        (Some(t), Some(m)) if !t.is_empty() || !m.is_empty() => format!(" ntfy hint: {t} — {m}"),
        (_, Some(m)) if !m.is_empty() => format!(" ntfy hint: {m}"),
        _ => String::new(),
    };
    let text = format!(
        "<task-notification>\n\
         <source>cortex-mesh-ntfy</source>\n\
         <ai-id>{ai_id}</ai-id>\n\
         <ntfy-id>{ntfy_id}</ntfy-id>\n\
         <tags>{tags}</tags>\n\
         <message>Mesh wake (ntfy doorbell): a mesh message is waiting for you. Your FIRST \
         action — before any orientation, PREFLIGHT summary, or asking what to work on — is to \
         check your inbox: run `empirica mailbox poll --ai-id {ai_id} --output json` (it defaults \
         to the accepted,changed actionable set). Then react per the mailbox protocol: act on \
         actionable items, treat FYI as read-on-poll, and ack completed work with `empirica \
         mailbox reply`.{hint}</message>\n\
         </task-notification>"
    );
    ResponseInputItem::Message {
        role: "user".to_string(),
        content: vec![ContentItem::InputText { text }],
        phase: None,
    }
}

/// One connection attempt: hold the ntfy stream and inject a wake per
/// `message` event until the stream ends or errors.
async fn run_stream_once(
    client: &reqwest::Client,
    url: &str,
    config: &NtfyListenerConfig,
    session: &Arc<Session>,
) -> Result<(), NtfyStreamError> {
    let mut req = client.get(url);
    if let Some((name, value)) = auth_header(config) {
        req = req.header(name, value);
    }
    let resp = req.send().await?;
    if !resp.status().is_success() {
        return Err(NtfyStreamError::Status(resp.status().as_u16()));
    }

    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        buf.extend_from_slice(&chunk);
        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            let text = String::from_utf8_lossy(&line);
            let Some(env) = parse_ntfy_line(&text) else {
                continue;
            };
            debug!(target: "ntfy_listener", event = %env.event, tags = ?env.tags, "ntfy stream line");
            if !is_wake_event(&env) {
                continue;
            }
            info!(target: "ntfy_listener", ntfy_id = ?env.id, tags = ?env.tags, "WAKE event received — injecting to start/queue a turn");
            let item = build_wake_item(&env, &config.ai_id);
            match session.inject_response_items(vec![item]).await {
                Ok(()) => info!(target: "ntfy_listener", "inject_response_items returned Ok (maybe_start_turn should fire if idle)"),
                Err(returned) => {
                    info!(target: "ntfy_listener", "inject_response_items returned Err — queuing for next turn");
                    session.queue_response_items_for_next_turn(returned).await;
                }
            }
        }
    }
    Ok(())
}

/// Spawn the held-connection listener loop. Runs until the task is aborted
/// (session shutdown). Reconnects with exponential backoff on error; resets to
/// the base delay after a clean stream end.
pub(crate) fn spawn_listener_loop(
    session: Arc<Session>,
    config: NtfyListenerConfig,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let url = build_subscribe_url(&config);
        let client = reqwest::Client::new();
        let mut backoff = RECONNECT_BASE;
        loop {
            match run_stream_once(&client, &url, &config, &session).await {
                Ok(()) => {
                    backoff = RECONNECT_BASE;
                }
                Err(err) => {
                    warn!(
                        target: "ntfy_listener",
                        error = %err,
                        backoff_secs = backoff.as_secs(),
                        "ntfy stream error; backing off before reconnect",
                    );
                }
            }
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(RECONNECT_MAX);
        }
    })
}

/// Best-effort mesh-listener startup, called at session boot. Resolves
/// `ai_id` + credentials; if the mesh isn't configured (no credentials file,
/// unresolved ai_id, incomplete creds) it logs at debug and returns without
/// error — mesh participation is opt-in by having credentials present. Never
/// blocks or fails session construction.
pub(crate) async fn try_start_mesh_listener(session: Arc<Session>, project_root: &Path) {
    // Never auto-open a network connection during the crate's own tests.
    if cfg!(test) {
        return;
    }
    let Some(creds_path) = default_credentials_path() else {
        debug!(target: "ntfy_listener", "no HOME; mesh listener not started");
        return;
    };
    if !creds_path.exists() {
        debug!(target: "ntfy_listener", "no ~/.empirica/credentials.yaml; mesh listener not started");
        return;
    }
    let ai_id = match resolve_ai_id(project_root) {
        Ok(id) => id,
        Err(err) => {
            debug!(target: "ntfy_listener", %err, "ai_id unresolved; mesh listener not started");
            return;
        }
    };
    let mut config = match load_config(&creds_path, ai_id) {
        Ok(config) => config,
        Err(err) => {
            debug!(target: "ntfy_listener", %err, "mesh creds incomplete; mesh listener not started");
            return;
        }
    };
    // Override the (bare) default tag with the canonical 3-form so the ntfy
    // `?tags=` subscription matches cortex's canonical event tags.
    config.ai_id_tag = resolve_ai_id_tag(project_root, &config.ai_id);
    let ai_id = config.ai_id.clone();
    let handle = spawn_listener_loop(session.clone(), config);
    session.services.ntfy_listener_registry.set(handle).await;
    info!(target: "ntfy_listener", %ai_id, "native ntfy mesh listener started");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> NtfyListenerConfig {
        NtfyListenerConfig {
            url: "https://ntfy.getempirica.com".to_string(),
            topic: "orchestration-events".to_string(),
            token: Some("tk_abc123".to_string()),
            user: None,
            password: None,
            ai_id: "ecodex".to_string(),
            ai_id_tag: "empirica.david.ecodex".to_string(),
        }
    }

    #[test]
    fn subscribe_url_has_tag_filter_and_json_endpoint() {
        assert_eq!(
            build_subscribe_url(&cfg()),
            "https://ntfy.getempirica.com/orchestration-events/json?tags=empirica.david.ecodex"
        );
    }

    #[test]
    fn subscribe_url_trims_trailing_slash_on_base() {
        let mut c = cfg();
        c.url = "https://ntfy.getempirica.com/".to_string();
        assert_eq!(
            build_subscribe_url(&c),
            "https://ntfy.getempirica.com/orchestration-events/json?tags=empirica.david.ecodex"
        );
    }

    #[test]
    fn auth_header_prefers_bearer_token() {
        assert_eq!(
            auth_header(&cfg()),
            Some(("Authorization".to_string(), "Bearer tk_abc123".to_string()))
        );
    }

    #[test]
    fn auth_header_falls_back_to_basic() {
        let mut c = cfg();
        c.token = None;
        c.user = Some("alice".to_string());
        c.password = Some("pw".to_string());
        let (name, value) = auth_header(&c).expect("basic auth header");
        assert_eq!(name, "Authorization");
        // base64("alice:pw") == "YWxpY2U6cHc="
        assert_eq!(value, "Basic YWxpY2U6cHc=");
    }

    #[test]
    fn auth_header_none_when_no_credentials() {
        let mut c = cfg();
        c.token = None;
        assert_eq!(auth_header(&c), None);
    }

    #[test]
    fn load_config_parses_ntfy_block_and_ignores_other_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.yaml");
        std::fs::write(
            &path,
            "version: 1\ncortex:\n  url: https://cortex.example\n  api_key: ctx_x\nntfy:\n  url: https://ntfy.getempirica.com\n  topic: orchestration-events\n  token: tk_xyz\n",
        )
        .unwrap();
        let c = load_config(&path, "ecodex".to_string()).expect("config");
        assert_eq!(c.url, "https://ntfy.getempirica.com");
        assert_eq!(c.topic, "orchestration-events");
        assert_eq!(c.token.as_deref(), Some("tk_xyz"));
        assert_eq!(c.ai_id, "ecodex");
    }

    #[test]
    fn load_config_errors_when_auth_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.yaml");
        std::fs::write(
            &path,
            "ntfy:\n  url: https://ntfy.getempirica.com\n  topic: orchestration-events\n",
        )
        .unwrap();
        let err = load_config(&path, "ecodex".to_string()).unwrap_err();
        assert!(matches!(err, NtfyConfigError::Incomplete(_)), "got {err:?}");
    }

    #[test]
    fn load_config_errors_when_file_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.yaml");
        assert!(matches!(
            load_config(&path, "ecodex".to_string()).unwrap_err(),
            NtfyConfigError::NotFound(_)
        ));
    }

    #[test]
    fn resolve_ai_id_reads_project_yaml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".empirica")).unwrap();
        std::fs::write(
            dir.path().join(".empirica").join("project.yaml"),
            "name: ecodex\nai_id: ecodex\n",
        )
        .unwrap();
        assert_eq!(resolve_ai_id(dir.path()).unwrap(), "ecodex");
    }

    #[test]
    fn resolve_ai_id_falls_back_to_basename_stripping_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("empirica-outreach");
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(resolve_ai_id(&root).unwrap(), "outreach");
    }

    #[test]
    fn parse_ntfy_line_accepts_message_envelope() {
        let line = r#"{"id":"abc","event":"message","topic":"orchestration-events","message":"hi","tags":["orchestration_event","ecodex"]}"#;
        let env = parse_ntfy_line(line).expect("parsed");
        assert_eq!(env.event, "message");
        assert_eq!(env.id.as_deref(), Some("abc"));
        assert_eq!(env.tags, vec!["orchestration_event", "ecodex"]);
        assert!(is_wake_event(&env));
    }

    #[test]
    fn parse_ntfy_line_rejects_blank_and_junk() {
        assert!(parse_ntfy_line("").is_none());
        assert!(parse_ntfy_line("   ").is_none());
        assert!(parse_ntfy_line("not json").is_none());
    }

    #[test]
    fn housekeeping_events_are_not_wakes() {
        for ev in ["open", "keepalive", "poll_request"] {
            let env = parse_ntfy_line(&format!(r#"{{"event":"{ev}"}}"#)).expect("parsed");
            assert!(!is_wake_event(&env), "{ev} should not wake");
        }
    }

    #[test]
    fn wake_item_is_user_role_and_directs_to_inbox_poll() {
        let env = NtfyEnvelope {
            event: "message".to_string(),
            id: Some("evt-1".to_string()),
            message: Some("Re: onboarding".to_string()),
            title: None,
            tags: vec!["ecodex".to_string()],
        };
        let item = build_wake_item(&env, "ecodex");
        match item {
            ResponseInputItem::Message { role, content, .. } => {
                assert_eq!(role, "user");
                let ContentItem::InputText { text } = &content[0] else {
                    panic!("expected input text");
                };
                // Wake instructs the CLI receive path (empirica mailbox poll), not the
                // MCP cortex_inbox_poll namespace call — a woken practitioner runs a shell
                // command reliably, sidestepping the mcp__cortex namespace aggregation.
                assert!(text.contains("empirica mailbox poll --ai-id ecodex"));
                assert!(text.contains("FIRST")); // poll-first imperative (before orienting)
                assert!(text.contains("evt-1"));
                assert!(text.contains("ntfy hint: Re: onboarding"));
                assert!(text.contains("read-on-poll"));
            }
            other => panic!("expected Message, got {other:?}"),
        }
    }
}
