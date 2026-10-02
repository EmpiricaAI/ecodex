//! Multi-upstream routing for the translator.
//!
//! A single translator process can serve multiple upstream providers,
//! selecting between them per-request based on the `model` field in the
//! incoming Responses request. Loaded from a TOML config:
//!
//! ```toml
//! [[upstream]]
//! name = "kimi"
//! model_match = "kimi-*"
//! base_url = "https://api.kimi.com/coding/v1"
//! protocol = "anthropic"
//! api_key_env = "KIMI_API_KEY"
//!
//! [[upstream]]
//! name = "deepseek"
//! model_match = "deepseek-*"
//! base_url = "https://api.deepseek.com/v1"
//! protocol = "chat"
//! api_key_env = "DEEPSEEK_API_KEY"
//! ```
//!
//! First-match-wins ordering — put more-specific patterns before any
//! catch-all (`model_match = "*"`).

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use crate::server::UpstreamProtocol;

/// One configured upstream. `model_match` selects; the rest describe
/// how to talk to it. `api_key` is resolved at config-load time from
/// `api_key_env` so per-request handling doesn't re-read environment.
#[derive(Clone, Debug)]
pub struct Upstream {
    pub name: String,
    pub model_match: String,
    pub base_url: String,
    pub protocol: UpstreamProtocol,
    pub api_key: Option<String>,
}

/// Ordered list of upstreams. Route per-request by first-match on the
/// incoming model name.
#[derive(Clone, Debug)]
pub struct UpstreamRouter {
    upstreams: Vec<Upstream>,
}

impl UpstreamRouter {
    pub fn new(upstreams: Vec<Upstream>) -> Self {
        Self { upstreams }
    }

    /// First-match-wins glob routing against the incoming request's
    /// `model` field. Returns None if nothing matches.
    pub fn route(&self, model: &str) -> Option<&Upstream> {
        self.upstreams
            .iter()
            .find(|u| glob_match(&u.model_match, model))
    }

    /// All registered upstreams — for /healthz inventory + diagnostics.
    pub fn upstreams(&self) -> &[Upstream] {
        &self.upstreams
    }

    /// Load + resolve a TOML config file. `api_key_env` is resolved from the
    /// current process environment at load time, falling back to `key_store`.
    ///
    /// A route whose key is in neither place is skipped with a warning rather
    /// than failing the whole file: the shipped routes list providers a user
    /// may have no key for, and those must not stop the routes they do have
    /// a key for. Loading fails only when no route is left.
    pub fn from_toml_file(path: &Path, key_store: &KeyStore) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("read upstreams config at {}", path.display()))?;
        let parsed: UpstreamsConfig =
            toml::from_str(&raw).with_context(|| format!("parse TOML at {}", path.display()))?;

        if parsed.upstream.is_empty() {
            anyhow::bail!(
                "upstreams config at {} has no [[upstream]] entries",
                path.display()
            );
        }

        let mut upstreams = Vec::new();
        let mut keyless = Vec::new();
        for raw in parsed.upstream {
            let name = raw.name.clone();
            let var = raw.api_key_env.clone().unwrap_or_default();
            match raw.resolve(key_store)? {
                Some(upstream) => upstreams.push(upstream),
                None => {
                    tracing::warn!(
                        upstream = %name,
                        api_key_env = %var,
                        "route skipped: no key in the environment or the empirica key store"
                    );
                    keyless.push(format!("`{name}` ({var})"));
                }
            }
        }

        if upstreams.is_empty() {
            anyhow::bail!(
                "no route in {} has a key: {}. Export the variable, or add <provider>.api_key to ~/.empirica/credentials.yaml (MISTRAL_API_KEY reads mistral.api_key)",
                path.display(),
                keyless.join(", ")
            );
        }
        Ok(Self::new(upstreams))
    }
}

/// Provider API keys from the empirica key store, `~/.empirica/credentials.yaml`,
/// where each provider is a section such as `mistral: {api_key: ...}`. An
/// upstream whose `api_key_env` is unset takes the key of the provider the
/// variable names: `MISTRAL_API_KEY` reads `mistral.api_key`.
#[derive(Debug, Default)]
pub struct KeyStore {
    keys: HashMap<String, String>,
}

impl KeyStore {
    /// Reads the key store at `path`. A missing file is an empty store.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(raw) => Self::from_yaml(&raw)
                .with_context(|| format!("parse key store at {}", path.display())),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err).with_context(|| format!("read key store at {}", path.display())),
        }
    }

    fn from_yaml(raw: &str) -> Result<Self> {
        let sections: Option<HashMap<String, serde_yaml::Value>> = serde_yaml::from_str(raw)?;
        let keys = sections
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(provider, section)| {
                let key = section.get("api_key")?.as_str()?.trim();
                (!key.is_empty()).then(|| (provider.to_ascii_lowercase(), key.to_string()))
            })
            .collect();
        Ok(Self { keys })
    }

    fn key_for_env(&self, var: &str) -> Option<String> {
        let provider = var.strip_suffix("_API_KEY")?.to_ascii_lowercase();
        self.keys.get(&provider).cloned()
    }
}

#[derive(Debug, Deserialize)]
struct UpstreamsConfig {
    #[serde(default)]
    upstream: Vec<RawUpstream>,
}

#[derive(Debug, Deserialize)]
struct RawUpstream {
    name: String,
    model_match: String,
    base_url: String,
    protocol: String,
    #[serde(default)]
    api_key_env: Option<String>,
}

impl RawUpstream {
    /// The resolved route, or `None` when it names an `api_key_env` that has
    /// no key in the environment or the key store. A bad protocol is an error.
    fn resolve(self, key_store: &KeyStore) -> Result<Option<Upstream>> {
        let protocol = UpstreamProtocol::parse(&self.protocol)
            .with_context(|| format!("upstream `{}` protocol", self.name))?;
        let api_key = match &self.api_key_env {
            Some(var) => {
                let Some(key) = std::env::var(var)
                    .ok()
                    .or_else(|| key_store.key_for_env(var))
                else {
                    return Ok(None);
                };
                Some(key)
            }
            None => None,
        };
        Ok(Some(Upstream {
            name: self.name,
            model_match: self.model_match,
            base_url: self.base_url,
            protocol,
            api_key,
        }))
    }
}

/// Tiny glob matcher: `*` matches anything, trailing `*` is prefix
/// match, anything else is exact. Sufficient for model names like
/// `kimi-for-coding`, `kimi-*`, `deepseek-chat-v3.1`. No regex/glob
/// dep needed — model names don't warrant the surface.
fn glob_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return value.starts_with(prefix);
    }
    pattern == value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_matches_exact() {
        assert!(glob_match("kimi-for-coding", "kimi-for-coding"));
        assert!(!glob_match("kimi-for-coding", "deepseek-chat"));
    }

    #[test]
    fn glob_matches_prefix_wildcard() {
        assert!(glob_match("kimi-*", "kimi-for-coding"));
        assert!(glob_match("kimi-*", "kimi-anything"));
        assert!(!glob_match("kimi-*", "deepseek-chat"));
        assert!(!glob_match("kimi-*", "ki"));
    }

    #[test]
    fn glob_matches_full_wildcard() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("*", ""));
    }

    #[test]
    fn router_first_match_wins() {
        let router = UpstreamRouter::new(vec![
            Upstream {
                name: "kimi".into(),
                model_match: "kimi-*".into(),
                base_url: "https://api.kimi.com/coding/v1".into(),
                protocol: UpstreamProtocol::Anthropic,
                api_key: None,
            },
            Upstream {
                name: "deepseek".into(),
                model_match: "deepseek-*".into(),
                base_url: "https://api.deepseek.com/v1".into(),
                protocol: UpstreamProtocol::Chat,
                api_key: None,
            },
        ]);

        assert_eq!(router.route("kimi-for-coding").unwrap().name, "kimi");
        assert_eq!(router.route("deepseek-chat").unwrap().name, "deepseek");
        assert!(router.route("openai/gpt-5").is_none());
    }

    #[test]
    fn router_catchall_last() {
        let router = UpstreamRouter::new(vec![
            Upstream {
                name: "kimi".into(),
                model_match: "kimi-*".into(),
                base_url: "https://api.kimi.com/coding/v1".into(),
                protocol: UpstreamProtocol::Anthropic,
                api_key: None,
            },
            Upstream {
                name: "openrouter".into(),
                model_match: "*".into(),
                base_url: "https://openrouter.ai/api/v1".into(),
                protocol: UpstreamProtocol::Chat,
                api_key: None,
            },
        ]);

        assert_eq!(router.route("kimi-for-coding").unwrap().name, "kimi");
        assert_eq!(router.route("openai/gpt-5").unwrap().name, "openrouter");
        assert_eq!(router.route("deepseek-chat").unwrap().name, "openrouter");
    }

    #[test]
    fn from_toml_parses_minimum_config() {
        // Use a temp file via std::env::temp_dir to avoid extra deps.
        let path = std::env::temp_dir().join(format!(
            "translator-upstreams-test-{}.toml",
            std::process::id()
        ));
        std::fs::write(
            &path,
            r#"
[[upstream]]
name = "kimi"
model_match = "kimi-*"
base_url = "https://api.kimi.com/coding/v1"
protocol = "anthropic"
# no api_key_env → api_key=None
"#,
        )
        .unwrap();

        let router = UpstreamRouter::from_toml_file(&path, &KeyStore::default()).expect("parse");
        let _ = std::fs::remove_file(&path);

        assert_eq!(router.upstreams().len(), 1);
        let u = &router.upstreams()[0];
        assert_eq!(u.name, "kimi");
        assert_eq!(u.protocol, UpstreamProtocol::Anthropic);
        assert!(u.api_key.is_none());
    }

    #[test]
    fn unset_key_env_falls_back_to_the_key_store() {
        let key_store = KeyStore::from_yaml(
            "mistral:\n  api_key: sk-from-store\necodextest:\n  api_key: sk-test\ncortex:\n  url: https://example.test\n",
        )
        .expect("key store");
        let resolve = |api_key_env: &str| {
            RawUpstream {
                name: "mistral".to_string(),
                model_match: "devstral-*".to_string(),
                base_url: "https://api.mistral.ai/v1".to_string(),
                protocol: "chat".to_string(),
                api_key_env: Some(api_key_env.to_string()),
            }
            .resolve(&key_store)
            .map(|upstream| upstream.map(|upstream| upstream.api_key))
            .map_err(|err| err.to_string())
        };

        assert_eq!(
            [
                resolve("ECODEXTEST_API_KEY"),
                resolve("ECODEX_TEST_UNSET_MISTRAL_API_KEY"),
                resolve("MISTRAL_API_KEY_ECODEX_TEST_UNSET"),
            ],
            [Ok(Some(Some("sk-test".to_string()))), Ok(None), Ok(None)]
        );
        assert_eq!(
            key_store.key_for_env("MISTRAL_API_KEY"),
            Some("sk-from-store".to_string())
        );
        assert_eq!(key_store.key_for_env("CORTEX_API_KEY"), None);
    }

    fn write_routes(contents: &str) -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "translator-upstreams-test-{}-{}.toml",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::write(&path, contents).expect("write routes");
        path
    }

    #[test]
    fn routes_without_a_key_are_skipped_not_fatal() {
        let path = write_routes(
            r#"
[[upstream]]
name = "deepseek"
model_match = "deepseek-*"
base_url = "https://api.deepseek.com/v1"
protocol = "chat"
api_key_env = "ECODEX_TEST_UNSET_DEEPSEEK_API_KEY"

[[upstream]]
name = "local"
model_match = "local-*"
base_url = "http://localhost:11434/v1"
protocol = "chat"
"#,
        );
        let router = UpstreamRouter::from_toml_file(&path, &KeyStore::default()).expect("load");
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            router
                .upstreams()
                .iter()
                .map(|u| u.name.as_str())
                .collect::<Vec<_>>(),
            vec!["local"]
        );
        assert!(router.route("deepseek-chat").is_none());
    }

    #[test]
    fn a_routes_file_with_no_usable_route_is_an_error() {
        let path = write_routes(
            r#"
[[upstream]]
name = "deepseek"
model_match = "deepseek-*"
base_url = "https://api.deepseek.com/v1"
protocol = "chat"
api_key_env = "ECODEX_TEST_UNSET_DEEPSEEK_API_KEY"
"#,
        );
        let err = UpstreamRouter::from_toml_file(&path, &KeyStore::default())
            .expect_err("no usable route");
        let _ = std::fs::remove_file(&path);

        assert!(
            err.to_string()
                .contains("has a key: `deepseek` (ECODEX_TEST_UNSET_DEEPSEEK_API_KEY)"),
            "{err}"
        );
    }
}
