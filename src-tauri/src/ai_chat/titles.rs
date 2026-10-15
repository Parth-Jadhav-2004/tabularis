use super::{
    run::RunManager,
    store::ConversationStore,
    types::{Conversation, ConversationSummary, TitleSource},
};
use crate::ai::{
    message::{Block, Message, Role},
    provider::{ChatEvent, ChatProvider, ChatRequest, ErrorKind, FinishReason, ProviderError},
};
use futures::{stream::BoxStream, StreamExt};
use std::collections::HashMap;
use tauri::{AppHandle, State};

fn title_request(chat: &Conversation, system: String) -> Option<ChatRequest> {
    if chat.title_source != TitleSource::FirstMessage {
        return None;
    }
    let pair = chat.messages.get(..2)?;
    if pair[0].role != Role::User || pair[1].role != Role::Assistant {
        return None;
    }
    let text = |message: &Message| {
        message
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
            .chars()
            .take(4000)
            .collect::<String>()
    };
    let user = text(&pair[0]);
    let assistant = text(&pair[1]);
    if user.trim().is_empty() || assistant.trim().is_empty() {
        return None;
    }
    Some(ChatRequest {
        provider: pair[1].provider.clone().or_else(|| chat.provider.clone())?,
        model: pair[1].model.clone().or_else(|| chat.model.clone())?,
        system,
        messages: vec![Message::user(
            serde_json::json!({
                "untrusted_conversation_excerpt": { "user": user, "assistant": assistant }
            })
            .to_string(),
        )],
        tools: Vec::new(),
        max_output_tokens: Some(128),
        reasoning: None,
        resolved_images: HashMap::new(),
    })
}

fn clean_title(text: &str) -> Option<String> {
    let title = text
        .trim()
        .trim_matches(['"', '\'', '`', '“', '”'])
        .split_whitespace()
        .take(6)
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(80)
        .collect::<String>();
    (!title.is_empty() && !title.starts_with('#') && !title.contains(['<', '>'])).then_some(title)
}

async fn collect_title(mut stream: BoxStream<'static, ChatEvent>) -> Result<String, ProviderError> {
    let mut text = String::new();
    while let Some(event) = stream.next().await {
        match event {
            ChatEvent::Done {
                finish: FinishReason::Stop | FinishReason::Length,
            } => {
                return clean_title(&text).ok_or_else(ProviderError::protocol);
            }
            ChatEvent::TextDelta { text: delta } if text.len() + delta.len() <= 4096 => {
                text.push_str(&delta)
            }
            ChatEvent::TextDelta { .. } | ChatEvent::Done { .. } => {
                return Err(ProviderError::protocol())
            }
            ChatEvent::Error { error } => return Err(error),
            _ => {}
        }
    }
    Err(ProviderError::protocol())
}

#[tauri::command]
pub async fn ai_chat_generate_title(
    app: AppHandle,
    manager: State<'_, RunManager>,
    connection_id: String,
    id: String,
) -> Result<Option<ConversationSummary>, ProviderError> {
    if crate::config::load_config_internal(&app).ai_enabled != Some(true) {
        return Err(ProviderError::new(
            ErrorKind::Other,
            "AI is disabled in Settings.",
        ));
    }
    let guard = manager.start(None)?;
    let snapshot = tokio::task::spawn_blocking(move || {
        ConversationStore::new(&crate::paths::get_app_config_dir()).load(&connection_id, &id)
    })
    .await
    .map_err(|_| ProviderError::protocol())?
    .map_err(|_| ProviderError::protocol())?;
    let Some(mut req) = title_request(&snapshot, crate::config::get_chattitle_prompt(app.clone()))
    else {
        return Ok(None);
    };
    let result = tokio::select! {
        _ = guard.cancel.cancelled() => return Ok(None),
        result = tokio::time::timeout(std::time::Duration::from_secs(30), async {
            let provider = crate::ai::providers::configured_provider(&app, &mut req, &guard.cancel).await?;
            collect_title(provider.stream(req, guard.cancel.clone()).await?).await
        }) => result.map_err(|_| ProviderError::network())??,
    };
    let cancel = guard.cancel.clone();
    let applied = tokio::task::spawn_blocking(move || {
        if cancel.is_cancelled() {
            return Ok(None);
        }
        ConversationStore::new(&crate::paths::get_app_config_dir())
            .apply_generated_title(&snapshot, result)
    })
    .await
    .map_err(|_| ProviderError::protocol())?
    .map_err(|_| ProviderError::protocol())?;
    Ok(applied)
}

#[cfg(test)]
#[path = "tests/titles.rs"]
mod tests;
