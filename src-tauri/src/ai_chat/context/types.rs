use serde::Serialize;

use crate::ai_chat::types::ChatMode;
use crate::ai_schema_context::detailed::{DetailedSchemaContext, SchemaCatalog};

/// Resolved metadata only: connection credentials and URIs have no place here.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct ConnectionContext<'a> {
    pub driver: &'a str,
    pub dialect: &'a str,
    pub identifier_quote: &'a str,
    pub database: Option<&'a str>,
    pub schema: Option<&'a str>,
    pub server_version: Option<&'a str>,
    pub readonly: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EditorContext<'a> {
    pub query: Option<&'a str>,
    pub selection: Option<&'a str>,
    pub cursor_statement: Option<&'a str>,
}

pub struct ContextInput<'a> {
    pub mode: ChatMode,
    pub language: &'a str,
    pub connection: Option<ConnectionContext<'a>>,
    pub instructions: Option<&'a str>,
    pub catalog: Option<&'a SchemaCatalog>,
    pub detailed: Option<&'a DetailedSchemaContext>,
    pub editor: EditorContext<'a>,
    /// The caller includes this only for a relevant error or an explicit choice.
    pub last_error: Option<&'a str>,
}

#[derive(Clone, Copy, Debug)]
pub struct ContextBudgets {
    pub connection: usize,
    pub instructions: usize,
    pub detailed_tables: usize,
    pub schema_overview: usize,
    pub editor: usize,
    pub last_error: usize,
}

impl Default for ContextBudgets {
    fn default() -> Self {
        Self {
            connection: 512,
            instructions: 2_000,
            detailed_tables: 24_000,
            schema_overview: 12_000,
            editor: 8_000,
            last_error: 2_000,
        }
    }
}

impl ContextBudgets {
    pub fn total(&self) -> usize {
        self.connection
            .saturating_add(self.instructions)
            .saturating_add(self.detailed_tables)
            .saturating_add(self.schema_overview)
            .saturating_add(self.editor)
            .saturating_add(self.last_error)
    }
}

/// Names and actual rendered sizes; the text of a context layer is never copied
/// into the message metadata that will be persisted by the chat orchestration.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ContextItemSummary {
    pub kind: String,
    pub id: String,
    pub label: String,
    pub chars: usize,
    pub truncated: bool,
}

#[derive(Debug)]
pub struct BuiltContext {
    pub system: String,
    pub items: Vec<ContextItemSummary>,
    pub truncated: bool,
    pub schema_truncated: bool,
    pub base_chars: usize,
    pub context_chars: usize,
}
