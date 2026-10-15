use crate::ai_activity::{self, AiActivityEvent};

/// Async-friendly mirror of the data we want to record on every tool call.
pub(crate) struct CallAudit {
    pub(crate) tool: String,
    pub(crate) connection_id: Option<String>,
    pub(crate) connection_name: Option<String>,
    pub(crate) query: Option<String>,
    pub(crate) query_kind: Option<String>,
    pub(crate) rows: Option<usize>,
    pub(crate) status: String,
    pub(crate) error: Option<String>,
    pub(crate) approval_id: Option<String>,
}

impl CallAudit {
    pub(crate) fn for_tool(tool: &str) -> Self {
        Self {
            tool: tool.to_string(),
            connection_id: None,
            connection_name: None,
            query: None,
            query_kind: None,
            rows: None,
            status: "success".to_string(),
            error: None,
            approval_id: None,
        }
    }
}

pub(crate) fn emit_audit(
    session_id: &str,
    client_hint: &Option<String>,
    audit: &CallAudit,
    start: std::time::Instant,
    max_entries: usize,
) {
    let event = AiActivityEvent {
        id: ai_activity::new_uuid(),
        session_id: session_id.to_string(),
        timestamp: ai_activity::now_iso8601(),
        tool: audit.tool.clone(),
        connection_id: audit.connection_id.clone(),
        connection_name: audit.connection_name.clone(),
        query: audit.query.clone(),
        query_kind: audit.query_kind.clone(),
        duration_ms: start.elapsed().as_millis() as u64,
        status: audit.status.clone(),
        rows: audit.rows,
        error: audit.error.clone(),
        client_hint: client_hint.clone(),
        approval_id: audit.approval_id.clone(),
    };
    if let Err(e) = ai_activity::append_and_rotate(&event, max_entries) {
        eprintln!("[MCP] Failed to write audit log: {}", e);
    }
}
