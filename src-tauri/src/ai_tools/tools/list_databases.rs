use crate::ai_tools::audit::CallAudit;
use crate::ai_tools::connections::resolve_db_driver;
use crate::ai_tools::output::{tool_result, ToolOutputFormat};
use crate::config::AppConfig;
use crate::mcp::protocol::JsonRpcError;
use serde_json::Value;

pub(crate) async fn tool_list_databases(
    args: &serde_json::Map<String, Value>,
    config: &AppConfig,
    audit: &mut CallAudit,
    format: ToolOutputFormat,
) -> Result<Value, JsonRpcError> {
    let conn_id = args
        .get("connection_id")
        .and_then(|v| v.as_str())
        .ok_or(JsonRpcError {
            code: -32602,
            message: "Missing connection_id".to_string(),
            data: None,
        })?;

    audit.connection_id = Some(conn_id.to_string());

    let (conn, db_params, driver) =
        resolve_db_driver(conn_id, config.active_external_drivers.as_deref()).await?;
    audit.connection_name = Some(conn.name.clone());

    let databases = crate::ai_tools::database::list_databases(driver.as_ref(), &db_params)
        .await
        .map_err(|message| JsonRpcError {
            code: -32000,
            message,
            data: None,
        })?;
    audit.rows = Some(databases.as_array().map_or(0, Vec::len));
    tool_result(&databases, format)
}
