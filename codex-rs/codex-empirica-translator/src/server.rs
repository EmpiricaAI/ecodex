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
use crate::tap::{EventEmitter, NoopEmitter, TapEvent, build_request_started, new_request_id};
use crate::upstreams::{Upstream, UpstreamRouter};

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
    /// Per-request upstream router. Resolves the incoming Responses
    /// request's `model` field to one of N configured upstreams. The
    /// single-upstream case from earlier versions is now a router with
    /// one catch-all entry — main.rs synthesizes that when the legacy
    /// `--upstream-base-url` flags are used instead of `--upstreams-config`.
    pub router: UpstreamRouter,
    /// Address to bind tiny_http on.
    pub bind_addr: String,
    /// Event tap. None disables emission (NoopEmitter).
    pub emitter: Arc<dyn EventEmitter>,
}

impl std::fmt::Debug for ServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerConfig")
            .field("router", &self.router)
            .field("bind_addr", &self.bind_addr)
            .finish()
    }
}

impl ServerConfig {
    /// Helper to build a single-upstream config with the no-op emitter
    /// (for tests + simple use). Keeps the pre-multiplex constructor
    /// shape callable by existing test fixtures.
    pub fn new(
        upstream_base_url: String,
        upstream_api_key: Option<String>,
        bind_addr: String,
    ) -> Self {
        let upstream = Upstream {
            name: "default".to_string(),
            model_match: "*".to_string(),
            base_url: upstream_base_url,
            protocol: UpstreamProtocol::Chat,
            api_key: upstream_api_key,
        };
        Self {
            router: UpstreamRouter::new(vec![upstream]),
            bind_addr,
            emitter: Arc::new(NoopEmitter),
        }
    }
}

pub fn run(config: ServerConfig) -> Result<()> {
    let server = Server::http(&config.bind_addr)
        .map_err(|e| anyhow::anyhow!("failed to bind {}: {e}", config.bind_addr))?;
    let upstream_names: Vec<&str> = config
        .router
        .upstreams()
        .iter()
        .map(|u| u.name.as_str())
        .collect();
    info!(
        bind = %config.bind_addr,
        upstreams = ?upstream_names,
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
    // body listing the configured upstreams so a probe can verify both
    // "translator alive" and "translator can route to expected provider".
    if method == "GET" && url == "/healthz" {
        let upstreams: Vec<serde_json::Value> = cfg
            .router
            .upstreams()
            .iter()
            .map(|u| {
                serde_json::json!({
                    "name": u.name,
                    "model_match": u.model_match,
                    "protocol": format!("{:?}", u.protocol).to_lowercase(),
                })
            })
            .collect();
        let body = serde_json::json!({
            "status": "ok",
            "upstreams": upstreams,
        })
        .to_string();
        let mut response = Response::from_string(body).with_status_code(200);
        if let Ok(content_type) = tiny_http::Header::from_str("Content-Type: application/json") {
            response = response.with_header(content_type);
        }
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
                request_id,
                stage: "parse_body".into(),
                message: e.to_string(),
                duration_ms: started.elapsed().as_millis() as u64,
            });
            return Err(e).context("parse Responses request body");
        }
    };

    // ─── Per-request upstream routing ───────────────────────────────
    // Read the incoming request's `model` field, route through
    // UpstreamRouter (first-match-wins glob). Single-upstream deploys
    // synthesize a router with one `model_match = "*"` entry — they
    // route uniformly, no per-request overhead.
    let model = body_value
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let upstream = match cfg.router.route(model) {
        Some(u) => u,
        None => {
            let upstream_names: Vec<&str> = cfg
                .router
                .upstreams()
                .iter()
                .map(|u| u.name.as_str())
                .collect();
            let msg = format!(
                "no upstream matches model `{model}` — configured upstreams: {upstream_names:?}"
            );
            warn!(model = %model, "{msg}");
            emitter.emit(&TapEvent::RequestErrored {
                request_id,
                stage: "route".into(),
                message: msg.clone(),
                duration_ms: started.elapsed().as_millis() as u64,
            });
            let response = Response::from_string(msg).with_status_code(400);
            return request
                .respond(response)
                .context("respond 400 unmatched model");
        }
    };

    let cif_request = responses::parse_request(&body_value)?;

    // Per-protocol request shape: URL suffix, auth header, body encoder, and
    // — separately, below — chunk-state + parser. Branching here keeps the
    // shared SSE plumbing (chunked HTTP write, CIF event re-encoding via
    // responses adapter) protocol-agnostic.
    let (upstream_url, protocol_label) = match upstream.protocol {
        UpstreamProtocol::Chat => (
            format!(
                "{}/chat/completions",
                upstream.base_url.trim_end_matches('/')
            ),
            "chat",
        ),
        UpstreamProtocol::Anthropic => (
            format!("{}/messages", upstream.base_url.trim_end_matches('/')),
            "anthropic",
        ),
    };
    emitter.emit(&build_request_started(
        &request_id,
        &cif_request,
        protocol_label,
        &upstream_url,
    ));

    let upstream_body = match upstream.protocol {
        UpstreamProtocol::Chat => chat::encode_request(&cif_request),
        UpstreamProtocol::Anthropic => anthropic::encode_request(&cif_request),
    };

    let mut req_builder = reqwest::blocking::Client::new()
        .post(&upstream_url)
        .json(&upstream_body);
    if let Some(key) = &upstream.api_key {
        req_builder = match upstream.protocol {
            UpstreamProtocol::Chat => req_builder.bearer_auth(key),
            UpstreamProtocol::Anthropic => req_builder
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01"),
        };
    }

    info!(
        upstream_name = %upstream.name,
        model = %model,
        url = %upstream_url,
        protocol = ?upstream.protocol,
        "forwarding to provider"
    );
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
            request_id,
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
        let events = match upstream.protocol {
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
    writer
        .write_all(b"0\r\n\r\n")
        .context("write trailing chunk")?;
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
