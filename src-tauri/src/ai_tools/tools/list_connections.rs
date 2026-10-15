use crate::ai_tools::audit::CallAudit;
use crate::ai_tools::output::{tool_result, ToolOutputFormat};
use crate::mcp::protocol::JsonRpcError;
use crate::{paths, persistence};
use serde_json::{json, Value};

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

    let list: Vec<_> = connections
        .iter()
        .map(|c| {
            json!({
                "id": c.id,
                "name": c.name,
                "driver": c.params.driver,
                "host": c.params.host,
                "database": c.params.database.as_vec()
            })
        })
        .collect();

    tool_result(&json!(list), format)
}
