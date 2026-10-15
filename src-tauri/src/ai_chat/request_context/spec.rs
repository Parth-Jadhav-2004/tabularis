use crate::ai::provider::{ErrorKind, ProviderError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ContextSelection {
    pub connection: bool,
    pub instructions: bool,
    pub schema: bool,
    pub query: bool,
    pub selection: bool,
    pub cursor_statement: bool,
    pub error: bool,
}

impl Default for ContextSelection {
    fn default() -> Self {
        Self {
            connection: true,
            instructions: true,
            schema: true,
            query: true,
            selection: true,
            cursor_statement: true,
            error: false,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct EditorSnapshot {
    pub query: Option<String>,
    pub selection: Option<String>,
    pub cursor_statement: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextSpec {
    #[serde(default)]
    pub command: Option<super::super::slash::ChatCommand>,
    pub connection_id: String,
    pub database: Option<String>,
    pub schema: Option<String>,
    pub language: String,
    #[serde(default)]
    pub(crate) include: ContextSelection,
    #[serde(default)]
    pub tables: Vec<String>,
    #[serde(default)]
    pub excluded_tables: Vec<String>,
    #[serde(default)]
    pub views: Vec<String>,
    #[serde(default)]
    pub excluded_views: Vec<String>,
    #[serde(default)]
    pub saved_queries: Vec<String>,
    #[serde(default)]
    pub(crate) editor: EditorSnapshot,
    #[serde(default)]
    pub last_error: Option<String>,
}

pub(super) fn context_error() -> ProviderError {
    ProviderError::new(ErrorKind::Other, "Could not prepare the AI context. Check the connection or reduce the selected context and try again.")
}

impl ContextSpec {
    pub(super) fn validate(&self) -> Result<(), ProviderError> {
        let valid_name = |name: &str| !name.is_empty() && name.len() <= 512 && !name.contains('\0');
        if !valid_name(&self.connection_id)
            || self
                .database
                .as_deref()
                .is_some_and(|name| !valid_name(name))
            || self.schema.as_deref().is_some_and(|name| !valid_name(name))
            || self.language.len() > 32
            || self.tables.len() > 100
            || self.excluded_tables.len() > 100
            || self.views.len() > 100
            || self.tables.len().saturating_add(self.views.len()) > 100
            || self.excluded_views.len() > 100
            || self.saved_queries.len() > 20
            || self
                .saved_queries
                .iter()
                .any(|id| !uuid::Uuid::parse_str(id).is_ok_and(|parsed| parsed.to_string() == *id))
            || self
                .tables
                .iter()
                .chain(&self.excluded_tables)
                .chain(&self.views)
                .chain(&self.excluded_views)
                .any(|name| !valid_name(name))
            || [
                &self.editor.query,
                &self.editor.selection,
                &self.editor.cursor_statement,
            ]
            .into_iter()
            .flatten()
            .any(|text| text.len() > 1_048_576)
            || self
                .last_error
                .as_ref()
                .is_some_and(|text| text.len() > 65_536)
        {
            return Err(context_error());
        }
        Ok(())
    }
}
