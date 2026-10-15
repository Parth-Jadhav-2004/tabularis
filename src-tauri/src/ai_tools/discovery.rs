use super::output::{output_format_schema, ToolOutputFormat};
use crate::config::AppConfig;
use crate::mcp::protocol::{JsonRpcError, Tool};
use serde_json::{json, Value};

pub(crate) fn handle_list_tools(config: &AppConfig) -> Result<Value, JsonRpcError> {
    let default_output_format = ToolOutputFormat::from_config(config).as_str();
    let tools = vec![
        Tool {
            name: "list_connections".to_string(),
            description: Some("List all saved database connections".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "output_format": output_format_schema(default_output_format)
                }
            }),
        },
        Tool {
            name: "list_databases".to_string(),
            description: Some("List all databases available for a connection".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "connection_id": { "type": "string", "description": "The ID or name of the connection" },
                    "output_format": output_format_schema(default_output_format)
                },
                "required": ["connection_id"]
            }),
        },
        Tool {
            name: "list_tables".to_string(),
            description: Some("List all tables in a database connection".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "connection_id": { "type": "string", "description": "The ID or name of the connection" },
                    "schema": { "type": "string", "description": "Schema name (optional, defaults to 'public' for PostgreSQL)" },
                    "database": { "type": "string", "description": "Database to use on a connection with several databases selected (optional, defaults to the first one)" },
                    "output_format": output_format_schema(default_output_format)
                },
                "required": ["connection_id"]
            }),
        },
        Tool {
            name: "describe_table".to_string(),
            description: Some(
                "Get the full schema of a table: columns, indexes, and foreign keys".to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "connection_id": { "type": "string", "description": "The ID or name of the connection" },
                    "table_name": { "type": "string", "description": "The name of the table to describe" },
                    "schema": { "type": "string", "description": "Schema name (optional, defaults to 'public' for PostgreSQL)" },
                    "database": { "type": "string", "description": "Database to use on a connection with several databases selected (optional, defaults to the first one)" },
                    "output_format": output_format_schema(default_output_format)
                },
                "required": ["connection_id", "table_name"]
            }),
        },
        Tool {
            name: "run_query".to_string(),
            description: Some("Execute a SQL query on a specific connection. If the query already contains a LIMIT clause, it will be respected.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "connection_id": { "type": "string", "description": "The ID or name of the connection" },
                    "query": { "type": "string", "description": "The SQL query to execute" },
                    "database": { "type": "string", "description": "Database to use on a connection with several databases selected (optional, defaults to the first one)" },
                    "limit": { "type": "integer", "description": "Maximum number of rows to return (default: 100). If the query already contains a LIMIT clause smaller than this value, the query's LIMIT takes precedence." },
                    "output_format": output_format_schema(default_output_format)
                },
                "required": ["connection_id", "query"]
            }),
        },
    ];

    Ok(json!({
        "tools": tools
    }))
}
