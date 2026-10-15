use super::*;
use crate::ai::message::{Message, ToolStatus};

#[path = "fixtures.rs"]
mod fixtures;

#[test]
fn markdown_export_preserves_transcript_metadata_and_references_without_blob_contents() {
    let mut chat = fixtures::conversation();
    chat.messages[1].blocks = vec![
        fixtures::text("Here is the query:\n\n```sql\nSELECT 1;\n```"),
        Block::ToolCall {
            id: "call".into(),
            name: "run_query</summary>".into(),
            args: serde_json::json!({"query":"SELECT '```';"}),
            status: ToolStatus::Completed,
        },
        Block::ToolResult {
            call_id: "call".into(),
            content: "result".into(),
            truncated: true,
            is_error: false,
        },
        Block::Image {
            blob_id: "image-id".into(),
            mime: "image/png".into(),
            width: 1,
            height: 1,
        },
        Block::Attachment {
            name: "source.csv".into(),
            kind: crate::ai::message::AttachmentKind::Csv,
            blob_id: Some("blob".into()),
            size: 3,
        },
    ];
    let markdown = conversation_markdown(&chat, "Local *connection*");
    assert!(markdown.starts_with("# Count users\n\n- **Connection:** Local \\*connection\\*"));
    assert!(markdown.contains("**Database:** analytics"));
    assert!(markdown.contains("**Schema:** public"));
    assert!(markdown.contains("**Created:** 1970-01-01T00:00:00+00:00"));
    assert!(markdown.contains("not encrypted"));
    assert!(markdown.contains("## User ·"));
    assert!(markdown.contains("```sql\nSELECT 1;\n```"));
    assert!(markdown.contains("<summary>Tool: run_query&lt;/summary&gt;</summary>"));
    assert!(markdown.contains("````json"));
    assert!(markdown.contains("Tool result (truncated)"));
    assert!(markdown.contains("Image: image-id.png"));
    assert!(markdown.contains("Attachment: source.csv"));
    assert!(!markdown.contains("data:image"));
    assert!(!markdown.contains("signature"));
}

#[test]
fn notebook_export_keeps_order_database_override_and_existing_file_shape() {
    let mut chat = fixtures::conversation();
    chat.messages[1].blocks = vec![fixtures::text("Before\n\n```sql\n-- Count users\nSELECT count(*) FROM users;\n```\n\nAfter\n\n~~~postgresql\nSELECT 2;\n~~~")];
    let notebook = conversation_notebook(&chat, "Local");
    let sql: Vec<_> = notebook
        .cells
        .iter()
        .filter(|cell| cell.cell_type == "sql")
        .collect();
    assert_eq!(sql.len(), 2);
    assert_eq!(sql[0].name.as_deref(), Some("Count users"));
    assert_eq!(sql[1].name.as_deref(), Some("Query 2"));
    assert!(sql
        .iter()
        .all(|cell| cell.schema.as_deref() == Some("analytics")));
    let value = serde_json::to_value(notebook).unwrap();
    assert_eq!(value["version"], 1);
    assert_eq!(value["title"], "Count users");
    assert!(value.get("createdAt").is_some());
    assert!(value.get("created_at").is_none());
    assert!(value["cells"]
        .as_array()
        .unwrap()
        .iter()
        .all(|cell| cell.get("result").is_none()));
    assert!(
        value.to_string().find("Before").unwrap() < value.to_string().find("SELECT count").unwrap()
    );
}

#[test]
fn only_explicit_sql_fences_become_cells_and_longer_fences_are_respected() {
    let mut index = 1;
    let cells = text_cells("```text\n```sql\nSELECT 0;\n```\n\n````sql\nSELECT '```';\n````\n\n```\nSELECT 2;\n```\n\n```sql\nSELECT 3;", None, &mut index);
    let sql: Vec<_> = cells
        .iter()
        .filter(|cell| cell.cell_type == "sql")
        .collect();
    assert_eq!(sql.len(), 2);
    assert_eq!(sql[0].content, "SELECT '```';");
    assert_eq!(sql[1].content, "SELECT 3;");
    assert!(sql.iter().all(|cell| cell.schema.is_none()));
    let mut empty = fixtures::conversation();
    empty.messages = vec![Message::user("")];
    assert!(conversation_notebook(&empty, "Local")
        .cells
        .iter()
        .all(|cell| cell.cell_type == "markdown"));
}
