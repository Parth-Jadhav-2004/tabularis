use crate::ai_tools::audit::CallAudit;
use crate::ai_tools::output::{tool_result, ToolOutputFormat};
use crate::mcp::protocol::JsonRpcError;
use crate::{paths, persistence};
use serde_json::Value;

pub(crate) async fn tool_list_connections(
    _audit: &mut CallAudit,
    format: ToolOutputFormat,
) -> Result<Value, JsonRpcError> {
    let config_path = paths::resolve_connections_path(&paths::get_app_config_dir());
    let connections = persistence::load_connections(&config_path).map_err(|e| JsonRpcError {
        code: -32000,
        message: e,
        data: None,
    })?;

    tool_result(
        &crate::ai_tools::database::connection_summaries(&connections),
        format,
    )
}
