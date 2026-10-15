use crate::ai::provider::{ChatEvent, FinishReason, ProviderError};
use futures::{stream::BoxStream, StreamExt};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const MAX_PENDING_BYTES: usize = 16 * 1024;

/// Preserve text/reasoning ordering while reducing IPC traffic to ~20 Hz.
#[derive(Default)]
struct Coalescer {
    pending: Option<ChatEvent>,
}

impl Coalescer {
    fn flush(&mut self, emit: &mut impl FnMut(ChatEvent)) {
        if let Some(event) = self.pending.take() {
            emit(event);
        }
    }

    fn push(&mut self, event: ChatEvent, emit: &mut impl FnMut(ChatEvent)) {
        match (&mut self.pending, &event) {
            (Some(ChatEvent::TextDelta { text }), ChatEvent::TextDelta { text: delta })
            | (
                Some(ChatEvent::ReasoningDelta { text }),
                ChatEvent::ReasoningDelta { text: delta },
            ) => text.push_str(delta),
            (_, ChatEvent::TextDelta { .. } | ChatEvent::ReasoningDelta { .. }) => {
                self.flush(emit);
                self.pending = Some(event);
            }
            _ => {
                self.flush(emit);
                emit(event);
            }
        }
        let size = match &self.pending {
            Some(ChatEvent::TextDelta { text } | ChatEvent::ReasoningDelta { text }) => text.len(),
            _ => 0,
        };
        if size >= MAX_PENDING_BYTES {
            self.flush(emit);
        }
    }
}

pub(super) async fn pump(
    mut stream: BoxStream<'static, ChatEvent>,
    cancel: CancellationToken,
    mut emit: impl FnMut(ChatEvent),
) {
    let mut coalescer = Coalescer::default();
    let mut interval = tokio::time::interval(Duration::from_millis(50));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                coalescer.flush(&mut emit);
                emit(ChatEvent::Done { finish: FinishReason::Cancelled });
                break;
            }
            _ = interval.tick() => coalescer.flush(&mut emit),
            event = stream.next() => {
                match event {
                    Some(ChatEvent::Error { error }) => {
                        coalescer.push(ChatEvent::Error { error }, &mut emit);
                        emit(ChatEvent::Done { finish: FinishReason::Error });
                        break;
                    }
                    Some(event) => {
                        let done = matches!(event, ChatEvent::Done { .. });
                        coalescer.push(event, &mut emit);
                        if done { break; }
                    }
                    None => {
                        coalescer.flush(&mut emit);
                        emit(ChatEvent::Error { error: ProviderError::protocol() });
                        emit(ChatEvent::Done { finish: FinishReason::Error });
                        break;
                    }
                }
            }
        }
    }
}
