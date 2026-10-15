//! Read-only context preparation. The prompt and editor contents stay in Rust;
//! previews share the send resolver and expose only bounded metadata summaries.
use super::context::{BuiltContext, ContextItemSummary};
use super::request_context::{resolve, ContextSpec};
use crate::ai::message::ContextItemRef;
use crate::ai::provider::{ErrorKind, ProviderError};
use serde::Serialize;
use tauri::AppHandle;

impl From<ContextItemSummary> for ContextItemRef {
    fn from(item: ContextItemSummary) -> Self {
        Self {
            kind: item.kind,
            id: item.id,
            label: item.label,
            chars: Some(item.chars),
            table_count: item.table_count,
            view_count: item.view_count,
            truncated: item.truncated,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ContextPreview {
    pub items: Vec<ContextItemRef>,
    pub truncated: bool,
    pub total_chars: usize,
}

impl From<BuiltContext> for ContextPreview {
    fn from(built: BuiltContext) -> Self {
        Self {
            items: built.items.into_iter().map(ContextItemRef::from).collect(),
            truncated: built.truncated,
            total_chars: built.base_chars + built.context_chars,
        }
    }
}

#[tauri::command]
pub async fn ai_chat_preview_context(
    app: AppHandle,
    context: ContextSpec,
) -> Result<ContextPreview, ProviderError> {
    if !crate::config::load_config_internal(&app)
        .ai_enabled
        .unwrap_or(false)
    {
        return Err(ProviderError::new(
            ErrorKind::Other,
            "AI is disabled in Settings.",
        ));
    }
    tokio::time::timeout(std::time::Duration::from_secs(15), resolve(&app, &context))
        .await
        .map_err(|_| ProviderError::new(ErrorKind::Other, "Preparing the AI context timed out."))?
        .map(ContextPreview::from)
}
