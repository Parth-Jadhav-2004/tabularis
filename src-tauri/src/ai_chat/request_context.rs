//! Resolve user-selected context in Rust. Model history cannot choose a driver,
//! credentials, mode or tools, and only metadata summaries go back over IPC.
mod spec;
use spec::context_error;
pub use spec::ContextSpec;

use super::context::{
    build_context, references::extract_referenced_tables, BuiltContext, ConnectionContext,
    ContextBudgets, ContextInput, EditorContext,
};
use super::schema_cache::{SchemaCache, ScopeKey};
use super::types::ChatMode;
use crate::ai::message::{Block, ContextItemRef, Role};
use crate::ai::provider::{ChatRequest, ProviderError};
use crate::ai_schema_context::detailed::{load_catalog, load_detailed_from_driver};
use crate::drivers::driver_trait::{DatabaseDriver, SqlDialect};
use crate::models::{ConnectionParams, DatabaseSelection};
use serde::Serialize;
use std::collections::HashSet;
use tauri::{AppHandle, Runtime};

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ContextEvent {
    Context {
        message_id: String,
        items: Vec<ContextItemRef>,
    },
}

fn dialect_name(dialect: Option<SqlDialect>) -> &'static str {
    match dialect {
        Some(SqlDialect::Postgres) => "postgres",
        Some(SqlDialect::Mysql) => "mysql",
        Some(SqlDialect::Mssql) => "mssql",
        Some(SqlDialect::Sqlite) => "sqlite",
        Some(SqlDialect::Oracle) => "oracle",
        _ => "generic",
    }
}

pub(crate) async fn resolve<R: Runtime>(
    app: &AppHandle<R>,
    spec: &ContextSpec,
) -> Result<BuiltContext, ProviderError> {
    spec.validate()?;
    let connection = crate::commands::find_connection_by_id(app, &spec.connection_id)
        .map_err(|_| context_error())?;
    let mut params = connection.params;
    if let Some(database) = &spec.database {
        params.database = DatabaseSelection::Single(database.clone());
    }
    let needs_metadata = spec.include.schema || !spec.tables.is_empty();
    let driver = if needs_metadata {
        let expanded = crate::commands::expand_ssh_connection_params(app, &params)
            .await
            .map_err(|_| context_error())?;
        let expanded = crate::commands::expand_k8s_connection_params(app, &expanded)
            .await
            .map_err(|_| context_error())?;
        params = crate::commands::resolve_connection_params_with_id(&expanded, &spec.connection_id)
            .map_err(|_| context_error())?;
        crate::drivers::registry::get_connection_driver(&params)
            .await
            .map_err(|_| context_error())?
    } else {
        crate::drivers::registry::get_driver(&params.driver)
            .await
            .ok_or_else(context_error)?
    };
    let config = crate::config::load_config_internal(app);
    let readonly = crate::config::is_connection_readonly(&config, &spec.connection_id)
        || driver.manifest().capabilities.readonly;
    resolve_with_driver(
        spec,
        &params,
        driver.as_ref(),
        readonly,
        super::schema_cache::shared(),
    )
    .await
}

async fn resolve_with_driver(
    spec: &ContextSpec,
    params: &ConnectionParams,
    driver: &dyn DatabaseDriver,
    readonly: bool,
    cache: &SchemaCache,
) -> Result<BuiltContext, ProviderError> {
    spec.validate()?;
    let capabilities = &driver.manifest().capabilities;
    let schema =
        spec.schema
            .as_deref()
            .or(if capabilities.sql_dialect == Some(SqlDialect::Postgres) {
                Some("public")
            } else {
                None
            });
    let database = spec.database.as_deref().or_else(|| {
        let primary = params.database.primary();
        (!primary.is_empty()).then_some(primary)
    });
    let scope = cache.scope(ScopeKey {
        connection_id: spec.connection_id.clone(),
        database: database.map(str::to_string),
        schema: schema.map(str::to_string),
        driver: params.driver.clone(),
    });
    let catalog = if spec.include.schema || !spec.tables.is_empty() {
        Some(
            scope
                .catalog(|| async {
                    load_catalog(driver, params, schema)
                        .await
                        .map_err(|_| context_error())
                })
                .await?,
        )
    } else {
        None
    };
    let query = spec
        .include
        .query
        .then_some(spec.editor.query.as_deref())
        .flatten();
    let selected = if let Some(catalog) = &catalog {
        let references = if spec.include.schema {
            extract_referenced_tables(query.unwrap_or_default(), &catalog.tables, database, schema)
                .tables
        } else {
            Vec::new()
        };
        let mut seen = HashSet::new();
        spec.tables
            .iter()
            .chain(&references)
            .filter(|name| !spec.excluded_tables.contains(name) && seen.insert((*name).clone()))
            .take(100)
            .cloned()
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let detailed = if let Some(catalog) = &catalog {
        Some(
            scope
                .details(&selected, || {
                    load_detailed_from_driver(
                        driver,
                        params,
                        schema,
                        catalog,
                        &selected,
                        crate::ai_schema_context::DEFAULT_MAX_TABLES,
                    )
                })
                .await,
        )
    } else {
        None
    };
    Ok(build_context(
        &ContextInput {
            mode: ChatMode::Ask,
            language: &spec.language,
            connection: spec.include.connection.then_some(ConnectionContext {
                driver: &params.driver,
                dialect: dialect_name(capabilities.sql_dialect),
                identifier_quote: &capabilities.identifier_quote,
                database,
                schema,
                server_version: None,
                readonly,
            }),
            instructions: None,
            catalog: if spec.include.schema {
                catalog.as_deref()
            } else {
                None
            },
            detailed: detailed.as_deref(),
            editor: EditorContext {
                query,
                selection: spec
                    .include
                    .selection
                    .then_some(spec.editor.selection.as_deref())
                    .flatten(),
                cursor_statement: spec
                    .include
                    .cursor_statement
                    .then_some(spec.editor.cursor_statement.as_deref())
                    .flatten(),
            },
            last_error: if spec.include.error {
                spec.last_error.as_deref()
            } else {
                None
            },
        },
        ContextBudgets::default(),
    ))
}

/// The latest user message receives only metadata; all provider encoders omit
/// context blocks. Replacing any client copy keeps Rust authoritative.
pub(crate) fn apply(
    req: &mut ChatRequest,
    built: BuiltContext,
) -> Result<ContextEvent, ProviderError> {
    let last = req
        .messages
        .last()
        .filter(|message| message.role == Role::User)
        .ok_or_else(context_error)?;
    if req
        .messages
        .iter()
        .filter(|message| message.id == last.id)
        .count()
        != 1
    {
        return Err(context_error());
    }
    let message_id = last.id.clone();
    let items = built
        .items
        .into_iter()
        .map(|item| ContextItemRef {
            kind: item.kind,
            id: item.id,
            label: item.label,
            chars: Some(item.chars),
            truncated: item.truncated,
        })
        .collect::<Vec<_>>();
    let last = req.messages.last_mut().unwrap();
    last.blocks
        .retain(|block| !matches!(block, Block::Context { .. }));
    last.blocks.push(Block::Context {
        items: items.clone(),
    });
    req.system = built.system;
    // M9 will advertise tools from a Rust-resolved mode and approval context.
    req.tools.clear();
    Ok(ContextEvent::Context { message_id, items })
}

#[cfg(test)]
mod tests;
