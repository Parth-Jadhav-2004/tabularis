//! Explicit, directory-scoped keychain fixtures. Tests never open the OS keychain.
use super::{key::KeyBackend, StoreError};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Default)]
pub(crate) struct MemoryKeychain {
    pub key: Mutex<Option<String>>,
    pub denied: std::sync::atomic::AtomicBool,
}

impl KeyBackend for Arc<MemoryKeychain> {
    fn get(&self) -> Result<Option<String>, StoreError> {
        if self.denied.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(StoreError::KeyUnavailable);
        }
        Ok(self.key.lock().unwrap().clone())
    }
    fn set(&self, key: &str) -> Result<(), StoreError> {
        self.get()?;
        *self.key.lock().unwrap() = Some(key.into());
        Ok(())
    }
    fn delete(&self) -> Result<(), StoreError> {
        self.get()?;
        *self.key.lock().unwrap() = None;
        Ok(())
    }
}

type Backends = HashMap<PathBuf, Arc<MemoryKeychain>>;
static BACKENDS: OnceLock<Mutex<Backends>> = OnceLock::new();
fn backends() -> &'static Mutex<Backends> {
    BACKENDS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn backend(base: &Path) -> Result<Arc<MemoryKeychain>, StoreError> {
    backends()
        .lock()
        .unwrap()
        .get(base)
        .cloned()
        .ok_or(StoreError::KeyUnavailable)
}

pub(crate) struct TestDirectory {
    dir: tempfile::TempDir,
    pub backend: Arc<MemoryKeychain>,
}
impl TestDirectory {
    pub fn new() -> std::io::Result<Self> {
        let dir = tempfile::tempdir()?;
        let backend = Arc::new(MemoryKeychain::default());
        backends()
            .lock()
            .unwrap()
            .insert(dir.path().to_path_buf(), backend.clone());
        Ok(Self { dir, backend })
    }
}
impl std::ops::Deref for TestDirectory {
    type Target = tempfile::TempDir;
    fn deref(&self) -> &Self::Target {
        &self.dir
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        backends().lock().unwrap().remove(self.dir.path());
        super::key::invalidate(self.dir.path());
    }
}
