use std::sync::Arc;

use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;

use super::approval::ApprovalGate;
use super::connections::{apply_database_override, resolve_db_driver, resolve_default_schema};
use crate::ai_chat::types::ChatMode;
use crate::config::{self, AppConfig};
use crate::drivers::driver_trait::DatabaseDriver;
use crate::models::{ConnectionParams, SavedConnection};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolMode {
    Read,
    Write,
}

#[derive(Clone, Debug)]
pub enum ToolOrigin {
    Mcp {
        session_id: String,
    },
    Chat {
        conversation_id: String,
        run_id: String,
    },
}

impl ToolOrigin {
    pub(crate) fn session_id(&self) -> &str {
        match self {
            Self::Mcp { session_id } => session_id,
            Self::Chat {
                conversation_id, ..
            } => conversation_id,
        }
    }
}

/// Constructed by Rust orchestration, never deserialized from model arguments.
/// The resolved connection and scope stay fixed for the duration of a call.
pub struct ToolContext {
    pub(crate) connection: SavedConnection,
    pub(crate) params: ConnectionParams,
    pub(crate) driver: Arc<dyn DatabaseDriver>,
    pub(crate) config: AppConfig,
    pub(crate) database: Option<String>,
    pub(crate) schema: Option<String>,
    pub(crate) readonly: bool,
    pub(crate) mode: ChatMode,
    pub(crate) origin: ToolOrigin,
    pub(crate) approval: Arc<dyn ApprovalGate>,
}

impl ToolContext {
    pub async fn resolve(
        connection_id: &str,
        database: Option<String>,
        schema: Option<String>,
        mode: ChatMode,
        origin: ToolOrigin,
        approval: Arc<dyn ApprovalGate>,
    ) -> Result<Self, ToolError> {
        let config = config::load_config_from_disk();
        let (connection, mut params, driver) =
            resolve_db_driver(connection_id, config.active_external_drivers.as_deref()).await?;
        apply_database_override(&mut params, database.as_deref());
        let readonly = config::is_connection_readonly(&config, &connection.id)
            || driver.manifest().capabilities.readonly;
        Ok(Self {
            connection,
            params,
            driver,
            config,
            database,
            schema,
            readonly,
            mode,
            origin,
            approval,
        })
    }

    pub fn connection_id(&self) -> &str {
        &self.connection.id
    }

    pub(crate) fn effective_schema(&self) -> Option<&str> {
        resolve_default_schema(&self.driver, self.schema.as_deref())
    }

    pub(crate) fn is_readonly(&self) -> bool {
        self.readonly
            || self.driver.manifest().capabilities.readonly
            || config::is_connection_readonly(&self.config, &self.connection.id)
    }
}

#[derive(Debug, Serialize, PartialEq)]
pub struct RowsPreview {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    pub affected_rows: u64,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct ToolOutput {
    pub content: String,
    pub truncated: bool,
    pub rows: Option<RowsPreview>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ToolError {
    InvalidArguments,
    NotAvailable,
    Safety(super::safety::SafetyError),
    Database(String),
    Approval {
        status: &'static str,
        message: String,
    },
}

impl ToolError {
    pub(crate) fn status(&self) -> &'static str {
        match self {
            Self::Safety(super::safety::SafetyError::ConnectionReadOnly) => "blocked_readonly",
            Self::Approval { status, .. } => status,
            _ => "error",
        }
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidArguments => f.write_str("Invalid database tool arguments."),
            Self::NotAvailable => f.write_str("This tool is not available in the current mode."),
            Self::Safety(error) => error.fmt(f),
            Self::Database(message) | Self::Approval { message, .. } => f.write_str(message),
        }
    }
}

impl std::error::Error for ToolError {}

impl From<super::safety::SafetyError> for ToolError {
    fn from(error: super::safety::SafetyError) -> Self {
        Self::Safety(error)
    }
}

impl From<crate::mcp::protocol::JsonRpcError> for ToolError {
    fn from(error: crate::mcp::protocol::JsonRpcError) -> Self {
        Self::Database(error.message)
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn input_schema(&self) -> Value;
    fn mode(&self) -> ToolMode;
    async fn run(&self, context: &ToolContext, arguments: Value) -> Result<ToolOutput, ToolError>;
}
