use crate::ai::message::Message;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TitleSource {
    FirstMessage,
    Generated,
    User,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatMode {
    Ask,
    Agent,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatTarget {
    Editor,
    Notebook { notebook_id: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Conversation {
    pub schema_version: u32,
    pub id: String,
    pub connection_id: String,
    pub database: Option<String>,
    pub schema: Option<String>,
    pub title: String,
    pub title_source: TitleSource,
    pub mode: ChatMode,
    #[serde(default)]
    pub target: Option<ChatTarget>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub messages: Vec<Message>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversationSummary {
    pub id: String,
    pub connection_id: String,
    pub title: String,
    pub title_source: TitleSource,
    pub created_at: i64,
    pub updated_at: i64,
    pub message_count: usize,
    pub mode: ChatMode,
    pub target: Option<ChatTarget>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub database: Option<String>,
    pub schema: Option<String>,
}

impl From<&Conversation> for ConversationSummary {
    fn from(chat: &Conversation) -> Self {
        Self {
            id: chat.id.clone(),
            connection_id: chat.connection_id.clone(),
            title: chat.title.clone(),
            title_source: chat.title_source.clone(),
            created_at: chat.created_at,
            updated_at: chat.updated_at,
            message_count: chat.messages.len(),
            mode: chat.mode.clone(),
            target: chat.target.clone(),
            provider: chat.provider.clone(),
            model: chat.model.clone(),
            database: chat.database.clone(),
            schema: chat.schema.clone(),
        }
    }
}

#[derive(Serialize)]
pub struct SaveResult {
    pub bytes: usize,
    pub large: bool,
}
