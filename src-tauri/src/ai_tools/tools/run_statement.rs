use crate::ai_approval::{self, PendingApproval, PollOutcome};
use crate::ai_tools::audit::CallAudit;
use crate::ai_tools::{database, safety, truncate, ToolContext, ToolError, ToolOutput};
use crate::config::DEFAULT_MCP_APPROVAL_TIMEOUT_SECONDS;

pub(crate) async fn run(
    context: &ToolContext,
    query: &str,
    limit: u32,
    audit: &mut CallAudit,
) -> Result<ToolOutput, ToolError> {
    audit.query = Some(query.into());
    audit.query_kind = Some(safety::classify_query_kind(query).into());
    if context.mode != crate::ai_chat::types::ChatMode::Agent {
        return Err(ToolError::NotAvailable);
    }
    let kind = safety::enforce_connection_policy(query, context.is_readonly())?;
    if kind == safety::StatementKind::Read {
        return Err(ToolError::InvalidArguments);
    }
    let id = ai_approval::new_approval_id();
    audit.approval_id = Some(id.clone());
    let pending = PendingApproval {
        id: id.clone(),
        created_at: crate::ai_activity::now_iso8601(),
        session_id: context.origin.session_id().into(),
        connection_id: context.connection.id.clone(),
        connection_name: context.connection.name.clone(),
        database: context
            .database
            .clone()
            .or_else(|| context.params.database.as_vec().first().cloned()),
        schema: context.effective_schema().map(str::to_string),
        query: query.into(),
        query_kind: safety::classify_query_kind(query).into(),
        client_hint: Some("Tabularis assistant".into()),
        explain_plan: None,
        explain_error: None,
    };
    let timeout = context
        .config
        .mcp_approval_timeout_seconds
        .unwrap_or(DEFAULT_MCP_APPROVAL_TIMEOUT_SECONDS) as u64;
    let decision = match context.approval.request(&pending, timeout).await {
        Ok(PollOutcome::Decided(decision)) if decision.approval_id == id => decision,
        Ok(PollOutcome::Decided(_)) => {
            return Err(approval_error(
                "error",
                "Approval does not match this request.",
            ))
        }
        Ok(PollOutcome::TimedOut) => return Err(approval_error("timeout", "Approval timed out.")),
        Ok(PollOutcome::HostUnavailable) => {
            return Err(approval_error(
                "host_unavailable",
                "Tabularis is unavailable for approval.",
            ))
        }
        Err(error) => {
            return Err(ToolError::Approval {
                status: "error",
                message: error.to_string(),
            })
        }
    };
    if decision.decision != "approve" {
        return Err(approval_error("denied", "Statement denied by user."));
    }
    let effective_query = decision
        .edited_query
        .as_deref()
        .map(str::trim)
        .filter(|query| !query.is_empty())
        .unwrap_or(query);
    let kind = safety::enforce_connection_policy(effective_query, context.is_readonly())?;
    audit.query = Some(effective_query.into());
    audit.query_kind = Some(safety::classify_query_kind(effective_query).into());
    let result = database::execute(
        context.driver.as_ref(),
        &context.params,
        effective_query,
        limit,
        context.effective_schema(),
        kind == safety::StatementKind::Read,
    )
    .await
    .map_err(ToolError::Database)?;
    audit.rows = Some(result.rows.len());
    Ok(truncate::query_result(result, limit))
}

fn approval_error(status: &'static str, message: &str) -> ToolError {
    ToolError::Approval {
        status,
        message: message.into(),
    }
}
