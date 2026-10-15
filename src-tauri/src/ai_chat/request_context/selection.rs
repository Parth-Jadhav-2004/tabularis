use super::ContextSpec;
use crate::ai_chat::context::references::extract_referenced_tables;
use crate::ai_schema_context::detailed::{reference_catalog, ObjectKind, ObjectRef, SchemaCatalog};
use crate::drivers::driver_trait::DriverCapabilities;
use crate::models::ConnectionParams;
use std::collections::HashSet;

pub(super) fn effective_database<'a>(
    spec: &'a ContextSpec,
    params: &'a ConnectionParams,
    capabilities: &DriverCapabilities,
) -> Option<&'a str> {
    let flat = !capabilities.schemas
        && !capabilities.file_based
        && !capabilities.folder_based
        && !capabilities.single_database;
    flat.then_some(spec.schema.as_deref())
        .flatten()
        .or(spec.database.as_deref())
        .or_else(|| (!params.database.primary().is_empty()).then_some(params.database.primary()))
}

pub(super) fn selected_objects(
    spec: &ContextSpec,
    catalog: &SchemaCatalog,
    query: &str,
    database: Option<&str>,
    schema: Option<&str>,
) -> Vec<ObjectRef> {
    let references = if spec.include.schema {
        extract_referenced_tables(query, &reference_catalog(catalog), database, schema).tables
    } else {
        Vec::new()
    };
    let mut seen = HashSet::new();
    spec.tables
        .iter()
        .map(|name| ObjectRef {
            kind: ObjectKind::Table,
            name: name.clone(),
        })
        .chain(spec.views.iter().map(|name| ObjectRef {
            kind: ObjectKind::View,
            name: name.clone(),
        }))
        .chain(references.into_iter().map(|name| ObjectRef {
            kind: if catalog.tables.iter().any(|table| table.name == name) {
                ObjectKind::Table
            } else {
                ObjectKind::View
            },
            name,
        }))
        .filter(|item| {
            let excluded = if item.kind == ObjectKind::Table {
                &spec.excluded_tables
            } else {
                &spec.excluded_views
            };
            !excluded.contains(&item.name) && seen.insert(item.clone())
        })
        .take(100)
        .collect()
}
