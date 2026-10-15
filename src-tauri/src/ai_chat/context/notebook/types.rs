use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NotebookCellSnapshot {
    pub id: String,
    pub index: usize,
    pub r#type: NotebookCellType,
    pub name: Option<String>,
    pub schema: Option<String>,
    pub content: String,
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotebookCellType {
    Sql,
    Markdown,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NotebookParamSnapshot {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NotebookErrorSnapshot {
    pub cell_id: String,
    pub index: usize,
    pub error: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NotebookResultSnapshot {
    pub cell_id: String,
    pub columns: Vec<String>,
    pub row_count: usize,
    pub rows: Vec<Vec<String>>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NotebookSnapshot {
    #[serde(default)]
    pub unavailable_cells: Vec<String>,
    pub id: String,
    pub title: String,
    pub cell_count: usize,
    pub stop_on_error: bool,
    pub params: Vec<NotebookParamSnapshot>,
    pub outline: Vec<NotebookCellSnapshot>,
    pub focused: Option<NotebookCellSnapshot>,
    pub neighbours: Vec<NotebookCellSnapshot>,
    pub mentioned: Vec<NotebookCellSnapshot>,
    pub errors: Vec<NotebookErrorSnapshot>,
    pub results: Vec<NotebookResultSnapshot>,
    pub truncated: bool,
}

#[derive(Clone, Copy)]
pub struct NotebookContext<'a> {
    pub snapshot: &'a NotebookSnapshot,
    pub include_errors: bool,
    pub include_results: bool,
}
