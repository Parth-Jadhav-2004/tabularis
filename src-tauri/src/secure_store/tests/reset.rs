use super::*;
use crate::secure_store::{
    testing::{activity_event, decision, pending, TestDirectory},
    with_store_in, StoreError,
};

#[test]
fn reset_requires_confirmation_and_preserves_existing_data_when_cancelled() {
    let dir = TestDirectory::new().unwrap();
    crate::ai_activity::append_event_in(dir.path(), &activity_event("one")).unwrap();
    assert_eq!(reset_in(dir.path(), false), Err(StoreError::InvalidInput));
    assert!(dir.path().join("ai_activity.jsonl").exists());
    assert!(dir.backend.key.lock().unwrap().is_some());
}

#[test]
fn reset_deletes_all_ai_artifacts_and_key_but_keeps_watched_directory_and_other_config() {
    let dir = TestDirectory::new().unwrap();
    crate::ai_activity::append_and_rotate_in(dir.path(), &activity_event("one"), 1).unwrap();
    crate::ai_approval::write_pending_in(dir.path(), &pending("one")).unwrap();
    crate::ai_approval::write_decision_in(dir.path(), &decision("one")).unwrap();
    crate::ai_chat::blobs::BlobStore::new(dir.path())
        .put(b"private", "text/plain")
        .unwrap();
    fs::write(dir.path().join("config.json"), "unrelated").unwrap();
    reset_in(dir.path(), true).unwrap();
    assert!(!dir.path().join("ai_chats").exists());
    assert!(!dir.path().join("ai_activity.1.jsonl").exists());
    assert!(!dir.path().join(".ai_store_key_id").exists());
    assert!(dir.path().join("pending_approvals").is_dir());
    assert_eq!(
        fs::read_dir(dir.path().join("pending_approvals"))
            .unwrap()
            .count(),
        0
    );
    assert!(dir.backend.key.lock().unwrap().is_none());
    assert_eq!(
        fs::read_to_string(dir.path().join("config.json")).unwrap(),
        "unrelated"
    );
    with_store_in(dir.path(), true, |_| Ok(())).unwrap();
    assert!(dir.backend.key.lock().unwrap().is_some());
}

#[test]
fn reset_recovers_when_the_old_key_is_missing() {
    let dir = TestDirectory::new().unwrap();
    crate::ai_activity::append_event_in(dir.path(), &activity_event("one")).unwrap();
    *dir.backend.key.lock().unwrap() = None;
    reset_in(dir.path(), true).unwrap();
    assert!(with_store_in(dir.path(), true, |_| Ok(())).is_ok());
}

#[test]
fn reset_cleans_identifiable_crash_staging_files() {
    let dir = TestDirectory::new().unwrap();
    with_store_in(dir.path(), true, |store| {
        store.write_sealed(&dir.path().join(".ai-store-crash"), b"private", "activity")?;
        store.write_sealed(
            &dir.path().join(".ai-migration-crash"),
            b"private",
            "mcp_session",
        )
    })
    .unwrap();
    reset_in(dir.path(), true).unwrap();
    assert!(!dir.path().join(".ai-store-crash").exists());
    assert!(!dir.path().join(".ai-migration-crash").exists());
}

#[cfg(unix)]
#[test]
fn reset_preflight_preserves_data_and_key_when_a_managed_directory_is_unsafe() {
    let dir = TestDirectory::new().unwrap();
    crate::ai_activity::append_event_in(dir.path(), &activity_event("one")).unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("untouched"), "external").unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join("pending_approvals")).unwrap();
    assert!(reset_in(dir.path(), true).is_err());
    assert!(dir.backend.key.lock().unwrap().is_some());
    assert!(dir.path().join("ai_activity.jsonl").exists());
    assert_eq!(
        fs::read_to_string(outside.path().join("untouched")).unwrap(),
        "external"
    );
}
