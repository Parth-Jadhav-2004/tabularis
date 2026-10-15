use crate::ai_tools::audit::CallAudit;
use crate::ai_tools::{safety, truncate, ToolContext, ToolError, ToolOutput};

pub(crate) async fn run(
    context: &ToolContext,
    query: &str,
    audit: &mut CallAudit,
) -> Result<ToolOutput, ToolError> {
    audit.query = Some(query.into());
    audit.query_kind = Some(safety::classify_query_kind(query).into());
    safety::require_read(query)?;
    let plan = context
        .driver
        .explain_query(&context.params, query, false, context.effective_schema())
        .await
        .map_err(ToolError::Database)?;
    let value = serde_json::to_string(&plan)
        .map_err(|_| ToolError::Database("Could not encode query plan.".into()))?;
    Ok(truncate::output(&value))
}
