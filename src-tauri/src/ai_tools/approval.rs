//! Shared approval transport; callers retain their read-only and SQL policies.
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::ai_approval::{self, PendingApproval, PollOutcome};

#[derive(Debug)]
pub enum ApprovalQueueError {
    Enqueue(String),
    Poll(String),
}

impl std::fmt::Display for ApprovalQueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Enqueue(message) => write!(f, "Failed to enqueue approval request: {}", message),
            Self::Poll(message) => f.write_str(message),
        }
    }
}

#[async_trait]
pub trait ApprovalGate: Send + Sync {
    async fn request(
        &self,
        pending: &PendingApproval,
        timeout_secs: u64,
    ) -> Result<PollOutcome, ApprovalQueueError>;
}

pub struct FileQueueGate {
    base: PathBuf,
    is_alive: Arc<dyn Fn() -> bool + Send + Sync>,
}

impl Default for FileQueueGate {
    fn default() -> Self {
        Self {
            base: crate::paths::get_app_config_dir(),
            is_alive: Arc::new(crate::heartbeat::is_alive),
        }
    }
}

impl FileQueueGate {
    /// Explicit paths and liveness also let tests use isolated encrypted stores.
    pub fn new(base: PathBuf, is_alive: Arc<dyn Fn() -> bool + Send + Sync>) -> Self {
        Self { base, is_alive }
    }
}

#[async_trait]
impl ApprovalGate for FileQueueGate {
    async fn request(
        &self,
        pending: &PendingApproval,
        timeout_secs: u64,
    ) -> Result<PollOutcome, ApprovalQueueError> {
        if !(self.is_alive)() {
            return Ok(PollOutcome::HostUnavailable);
        }
        ai_approval::write_pending_in(&self.base, pending).map_err(ApprovalQueueError::Enqueue)?;
        ai_approval::poll_decision_with_liveness_in(
            &self.base,
            &pending.id,
            timeout_secs,
            500,
            || (self.is_alive)(),
        )
        .await
        .map_err(ApprovalQueueError::Poll)
    }
}
