use std::sync::Arc;

use crate::ai_approval::{self, PollOutcome};
use crate::ai_tools::approval::{ApprovalGate, FileQueueGate};
use crate::secure_store::testing::{decision, pending, TestDirectory};

#[tokio::test]
async fn file_gate_round_trip_uses_encrypted_queue_and_consumes_decision() {
    let dir = TestDirectory::new().unwrap();
    let request = pending("gate-round-trip");
    let gate = FileQueueGate::new(dir.path().into(), Arc::new(|| true));
    let respond = async {
        for _ in 0..100 {
            if ai_approval::read_pending_in(dir.path(), &request.id)
                .unwrap()
                .is_some()
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert_eq!(
            ai_approval::read_pending_in(dir.path(), &request.id).unwrap(),
            Some(request.clone())
        );
        let bytes = std::fs::read(
            dir.path()
                .join("pending_approvals/gate-round-trip.pending.json"),
        )
        .unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("private-artifact-marker"));
        ai_approval::write_decision_in(dir.path(), &decision(&request.id)).unwrap();
    };
    let (result, ()) = tokio::join!(gate.request(&request, 2), respond);
    assert_eq!(result.unwrap(), PollOutcome::Decided(decision(&request.id)));
    assert_eq!(
        ai_approval::read_pending_in(dir.path(), &request.id).unwrap(),
        None
    );
    assert_eq!(
        ai_approval::read_decision_in(dir.path(), &request.id).unwrap(),
        None
    );
}

#[tokio::test]
async fn file_gate_times_out_and_fails_fast_without_host() {
    let dir = TestDirectory::new().unwrap();
    let request = pending("gate-timeout");
    let gate = FileQueueGate::new(dir.path().into(), Arc::new(|| true));
    assert_eq!(
        gate.request(&request, 0).await.unwrap(),
        PollOutcome::TimedOut
    );
    assert_eq!(
        ai_approval::read_pending_in(dir.path(), &request.id).unwrap(),
        None
    );
    let gate = FileQueueGate::new(dir.path().into(), Arc::new(|| false));
    assert_eq!(
        gate.request(&request, 100).await.unwrap(),
        PollOutcome::HostUnavailable
    );
    assert_eq!(
        ai_approval::read_pending_in(dir.path(), &request.id).unwrap(),
        None
    );
}

#[test]
fn optional_schema_preserves_legacy_wire_shape_and_round_trips_encrypted() {
    let dir = TestDirectory::new().unwrap();
    let mut request = pending("gate-schema");
    let legacy = serde_json::to_value(&request).unwrap();
    assert!(legacy.get("schema").is_none());
    assert_eq!(
        serde_json::from_value::<crate::ai_approval::PendingApproval>(legacy).unwrap(),
        request
    );
    request.schema = Some("private-schema-marker".into());
    ai_approval::write_pending_in(dir.path(), &request).unwrap();
    assert_eq!(
        ai_approval::read_pending_in(dir.path(), &request.id).unwrap(),
        Some(request)
    );
    let bytes = std::fs::read(
        dir.path()
            .join("pending_approvals/gate-schema.pending.json"),
    )
    .unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("private-schema-marker"));
}
