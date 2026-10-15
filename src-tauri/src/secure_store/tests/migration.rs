use super::*;
use crate::secure_store::testing::{
    activity_event, assert_no_plaintext, decision, pending, TestDirectory,
};

#[test]
fn legacy_files_migrate_without_plaintext_and_are_readable_and_idempotent() {
    let dir = TestDirectory::new().unwrap();
    fs::write(
        dir.path().join("ai_activity.jsonl"),
        format!(
            "{}\n",
            serde_json::to_string(&activity_event("one")).unwrap()
        ),
    )
    .unwrap();
    fs::write(
        dir.path().join("ai_activity.1.jsonl"),
        format!(
            "{}\n",
            serde_json::to_string(&activity_event("two")).unwrap()
        ),
    )
    .unwrap();
    let state = crate::ai_activity::SessionState {
        session_id: "session".into(),
        last_activity_at: "2026-10-15T12:00:00Z".into(),
        client_hint: Some("private-artifact-marker".into()),
    };
    fs::write(
        dir.path().join(".mcp_session_state.json"),
        serde_json::to_vec(&state).unwrap(),
    )
    .unwrap();
    fs::create_dir(dir.path().join("pending_approvals")).unwrap();
    fs::write(
        dir.path().join("pending_approvals/one.pending.json"),
        serde_json::to_vec(&pending("one")).unwrap(),
    )
    .unwrap();
    fs::write(
        dir.path().join("pending_approvals/one.decision.json"),
        serde_json::to_vec(&decision("one")).unwrap(),
    )
    .unwrap();
    let report = migrate_in(dir.path()).unwrap();
    assert_eq!((report.files, report.lines), (5, 2));
    assert_no_plaintext(dir.path(), b"private-artifact-marker");
    assert_eq!(
        crate::ai_activity::read_events_in(dir.path(), &crate::ai_activity::EventFilter::default())
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        crate::ai_activity::load_session_state_in(dir.path()).unwrap(),
        state
    );
    assert_eq!(
        crate::ai_approval::read_pending_in(dir.path(), "one")
            .unwrap()
            .unwrap(),
        pending("one")
    );
    assert_eq!(migrate_in(dir.path()).unwrap().files, 0);
}

#[test]
fn mixed_logs_preserve_existing_sealed_lines_and_encrypt_malformed_legacy_lines() {
    let dir = TestDirectory::new().unwrap();
    crate::ai_activity::append_event_in(dir.path(), &activity_event("one")).unwrap();
    let path = dir.path().join("ai_activity.jsonl");
    let original = fs::read_to_string(&path).unwrap();
    use std::io::Write;
    writeln!(
        fs::OpenOptions::new().append(true).open(&path).unwrap(),
        "private-artifact-marker invalid JSON"
    )
    .unwrap();
    assert_eq!(migrate_in(dir.path()).unwrap().lines, 1);
    assert!(fs::read_to_string(path).unwrap().starts_with(&original));
    assert_no_plaintext(dir.path(), b"private-artifact-marker");
    assert_eq!(
        crate::ai_activity::read_events_in(dir.path(), &crate::ai_activity::EventFilter::default())
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn denied_keychain_and_invalid_json_leave_originals_untouched() {
    let dir = TestDirectory::new().unwrap();
    let path = dir.path().join(".mcp_session_state.json");
    fs::write(&path, "invalid-private-marker").unwrap();
    dir.backend
        .denied
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(migrate_in(dir.path()).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "invalid-private-marker");
    dir.backend
        .denied
        .store(false, std::sync::atomic::Ordering::SeqCst);
    assert!(migrate_in(dir.path()).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "invalid-private-marker");
}

#[test]
fn empty_profile_does_not_create_a_key_or_files() {
    let dir = TestDirectory::new().unwrap();
    assert_eq!(migrate_in(dir.path()).unwrap().files, 0);
    assert!(dir.backend.key.lock().unwrap().is_none());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn migration_rejects_symlinks_without_modifying_external_files() {
    let dir = TestDirectory::new().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let external = outside.path().join("state");
    fs::write(&external, "external-private-marker").unwrap();
    std::os::unix::fs::symlink(&external, dir.path().join(".mcp_session_state.json")).unwrap();
    assert!(migrate_in(dir.path()).is_err());
    assert_eq!(
        fs::read_to_string(external).unwrap(),
        "external-private-marker"
    );
}
