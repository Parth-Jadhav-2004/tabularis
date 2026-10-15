use super::{envelope, io::write_private_atomic, CipherStore, StoreError};
use aes_gcm::aead::{rand_core::RngCore, OsRng};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use fs2::FileExt;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use zeroize::Zeroizing;

const MARKER: &str = ".ai_store_key_id";
static PROCESS_LOCK: Mutex<()> = Mutex::new(());
static CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedKey>>> = OnceLock::new();

struct CachedKey {
    fingerprint: String,
    store: CipherStore,
}

pub(super) trait KeyBackend {
    fn get(&self) -> Result<Option<String>, StoreError>;
    fn set(&self, key: &str) -> Result<(), StoreError>;
    fn delete(&self) -> Result<(), StoreError>;
}

#[cfg(not(test))]
struct SystemKeychain;

#[cfg(not(test))]
impl KeyBackend for SystemKeychain {
    fn get(&self) -> Result<Option<String>, StoreError> {
        crate::keychain_utils::get_ai_store_key().map_err(|_| StoreError::KeyUnavailable)
    }
    fn set(&self, key: &str) -> Result<(), StoreError> {
        crate::keychain_utils::set_ai_store_key(key).map_err(|_| StoreError::KeyUnavailable)
    }
    fn delete(&self) -> Result<(), StoreError> {
        crate::keychain_utils::delete_ai_store_key().map_err(|_| StoreError::KeyUnavailable)
    }
}

fn cache() -> &'static Mutex<HashMap<PathBuf, CachedKey>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Serializes key creation, reset and all encrypted read/modify/write operations
/// across both threads and the separate GUI/MCP processes. Closures must use the
/// supplied cipher directly rather than nesting another storage transaction.
pub(crate) fn exclusive_in<T>(
    base: &Path,
    work: impl FnOnce() -> Result<T, StoreError>,
) -> Result<T, StoreError> {
    let _process = PROCESS_LOCK.lock().map_err(|_| StoreError::Io)?;
    // The keychain account is shared even when the user changes storage roots.
    #[cfg(not(test))]
    let _key_lock =
        lock_file(&crate::paths::get_default_app_config_dir().join(".ai_store.key.lock"))?;
    fs::create_dir_all(base)?;
    let _data_lock = lock_file(&base.join(".ai_store.lock"))?;
    work()
}

fn lock_file(path: &Path) -> Result<fs::File, StoreError> {
    fs::create_dir_all(path.parent().ok_or(StoreError::InvalidInput)?)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options.open(path)?;
    FileExt::lock_exclusive(&lock)?;
    Ok(lock)
}

pub(crate) fn with_store_in<T>(
    base: &Path,
    create: bool,
    work: impl FnOnce(&CipherStore) -> Result<T, StoreError>,
) -> Result<T, StoreError> {
    exclusive_in(base, || {
        #[cfg(not(test))]
        let backend = SystemKeychain;
        #[cfg(test)]
        let backend = super::testing::backend(base)?;
        let store = load_in(base, create, &backend)?;
        work(&store)
    })
}

fn load_in(
    base: &Path,
    create: bool,
    backend: &impl KeyBackend,
) -> Result<CipherStore, StoreError> {
    let marker_path = base.join(MARKER);
    let marker = match fs::read_to_string(&marker_path) {
        Ok(marker) => Some(marker),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return Err(StoreError::Io),
    };
    if let Some(cached) = cache().lock().map_err(|_| StoreError::Io)?.get(base) {
        if marker.as_deref() == Some(&cached.fingerprint) {
            return Ok(cached.store.clone());
        }
    }
    cache().lock().map_err(|_| StoreError::Io)?.remove(base);
    let encoded = match backend.get()? {
        Some(key) => Zeroizing::new(key),
        None if !create || marker.is_some() || has_sealed_artifacts(base)? => {
            return Err(StoreError::KeyUnavailable)
        }
        None => {
            let mut key = Zeroizing::new([0; 32]);
            OsRng
                .try_fill_bytes(key.as_mut())
                .map_err(|_| StoreError::KeyUnavailable)?;
            let encoded = Zeroizing::new(BASE64.encode(key.as_ref()));
            backend.set(&encoded)?;
            let verified = Zeroizing::new(backend.get()?.ok_or(StoreError::KeyUnavailable)?);
            if *verified != *encoded {
                return Err(StoreError::KeyUnavailable);
            }
            encoded
        }
    };
    let decoded = Zeroizing::new(
        BASE64
            .decode(encoded.as_bytes())
            .map_err(|_| StoreError::KeyUnavailable)?,
    );
    let key: [u8; 32] = decoded
        .as_slice()
        .try_into()
        .map_err(|_| StoreError::KeyUnavailable)?;
    let key = Zeroizing::new(key);
    let fingerprint = BASE64.encode(Sha256::digest(key.as_ref()));
    if marker.as_ref().is_some_and(|marker| marker != &fingerprint) {
        return Err(StoreError::KeyUnavailable);
    }
    if marker.is_none() {
        write_private_atomic(&marker_path, fingerprint.as_bytes())?;
    }
    let store = CipherStore::from_key(key);
    cache().lock().map_err(|_| StoreError::Io)?.insert(
        base.to_path_buf(),
        CachedKey {
            fingerprint,
            store: store.clone(),
        },
    );
    Ok(store)
}

fn has_sealed_artifacts(base: &Path) -> Result<bool, StoreError> {
    let chats = base.join("ai_chats");
    super::check_directory(&chats)?;
    if chats.exists() && contains_files(&chats)? {
        return Ok(true);
    }
    for entry in fs::read_dir(base)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("ai_activity") && name.ends_with(".jsonl") {
            use std::io::{BufRead, BufReader};
            for line in BufReader::new(fs::File::open(entry.path())?).lines() {
                let line = line?;
                if BASE64
                    .decode(line.trim())
                    .is_ok_and(|bytes| envelope::is_sealed(&bytes))
                {
                    return Ok(true);
                }
            }
        } else if name == ".mcp_session_state.json" && envelope::is_sealed(&fs::read(entry.path())?)
        {
            return Ok(true);
        }
    }
    let approvals = base.join("pending_approvals");
    super::check_directory(&approvals)?;
    if approvals.exists() {
        for entry in fs::read_dir(approvals)? {
            let entry = entry?;
            if entry.file_type()?.is_file() && envelope::is_sealed(&fs::read(entry.path())?) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn contains_files(dir: &Path) -> Result<bool, StoreError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_file() || (kind.is_dir() && contains_files(&entry.path())?) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Called under exclusive_in only, after removing every AI artifact.
pub(crate) fn delete_key_locked(base: &Path) -> Result<(), StoreError> {
    #[cfg(not(test))]
    let backend = SystemKeychain;
    #[cfg(test)]
    let backend = super::testing::backend(base)?;
    backend.delete()?;
    match fs::remove_file(base.join(MARKER)) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(StoreError::Io),
    }
    invalidate(base);
    Ok(())
}

pub(super) fn invalidate(base: &Path) {
    if let Ok(mut cache) = cache().lock() {
        cache.remove(base);
    }
}

#[cfg(test)]
#[path = "tests/key.rs"]
mod tests;
