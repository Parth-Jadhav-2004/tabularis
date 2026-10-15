use super::message::{Message, Usage};
use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatRequest {
    pub provider: String,
    pub model: String,
    pub system: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub tools: Vec<ToolDef>,
    #[serde(default)]
    pub max_output_tokens: Option<u32>,
    #[serde(default)]
    pub reasoning: Option<ReasoningLevel>,
    /// Populated in Rust by the encrypted blob store, never accepted over IPC.
    #[serde(skip)]
    pub resolved_images: HashMap<String, ImageData>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Clone, Debug)]
pub struct ImageData {
    pub mime: String,
    pub base64: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningLevel {
    Low,
    Medium,
    High,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatEvent {
    TextDelta {
        text: String,
    },
    ReasoningDelta {
        text: String,
    },
    ReasoningSignature {
        signature: String,
    },
    ToolCallStart {
        id: String,
        name: String,
    },
    ToolCallArgsDelta {
        id: String,
        delta: String,
    },
    ToolCallEnd {
        id: String,
        args: Value,
    },
    Usage {
        usage: Usage,
    },
    Notice {
        message_id: Option<String>,
        reason: DropReason,
    },
    Done {
        finish: FinishReason,
    },
    Error {
        error: ProviderError,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DropReason {
    ImagesUnsupported,
    ImageUnavailable,
    ReasoningOmitted,
    ToolsFlattened,
    ToolsUnsupported,
    ReasoningUnsupported,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolUse,
    Cancelled,
    Error,
    ContentFilter,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Auth,
    RateLimit,
    ContextLength,
    Network,
    Other,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderError {
    pub kind: ErrorKind,
    pub message: String,
}

impl ProviderError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn protocol() -> Self {
        Self::new(
            ErrorKind::Other,
            "The AI provider returned an invalid or incomplete stream.",
        )
    }

    pub fn network() -> Self {
        Self::new(
            ErrorKind::Network,
            "Could not reach the AI provider. Check your connection and proxy settings.",
        )
    }

    /// Never surface raw upstream bodies: they may echo prompts or credentials.
    pub fn from_status(status: u16, body: &str) -> Self {
        let lower = body.to_ascii_lowercase();
        let kind = if status == 401 || status == 403 || lower.contains("authentication_error") {
            ErrorKind::Auth
        } else if status == 429
            || lower.contains("rate_limit")
            || lower.contains("overloaded_error")
        {
            ErrorKind::RateLimit
        } else if status == 413
            || lower.contains("context_length")
            || lower.contains("context length")
            || lower.contains("too many tokens")
            || lower.contains("prompt is too long")
        {
            ErrorKind::ContextLength
        } else {
            ErrorKind::Other
        };
        let message = match kind {
            ErrorKind::Auth => "The AI provider rejected the credentials. Check the API key in Settings.",
            ErrorKind::RateLimit => "The AI provider is busy or its rate limit was reached. Try again later.",
            ErrorKind::ContextLength => "This conversation exceeds the model's context limit. Use a larger model or reduce the context.",
            _ => "The AI provider rejected the request. Check the provider and model settings.",
        };
        Self::new(kind, message)
    }
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ProviderError {}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Capabilities {
    pub tools: bool,
    pub images: bool,
    pub reasoning: bool,
    pub context_window: Option<u32>,
}

#[async_trait]
pub trait ChatProvider: Send + Sync {
    async fn stream(
        &self,
        req: ChatRequest,
        cancel: CancellationToken,
    ) -> Result<BoxStream<'static, ChatEvent>, ProviderError>;
    fn capabilities(&self, model: &str) -> Capabilities;
}
