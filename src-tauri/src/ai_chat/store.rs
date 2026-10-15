use super::{
    store_index, store_paths,
    types::{Conversation, ConversationSummary, SaveResult, TitleSource},
};
use crate::secure_store::{exclusive_in, with_store_in, CipherStore, StoreError};
use std::fs;
use std::path::{Path, PathBuf};

pub struct ConversationStore {
    base: PathBuf,
}

impl ConversationStore {
    pub fn new(base: &Path) -> Self {
        Self {
            base: base.to_path_buf(),
        }
    }

    pub fn list(&self, connection: Option<&str>) -> Result<Vec<ConversationSummary>, StoreError> {
        if let Some(connection) = connection {
            store_paths::connection_dir(&self.base, connection)?;
        }
        if !self.base.join("ai_chats").exists() {
            return Ok(Vec::new());
        }
        with_store_in(&self.base, false, |store| {
            let connections = if let Some(connection) = connection {
                vec![connection.to_string()]
            } else {
                store_paths::connections(&self.base)?
            };
            let mut summaries = Vec::new();
            for connection in connections {
                summaries.extend(store_index::list_locked(&self.base, &connection, store)?);
            }
            summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.id.cmp(&b.id)));
            Ok(summaries)
        })
    }

    pub fn load(&self, connection: &str, id: &str) -> Result<Conversation, StoreError> {
        store_paths::chat_path(&self.base, connection, id)?;
        with_store_in(&self.base, false, |store| {
            store_index::load_locked(&self.base, connection, id, store)
        })
    }

    pub fn save(&self, chat: Conversation) -> Result<SaveResult, StoreError> {
        store_paths::chat_path(&self.base, &chat.connection_id, &chat.id)?;
        with_store_in(&self.base, true, |store| self.save_locked(chat, store))
    }

    /// Validate the live connection under the same lock used by cascade cleanup,
    /// so a late frontend save cannot recreate history for a deleted connection.
    pub(crate) fn save_for_existing_connection(
        &self,
        chat: Conversation,
    ) -> Result<SaveResult, StoreError> {
        store_paths::chat_path(&self.base, &chat.connection_id, &chat.id)?;
        with_store_in(&self.base, true, |store| {
            let path = crate::paths::resolve_connections_path(&self.base);
            let connections =
                crate::persistence::load_connections(&path).map_err(|_| StoreError::Io)?;
            if !connections
                .iter()
                .any(|connection| connection.id == chat.connection_id)
            {
                return Err(StoreError::InvalidInput);
            }
            self.save_locked(chat, store)
        })
    }

    fn save_locked(
        &self,
        mut chat: Conversation,
        store: &CipherStore,
    ) -> Result<SaveResult, StoreError> {
        if chat.schema_version != 1 {
            return Err(StoreError::UnsupportedVersion);
        }
        if chat.title.trim().is_empty() || chat.title.chars().count() > 500 {
            return Err(StoreError::InvalidInput);
        }
        let path = store_paths::chat_path(&self.base, &chat.connection_id, &chat.id)?;
        if path.exists() {
            store_index::supports_mutation(&self.base, &chat.connection_id, &chat.id, store)?;
            // Refuse to overwrite unreadable/newer history; preserve its creation date.
            let saved = store_index::load_locked(&self.base, &chat.connection_id, &chat.id, store)?;
            chat.created_at = saved.created_at;
            // Background titles and manual renames must survive an older transcript save.
            if saved.title_source == TitleSource::User && chat.title_source != TitleSource::User
                || saved.title_source == TitleSource::Generated
                    && chat.title_source == TitleSource::FirstMessage
            {
                chat.title = saved.title;
                chat.title_source = saved.title_source;
            }
        }
        chat.updated_at = chrono::Utc::now().timestamp_millis();
        let bytes = serde_json::to_vec(&chat).map_err(|_| StoreError::InvalidInput)?;
        if bytes.len() > store_paths::MAX_CHAT_BYTES {
            return Err(StoreError::TooLarge);
        }
        store_paths::private_dir(&self.base.join("ai_chats"))?;
        store_paths::private_dir(path.parent().ok_or(StoreError::InvalidInput)?)?;
        store.write_sealed(
            &path,
            &bytes,
            &format!("chat:{}:{}", chat.connection_id, chat.id),
        )?;
        store_index::rebuild_locked(&self.base, &chat.connection_id, store)?;
        Ok(SaveResult {
            large: bytes.len() > 2 * 1024 * 1024,
            bytes: bytes.len(),
        })
    }

    pub fn rename(&self, connection: &str, id: &str, title: String) -> Result<(), StoreError> {
        store_paths::chat_path(&self.base, connection, id)?;
        with_store_in(&self.base, false, |store| {
            let mut chat = store_index::load_locked(&self.base, connection, id, store)?;
            chat.title = title;
            chat.title_source = TitleSource::User;
            self.save_locked(chat, store).map(|_| ())
        })
    }

    pub(super) fn apply_generated_title(
        &self,
        snapshot: &Conversation,
        title: String,
    ) -> Result<Option<ConversationSummary>, StoreError> {
        with_store_in(&self.base, false, |store| {
            let mut chat =
                store_index::load_locked(&self.base, &snapshot.connection_id, &snapshot.id, store)?;
            if chat.title_source != TitleSource::FirstMessage
                || chat.created_at != snapshot.created_at
                || chat.messages.get(..2) != snapshot.messages.get(..2)
                || snapshot.messages.len() < 2
            {
                return Ok(None);
            }
            chat.title = title;
            chat.title_source = TitleSource::Generated;
            self.save_locked(chat.clone(), store)?;
            Ok(Some(ConversationSummary::from(&chat)))
        })
    }

    pub fn delete(&self, connection: &str, id: &str) -> Result<(), StoreError> {
        let path = store_paths::chat_path(&self.base, connection, id)?;
        if !path.exists() {
            return Ok(());
        }
        with_store_in(&self.base, false, |store| {
            fs::remove_file(&path)?;
            store_index::rebuild_locked(&self.base, connection, store).map(|_| ())
        })
    }

    pub fn delete_all(&self) -> Result<(), StoreError> {
        exclusive_in(&self.base, || {
            let root = self.base.join("ai_chats");
            store_paths::check_dir(&root)?;
            if root.exists() {
                fs::remove_dir_all(root)?;
            }
            Ok(())
        })
    }

    pub fn delete_connection(&self, connection: &str) -> Result<(), StoreError> {
        let dir = store_paths::connection_dir(&self.base, connection)?;
        if !dir.exists() {
            return Ok(());
        }
        exclusive_in(&self.base, || {
            if dir.exists() {
                fs::remove_dir_all(dir)?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
#[path = "tests/store.rs"]
mod tests;
