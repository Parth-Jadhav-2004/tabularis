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

pub(crate) fn activity_event(id: &str) -> crate::ai_activity::AiActivityEvent {
    crate::ai_activity::AiActivityEvent {
        id: id.into(),
        session_id: "session".into(),
        timestamp: "2026-10-15T12:00:00Z".into(),
        tool: "run_query".into(),
        connection_id: Some("c".into()),
        connection_name: Some("test".into()),
        query: Some("private-artifact-marker".into()),
        query_kind: Some("select".into()),
        duration_ms: 1,
        status: "ok".into(),
        rows: Some(1),
        error: None,
        client_hint: None,
        approval_id: None,
    }
}

pub(crate) fn pending(id: &str) -> crate::ai_approval::PendingApproval {
    crate::ai_approval::PendingApproval {
        id: id.into(),
        created_at: "2026-10-15T12:00:00Z".into(),
        session_id: "session".into(),
        connection_id: "c".into(),
        connection_name: "test".into(),
        database: None,
        query: "private-artifact-marker".into(),
        query_kind: "write".into(),
        client_hint: None,
        explain_plan: None,
        explain_error: None,
    }
}

pub(crate) fn decision(id: &str) -> crate::ai_approval::ApprovalDecision {
    crate::ai_approval::ApprovalDecision {
        approval_id: id.into(),
        decided_at: "2026-10-15T12:00:01Z".into(),
        decision: "approve".into(),
        reason: Some("private-artifact-marker".into()),
        edited_query: None,
    }
}

pub(crate) fn assert_no_plaintext(dir: &Path, marker: &[u8]) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            assert_no_plaintext(&entry.path(), marker);
        } else {
            let bytes = std::fs::read(entry.path()).unwrap();
            assert!(
                !bytes.windows(marker.len()).any(|part| part == marker),
                "Plaintext in {:?}",
                entry.file_name()
            );
        }
    }
}
