//! Chat metadata alongside the stable schema format used by legacy commands.
use futures::{stream, FutureExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::drivers::driver_trait::DatabaseDriver;
use crate::models::{ConnectionParams, ForeignKey, Index, TableColumn, TableInfo};

const MAX_CATALOG_TABLES: usize = 10_000;
const MAX_DETAILED_TABLES: usize = 100;
const METADATA_CONCURRENCY: usize = 4;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SchemaCatalog {
    pub tables: Vec<TableInfo>,
    pub total_table_count: usize,
    pub truncated: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DetailedTable {
    pub name: String,
    pub comment: Option<String>,
    pub columns: Vec<TableColumn>,
    pub foreign_keys: Vec<ForeignKey>,
    pub indexes: Vec<Index>,
    /// A failed optional metadata call is different from an empty result.
    pub unavailable: Vec<MetadataKind>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetadataKind {
    Columns,
    ForeignKeys,
    Indexes,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DetailedSchemaContext {
    pub tables: Vec<DetailedTable>,
    pub missing_tables: Vec<String>,
    pub omitted_tables: Vec<String>,
    pub truncated: bool,
}

/// Retain a bounded catalog in deterministic order, without loading columns.
pub async fn load_catalog<D: DatabaseDriver + ?Sized>(
    driver: &D,
    params: &ConnectionParams,
    schema: Option<&str>,
) -> Result<SchemaCatalog, String> {
    let mut tables = driver.get_tables(params, schema).await?;
    tables.sort_by(|left, right| left.name.cmp(&right.name));
    tables.dedup_by(|left, right| left.name == right.name);
    let total_table_count = tables.len();
    tables.truncate(MAX_CATALOG_TABLES);
    Ok(SchemaCatalog {
        truncated: total_table_count > tables.len(),
        total_table_count,
        tables,
    })
}

/// Load explicitly selected, catalog-validated names in priority order. No rows
/// are read, and a failed metadata call never exposes a raw database error.
pub async fn load_detailed_from_driver<D: DatabaseDriver + ?Sized>(
    driver: &D,
    params: &ConnectionParams,
    schema: Option<&str>,
    catalog: &SchemaCatalog,
    requested_tables: &[String],
    max_tables: usize,
) -> DetailedSchemaContext {
    let mut context = DetailedSchemaContext::default();
    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    let limit = max_tables.min(MAX_DETAILED_TABLES);
    for name in requested_tables {
        if !seen.insert(name.as_str()) {
            continue;
        }
        match catalog.tables.iter().find(|table| table.name == *name) {
            None => context.missing_tables.push(name.clone()),
            Some(_) if selected.len() >= limit => context.omitted_tables.push(name.clone()),
            Some(table) => selected.push(table),
        }
    }
    context.truncated = !context.omitted_tables.is_empty();
    let mut loads = Vec::new();
    for table in selected {
        loads.push(
            async move {
                let (columns, foreign_keys, indexes) = tokio::join!(
                    driver.get_columns(params, &table.name, schema),
                    driver.get_foreign_keys(params, &table.name, schema),
                    driver.get_indexes(params, &table.name, schema),
                );
                let mut unavailable = Vec::new();
                if columns.is_err() {
                    unavailable.push(MetadataKind::Columns);
                }
                if foreign_keys.is_err() {
                    unavailable.push(MetadataKind::ForeignKeys);
                }
                if indexes.is_err() {
                    unavailable.push(MetadataKind::Indexes);
                }
                DetailedTable {
                    name: table.name.clone(),
                    comment: table.comment.clone(),
                    columns: columns.unwrap_or_default(),
                    foreign_keys: foreign_keys.unwrap_or_default(),
                    indexes: indexes.unwrap_or_default(),
                    unavailable,
                }
            }
            .boxed(),
        );
    }
    context.tables = stream::iter(loads)
        .buffered(METADATA_CONCURRENCY)
        .collect()
        .await;
    context
}

#[cfg(test)]
mod tests;
