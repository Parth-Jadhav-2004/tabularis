//! Bounded, untrusted notebook data. No notebook or result text is persisted in
//! context summaries; execution and notebook mutation are outside this module.
mod types;
pub use types::*;

use super::{budget::render_data, ContextItemSummary};
use std::collections::HashSet;

pub const NOTEBOOK_BUDGET: usize = 36_000;
pub const NOTEBOOK_RULES: &str = "Notebook context describes the whole notebook, including Markdown documentation. Refer to cells as #<index> (one-based) or by name. Notebook parameters use @name or ${name}; their values are substituted verbatim, so preserve these references in proposed SQL. A cell's schema override applies only to that cell; database metadata supplied above belongs to the selected scope. Results are absent unless explicitly included; never infer omitted rows or claim that a result sample is complete. Propose changes for user review; never claim to have edited or executed a cell.";

impl NotebookSnapshot {
    pub(crate) fn validate(&self) -> bool {
        let bounded = |value: &str, limit| value.chars().count() <= limit && !value.contains('\0');
        let id = |value: &str| !value.is_empty() && bounded(value, 128);
        let cell = |cell: &NotebookCellSnapshot, limit| {
            id(&cell.id)
                && cell.index > 0
                && cell.index <= self.cell_count
                && bounded(&cell.content, limit)
                && cell.name.as_deref().is_none_or(|name| bounded(name, 128))
                && cell
                    .schema
                    .as_deref()
                    .is_none_or(|schema| bounded(schema, 128))
        };
        if !id(&self.id)
            || self.unavailable_cells.len() > 20
            || self.unavailable_cells.iter().any(|value| !id(value))
            || !bounded(&self.title, 200)
            || self.cell_count > 1_000_000
            || self.params.len() > 40
            || self
                .params
                .iter()
                .any(|param| !bounded(&param.name, 128) || !bounded(&param.value, 256))
            || self.outline.len() > 80
            || self.outline.iter().any(|entry| !cell(entry, 150))
            || self
                .focused
                .as_ref()
                .is_some_and(|entry| !cell(entry, 6_000))
            || self.neighbours.len() > 2
            || self.neighbours.iter().any(|entry| !cell(entry, 2_000))
            || self.mentioned.len() > 20
            || self.mentioned.iter().any(|entry| !cell(entry, 6_000))
            || self
                .mentioned
                .iter()
                .map(|entry| entry.content.chars().count())
                .sum::<usize>()
                > 8_000
            || self.errors.len() > 40
            || self.errors.iter().any(|error| {
                !id(&error.cell_id)
                    || error.index == 0
                    || error.index > self.cell_count
                    || !bounded(&error.error, 2_000)
            })
            || self
                .errors
                .iter()
                .map(|error| error.error.chars().count())
                .sum::<usize>()
                > 2_000
            || self.results.len() > 21
            || self.results.iter().any(|result| {
                !id(&result.cell_id)
                    || result.columns.len() > 40
                    || result.rows.len() > 5
                    || result.rows.len() > result.row_count
                    || result
                        .rows
                        .iter()
                        .any(|row| row.len() > result.columns.len())
                    || result
                        .columns
                        .iter()
                        .chain(result.rows.iter().flatten())
                        .any(|value| !bounded(value, 200))
            })
            || self
                .results
                .iter()
                .flat_map(|result| result.columns.iter().chain(result.rows.iter().flatten()))
                .map(|value| value.chars().count())
                .sum::<usize>()
                > 6_000
        {
            return false;
        }
        let selected = self
            .focused
            .iter()
            .chain(&self.mentioned)
            .map(|cell| cell.id.as_str())
            .collect::<HashSet<_>>();
        let unique = |cells: &[NotebookCellSnapshot]| {
            cells
                .iter()
                .map(|cell| &cell.id)
                .collect::<HashSet<_>>()
                .len()
                == cells.len()
        };
        unique(&self.outline)
            && unique(&self.mentioned)
            && unique(&self.neighbours)
            && self
                .results
                .iter()
                .all(|result| selected.contains(result.cell_id.as_str()))
            && serde_json::to_vec(self).is_ok_and(|bytes| bytes.len() <= 262_144)
    }

    /// Infer metadata only from focused/mentioned SQL in the selected scope.
    /// Other cells' schema overrides must not select metadata in this scope.
    pub(crate) fn reference_sql(&self, schema: Option<&str>) -> String {
        self.focused
            .iter()
            .chain(&self.mentioned)
            .filter(|cell| {
                cell.r#type == NotebookCellType::Sql
                    && cell
                        .schema
                        .as_deref()
                        .is_none_or(|scope| scope.is_empty() || Some(scope) == schema)
            })
            .map(|cell| cell.content.as_str())
            .collect::<Vec<_>>()
            .join(";\n")
    }
}

pub(super) fn append(
    context: NotebookContext<'_>,
    text: &mut String,
    items: &mut Vec<ContextItemSummary>,
    limit: usize,
) -> bool {
    let snapshot = context.snapshot;
    let mut remaining = limit;
    let mut truncated = snapshot.truncated;
    let mut add =
        |kind: &str, id: &str, label: &str, data: String, budget: usize, incomplete: bool| {
            let rendered = render_data(kind, &data, budget.min(remaining));
            remaining = remaining.saturating_sub(rendered.chars);
            let incomplete = incomplete || rendered.truncated;
            truncated |= incomplete;
            if rendered.chars > 0 {
                text.push_str(&rendered.text);
                items.push(ContextItemSummary {
                    kind: kind.into(),
                    id: id.into(),
                    label: label.into(),
                    chars: rendered.chars,
                    table_count: None,
                    view_count: None,
                    truncated: incomplete,
                });
            }
        };
    add(
        "notebook",
        &snapshot.id,
        &snapshot.title,
        serde_json::json!({
            "id":snapshot.id, "title":snapshot.title, "cell_count":snapshot.cell_count,
            "stop_on_error":snapshot.stop_on_error, "params":snapshot.params,
            "unavailable_cells":snapshot.unavailable_cells,
        })
        .to_string(),
        2_000,
        false,
    );
    add("notebook_outline", &snapshot.id, "Notebook outline", serde_json::json!({
        "cells":snapshot.outline, "omitted_cells":snapshot.cell_count.saturating_sub(snapshot.outline.len()),
    }).to_string(), 8_000, snapshot.outline.iter().any(|cell| cell.truncated) || snapshot.cell_count > snapshot.outline.len());
    if let Some(cell) = &snapshot.focused {
        add(
            "notebook_cell",
            &cell.id,
            &format!(
                "#{}{}",
                cell.index,
                cell.name
                    .as_ref()
                    .map(|name| format!(" · {name}"))
                    .unwrap_or_default()
            ),
            serde_json::json!({"role":"focused", "cell":cell}).to_string(),
            6_000,
            cell.truncated,
        );
    }
    if !snapshot.neighbours.is_empty() {
        add(
            "notebook_cell",
            "neighbours",
            &snapshot
                .neighbours
                .iter()
                .map(|cell| format!("#{}", cell.index))
                .collect::<Vec<_>>()
                .join(", "),
            serde_json::json!({"role":"neighbours", "cells":snapshot.neighbours}).to_string(),
            4_000,
            snapshot.neighbours.iter().any(|cell| cell.truncated),
        );
    }
    if !snapshot.mentioned.is_empty() {
        add(
            "notebook_cell",
            "mentioned",
            &snapshot
                .mentioned
                .iter()
                .map(|cell| format!("#{}", cell.index))
                .collect::<Vec<_>>()
                .join(", "),
            serde_json::json!({"role":"mentioned", "cells":snapshot.mentioned}).to_string(),
            8_000,
            snapshot.mentioned.iter().any(|cell| cell.truncated),
        );
    }
    if context.include_errors && !snapshot.errors.is_empty() {
        add(
            "notebook_errors",
            &snapshot.id,
            "Notebook errors",
            serde_json::to_string(&snapshot.errors).unwrap(),
            2_000,
            false,
        );
    }
    if context.include_results && !snapshot.results.is_empty() {
        add(
            "notebook_results",
            &snapshot.id,
            "Notebook result samples",
            serde_json::to_string(&snapshot.results).unwrap(),
            6_000,
            snapshot.results.iter().any(|result| result.truncated),
        );
    }
    // Persist only the bounded identity/index mapping used by this request.
    // Zero-sized entries are internal navigation metadata, not prompt layers.
    if items
        .iter()
        .any(|item| item.kind == "notebook" && item.id == snapshot.id)
    {
        let mut seen = HashSet::new();
        for cell in snapshot
            .outline
            .iter()
            .chain(snapshot.focused.iter())
            .chain(&snapshot.mentioned)
        {
            if seen.insert(&cell.id) {
                items.push(ContextItemSummary {
                    kind: "notebook_cell_ref".into(),
                    id: cell.id.clone(),
                    label: format!("#{}", cell.index),
                    chars: 0,
                    table_count: None,
                    view_count: None,
                    truncated: false,
                });
            }
        }
    }
    truncated
}

#[cfg(test)]
mod tests;
