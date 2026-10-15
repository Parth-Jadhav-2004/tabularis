use crate::ai::message::{Block, Message, Role};
use crate::ai_chat::types::{ChatMode, Conversation, TitleSource};

pub fn conversation() -> Conversation {
    let mut assistant = Message::user("Use a grouped query.");
    assistant.role = Role::Assistant;
    assistant.provider = Some("openai".into());
    assistant.model = Some("reply-model".into());
    Conversation {
        schema_version: 1,
        id: "chat".into(),
        connection_id: "connection".into(),
        title: "Count users".into(),
        title_source: TitleSource::FirstMessage,
        database: Some("analytics".into()),
        schema: Some("public".into()),
        provider: Some("anthropic".into()),
        model: Some("next-model".into()),
        created_at: 0,
        updated_at: 0,
        mode: ChatMode::Ask,
        target: None,
        messages: vec![Message::user("Count users"), assistant],
    }
}

pub fn text(value: &str) -> Block {
    Block::Text { text: value.into() }
}
