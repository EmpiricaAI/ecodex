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
use codex_empirica_translator::{run, EventEmitter, JsonlFileEmitter, NoopEmitter, ServerConfig};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(version, about = "Translate Responses API ↔ Chat Completions for ecodex")]
struct Args {
    /// Upstream provider's chat-completions base URL (e.g.
    /// https://api.deepseek.com/v1, https://api.moonshot.cn/v1,
    /// http://localhost:11434/v1).
    #[arg(long, env = "ECODEX_TRANSLATOR_UPSTREAM_BASE_URL")]
    upstream_base_url: String,

    /// Name of the env var holding the upstream provider's API key. The key
    /// is read at startup and forwarded as `Authorization: Bearer <key>`.
    /// Omit for providers that don't require auth (Ollama, LMStudio).
    #[arg(long, env = "ECODEX_TRANSLATOR_UPSTREAM_API_KEY_ENV")]
    upstream_api_key_env: Option<String>,

    /// Address to bind the translator server on.
    #[arg(long, env = "ECODEX_TRANSLATOR_BIND", default_value = "127.0.0.1:18080")]
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

    let upstream_api_key = match &args.upstream_api_key_env {
        Some(var) => Some(
            std::env::var(var)
                .with_context(|| format!("upstream API key env var '{var}' is unset"))?,
        ),
        None => None,
    };

    let emitter: Arc<dyn EventEmitter> = match args.event_log {
        Some(path) => {
            let e = JsonlFileEmitter::new(path)
                .context("initialising event log file")?;
            tracing::info!(event_log = %e.path().display(), "event tap enabled");
            Arc::new(e)
        }
        None => Arc::new(NoopEmitter),
    };

    run(ServerConfig {
        upstream_base_url: args.upstream_base_url,
        upstream_api_key,
        bind_addr: args.bind,
        emitter,
    })
}
