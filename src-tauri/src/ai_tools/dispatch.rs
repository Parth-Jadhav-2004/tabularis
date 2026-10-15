use super::audit::{emit_audit, CallAudit};
use super::output::{requested_output_format, require_args};
use super::tools::*;
use crate::ai_activity;
use crate::config::{
    self, AppConfig, DEFAULT_AI_AUDIT_ENABLED, DEFAULT_AI_AUDIT_MAX_ENTRIES,
    DEFAULT_AI_SESSION_GAP_MINUTES,
};
use crate::mcp::protocol::JsonRpcError;
use serde_json::Value;

pub(crate) async fn handle_call_tool(params: Option<Value>) -> Result<Value, JsonRpcError> {
    let start = std::time::Instant::now();
    let app_config = config::load_config_from_disk();
    let audit_enabled = app_config
        .ai_audit_enabled
        .unwrap_or(DEFAULT_AI_AUDIT_ENABLED);
    let max_entries = app_config
        .ai_audit_max_entries
        .unwrap_or(DEFAULT_AI_AUDIT_MAX_ENTRIES) as usize;
    let gap_minutes = app_config
        .ai_session_gap_minutes
        .unwrap_or(DEFAULT_AI_SESSION_GAP_MINUTES);

    let session_id = ai_activity::compute_or_rotate_session_id(gap_minutes);
    let client_hint = ai_activity::get_client_hint();

    let params_value = match params {
        Some(p) => p,
        None => {
            let err = JsonRpcError {
                code: -32602,
                message: "Missing params".to_string(),
                data: None,
            };
            if audit_enabled {
                let mut audit = CallAudit::for_tool("unknown");
                audit.status = "error".to_string();
                audit.error = Some(err.message.clone());
                emit_audit(&session_id, &client_hint, &audit, start, max_entries);
            }
            return Err(err);
        }
    };
    let name = params_value["name"].as_str().unwrap_or("").to_string();
    let args = params_value
        .get("arguments")
        .and_then(|v| v.as_object())
        .cloned();

    let mut audit = CallAudit::for_tool(&name);
    let result = dispatch_tool(&name, args.as_ref(), &app_config, &session_id, &mut audit).await;

    match &result {
        Ok(_) => {
            if audit.status == "success" {
                audit.status = "success".to_string();
            }
        }
        Err(e) => {
            // Inner code may have already set a more specific status (e.g.
            // blocked_readonly, denied, timeout). Only fall back to "error"
            // when nothing was set.
            if audit.status == "success" {
                audit.status = "error".to_string();
            }
            if audit.error.is_none() {
                audit.error = Some(e.message.clone());
            }
        }
    }

    if audit_enabled {
        emit_audit(&session_id, &client_hint, &audit, start, max_entries);
    }

    result
}

pub(crate) async fn dispatch_tool(
    name: &str,
    args: Option<&serde_json::Map<String, Value>>,
    config: &AppConfig,
    session_id: &str,
    audit: &mut CallAudit,
) -> Result<Value, JsonRpcError> {
    match name {
        "list_connections" => {
            let format = requested_output_format(args, config)?;
            tool_list_connections(audit, format).await
        }
        "list_databases" => {
            let args = require_args(args)?;
            let format = requested_output_format(Some(args), config)?;
            tool_list_databases(args, config, audit, format).await
        }
        "list_tables" => {
            let args = require_args(args)?;
            let format = requested_output_format(Some(args), config)?;
            tool_list_tables(args, config, audit, format).await
        }
        "describe_table" => {
            let args = require_args(args)?;
            let format = requested_output_format(Some(args), config)?;
            tool_describe_table(args, config, audit, format).await
        }
        "run_query" => {
            let args = require_args(args)?;
            let format = requested_output_format(Some(args), config)?;
            tool_run_query(args, config, session_id, audit, format).await
        }
        _ => Err(JsonRpcError {
            code: -32601,
            message: "Tool not found".to_string(),
            data: None,
        }),
    }
}
