//! tiny_http-based HTTP server: receives Responses-format POSTs from ecodex,
//! routes through CIF adapters to upstream chat-completions providers,
//! streams CIF events back as Responses-format SSE.

use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::Arc;
use tiny_http::{Request, Response, Server};
use tracing::{error, info, warn};

use crate::adapters::{chat, responses};

/// Configuration for the translator server.
#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// Upstream provider's chat-completions endpoint base URL.
    pub upstream_base_url: String,
    /// Optional bearer token forwarded as `Authorization: Bearer <token>`.
    pub upstream_api_key: Option<String>,
    /// Address to bind tiny_http on.
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

    if !(method == "POST" && (url == "/v1/responses" || url == "/responses")) {
        let response = Response::from_string("not found").with_status_code(404);
        return request.respond(response).context("respond 404");
    }

    let mut body = String::new();
    request
        .as_reader()
        .read_to_string(&mut body)
        .context("read request body")?;

    let body_value: serde_json::Value =
        serde_json::from_str(&body).context("parse Responses request body")?;
    let cif_request = responses::parse_request(&body_value)?;
    let chat_body = chat::encode_request(&cif_request);

    let upstream_url = format!(
        "{}/chat/completions",
        cfg.upstream_base_url.trim_end_matches('/')
    );
    let mut req_builder = reqwest::blocking::Client::new()
        .post(&upstream_url)
        .json(&chat_body);
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

    let mut writer = request.into_writer();
    writer
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nTransfer-Encoding: chunked\r\n\r\n")
        .context("write response headers")?;

    let mut state = chat::ChunkState::default();
    let reader = BufReader::new(upstream_resp);
    for line in reader.lines() {
        let line = line.context("read upstream line")?;
        let Some(data) = line.strip_prefix("data: ") else {
            continue;
        };
        for event in chat::parse_chunk(data, &mut state)? {
            if let Some(bytes) = responses::encode_event(&event) {
                write_chunked(&mut writer, &bytes)?;
            }
        }
    }
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

// Silence unused-import warning for Read (used implicitly via as_reader().read_to_string)
#[allow(dead_code)]
fn _force_read_in_scope() -> impl Read {
    std::io::empty()
}
