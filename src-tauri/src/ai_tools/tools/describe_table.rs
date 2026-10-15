use crate::ai_tools::audit::CallAudit;
use crate::ai_tools::connections::{
    apply_database_override, database_arg, resolve_db_driver, resolve_default_schema,
};
use crate::ai_tools::output::{tool_result, ToolOutputFormat};
use crate::config::AppConfig;
use crate::mcp::protocol::JsonRpcError;
use serde_json::Value;

pub(crate) async fn tool_describe_table(
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
    let table_name = args
        .get("table_name")
        .and_then(|v| v.as_str())
        .ok_or(JsonRpcError {
            code: -32602,
            message: "Missing table_name".to_string(),
            data: None,
        })?;
    let schema = args.get("schema").and_then(|v| v.as_str());

    audit.connection_id = Some(conn_id.to_string());

    let (conn, mut db_params, driver) =
        resolve_db_driver(conn_id, config.active_external_drivers.as_deref()).await?;
    audit.connection_name = Some(conn.name.clone());
    apply_database_override(&mut db_params, database_arg(args));

    let effective_schema = resolve_default_schema(&driver, schema);
    let result = crate::ai_tools::database::describe_table(
        driver.as_ref(),
        &db_params,
        table_name,
        effective_schema,
    )
    .await
    .map_err(|message| JsonRpcError {
        code: -32000,
        message,
        data: None,
    })?;
    tool_result(&result, format)
}
