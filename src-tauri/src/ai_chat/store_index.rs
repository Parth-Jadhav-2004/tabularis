use super::{
    store_paths,
    types::{Conversation, ConversationSummary},
};
use crate::secure_store::{CipherStore, StoreError};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub(super) fn load_locked(
    base: &Path,
    connection: &str,
    id: &str,
    store: &CipherStore,
) -> Result<Conversation, StoreError> {
    let path = store_paths::chat_path(base, connection, id)?;
    let bytes = store.open(
        &store_paths::read_bounded(&path)?,
        &format!("chat:{connection}:{id}"),
    )?;
    let chat: Conversation = serde_json::from_slice(&bytes).map_err(|_| StoreError::Corrupted)?;
    if chat.schema_version != 1 {
        return Err(StoreError::UnsupportedVersion);
    }
    if chat.id != id || chat.connection_id != connection {
        return Err(StoreError::WrongContext);
    }
    Ok(chat)
}

pub(super) fn chat_ids(base: &Path, connection: &str) -> Result<Vec<String>, StoreError> {
    let dir = store_paths::connection_dir(base, connection)?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut ids = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(id) = name.to_str().and_then(|n| n.strip_suffix(".chat")) else {
            continue;
        };
        if id != "index" {
            store_paths::validate_id(id)?;
            ids.push(id.to_string());
        }
    }
    ids.sort();
    Ok(ids)
}

pub(super) fn rebuild_locked(
    base: &Path,
    connection: &str,
    store: &CipherStore,
) -> Result<Vec<ConversationSummary>, StoreError> {
    let mut summaries = Vec::new();
    for id in chat_ids(base, connection)? {
        summaries.push(ConversationSummary::from(&load_locked(
            base, connection, &id, store,
        )?));
    }
    summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.id.cmp(&b.id)));
    let dir = store_paths::connection_dir(base, connection)?;
    if dir.exists() {
        let bytes = serde_json::to_vec(&summaries).map_err(|_| StoreError::Corrupted)?;
        store.write_sealed(
            &dir.join("index.chat"),
            &bytes,
            &format!("index:{connection}"),
        )?;
    }
    Ok(summaries)
}

pub(super) fn list_locked(
    base: &Path,
    connection: &str,
    store: &CipherStore,
) -> Result<Vec<ConversationSummary>, StoreError> {
    let dir = store_paths::connection_dir(base, connection)?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let path = dir.join("index.chat");
    let ids = chat_ids(base, connection)?;
    let index = store_paths::read_bounded(&path)
        .and_then(|bytes| store.open(&bytes, &format!("index:{connection}")))
        .and_then(|bytes| {
            serde_json::from_slice::<Vec<ConversationSummary>>(&bytes)
                .map_err(|_| StoreError::Corrupted)
        });
    if let Ok(index) = index {
        let indexed: HashSet<_> = index.iter().map(|s| s.id.clone()).collect();
        let actual: HashSet<_> = ids.iter().cloned().collect();
        let modified = fs::metadata(&path)?.modified()?;
        let fresh = ids.iter().all(|id| {
            store_paths::chat_path(base, connection, id)
                .ok()
                .and_then(|p| fs::metadata(p).ok())
                .and_then(|m| m.modified().ok())
                .is_some_and(|at| at < modified)
        });
        if indexed == actual
            && indexed.len() == index.len()
            && index.iter().all(|s| s.connection_id == connection)
            && fresh
        {
            return Ok(index);
        }
    }
    rebuild_locked(base, connection, store)
}

pub(crate) fn all_conversations_locked(
    base: &Path,
    store: &CipherStore,
) -> Result<Vec<Conversation>, StoreError> {
    let mut chats = Vec::new();
    for connection in store_paths::connections(base)? {
        for id in chat_ids(base, &connection)? {
            supports_mutation(base, &connection, &id, store)?;
            chats.push(load_locked(base, &connection, &id, store)?);
        }
    }
    Ok(chats)
}

pub(super) fn supports_mutation(
    base: &Path,
    connection: &str,
    id: &str,
    store: &CipherStore,
) -> Result<(), StoreError> {
    let path = store_paths::chat_path(base, connection, id)?;
    let bytes = store.open(
        &store_paths::read_bounded(&path)?,
        &format!("chat:{connection}:{id}"),
    )?;
    let raw: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| StoreError::Corrupted)?;
    if let Some(messages) = raw["messages"].as_array() {
        for message in messages {
            if let Some(blocks) = message["blocks"].as_array() {
                for block in blocks {
                    if !matches!(
                        block["type"].as_str(),
                        Some(
                            "text"
                                | "reasoning"
                                | "tool_call"
                                | "tool_result"
                                | "image"
                                | "attachment"
                                | "context"
                                | "error"
                        )
                    ) {
                        return Err(StoreError::UnsupportedVersion);
                    }
                }
            }
        }
    }
    Ok(())
}
