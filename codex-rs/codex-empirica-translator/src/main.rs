//! ecodex chat-completions ↔ Responses API translator (binary entrypoint).
//!
//! Usage:
//!   codex-empirica-translator \
//!     --upstream-base-url https://api.deepseek.com/v1 \
//!     --upstream-api-key-env DEEPSEEK_API_KEY \
//!     --bind 127.0.0.1:18080
//!
//! Then point ecodex's provider config at `http://127.0.0.1:18080/v1`.

use anyhow::{Context, Result};
use clap::Parser;
use codex_empirica_translator::{
    EventEmitter, JsonlFileEmitter, NoopEmitter, ServerConfig, Upstream, UpstreamProtocol,
    UpstreamRouter, run,
};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(
    version,
    about = "Translate Responses API ↔ {chat,anthropic} for ecodex (multi-upstream router)"
)]
struct Args {
    /// Path to a TOML file declaring multiple upstreams. When set, the
    /// translator routes per-request based on the incoming `model` field
    /// (first-match-wins glob against each upstream's `model_match`).
    /// Mutually exclusive with the single-upstream `--upstream-*` flags.
    /// See `docs/ecodex/integrations/translator-multiplex.md` for the
    /// schema and worked Kimi+DeepSeek example.
    #[arg(long, env = "ECODEX_TRANSLATOR_UPSTREAMS_CONFIG")]
    upstreams_config: Option<PathBuf>,

    /// Upstream provider's base URL (without the per-protocol path suffix).
    /// Examples:
    ///   chat     : https://api.deepseek.com/v1, http://localhost:11434/v1
    ///   anthropic: https://api.anthropic.com/v1, https://api.kimi.com/coding/v1
    /// Ignored when `--upstreams-config` is set.
    #[arg(long, env = "ECODEX_TRANSLATOR_UPSTREAM_BASE_URL")]
    upstream_base_url: Option<String>,

    /// Name of the env var holding the upstream provider's API key.
    /// Forwarded as `Authorization: Bearer` for chat protocol or
    /// `x-api-key` (+ `anthropic-version: 2023-06-01`) for anthropic protocol.
    /// Omit for providers that don't require auth (Ollama, LMStudio).
    /// Ignored when `--upstreams-config` is set.
    #[arg(long, env = "ECODEX_TRANSLATOR_UPSTREAM_API_KEY_ENV")]
    upstream_api_key_env: Option<String>,

    /// Wire format the upstream speaks. `chat` (default) talks OpenAI Chat
    /// Completions to `<base>/chat/completions`. `anthropic` talks Anthropic
    /// Messages API to `<base>/messages` — required for Kimi For Coding
    /// because the OpenAI endpoint enforces an X-Msh-Platform allowlist that
    /// blocks unregistered clients (see HKUDS/nanobot#354). Ignored when
    /// `--upstreams-config` is set.
    #[arg(
        long,
        env = "ECODEX_TRANSLATOR_UPSTREAM_PROTOCOL",
        default_value = "chat"
    )]
    upstream_protocol: String,

    /// Address to bind the translator server on.
    #[arg(
        long,
        env = "ECODEX_TRANSLATOR_BIND",
        default_value = "127.0.0.1:18080"
    )]
    bind: String,

    /// Event tap path. Translator appends one JSONL line per request lifecycle
    /// event (request_started, stream_event, request_completed,
    /// request_errored). Subscribers (Empirica MCP, Cockpit TUI, replay log)
    /// consume by tailing. Omit to disable the tap.
    #[arg(long, env = "ECODEX_TRANSLATOR_EVENT_LOG")]
    event_log: Option<PathBuf>,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    // Two construction paths:
    //   1. --upstreams-config <path>       → multi-upstream router from TOML
    //   2. --upstream-base-url ... (legacy) → single-upstream router with
    //                                         catch-all glob (backwards compat)
    let router = match &args.upstreams_config {
        Some(path) => UpstreamRouter::from_toml_file(path)?,
        None => {
            let base_url = args.upstream_base_url.clone().ok_or_else(|| {
                anyhow::anyhow!(
                    "either --upstreams-config <path> or --upstream-base-url <url> must be provided"
                )
            })?;
            let protocol = UpstreamProtocol::parse(&args.upstream_protocol)?;
            let api_key = match &args.upstream_api_key_env {
                Some(var) => Some(
                    std::env::var(var)
                        .with_context(|| format!("upstream API key env var '{var}' is unset"))?,
                ),
                None => None,
            };
            UpstreamRouter::new(vec![Upstream {
                name: "default".to_string(),
                model_match: "*".to_string(),
                base_url,
                protocol,
                api_key,
            }])
        }
    };

    let emitter: Arc<dyn EventEmitter> = match args.event_log {
        Some(path) => {
            let e = JsonlFileEmitter::new(path).context("initialising event log file")?;
            tracing::info!(event_log = %e.path().display(), "event tap enabled");
            Arc::new(e)
        }
        None => Arc::new(NoopEmitter),
    };

    run(ServerConfig {
        router,
        bind_addr: args.bind,
        emitter,
    })
}
