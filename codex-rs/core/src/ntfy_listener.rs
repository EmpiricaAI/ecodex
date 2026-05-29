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

use serde::Deserialize;

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
    /// This instance's `ai_id`, used as the ntfy tag filter so only events
    /// touching this instance are delivered (`?tags=<ai_id>`).
    pub ai_id: String,
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

/// Minimal projection of `.empirica/project.yaml` for `ai_id` resolution.
#[derive(Debug, Default, Deserialize)]
struct ProjectFile {
    ai_id: Option<String>,
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
/// instance's `ai_id`. Shape matches the reference listener:
/// `<url>/<topic>/json?tags=<ai_id>`. Cortex publishes events with
/// `X-Tags: …,<source_claude>,<target_claudes…>`, so the tag filter shrinks
/// per-event wake traffic to only events touching this instance.
pub(crate) fn build_subscribe_url(config: &NtfyListenerConfig) -> String {
    let base = format!(
        "{}/{}/json",
        config.url.trim_end_matches('/'),
        encode_path_segment(&config.topic),
    );
    format!("{base}?tags={}", encode_query_value(&config.ai_id))
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
        }
    }

    #[test]
    fn subscribe_url_has_tag_filter_and_json_endpoint() {
        assert_eq!(
            build_subscribe_url(&cfg()),
            "https://ntfy.getempirica.com/orchestration-events/json?tags=ecodex"
        );
    }

    #[test]
    fn subscribe_url_trims_trailing_slash_on_base() {
        let mut c = cfg();
        c.url = "https://ntfy.getempirica.com/".to_string();
        assert_eq!(
            build_subscribe_url(&c),
            "https://ntfy.getempirica.com/orchestration-events/json?tags=ecodex"
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
}
