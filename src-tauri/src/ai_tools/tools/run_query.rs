use crate::ai_activity;
use crate::ai_approval::{self, PendingApproval, PollOutcome};
use crate::ai_tools::audit::CallAudit;
use crate::ai_tools::connections::{apply_database_override, database_arg, resolve_db_driver};
use crate::ai_tools::output::{tool_result, ToolOutputFormat};
use crate::ai_tools::preflight;
use crate::config::{
    self, AppConfig, DEFAULT_MCP_APPROVAL_MODE, DEFAULT_MCP_APPROVAL_TIMEOUT_SECONDS,
    DEFAULT_MCP_PREFLIGHT_EXPLAIN,
};
use crate::heartbeat;
use crate::mcp::protocol::JsonRpcError;
use serde_json::Value;

const APPROVAL_POLL_INTERVAL_MS: u64 = 500;

pub(crate) async fn tool_run_query(
    args: &serde_json::Map<String, Value>,
    config: &AppConfig,
    session_id: &str,
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
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or(JsonRpcError {
            code: -32602,
            message: "Missing query".to_string(),
            data: None,
        })?;

    let max_rows = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as u32;

    audit.connection_id = Some(conn_id.to_string());
    audit.query = Some(query.to_string());
    let kind = ai_activity::classify_query_kind(query);
    audit.query_kind = Some(kind.to_string());

    let (conn, mut db_params, driver) =
        resolve_db_driver(conn_id, config.active_external_drivers.as_deref()).await?;
    audit.connection_name = Some(conn.name.clone());
    let database = database_arg(args);
    apply_database_override(&mut db_params, database);

    // Read-only enforcement (fail-closed: unknown counts as write).
    if config::is_connection_readonly(config, &conn.id) && kind != "select" {
        audit.status = "blocked_readonly".to_string();
        let msg = "Query blocked by Tabularis read-only mode. Enable writes for this connection in Settings → MCP → Read-only mode.".to_string();
        audit.error = Some(msg.clone());
        return Err(JsonRpcError {
            code: -32000,
            message: msg,
            data: None,
        });
    }

    // Approval gate
    let approval_mode = config
        .mcp_approval_mode
        .clone()
        .unwrap_or_else(|| DEFAULT_MCP_APPROVAL_MODE.to_string());
    let needs_approval = match approval_mode.as_str() {
        "off" => false,
        "all" => true,
        // writes_only — anything that isn't a clean select.
        _ => kind != "select",
    };

    let mut effective_query: String = query.to_string();

    if needs_approval {
        // Fail fast if the GUI is not running — otherwise we'd queue a
        // pending approval that nobody can approve and wait the full
        // `mcp_approval_timeout_seconds` (default 120s) for nothing.
        if !heartbeat::is_alive() {
            audit.status = "host_unavailable".to_string();
            let msg = "Tabularis app is not running — open it to approve writes".to_string();
            audit.error = Some(msg.clone());
            return Err(JsonRpcError {
                code: -32000,
                message: msg,
                data: None,
            });
        }

        let timeout_secs = config
            .mcp_approval_timeout_seconds
            .unwrap_or(DEFAULT_MCP_APPROVAL_TIMEOUT_SECONDS) as u64;
        let want_explain = config
            .mcp_preflight_explain
            .unwrap_or(DEFAULT_MCP_PREFLIGHT_EXPLAIN);

        // Pre-flight EXPLAIN — best effort, never blocks.
        let (explain_plan, explain_error) = if want_explain {
            let outcome =
                preflight::preflight_explain(&conn.params.driver, &db_params, query, None).await;
            (outcome.plan, outcome.error)
        } else {
            (None, None)
        };

        let approval_id = ai_approval::new_approval_id();
        audit.approval_id = Some(approval_id.clone());

        let pending = PendingApproval {
            id: approval_id.clone(),
            created_at: ai_activity::now_iso8601(),
            session_id: session_id.to_string(),
            connection_id: conn.id.clone(),
            connection_name: conn.name.clone(),
            database: database.map(str::to_string),
            query: query.to_string(),
            query_kind: kind.to_string(),
            client_hint: ai_activity::get_client_hint(),
            explain_plan,
            explain_error,
        };

        if let Err(e) = ai_approval::write_pending(&pending) {
            audit.status = "error".to_string();
            let msg = format!("Failed to enqueue approval request: {}", e);
            audit.error = Some(msg.clone());
            return Err(JsonRpcError {
                code: -32000,
                message: msg,
                data: None,
            });
        }

        match ai_approval::poll_decision_with_liveness(
            &approval_id,
            timeout_secs,
            APPROVAL_POLL_INTERVAL_MS,
            heartbeat::is_alive,
        )
        .await
        {
            Ok(PollOutcome::Decided(decision)) => {
                if decision.decision == "approve" {
                    if let Some(edited) = decision
                        .edited_query
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        // Re-classify and re-enforce read-only on the edited
                        // query. Without this, an approver can flip a benign
                        // SELECT into a DELETE/DROP (or a multi-statement
                        // payload) and slip past the gate that runs against
                        // the *original* query above.
                        effective_query = edited.to_string();
                        let new_kind = ai_activity::classify_query_kind(&effective_query);
                        audit.query = Some(effective_query.clone());
                        audit.query_kind = Some(new_kind.to_string());

                        if config::is_connection_readonly(config, &conn.id) && new_kind != "select"
                        {
                            audit.status = "blocked_readonly".to_string();
                            let msg = "Edited query blocked by Tabularis read-only mode. Enable writes for this connection in Settings → MCP → Read-only mode.".to_string();
                            audit.error = Some(msg.clone());
                            return Err(JsonRpcError {
                                code: -32000,
                                message: msg,
                                data: None,
                            });
                        }
                    }
                } else {
                    let reason = decision
                        .reason
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(|r| format!(": {}", r))
                        .unwrap_or_default();
                    audit.status = "denied".to_string();
                    let msg = format!("Query denied by user{}", reason);
                    audit.error = Some(msg.clone());
                    return Err(JsonRpcError {
                        code: -32000,
                        message: msg,
                        data: None,
                    });
                }
            }
            Ok(PollOutcome::TimedOut) => {
                audit.status = "timeout".to_string();
                let msg = format!(
                    "Approval timed out after {}s — open Tabularis to approve writes",
                    timeout_secs
                );
                audit.error = Some(msg.clone());
                return Err(JsonRpcError {
                    code: -32000,
                    message: msg,
                    data: None,
                });
            }
            Ok(PollOutcome::HostUnavailable) => {
                audit.status = "host_unavailable".to_string();
                let msg =
                    "Tabularis app closed during approval — open it to approve writes".to_string();
                audit.error = Some(msg.clone());
                return Err(JsonRpcError {
                    code: -32000,
                    message: msg,
                    data: None,
                });
            }
            Err(e) => {
                audit.status = "error".to_string();
                audit.error = Some(e.clone());
                return Err(JsonRpcError {
                    code: -32000,
                    message: e,
                    data: None,
                });
            }
        }
    }

    // A query that reached this point without the user approving it is only
    // here because it was classified as a read, and so is anything sent to a
    // read-only connection. Classification cannot see what a function call
    // does, so have the database refuse writes too.
    let run_read_only = config::is_connection_readonly(config, &conn.id)
        || (approval_mode != "off" && !needs_approval);
    let result = if run_read_only {
        driver
            .execute_read_only_query(&db_params, &effective_query, Some(max_rows), 1, None)
            .await
    } else {
        driver
            .execute_query(&db_params, &effective_query, Some(max_rows), 1, None)
            .await
    }
    .map_err(|e| JsonRpcError {
        code: -32000,
        message: e,
        data: None,
    })?;

    audit.rows = Some(result.rows.len());

    tool_result(&result, format)
}
