use crate::secure_store::testing::{
    activity_event, assert_no_plaintext, decision, pending, TestDirectory,
};
use crate::{ai_activity as activity, ai_approval as approval};

#[test]
fn activity_session_and_approval_writes_leave_no_plaintext_marker() {
    let dir = TestDirectory::new().unwrap();
    activity::append_and_rotate_in(dir.path(), &activity_event("one"), 1).unwrap();
    activity::save_session_state_in(
        dir.path(),
        &activity::SessionState {
            session_id: "session".into(),
            last_activity_at: "2026-10-15T12:00:00Z".into(),
            client_hint: Some("private-artifact-marker".into()),
        },
    )
    .unwrap();
    approval::write_pending_in(dir.path(), &pending("p1")).unwrap();
    approval::write_decision_in(dir.path(), &decision("p1")).unwrap();
    assert_no_plaintext(dir.path(), b"private-artifact-marker");
    assert_eq!(
        activity::read_sessions_in(dir.path()).unwrap()[0].event_count,
        1
    );
    assert_eq!(
        approval::read_decision_in(dir.path(), "p1")
            .unwrap()
            .unwrap()
            .decision,
        "approve"
    );
}

#[test]
fn one_corrupted_activity_line_does_not_hide_other_events() {
    let dir = TestDirectory::new().unwrap();
    activity::append_event_in(dir.path(), &activity_event("one")).unwrap();
    let path = dir.path().join("ai_activity.jsonl");
    use std::io::Write;
    writeln!(
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap(),
        "corrupted-line"
    )
    .unwrap();
    activity::append_event_in(dir.path(), &activity_event("two")).unwrap();
    assert_eq!(
        activity::read_events_in(dir.path(), &activity::EventFilter::default())
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn copied_and_corrupted_decisions_cannot_approve_a_different_request() {
    let dir = TestDirectory::new().unwrap();
    approval::write_decision_in(dir.path(), &decision("one")).unwrap();
    std::fs::copy(
        dir.path().join("pending_approvals/one.decision.json"),
        dir.path().join("pending_approvals/two.decision.json"),
    )
    .unwrap();
    assert!(approval::poll_decision_in(dir.path(), "two", 0, 1)
        .await
        .is_err());
    approval::write_pending_in(dir.path(), &pending("three")).unwrap();
    std::fs::copy(
        dir.path().join("pending_approvals/three.pending.json"),
        dir.path().join("pending_approvals/three.decision.json"),
    )
    .unwrap();
    assert!(approval::read_decision_in(dir.path(), "three").is_err());
    std::fs::write(
        dir.path().join("pending_approvals/one.decision.json"),
        "corrupt",
    )
    .unwrap();
    assert!(approval::read_decision_in(dir.path(), "one").is_err());
}

#[test]
fn unsafe_approval_ids_and_invalid_decisions_are_rejected() {
    let dir = TestDirectory::new().unwrap();
    assert!(approval::write_pending_in(dir.path(), &pending("../outside")).is_err());
    assert!(approval::read_decision_in(dir.path(), "../outside").is_err());
    let mut invalid = decision("one");
    invalid.decision = "unknown".into();
    assert!(approval::write_decision_in(dir.path(), &invalid).is_err());
}

#[test]
fn keychain_denial_does_not_write_plaintext_artifacts() {
    let dir = TestDirectory::new().unwrap();
    dir.backend
        .denied
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(activity::append_event_in(dir.path(), &activity_event("one")).is_err());
    assert!(approval::write_pending_in(dir.path(), &pending("one")).is_err());
    assert!(!dir.path().join("ai_activity.jsonl").exists());
    assert!(!dir
        .path()
        .join("pending_approvals/one.pending.json")
        .exists());
}

#[test]
fn concurrent_appenders_do_not_interleave_encrypted_lines() {
    let dir = TestDirectory::new().unwrap();
    let base = dir.path().to_path_buf();
    let workers: Vec<_> = (0..16)
        .map(|i| {
            let base = base.clone();
            std::thread::spawn(move || {
                activity::append_event_in(&base, &activity_event(&i.to_string())).unwrap()
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(
        activity::read_events_in(dir.path(), &activity::EventFilter::default())
            .unwrap()
            .len(),
        16
    );
}
