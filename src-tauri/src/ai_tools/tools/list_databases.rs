use crate::ai_tools::audit::CallAudit;
use crate::ai_tools::connections::resolve_db_driver;
use crate::ai_tools::output::{tool_result, ToolOutputFormat};
use crate::config::AppConfig;
use crate::mcp::protocol::JsonRpcError;
use serde_json::{json, Value};

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

    let databases = driver
        .get_databases(&db_params)
        .await
        .map_err(|e| JsonRpcError {
            code: -32000,
            message: e,
            data: None,
        })?;

    audit.rows = Some(databases.len());
    tool_result(&json!(databases), format)
}
