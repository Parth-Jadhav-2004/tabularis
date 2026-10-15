use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub id: String,
    pub role: Role,
    #[serde(deserialize_with = "deserialize_blocks")]
    pub blocks: Vec<Block>,
    pub created_at: i64,
    #[serde(default)]
    pub usage: Option<Usage>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: Role::User,
            blocks: vec![Block::Text { text: text.into() }],
            created_at: chrono::Utc::now().timestamp_millis(),
            usage: None,
            provider: None,
            model: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Text {
        text: String,
    },
    Reasoning {
        text: String,
        signature: Option<String>,
    },
    ToolCall {
        id: String,
        name: String,
        args: Value,
        status: ToolStatus,
    },
    ToolResult {
        call_id: String,
        content: String,
        truncated: bool,
        is_error: bool,
    },
    Image {
        blob_id: String,
        mime: String,
        width: u32,
        height: u32,
    },
    Attachment {
        kind: AttachmentKind,
        name: String,
        blob_id: Option<String>,
        size: u64,
    },
    Context {
        items: Vec<ContextItemRef>,
    },
    Error {
        message: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Pending,
    Running,
    Completed,
    Rejected,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    Text,
    Csv,
    Sql,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextItemRef {
    pub kind: String,
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chars: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table_count: Option<usize>,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_tokens: u64,
    #[serde(default)]
    pub cache_write_tokens: u64,
}

fn deserialize_blocks<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Block>, D::Error> {
    let values = Vec::<Value>::deserialize(deserializer)?;
    let mut blocks = Vec::with_capacity(values.len());
    for value in values {
        match value.get("type").and_then(Value::as_str) {
            Some(
                "text" | "reasoning" | "tool_call" | "tool_result" | "image" | "attachment"
                | "context" | "error",
            ) => {
                blocks.push(serde_json::from_value(value).map_err(serde::de::Error::custom)?);
            }
            _ => log::warn!("Skipping an unknown AI message block type"),
        }
    }
    Ok(blocks)
}
