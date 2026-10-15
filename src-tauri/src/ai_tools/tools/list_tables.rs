use crate::ai_tools::audit::CallAudit;
use crate::ai_tools::connections::{
    apply_database_override, database_arg, resolve_db_driver, resolve_default_schema,
};
use crate::ai_tools::output::{tool_result, ToolOutputFormat};
use crate::config::AppConfig;
use crate::mcp::protocol::JsonRpcError;
use serde_json::{json, Value};

pub(crate) async fn tool_list_tables(
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
    let schema = args.get("schema").and_then(|v| v.as_str());

    audit.connection_id = Some(conn_id.to_string());

    let (conn, mut db_params, driver) =
        resolve_db_driver(conn_id, config.active_external_drivers.as_deref()).await?;
    audit.connection_name = Some(conn.name.clone());
    apply_database_override(&mut db_params, database_arg(args));

    let effective_schema = resolve_default_schema(&driver, schema);
    let tables = driver
        .get_tables(&db_params, effective_schema)
        .await
        .map_err(|e| JsonRpcError {
            code: -32000,
            message: e,
            data: None,
        })?;

    let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
    audit.rows = Some(names.len());
    tool_result(&json!(names), format)
}
