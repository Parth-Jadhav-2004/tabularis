use super::{store_index, store_paths};
use crate::ai::message::Block;
use crate::secure_store::{with_store_in, CipherStore, StoreError};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const MAX_BLOB_BYTES: usize = 20 * 1024 * 1024;

#[derive(Serialize)]
pub struct Blob {
    pub bytes: Vec<u8>,
    pub mime: String,
}

pub struct BlobStore {
    base: PathBuf,
}
impl BlobStore {
    pub fn new(base: &Path) -> Self {
        Self {
            base: base.to_path_buf(),
        }
    }

    fn dir(&self) -> Result<PathBuf, StoreError> {
        store_paths::check_dir(&self.base.join("ai_chats"))?;
        let dir = self.base.join("ai_chats/blobs");
        store_paths::check_dir(&dir)?;
        Ok(dir)
    }

    fn path(&self, id: &str) -> Result<PathBuf, StoreError> {
        if id.len() != 64
            || !id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(StoreError::InvalidInput);
        }
        Ok(self.dir()?.join(format!("{id}.bin")))
    }

    pub fn put(&self, bytes: &[u8], mime: &str) -> Result<String, StoreError> {
        if bytes.len() > MAX_BLOB_BYTES {
            return Err(StoreError::TooLarge);
        }
        if mime.is_empty()
            || mime.len() > 128
            || !mime.contains('/')
            || !mime.bytes().all(|b| b.is_ascii_graphic() || b == b' ')
        {
            return Err(StoreError::InvalidInput);
        }
        let id = format!("{:x}", Sha256::digest(bytes));
        let path = self.path(&id)?;
        with_store_in(&self.base, true, |store| {
            if path.exists() {
                self.get_locked(&id, store)?;
                return Ok(id.clone());
            }
            store_paths::private_dir(&self.base.join("ai_chats"))?;
            store_paths::private_dir(&self.dir()?)?;
            let mut payload = Vec::with_capacity(bytes.len() + mime.len() + 1);
            payload.extend_from_slice(mime.as_bytes());
            payload.push(b'\n');
            payload.extend_from_slice(bytes);
            store.write_sealed(&path, &payload, &format!("blob:{id}"))?;
            Ok(id.clone())
        })
    }

    pub fn get(&self, id: &str) -> Result<Blob, StoreError> {
        self.path(id)?;
        with_store_in(&self.base, false, |store| self.get_locked(id, store))
    }

    fn get_locked(&self, id: &str, store: &CipherStore) -> Result<Blob, StoreError> {
        let path = self.path(id)?;
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(StoreError::InvalidInput);
        }
        let limit = (MAX_BLOB_BYTES * 2 + 4096) as u64;
        if metadata.len() > limit {
            return Err(StoreError::TooLarge);
        }
        let mut sealed = Vec::new();
        fs::File::open(&path)?
            .take(limit + 1)
            .read_to_end(&mut sealed)?;
        if sealed.len() as u64 > limit {
            return Err(StoreError::TooLarge);
        }
        let payload = store.open(&sealed, &format!("blob:{id}"))?;
        let separator = payload
            .iter()
            .take(129)
            .position(|&b| b == b'\n')
            .ok_or(StoreError::Corrupted)?;
        let mime = std::str::from_utf8(&payload[..separator])
            .map_err(|_| StoreError::Corrupted)?
            .to_string();
        let bytes = payload[separator + 1..].to_vec();
        if bytes.len() > MAX_BLOB_BYTES || format!("{:x}", Sha256::digest(&bytes)) != id {
            return Err(StoreError::Corrupted);
        }
        Ok(Blob { bytes, mime })
    }

    /// Never collect new uploads, or anything when a conversation cannot be read.
    pub fn gc(&self, minimum_age: Duration) -> Result<usize, StoreError> {
        let dir = self.dir()?;
        if !dir.exists() {
            return Ok(0);
        }
        with_store_in(&self.base, false, |store| {
            let mut referenced = HashSet::new();
            for chat in store_index::all_conversations_locked(&self.base, store)? {
                for message in chat.messages {
                    for block in message.blocks {
                        match block {
                            Block::Image { blob_id, .. }
                            | Block::Attachment {
                                blob_id: Some(blob_id),
                                ..
                            } => {
                                referenced.insert(blob_id);
                            }
                            _ => {}
                        }
                    }
                }
            }
            let mut removed = 0;
            let now = SystemTime::now();
            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                let name = entry.file_name();
                let Some(id) = name.to_str().and_then(|n| n.strip_suffix(".bin")) else {
                    continue;
                };
                if self.path(id).is_err()
                    || !entry.file_type()?.is_file()
                    || referenced.contains(id)
                {
                    continue;
                }
                if now
                    .duration_since(entry.metadata()?.modified()?)
                    .is_ok_and(|age| age >= minimum_age)
                {
                    fs::remove_file(entry.path())?;
                    removed += 1;
                }
            }
            Ok(removed)
        })
    }
}

pub fn spawn_startup_gc() {
    tauri::async_runtime::spawn(async {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let _ = tokio::task::spawn_blocking(|| {
            if BlobStore::new(&crate::paths::get_app_config_dir())
                .gc(Duration::from_secs(3600))
                .is_err()
            {
                log::warn!(
                    "AI blob cleanup was skipped because storage is unavailable or unreadable"
                );
            }
        })
        .await;
    });
}

#[cfg(test)]
#[path = "tests/blobs.rs"]
mod tests;
