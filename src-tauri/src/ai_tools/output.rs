//! MCP tool-output negotiation and serialization.
//!
//! MCP itself continues to use JSON-RPC. This module only controls the text
//! carried by a successful tool result's `content` item.

use crate::config::{AppConfig, DEFAULT_MCP_OUTPUT_FORMAT};
use crate::mcp::protocol::JsonRpcError;
use serde::Serialize;
use serde_json::{json, Map, Value};

const OUTPUT_FORMAT_ARGUMENT: &str = "output_format";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ToolOutputFormat {
    #[default]
    Json,
    Toon,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum OutputFormatError {
    InvalidArgument,
    Encoding(String),
}

impl ToolOutputFormat {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Toon => "toon",
        }
    }

    pub(crate) fn from_config(config: &AppConfig) -> Self {
        match config
            .mcp_output_format
            .as_deref()
            .unwrap_or(DEFAULT_MCP_OUTPUT_FORMAT)
        {
            "toon" => Self::Toon,
            _ => Self::Json,
        }
    }

    pub(crate) fn from_arguments(
        arguments: Option<&Map<String, Value>>,
        default: Self,
    ) -> Result<Self, OutputFormatError> {
        let Some(value) = arguments.and_then(|args| args.get(OUTPUT_FORMAT_ARGUMENT)) else {
            return Ok(default);
        };

        match value.as_str() {
            Some("json") => Ok(Self::Json),
            Some("toon") => Ok(Self::Toon),
            _ => Err(OutputFormatError::InvalidArgument),
        }
    }

    pub(crate) fn encode<T: Serialize>(self, value: &T) -> Result<String, OutputFormatError> {
        match self {
            // Keep the existing default representation byte-for-byte compatible.
            Self::Json => serde_json::to_string_pretty(value)
                .map_err(|error| OutputFormatError::Encoding(error.to_string())),
            Self::Toon => toon_format::encode_default(value)
                .map_err(|error| OutputFormatError::Encoding(error.to_string())),
        }
    }
}

pub(crate) fn require_args(
    args: Option<&serde_json::Map<String, Value>>,
) -> Result<&serde_json::Map<String, Value>, JsonRpcError> {
    args.ok_or(JsonRpcError {
        code: -32602,
        message: "Missing arguments".to_string(),
        data: None,
    })
}

pub(crate) fn output_format_schema(default: &str) -> Value {
    json!({
        "type": "string",
        "enum": ["json", "toon"],
        "default": default,
        "description": "Text output encoding. JSON is the backward-compatible default; TOON is optimized for LLM token usage."
    })
}

pub(crate) fn requested_output_format(
    args: Option<&serde_json::Map<String, Value>>,
    config: &AppConfig,
) -> Result<ToolOutputFormat, JsonRpcError> {
    ToolOutputFormat::from_arguments(args, ToolOutputFormat::from_config(config)).map_err(|_| {
        JsonRpcError {
            code: -32602,
            message: "Invalid output_format: expected 'json' or 'toon'".to_string(),
            data: None,
        }
    })
}

pub(crate) fn tool_result<T: Serialize>(
    value: &T,
    format: ToolOutputFormat,
) -> Result<Value, JsonRpcError> {
    let text = format.encode(value).map_err(|error| JsonRpcError {
        code: -32603,
        message: match error {
            OutputFormatError::Encoding(message) => {
                format!("Failed to encode MCP tool output: {message}")
            }
            OutputFormatError::InvalidArgument => "Failed to encode MCP tool output".to_string(),
        },
        data: None,
    })?;

    Ok(json!({
        "content": [{
            "type": "text",
            "text": text
        }]
    }))
}
