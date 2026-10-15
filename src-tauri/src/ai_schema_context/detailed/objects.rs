use super::{DetailedSchemaContext, DetailedTable, MetadataKind, SchemaCatalog};
use crate::drivers::driver_trait::DatabaseDriver;
use crate::models::{ConnectionParams, TableInfo};
use futures::{stream, FutureExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind {
    #[default]
    Table,
    View,
}

impl ObjectKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Table => "table",
            Self::View => "view",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ObjectRef {
    pub kind: ObjectKind,
    pub name: String,
}

pub fn reference_catalog(catalog: &SchemaCatalog) -> Vec<TableInfo> {
    catalog
        .tables
        .iter()
        .chain(&catalog.views)
        .map(|item| TableInfo {
            name: item.name.clone(),
            comment: None,
        })
        .collect()
}

/// Table and view mentions share one priority order and one hard limit. Only
/// catalog-backed names reach a driver; metadata failures never expose errors.
pub async fn load_detailed_objects_from_driver<D: DatabaseDriver + ?Sized>(
    driver: &D,
    params: &ConnectionParams,
    schema: Option<&str>,
    catalog: &SchemaCatalog,
    requested: &[ObjectRef],
    max_objects: usize,
) -> DetailedSchemaContext {
    let mut context = DetailedSchemaContext::default();
    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    for object in requested {
        if !seen.insert(object) {
            continue;
        }
        let objects = if object.kind == ObjectKind::Table {
            &catalog.tables
        } else {
            &catalog.views
        };
        match objects.iter().find(|item| item.name == object.name) {
            None => {
                if object.kind == ObjectKind::Table {
                    context.missing_tables.push(object.name.clone());
                } else {
                    context.missing_views.push(object.name.clone());
                }
            }
            Some(_) if selected.len() >= max_objects.min(100) => {
                if object.kind == ObjectKind::Table {
                    context.omitted_tables.push(object.name.clone());
                } else {
                    context.omitted_views.push(object.name.clone());
                }
            }
            Some(info) => selected.push((object, info)),
        }
    }
    context.truncated = !context.omitted_tables.is_empty() || !context.omitted_views.is_empty();
    let mut loads = Vec::new();
    for (object, info) in selected {
        loads.push(
            async move {
                let mut unavailable = Vec::new();
                let (columns, foreign_keys, indexes, definition) =
                    if object.kind == ObjectKind::View {
                        let (columns, definition) = tokio::join!(
                            driver.get_view_columns(params, &object.name, schema),
                            driver.get_view_definition(params, &object.name, schema)
                        );
                        if columns.is_err() {
                            unavailable.push(MetadataKind::Columns);
                        }
                        if definition.is_err() {
                            unavailable.push(MetadataKind::Definition);
                        }
                        (
                            columns.unwrap_or_default(),
                            Vec::new(),
                            Vec::new(),
                            definition.ok(),
                        )
                    } else {
                        let (columns, foreign_keys, indexes) = tokio::join!(
                            driver.get_columns(params, &object.name, schema),
                            driver.get_foreign_keys(params, &object.name, schema),
                            driver.get_indexes(params, &object.name, schema)
                        );
                        if columns.is_err() {
                            unavailable.push(MetadataKind::Columns);
                        }
                        if foreign_keys.is_err() {
                            unavailable.push(MetadataKind::ForeignKeys);
                        }
                        if indexes.is_err() {
                            unavailable.push(MetadataKind::Indexes);
                        }
                        (
                            columns.unwrap_or_default(),
                            foreign_keys.unwrap_or_default(),
                            indexes.unwrap_or_default(),
                            None,
                        )
                    };
                DetailedTable {
                    kind: object.kind,
                    name: object.name.clone(),
                    definition,
                    comment: info.comment.clone(),
                    columns,
                    foreign_keys,
                    indexes,
                    unavailable,
                }
            }
            .boxed(),
        );
    }
    context.tables = stream::iter(loads).buffered(4).collect().await;
    context
}
