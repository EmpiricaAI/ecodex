//! Canonical Intermediate Format (CIF).
//!
//! Provider-neutral representation of a chat-style inference request and its
//! streaming response. All adapters translate to/from CIF; CIF never knows
//! about any specific provider's wire format.
//!
//! Design intent (per David's Phase 3a direction):
//!   responses ↔ CIF ↔ chat
//!   responses ↔ CIF ↔ anthropic
//!   responses ↔ CIF ↔ vllm
//!
//! O(N) adapters instead of O(N²) pairwise translators. Future Empirica
//! event subscribers consume CIF stream events, not provider-specific shapes.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A model inference request, provider-neutral.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub model: String,
    /// System / instructions block (separate from message turns).
    pub system: Option<String>,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub tools: Vec<Tool>,
    pub temperature: Option<f32>,
    pub max_output_tokens: Option<u32>,
    #[serde(default = "default_true")]
    pub stream: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Message {
    User {
        content: Vec<Content>,
    },
    Assistant {
        #[serde(default)]
        content: Vec<Content>,
        #[serde(default)]
        tool_calls: Vec<ToolCall>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
    /// Reasoning items (o1/o3-style chain-of-thought). Lossy across providers
    /// that don't support reasoning round-trip — adapters drop unless the
    /// target speaks reasoning.
    Reasoning {
        content: String,
        summary: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    Text { text: String },
    Image { url: String, mime: Option<String> },
    // Audio/video deferred to Phase 4+
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// JSON-encoded argument string (per OpenAI tool-call convention).
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: Option<String>,
    /// JSON Schema describing the tool's parameters.
    pub parameters: Value,
}

/// One streaming event in the response. Adapters parse provider chunks
/// into a sequence of these; downstream encoders re-emit them per provider's
/// streaming wire format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// New text fragment for the assistant's text output.
    TextDelta { text: String },
    /// New fragment of a tool-call. `index` identifies which tool-call this
    /// belongs to within the current turn (for multi-tool concurrent calls).
    ToolCallDelta {
        index: u32,
        id: Option<String>,
        name: Option<String>,
        arguments_delta: Option<String>,
    },
    /// Reasoning token fragment (o-series only).
    ReasoningDelta { text: String },
    /// Final event: stream is complete.
    Completed {
        /// Full assembled assistant text.
        text: String,
        /// Fully assembled tool calls (if any).
        #[serde(default)]
        tool_calls: Vec<ToolCall>,
        finish_reason: FinishReason,
        /// Provider-assigned response id, if it sent one.
        response_id: Option<String>,
    },
    /// Stream errored mid-flight.
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Other(String),
}

impl FinishReason {
    pub fn from_chat_str(s: &str) -> Self {
        match s {
            "stop" => Self::Stop,
            "length" => Self::Length,
            "tool_calls" | "function_call" => Self::ToolCalls,
            "content_filter" => Self::ContentFilter,
            other => Self::Other(other.to_string()),
        }
    }
}
