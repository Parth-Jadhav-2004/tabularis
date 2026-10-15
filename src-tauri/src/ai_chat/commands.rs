use super::{
    store::ConversationStore,
    types::{Conversation, ConversationSummary, SaveResult},
};
use crate::secure_store::StoreError;

#[tauri::command]
pub async fn ai_chat_put_blob(bytes: Vec<u8>, mime: String) -> Result<String, StoreError> {
    tokio::task::spawn_blocking(move || {
        super::blobs::BlobStore::new(&crate::paths::get_app_config_dir()).put(&bytes, &mime)
    })
    .await
    .map_err(|_| StoreError::Io)?
}

#[tauri::command]
pub async fn ai_chat_get_blob(blob_id: String) -> Result<super::blobs::Blob, StoreError> {
    tokio::task::spawn_blocking(move || {
        super::blobs::BlobStore::new(&crate::paths::get_app_config_dir()).get(&blob_id)
    })
    .await
    .map_err(|_| StoreError::Io)?
}

#[tauri::command]
pub async fn ai_chat_list(
    connection_id: Option<String>,
) -> Result<Vec<ConversationSummary>, StoreError> {
    tokio::task::spawn_blocking(move || {
        ConversationStore::new(&crate::paths::get_app_config_dir()).list(connection_id.as_deref())
    })
    .await
    .map_err(|_| StoreError::Io)?
}

#[tauri::command]
pub async fn ai_chat_load(connection_id: String, id: String) -> Result<Conversation, StoreError> {
    tokio::task::spawn_blocking(move || {
        ConversationStore::new(&crate::paths::get_app_config_dir()).load(&connection_id, &id)
    })
    .await
    .map_err(|_| StoreError::Io)?
}

#[tauri::command]
pub async fn ai_chat_save(conversation: Conversation) -> Result<SaveResult, StoreError> {
    tokio::task::spawn_blocking(move || {
        ConversationStore::new(&crate::paths::get_app_config_dir()).save(conversation)
    })
    .await
    .map_err(|_| StoreError::Io)?
}

#[tauri::command]
pub async fn ai_chat_rename(
    connection_id: String,
    id: String,
    title: String,
) -> Result<(), StoreError> {
    tokio::task::spawn_blocking(move || {
        ConversationStore::new(&crate::paths::get_app_config_dir()).rename(
            &connection_id,
            &id,
            title,
        )
    })
    .await
    .map_err(|_| StoreError::Io)?
}

#[tauri::command]
pub async fn ai_chat_delete(connection_id: String, id: String) -> Result<(), StoreError> {
    tokio::task::spawn_blocking(move || {
        ConversationStore::new(&crate::paths::get_app_config_dir()).delete(&connection_id, &id)
    })
    .await
    .map_err(|_| StoreError::Io)?
}

#[tauri::command]
pub async fn ai_chat_delete_all() -> Result<(), StoreError> {
    tokio::task::spawn_blocking(|| {
        ConversationStore::new(&crate::paths::get_app_config_dir()).delete_all()
    })
    .await
    .map_err(|_| StoreError::Io)?
}
