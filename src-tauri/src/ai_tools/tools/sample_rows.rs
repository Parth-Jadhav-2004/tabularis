use crate::ai_tools::audit::CallAudit;
use crate::ai_tools::{database, safety, truncate, ToolContext, ToolError, ToolOutput};

pub(crate) async fn run(
    context: &ToolContext,
    table: &str,
    limit: u32,
    audit: &mut CallAudit,
) -> Result<ToolOutput, ToolError> {
    let schema = context.effective_schema();
    let query = database::sample_query(
        context.driver.as_ref(),
        &context.params,
        table,
        schema,
        limit,
    )
    .await
    .map_err(ToolError::Database)?;
    safety::require_read(&query)?;
    audit.query = Some(query.clone());
    audit.query_kind = Some("select".into());
    let result = database::execute(
        context.driver.as_ref(),
        &context.params,
        &query,
        limit,
        schema,
        true,
    )
    .await
    .map_err(ToolError::Database)?;
    audit.rows = Some(result.rows.len());
    Ok(truncate::query_result(result, limit))
}
