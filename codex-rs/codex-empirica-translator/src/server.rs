//! tiny_http-based HTTP server: receives Responses-format POSTs from ecodex,
//! translates to chat-completions, forwards to an upstream provider, streams
//! the chat-completions SSE back as Responses-format SSE.

use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Read, Write};

#[allow(unused_imports)]
use Read as _Read;
use std::sync::Arc;
use tiny_http::{Header, Request, Response, Server};
use tracing::{error, info, warn};

use crate::translate::{chat_chunk_to_responses_sse, responses_to_chat_request, StreamState};

/// Configuration for the translator server.
#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// Upstream provider's chat-completions endpoint base URL (e.g.
    /// `https://api.deepseek.com/v1`). The translator appends `/chat/completions`.
    pub upstream_base_url: String,
    /// Optional bearer token forwarded as `Authorization: Bearer <token>`
    /// to the upstream provider. If `None`, no auth header is added (useful
    /// for local providers like Ollama / LMStudio).
    pub upstream_api_key: Option<String>,
    /// Address to bind tiny_http on (e.g. `127.0.0.1:18080`).
    pub bind_addr: String,
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
        // Single-threaded handling for v1 — concurrent translation is Phase 3.
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

    // Route: POST /v1/responses (the only thing codex hits for inference)
    if !(method == "POST" && (url == "/v1/responses" || url == "/responses")) {
        let response = Response::from_string("not found").with_status_code(404);
        return request.respond(response).context("respond 404");
    }

    let mut body = String::new();
    request
        .as_reader()
        .read_to_string(&mut body)
        .context("read request body")?;

    let responses_req: serde_json::Value =
        serde_json::from_str(&body).context("parse Responses request body")?;
    let chat_req = responses_to_chat_request(&responses_req)?;

    let upstream_url = format!("{}/chat/completions", cfg.upstream_base_url.trim_end_matches('/'));
    let mut req_builder = reqwest::blocking::Client::new()
        .post(&upstream_url)
        .json(&chat_req);
    if let Some(key) = &cfg.upstream_api_key {
        req_builder = req_builder.bearer_auth(key);
    }

    info!(upstream = %upstream_url, "forwarding to provider");
    let upstream_resp = req_builder.send().context("upstream request")?;

    if !upstream_resp.status().is_success() {
        let status = upstream_resp.status().as_u16();
        let err_body = upstream_resp.text().unwrap_or_default();
        warn!(status, body = %err_body, "upstream returned error");
        let response = Response::from_string(format!("upstream {status}: {err_body}"))
            .with_status_code(status);
        return request.respond(response).context("respond upstream error");
    }

    // Stream the upstream SSE through the translator into our response.
    let response = Response::empty(200)
        .with_header(
            "content-type: text/event-stream"
                .parse::<Header>()
                .unwrap(),
        )
        .with_header("cache-control: no-cache".parse::<Header>().unwrap())
        .with_chunked_threshold(0); // force chunked encoding for streaming

    let mut writer = request.into_writer();

    // Tiny_http expects us to write the HTTP response ourselves when streaming.
    // Easier: write status + headers manually.
    writer
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nTransfer-Encoding: chunked\r\n\r\n")
        .context("write response headers")?;
    drop(response); // not actually using the Response object — manual streaming

    let mut state = StreamState::default();
    let reader = BufReader::new(upstream_resp);
    for line in reader.lines() {
        let line = line.context("read upstream line")?;
        // Chat-completions SSE: lines starting with `data: ` carry payloads.
        let Some(data) = line.strip_prefix("data: ") else {
            continue;
        };
        let translated = chat_chunk_to_responses_sse(data, &mut state)?;
        if translated.is_empty() {
            continue;
        }
        // Write as a chunked-encoding chunk: `<len-hex>\r\n<bytes>\r\n`
        write_chunked(&mut writer, &translated)?;
    }
    // Final 0-length chunk to terminate
    writer.write_all(b"0\r\n\r\n").context("write trailing chunk")?;
    writer.flush().context("flush response")?;
    Ok(())
}

fn write_chunked(writer: &mut dyn Write, bytes: &[u8]) -> Result<()> {
    write!(writer, "{:X}\r\n", bytes.len()).context("write chunk len")?;
    writer.write_all(bytes).context("write chunk body")?;
    writer.write_all(b"\r\n").context("write chunk crlf")?;
    Ok(())
}
