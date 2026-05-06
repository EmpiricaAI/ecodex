//! tiny_http-based HTTP server: receives Responses-format POSTs from ecodex,
//! routes through CIF adapters to upstream chat-completions providers,
//! streams CIF events back as Responses-format SSE.

use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Read, Write};
use std::str::FromStr;
use std::sync::Arc;
use tiny_http::{Request, Response, Server};
use tracing::{error, info, warn};

use crate::adapters::{anthropic, chat, responses};
use crate::tap::{build_request_started, new_request_id, EventEmitter, NoopEmitter, TapEvent};

/// Wire format the translator speaks to the upstream provider. The codex-side
/// (request in / response out) is always Responses API; only the provider-side
/// changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpstreamProtocol {
    /// OpenAI Chat Completions: POST `<base>/chat/completions`, Bearer auth.
    /// Default. Covers DeepSeek, Qwen, GLM, Ollama, LMStudio, vLLM, etc.
    Chat,
    /// Anthropic Messages API: POST `<base>/messages`, x-api-key auth +
    /// `anthropic-version` header. Covers Anthropic direct and providers that
    /// expose Anthropic-compat surfaces — notably Kimi For Coding, whose
    /// OpenAI endpoint enforces an X-Msh-Platform allowlist that the
    /// Anthropic endpoint does not.
    Anthropic,
}

impl UpstreamProtocol {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "chat" | "openai" | "chat-completions" => Ok(Self::Chat),
            "anthropic" | "messages" => Ok(Self::Anthropic),
            other => anyhow::bail!(
                "unknown upstream protocol `{other}` — expected `chat` or `anthropic`"
            ),
        }
    }
}

/// Configuration for the translator server.
#[derive(Clone)]
pub struct ServerConfig {
    /// Upstream provider's endpoint base URL. The protocol-specific suffix
    /// (`/chat/completions` or `/messages`) is appended at request time.
    pub upstream_base_url: String,
    /// Optional API key. Forwarded as `Authorization: Bearer` for Chat,
    /// `x-api-key` for Anthropic.
    pub upstream_api_key: Option<String>,
    /// Wire format the upstream speaks. Defaults to Chat for backward compat.
    pub upstream_protocol: UpstreamProtocol,
    /// Address to bind tiny_http on.
    pub bind_addr: String,
    /// Event tap. None disables emission (NoopEmitter).
    pub emitter: Arc<dyn EventEmitter>,
}

impl std::fmt::Debug for ServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerConfig")
            .field("upstream_base_url", &self.upstream_base_url)
            .field("upstream_api_key", &self.upstream_api_key.as_ref().map(|_| "***"))
            .field("upstream_protocol", &self.upstream_protocol)
            .field("bind_addr", &self.bind_addr)
            .finish()
    }
}

impl ServerConfig {
    /// Helper to build a config with the no-op emitter (for tests + simple use).
    pub fn new(upstream_base_url: String, upstream_api_key: Option<String>, bind_addr: String) -> Self {
        Self {
            upstream_base_url,
            upstream_api_key,
            upstream_protocol: UpstreamProtocol::Chat,
            bind_addr,
            emitter: Arc::new(NoopEmitter),
        }
    }
}

pub fn run(config: ServerConfig) -> Result<()> {
    let server = Server::http(&config.bind_addr)
        .map_err(|e| anyhow::anyhow!("failed to bind {}: {e}", config.bind_addr))?;
    info!(
        bind = %config.bind_addr,
        upstream = %config.upstream_base_url,
        "codex-empirica-translator listening"
    );

    let cfg = Arc::new(config);
    for request in server.incoming_requests() {
        let cfg = Arc::clone(&cfg);
        if let Err(e) = handle_request(request, cfg) {
            error!(error = %e, "request handler error");
        }
    }
    Ok(())
}

fn handle_request(mut request: Request, cfg: Arc<ServerConfig>) -> Result<()> {
    let url = request.url().to_string();
    let method = format!("{}", request.method());
    info!(%method, %url, "incoming request");

    // ecodex T81: /healthz probe surface for `empirica diagnose --frontend
    // ecodex` and other liveness checkers. Returns 200 with a tiny JSON
    // body identifying the upstream protocol so a probe can verify both
    // "translator alive" and "translator pointed at the right provider".
    if method == "GET" && url == "/healthz" {
        let body = serde_json::json!({
            "status": "ok",
            "upstream_protocol": format!("{:?}", cfg.upstream_protocol).to_lowercase(),
        })
        .to_string();
        let response = Response::from_string(body)
            .with_status_code(200)
            .with_header(
                tiny_http::Header::from_str("Content-Type: application/json").unwrap(),
            );
        return request.respond(response).context("respond /healthz");
    }

    if !(method == "POST" && (url == "/v1/responses" || url == "/responses")) {
        let response = Response::from_string("not found").with_status_code(404);
        return request.respond(response).context("respond 404");
    }

    let request_id = new_request_id();
    let started = std::time::Instant::now();
    let emitter = Arc::clone(&cfg.emitter);

    let mut body = String::new();
    request
        .as_reader()
        .read_to_string(&mut body)
        .context("read request body")?;

    let body_value: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            emitter.emit(&TapEvent::RequestErrored {
                request_id: request_id.clone(),
                stage: "parse_body".into(),
                message: e.to_string(),
                duration_ms: started.elapsed().as_millis() as u64,
            });
            return Err(e).context("parse Responses request body");
        }
    };
    let cif_request = responses::parse_request(&body_value)?;

    // Per-protocol request shape: URL suffix, auth header, body encoder, and
    // — separately, below — chunk-state + parser. Branching here keeps the
    // shared SSE plumbing (chunked HTTP write, CIF event re-encoding via
    // responses adapter) protocol-agnostic.
    let (upstream_url, protocol_label) = match cfg.upstream_protocol {
        UpstreamProtocol::Chat => (
            format!("{}/chat/completions", cfg.upstream_base_url.trim_end_matches('/')),
            "chat",
        ),
        UpstreamProtocol::Anthropic => (
            format!("{}/messages", cfg.upstream_base_url.trim_end_matches('/')),
            "anthropic",
        ),
    };
    emitter.emit(&build_request_started(
        &request_id,
        &cif_request,
        protocol_label,
        &upstream_url,
    ));

    let upstream_body = match cfg.upstream_protocol {
        UpstreamProtocol::Chat => chat::encode_request(&cif_request),
        UpstreamProtocol::Anthropic => anthropic::encode_request(&cif_request),
    };

    let mut req_builder = reqwest::blocking::Client::new()
        .post(&upstream_url)
        .json(&upstream_body);
    if let Some(key) = &cfg.upstream_api_key {
        req_builder = match cfg.upstream_protocol {
            UpstreamProtocol::Chat => req_builder.bearer_auth(key),
            UpstreamProtocol::Anthropic => req_builder
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01"),
        };
    }

    info!(upstream = %upstream_url, protocol = ?cfg.upstream_protocol, "forwarding to provider");
    // ecodex T81 Tx-S diagnostic: log the full upstream body so we can verify
    // that prior assistant tool_use blocks are being followed by user tool_result
    // blocks in the right shape. Symptom we're chasing: Kimi rejects with
    // "an assistant message with tool_calls must be followed by tool messages".
    // TODO: gate this behind a --debug-bodies flag once we've root-caused.
    // Log just the messages array tail (where tool_use/tool_result blocks
    // live) — the system prompts at the front bloat the body to >100KB and
    // we only need the conversation lifecycle to diagnose Tx-S.
    if let Some(messages) = upstream_body.get("messages").and_then(|v| v.as_array()) {
        let last_n = messages.iter().rev().take(8).rev().collect::<Vec<_>>();
        if let Ok(tail_str) = serde_json::to_string_pretty(&last_n) {
            info!(messages_tail = %tail_str, "upstream messages tail (Tx-S diagnostic)");
        }
    }
    let upstream_resp = req_builder.send().context("upstream request")?;

    if !upstream_resp.status().is_success() {
        let status = upstream_resp.status().as_u16();
        let err_body = upstream_resp.text().unwrap_or_default();
        warn!(status, body = %err_body, "upstream returned error");
        emitter.emit(&TapEvent::RequestErrored {
            request_id: request_id.clone(),
            stage: "upstream".into(),
            message: format!("status {status}: {err_body}"),
            duration_ms: started.elapsed().as_millis() as u64,
        });
        let response = Response::from_string(format!("upstream {status}: {err_body}"))
            .with_status_code(status);
        return request.respond(response).context("respond upstream error");
    }

    let mut writer = request.into_writer();
    writer
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nTransfer-Encoding: chunked\r\n\r\n")
        .context("write response headers")?;

    // Per-protocol SSE chunk state. Both adapters expose the same shape:
    // `parse_chunk(data: &str, state: &mut ChunkState) -> Result<Vec<StreamEvent>>`,
    // which keeps the inner per-line loop identical regardless of which
    // adapter is active.
    let mut chat_state = chat::ChunkState::default();
    let mut anthropic_state = anthropic::ChunkState::default();
    // Per-stream encoder state. Tracks whether we've opened an
    // assistant-message item via response.output_item.added so the matching
    // response.output_item.done emits at Completed. Without this state,
    // codex's response parser drops every text delta with the silent error
    // "OutputTextDelta without active item" because no item was ever opened.
    let mut response_encoder = responses::EncoderState::default();
    let mut text_chars: usize = 0;
    let mut tool_calls_count: usize = 0;
    let reader = BufReader::new(upstream_resp);
    for line in reader.lines() {
        let line = line.context("read upstream line")?;
        // Per W3C SSE spec, the space after `data:` is optional. Strip the
        // colon, then trim a single leading space if present. Kimi's
        // Anthropic-protocol endpoint emits `data:{...}` (no space); the
        // OpenAI / Anthropic-direct endpoints emit `data: {...}` (with
        // space). Both must parse.
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.strip_prefix(' ').unwrap_or(data);
        let events = match cfg.upstream_protocol {
            UpstreamProtocol::Chat => chat::parse_chunk(data, &mut chat_state)?,
            UpstreamProtocol::Anthropic => anthropic::parse_chunk(data, &mut anthropic_state)?,
        };
        for event in events {
            // Per-event metrics for completed-event payload
            if let crate::cif::StreamEvent::TextDelta { text } = &event {
                text_chars += text.chars().count();
            }
            if let crate::cif::StreamEvent::Completed { tool_calls, .. } = &event {
                tool_calls_count = tool_calls.len();
            }
            emitter.emit(&TapEvent::StreamEvent {
                request_id: request_id.clone(),
                event: event.clone(),
            });
            for bytes in responses::encode_events(&event, &mut response_encoder) {
                write_chunked(&mut writer, &bytes)?;
            }
        }
    }
    writer.write_all(b"0\r\n\r\n").context("write trailing chunk")?;
    writer.flush().context("flush response")?;

    emitter.emit(&TapEvent::RequestCompleted {
        request_id,
        duration_ms: started.elapsed().as_millis() as u64,
        text_chars,
        tool_calls_count,
    });

    Ok(())
}

fn write_chunked(writer: &mut dyn Write, bytes: &[u8]) -> Result<()> {
    write!(writer, "{:X}\r\n", bytes.len()).context("write chunk len")?;
    writer.write_all(bytes).context("write chunk body")?;
    writer.write_all(b"\r\n").context("write chunk crlf")?;
    Ok(())
}

// Silence unused-import warning for Read (used implicitly via as_reader().read_to_string)
#[allow(dead_code)]
fn _force_read_in_scope() -> impl Read {
    std::io::empty()
}
