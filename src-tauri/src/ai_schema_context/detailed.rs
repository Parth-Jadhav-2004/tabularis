//! Chat metadata alongside the stable schema format used by legacy commands.
use serde::{Deserialize, Serialize};

use crate::drivers::driver_trait::DatabaseDriver;
use crate::models::{ConnectionParams, ForeignKey, Index, TableColumn, TableInfo};

mod objects;
pub use objects::{load_detailed_objects_from_driver, reference_catalog, ObjectKind, ObjectRef};

const MAX_CATALOG_TABLES: usize = 10_000;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SchemaCatalog {
    pub tables: Vec<TableInfo>,
    #[serde(default)]
    pub views: Vec<TableInfo>,
    pub total_table_count: usize,
    #[serde(default)]
    pub total_view_count: usize,
    #[serde(default)]
    pub views_unavailable: bool,
    pub truncated: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DetailedTable {
    #[serde(default)]
    pub kind: ObjectKind,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition: Option<String>,
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
    Definition,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DetailedSchemaContext {
    pub tables: Vec<DetailedTable>,
    pub missing_tables: Vec<String>,
    pub omitted_tables: Vec<String>,
    #[serde(default)]
    pub missing_views: Vec<String>,
    #[serde(default)]
    pub omitted_views: Vec<String>,
    pub truncated: bool,
}

/// Retain a bounded catalog in deterministic order, without loading columns.
pub async fn load_catalog<D: DatabaseDriver + ?Sized>(
    driver: &D,
    params: &ConnectionParams,
    schema: Option<&str>,
) -> Result<SchemaCatalog, String> {
    let (tables, views) = tokio::join!(driver.get_tables(params, schema), async {
        if driver.manifest().capabilities.views {
            driver.get_views(params, schema).await
        } else {
            Ok(Vec::new())
        }
    });
    let mut tables = tables?;
    tables.sort_by(|left, right| left.name.cmp(&right.name));
    tables.dedup_by(|left, right| left.name == right.name);
    let total_table_count = tables.len();
    tables.truncate(MAX_CATALOG_TABLES);
    let views_unavailable = views.is_err();
    let mut views = views
        .unwrap_or_default()
        .into_iter()
        .map(|view| TableInfo {
            name: view.name,
            comment: None,
        })
        .collect::<Vec<_>>();
    views.sort_by(|left, right| left.name.cmp(&right.name));
    views.dedup_by(|left, right| left.name == right.name);
    let total_view_count = views.len();
    views.truncate(MAX_CATALOG_TABLES.saturating_sub(tables.len()));
    Ok(SchemaCatalog {
        truncated: total_table_count > tables.len()
            || total_view_count > views.len()
            || views_unavailable,
        total_table_count,
        tables,
        total_view_count,
        views,
        views_unavailable,
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
    let requested = requested_tables
        .iter()
        .map(|name| ObjectRef {
            kind: ObjectKind::Table,
            name: name.clone(),
        })
        .collect::<Vec<_>>();
    load_detailed_objects_from_driver(driver, params, schema, catalog, &requested, max_tables).await
}

#[cfg(test)]
mod tests;
