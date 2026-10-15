use async_trait::async_trait;
use serde_json::{json, Value};

use super::audit::{emit_audit, CallAudit};
use super::{
    database, safety, tools, truncate, Tool, ToolContext, ToolError, ToolMode, ToolOutput,
};
use crate::ai::provider::ToolDef;
use crate::ai_chat::types::ChatMode;
use crate::config::{DEFAULT_AI_AUDIT_ENABLED, DEFAULT_AI_AUDIT_MAX_ENTRIES};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToolKind {
    Connections,
    Databases,
    Tables,
    Describe,
    Sample,
    Query,
    Explain,
    Statement,
}

struct DatabaseTool(ToolKind);

#[derive(Default)]
pub struct ToolRegistry;

impl ToolRegistry {
    fn tools(&self) -> [DatabaseTool; 8] {
        use ToolKind::*;
        [
            Connections,
            Databases,
            Tables,
            Describe,
            Sample,
            Query,
            Explain,
            Statement,
        ]
        .map(DatabaseTool)
    }

    /// Definitions are connection-scoped; the model never selects a target ID.
    pub fn tools_for(&self, mode: ChatMode, context: &ToolContext) -> Vec<ToolDef> {
        self.tools()
            .iter()
            .filter(|tool| tool.available(&mode, context))
            .map(|tool| ToolDef {
                name: tool.name().into(),
                description: tool.description().into(),
                parameters: tool.input_schema(),
            })
            .collect()
    }

    pub async fn run(
        &self,
        name: &str,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolOutput, ToolError> {
        let tool = self
            .tools()
            .into_iter()
            .find(|tool| tool.name() == name)
            .ok_or(ToolError::NotAvailable)?;
        tool.run(context, arguments).await
    }
}

impl DatabaseTool {
    fn available(&self, mode: &ChatMode, context: &ToolContext) -> bool {
        (self.mode() != ToolMode::Write || (*mode == ChatMode::Agent && !context.is_readonly()))
            && (self.0 != ToolKind::Explain || context.driver.manifest().capabilities.explain)
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
        audit: &mut CallAudit,
    ) -> Result<ToolOutput, ToolError> {
        if !self.available(&context.mode, context) {
            return Err(ToolError::NotAvailable);
        }
        let schema = self.input_schema();
        let validator =
            jsonschema::JSONSchema::compile(&schema).map_err(|_| ToolError::InvalidArguments)?;
        if !validator.is_valid(&arguments) {
            return Err(ToolError::InvalidArguments);
        }
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(truncate::DEFAULT_ROW_LIMIT as u64)
            .min(truncate::MAX_ROW_LIMIT as u64) as u32;
        let driver = context.driver.as_ref();
        let params = &context.params;
        let schema = context.effective_schema();
        let table = arguments["table_name"].as_str().unwrap_or_default();
        let query = arguments["query"].as_str().unwrap_or_default();
        let result = match self.0 {
            ToolKind::Connections => {
                database::connection_summaries(std::slice::from_ref(&context.connection))
            }
            ToolKind::Databases => database::list_databases(driver, params)
                .await
                .map_err(ToolError::Database)?,
            ToolKind::Tables => database::list_tables(driver, params, schema)
                .await
                .map_err(ToolError::Database)?,
            ToolKind::Describe => database::describe_table(driver, params, table, schema)
                .await
                .map_err(ToolError::Database)?,
            ToolKind::Sample => return tools::sample_rows::run(context, table, limit, audit).await,
            ToolKind::Query => {
                audit.query = Some(query.into());
                audit.query_kind = Some(safety::classify_query_kind(query).into());
                safety::require_read(query)?;
                let result = database::execute(driver, params, query, limit, schema, true)
                    .await
                    .map_err(ToolError::Database)?;
                audit.rows = Some(result.rows.len());
                return Ok(truncate::query_result(result, limit));
            }
            ToolKind::Explain => return tools::explain_query::run(context, query, audit).await,
            ToolKind::Statement => {
                return tools::run_statement::run(context, query, limit, audit).await
            }
        };
        audit.rows = result.as_array().map(Vec::len);
        Ok(truncate::output(&result.to_string()))
    }
}

#[async_trait]
impl Tool for DatabaseTool {
    fn name(&self) -> &'static str {
        match self.0 {
            ToolKind::Connections => "list_connections",
            ToolKind::Databases => "list_databases",
            ToolKind::Tables => "list_tables",
            ToolKind::Describe => "describe_table",
            ToolKind::Sample => "sample_rows",
            ToolKind::Query => "run_query",
            ToolKind::Explain => "explain_query",
            ToolKind::Statement => "run_statement",
        }
    }

    fn description(&self) -> &'static str {
        match self.0 {
            ToolKind::Connections => "Describe the current connection without credentials.",
            ToolKind::Databases => "List databases available on the current connection.",
            ToolKind::Tables => "List tables in the current database and schema.",
            ToolKind::Describe => "Describe a table's columns, indexes and foreign keys in the current scope.",
            ToolKind::Sample => "Read a bounded sample from a table without supplying SQL.",
            ToolKind::Query => "Run one read-only SQL query in the current scope. Results are bounded and may be truncated.",
            ToolKind::Explain => "Plan one read-only query without ANALYZE or executing it.",
            ToolKind::Statement => "Propose one write or DDL statement. Execution always requires approval for this SQL and target.",
        }
    }

    fn input_schema(&self) -> Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();
        if matches!(self.0, ToolKind::Describe | ToolKind::Sample) {
            properties.insert(
                "table_name".into(),
                json!({"type":"string", "minLength":1, "maxLength":512}),
            );
            required.push("table_name");
        }
        if matches!(
            self.0,
            ToolKind::Query | ToolKind::Explain | ToolKind::Statement
        ) {
            properties.insert(
                "query".into(),
                json!({"type":"string", "minLength":1, "maxLength":131072}),
            );
            required.push("query");
        }
        if matches!(
            self.0,
            ToolKind::Sample | ToolKind::Query | ToolKind::Statement
        ) {
            properties.insert(
                "limit".into(),
                json!({"type":"integer", "minimum":1, "maximum":4294967295u64,
                "default":50, "description":"Row limit, capped at 1000."}),
            );
        }
        json!({ "type":"object", "properties":properties, "required":required, "additionalProperties":false })
    }

    fn mode(&self) -> ToolMode {
        if self.0 == ToolKind::Statement {
            ToolMode::Write
        } else {
            ToolMode::Read
        }
    }

    async fn run(&self, context: &ToolContext, arguments: Value) -> Result<ToolOutput, ToolError> {
        let start = std::time::Instant::now();
        let mut audit = CallAudit::for_tool(self.name());
        audit.connection_id = Some(context.connection.id.clone());
        audit.connection_name = Some(context.connection.name.clone());
        let result = self.execute(context, arguments, &mut audit).await;
        if let Err(error) = &result {
            audit.status = error.status().into();
            audit.error = Some(error.to_string());
        }
        if context
            .config
            .ai_audit_enabled
            .unwrap_or(DEFAULT_AI_AUDIT_ENABLED)
        {
            let hint = match context.origin {
                super::ToolOrigin::Mcp { .. } => crate::ai_activity::get_client_hint(),
                super::ToolOrigin::Chat { .. } => Some("Tabularis assistant".into()),
            };
            emit_audit(
                context.origin.session_id(),
                &hint,
                &audit,
                start,
                context
                    .config
                    .ai_audit_max_entries
                    .unwrap_or(DEFAULT_AI_AUDIT_MAX_ENTRIES) as usize,
            );
        }
        result
    }
}
