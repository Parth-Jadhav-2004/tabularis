use super::events::pump;
use crate::ai::provider::{
    ChatEvent, ChatProvider, ChatRequest, ErrorKind, FinishReason, ProviderError,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Default)]
pub struct RunManager {
    runs: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

pub(super) struct RunGuard {
    pub id: String,
    pub cancel: CancellationToken,
    manager: RunManager,
}

impl Drop for RunGuard {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.manager.runs.lock().unwrap().remove(&self.id);
    }
}

impl RunManager {
    pub fn cancel_all(&self) {
        for token in self.runs.lock().unwrap().values() {
            token.cancel();
        }
    }

    pub(super) fn start(&self, requested_id: Option<String>) -> Result<RunGuard, ProviderError> {
        let id = requested_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        if !uuid::Uuid::parse_str(&id).is_ok_and(|parsed| parsed.to_string() == id) {
            return Err(ProviderError::new(
                ErrorKind::Other,
                "An AI run ID must be a canonical UUID.",
            ));
        }
        let mut runs = self.runs.lock().unwrap();
        if runs.contains_key(&id) {
            return Err(ProviderError::new(
                ErrorKind::Other,
                "This AI run is already active.",
            ));
        }
        if runs.len() >= 8 {
            return Err(ProviderError::new(
                ErrorKind::Other,
                "Too many active AI requests. Stop a request first.",
            ));
        }
        let cancel = CancellationToken::new();
        runs.insert(id.clone(), cancel.clone());
        Ok(RunGuard {
            id,
            cancel,
            manager: self.clone(),
        })
    }

    pub fn cancel(&self, id: &str) {
        if let Some(token) = self.runs.lock().unwrap().get(id) {
            token.cancel();
        }
    }
}

/// Supply a frontend-generated UUID after subscribing to avoid losing early events.
#[tauri::command]
pub async fn ai_chat_send(
    app: AppHandle,
    manager: State<'_, RunManager>,
    mut req: ChatRequest,
    run_id: Option<String>,
) -> Result<String, ProviderError> {
    if !crate::config::load_config_internal(&app)
        .ai_enabled
        .unwrap_or(false)
    {
        return Err(ProviderError::new(
            ErrorKind::Other,
            "AI is disabled in Settings.",
        ));
    }
    let guard = manager.start(run_id)?;
    let id = guard.id.clone();
    tauri::async_runtime::spawn(async move {
        let channel = format!("ai-chat://{}", guard.id);
        let emit = |event: ChatEvent| {
            let _ = app.emit(&channel, event);
        };
        let result = async {
            let provider =
                crate::ai::providers::configured_provider(&app, &mut req, &guard.cancel).await?;
            provider.stream(req, guard.cancel.clone()).await
        }
        .await;
        match result {
            _ if guard.cancel.is_cancelled() => emit(ChatEvent::Done {
                finish: FinishReason::Cancelled,
            }),
            Ok(stream) => pump(stream, guard.cancel.clone(), emit).await,
            Err(error) => {
                emit(ChatEvent::Error { error });
                emit(ChatEvent::Done {
                    finish: FinishReason::Error,
                });
            }
        }
        drop(guard);
    });
    Ok(id)
}

#[tauri::command]
pub fn ai_chat_cancel(manager: State<'_, RunManager>, run_id: String) {
    manager.cancel(&run_id);
}
