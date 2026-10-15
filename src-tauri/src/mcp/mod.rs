use crate::ai_activity;
use crate::ai_tools::{handle_call_tool, handle_list_tools, resolve_db_driver, resolve_default_schema};
use crate::config;
use crate::drivers::registry as driver_registry;
use crate::drivers::{mysql, postgres, sqlite};
use crate::{paths, persistence, plugins};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

pub mod install;
pub mod protocol;
pub use crate::ai_tools::preflight;
use protocol::*;

#[cfg(test)]
use crate::ai_tools::{apply_database_override, database_arg, dispatch_tool, requested_output_format, resolve_driver_for_params, CallAudit, ToolOutputFormat};
#[cfg(test)]
use crate::config::AppConfig;
#[cfg(test)]
use crate::drivers::driver_trait::DatabaseDriver;
#[cfg(test)]
use crate::models::{ConnectionParams, DatabaseSelection};
#[cfg(test)]
use std::sync::Arc;
#[cfg(test)]
mod tests;

/// Populate the driver registry for the standalone MCP subprocess: the three
/// built-in drivers plus any installed plugin drivers, honoring the user's
/// `active_external_drivers` preference. Without this, MCP can only reach
/// mysql/postgres/sqlite connections — every other driver fails with
/// "Unsupported driver".
async fn register_drivers_for_mcp() {
    driver_registry::register_driver(mysql::MysqlDriver::new()).await;
    driver_registry::register_driver(postgres::PostgresDriver::new()).await;
    driver_registry::register_driver(sqlite::SqliteDriver::new()).await;

    plugins::manager::reload_plugins_from_disk_config().await;

    // Must run after driver registration above — the migration resolves each
    // connection's driver dialect via the same registry, so it needs plugin
    // drivers already loaded. This is the standalone MCP process's own entry
    // point for a migration that previously only ran from the GUI process's
    // Tauri commands (issue #639) — never blocks server startup on failure.
    let conn_path = paths::resolve_connections_path(&paths::get_app_config_dir());
    crate::connection_migrations::migrate_postgres_ssl_mode_spelling_at_path(&conn_path)
        .await
        .ok();
}

pub async fn run_mcp_server() {
    eprintln!("[MCP] Starting Tabularis MCP Server...");

    let storage = tokio::task::spawn_blocking(|| {
        crate::secure_store::migration::migrate_in(&paths::get_app_config_dir())?;
        crate::secure_store::ensure_available()
    }).await;
    if !matches!(storage, Ok(Ok(()))) {
        eprintln!("[MCP] AI storage is unavailable. Unlock the keychain or recover AI storage in Settings.");
        return;
    }

    register_drivers_for_mcp().await;

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut iterator = stdin.lock().lines();

    while let Some(line_result) = iterator.next() {
        match line_result {
            Ok(line) => {
                if line.trim().is_empty() {
                    continue;
                }


                match serde_json::from_str::<JsonRpcRequest>(&line) {
                    Ok(request) => {
                        let response = handle_request(request).await;
                        if let Some(resp) = response {
                            let json = serde_json::to_string(&resp)
                                .expect("serializing a JsonRpcResponse cannot fail");
                            // Stop cleanly if the client has gone away (BrokenPipe)
                            // rather than panicking the whole server.
                            if let Err(e) = stdout
                                .write_all(json.as_bytes())
                                .and_then(|_| stdout.write_all(b"\n"))
                                .and_then(|_| stdout.flush())
                            {
                                eprintln!("[MCP] Failed to write response, stopping: {}", e);
                                break;
                            }
                        }
                    }
                    Err(_) => {
                        eprintln!("[MCP] Received invalid JSON-RPC");
                    }
                }
            }
            Err(e) => {
                eprintln!("[MCP] Error reading stdin: {}", e);
                break;
            }
        }
    }
}

async fn handle_request(req: JsonRpcRequest) -> Option<JsonRpcResponse> {
    // Notifications (no id)
    if req.id.is_none() {
        if req.method == "notifications/initialized" {
            eprintln!("[MCP] Client initialized.");
        }
        return None;
    }

    let id = req.id.clone();
    let result = match req.method.as_str() {
        "initialize" => handle_initialize(req.params),
        "resources/list" => handle_list_resources().await,
        "resources/read" => handle_read_resource(req.params).await,
        "tools/list" => handle_list_tools(&config::load_config_from_disk()),
        "tools/call" => handle_call_tool(req.params).await,
        _ => Err(JsonRpcError {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
        }),
    };

    let (res, err) = match result {
        Ok(val) => (Some(val), None),
        Err(e) => (None, Some(e)),
    };

    Some(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: res,
        error: err,
    })
}

fn handle_initialize(params: Option<Value>) -> Result<Value, JsonRpcError> {
    // Capture client name for the audit log session metadata. Best-effort:
    // if parsing fails we still complete the handshake.
    if let Some(p) = &params {
        if let Some(name) = p
            .get("clientInfo")
            .and_then(|c| c.get("name"))
            .and_then(|v| v.as_str())
        {
            ai_activity::set_client_hint(Some(name.to_string()));
        }
    }

    let result = InitializeResult {
        protocol_version: "2024-11-05".to_string(),
        capabilities: ServerCapabilities {
            resources: Some(json!({ "listChanged": false })),
            tools: Some(json!({ "listChanged": false })),
            prompts: None,
        },
        server_info: ServerInfo {
            name: "tabularis-mcp".to_string(),
            version: "0.1.0".to_string(),
        },
    };
    Ok(serde_json::to_value(result).unwrap())
}

async fn handle_list_resources() -> Result<Value, JsonRpcError> {
    let config_path = paths::resolve_connections_path(&paths::get_app_config_dir());
    let connections = persistence::load_connections(&config_path).map_err(|e| JsonRpcError {
        code: -32000,
        message: format!("Failed to load connections: {}", e),
        data: None,
    })?;

    let mut resources = Vec::new();

    // Add connection list resource
    resources.push(Resource {
        uri: "tabularis://connections".to_string(),
        name: "Connections List".to_string(),
        description: Some("List of all configured database connections".to_string()),
        mime_type: Some("application/json".to_string()),
    });

    // Add schema resources for each connection
    for conn in connections {
        resources.push(Resource {
            uri: format!("tabularis://{}/schema", conn.id),
            name: format!("Schema: {}", conn.name),
            description: Some(format!("Database schema for {}", conn.name)),
            mime_type: Some("application/json".to_string()),
        });
    }

    Ok(json!({
        "resources": resources
    }))
}

async fn handle_read_resource(params: Option<Value>) -> Result<Value, JsonRpcError> {
    let params = params.ok_or(JsonRpcError {
        code: -32602,
        message: "Missing params".to_string(),
        data: None,
    })?;

    let uri = params["uri"].as_str().ok_or(JsonRpcError {
        code: -32602,
        message: "Missing uri".to_string(),
        data: None,
    })?;

    if uri == "tabularis://connections" {
        let config_path = paths::resolve_connections_path(&paths::get_app_config_dir());
        let connections =
            persistence::load_connections(&config_path).map_err(|e| JsonRpcError {
                code: -32000,
                message: e,
                data: None,
            })?;

        let safe_list: Vec<_> = connections
            .iter()
            .map(|c| {
                json!({
                    "id": c.id,
                    "name": c.name,
                    "driver": c.params.driver,
                    "host": c.params.host,
                    "database": c.params.database
                })
            })
            .collect();

        return Ok(json!({
            "contents": [{
                "uri": uri,
                "mime_type": "application/json",
                "text": serde_json::to_string_pretty(&safe_list).unwrap()
            }]
        }));
    }

    if uri.starts_with("tabularis://") && uri.ends_with("/schema") {
        let parts: Vec<&str> = uri.split('/').collect();
        // uri format: tabularis://{id}/schema -> ["tabularis:", "", "{id}", "schema"]
        if parts.len() < 4 {
            return Err(JsonRpcError {
                code: -32602,
                message: "Invalid URI format".to_string(),
                data: None,
            });
        }
        let conn_id = parts[2];

        // Resolve through the same path as the tools so keychain passwords and
        // SSH tunnels are applied — not just the raw saved params.
        let active_ids = config::load_config_from_disk().active_external_drivers;
        let (_conn, params, driver) = resolve_db_driver(conn_id, active_ids.as_deref()).await?;
        let schema = resolve_default_schema(&driver, None);
        let tables = driver
            .get_tables(&params, schema)
            .await
            .map_err(|e| JsonRpcError {
                code: -32000,
                message: e,
                data: None,
            })?;

        // Format as simplified DDL or JSON
        let schema_json = serde_json::to_string_pretty(&tables).unwrap();

        return Ok(json!({
            "contents": [{
                "uri": uri,
                "mime_type": "application/json",
                "text": schema_json
            }]
        }));
    }

    Err(JsonRpcError {
        code: -32602,
        message: "Resource not found".to_string(),
        data: None,
    })
}

