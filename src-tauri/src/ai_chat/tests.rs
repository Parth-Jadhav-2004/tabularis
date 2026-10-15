use super::{events::pump, run::RunManager};
use crate::ai::provider::{ChatEvent, FinishReason, ProviderError};
use futures::{stream, StreamExt};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[test]
fn run_lifecycle_rejects_duplicates_cancels_independently_and_cleans_up() {
    let manager = RunManager::default();
    let id = uuid::Uuid::new_v4().to_string();
    let first = manager.start(Some(id.clone())).unwrap();
    let second = manager.start(None).unwrap();
    assert!(manager.start(Some(id.clone())).is_err());
    manager.cancel(&id);
    manager.cancel("unknown-run");
    assert!(first.cancel.is_cancelled());
    assert!(!second.cancel.is_cancelled());
    let token = second.cancel.clone();
    drop(second);
    assert!(token.is_cancelled());
    drop(first);
    assert!(manager.start(Some(id)).is_ok());
    assert!(manager.start(Some("invalid/event/name".into())).is_err());
}

#[test]
fn run_manager_limits_active_requests_and_releases_slots() {
    let manager = RunManager::default();
    let mut guards: Vec<_> = (0..8).map(|_| manager.start(None).unwrap()).collect();
    assert!(manager.start(None).is_err());
    guards.pop();
    assert!(manager.start(None).is_ok());
}

#[test]
fn run_ids_must_match_the_canonical_event_channel() {
    let manager = RunManager::default();
    let id = uuid::Uuid::new_v4();
    assert!(manager.start(Some(id.to_string())).is_ok());
    assert!(manager.start(Some(id.simple().to_string())).is_err());
    assert!(manager.start(Some(format!("{{{id}}}"))).is_err());
}

#[tokio::test]
async fn pump_coalesces_deltas_and_flushes_before_metadata() {
    let source = stream::iter(vec![
        ChatEvent::TextDelta { text: "a".into() },
        ChatEvent::TextDelta { text: "b".into() },
        ChatEvent::ReasoningDelta { text: "c".into() },
        ChatEvent::ReasoningDelta { text: "d".into() },
        ChatEvent::ReasoningSignature {
            signature: "sig".into(),
        },
        ChatEvent::Done {
            finish: FinishReason::Stop,
        },
    ])
    .boxed();
    let mut events = Vec::new();
    pump(source, CancellationToken::new(), |event| events.push(event)).await;
    assert_eq!(
        events,
        vec![
            ChatEvent::TextDelta { text: "ab".into() },
            ChatEvent::ReasoningDelta { text: "cd".into() },
            ChatEvent::ReasoningSignature {
                signature: "sig".into()
            },
            ChatEvent::Done {
                finish: FinishReason::Stop
            }
        ]
    );
}

#[tokio::test]
async fn cancellation_terminates_an_idle_stream_with_one_terminal_event() {
    let cancel = CancellationToken::new();
    cancel.cancel();
    let mut events = Vec::new();
    pump(stream::pending().boxed(), cancel, |event| {
        events.push(event)
    })
    .await;
    assert_eq!(
        events,
        vec![ChatEvent::Done {
            finish: FinishReason::Cancelled
        }]
    );
}

#[tokio::test]
async fn pump_errors_flush_text_and_do_not_duplicate_terminal_events() {
    let source = stream::iter(vec![
        ChatEvent::TextDelta {
            text: "partial".into(),
        },
        ChatEvent::Error {
            error: ProviderError::network(),
        },
        ChatEvent::Done {
            finish: FinishReason::Error,
        },
    ])
    .boxed();
    let mut events = Vec::new();
    pump(source, CancellationToken::new(), |event| events.push(event)).await;
    assert_eq!(events.len(), 3);
    assert_eq!(
        events.last(),
        Some(&ChatEvent::Done {
            finish: FinishReason::Error
        })
    );
}

#[tokio::test]
async fn pump_flushes_pending_text_on_timer_before_completion() {
    let cancel = CancellationToken::new();
    let source = stream::once(async {
        ChatEvent::TextDelta {
            text: "visible".into(),
        }
    })
    .chain(stream::pending())
    .boxed();
    let events = Arc::new(Mutex::new(Vec::new()));
    let captured = events.clone();
    let token = cancel.clone();
    let task = tokio::spawn(async move {
        pump(source, token, |event| captured.lock().unwrap().push(event)).await;
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if !events.lock().unwrap().is_empty() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        events.lock().unwrap()[0],
        ChatEvent::TextDelta {
            text: "visible".into()
        }
    );
    cancel.cancel();
    task.await.unwrap();
    assert_eq!(
        events.lock().unwrap().last(),
        Some(&ChatEvent::Done {
            finish: FinishReason::Cancelled
        })
    );
}
