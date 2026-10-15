use super::{store::ConversationStore, types::Conversation};
use crate::ai::message::{Block, Role};
use crate::ai_notebook_export::{derive_cell_name, NotebookCellExport, NotebookExport};
use crate::secure_store::StoreError;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Markdown,
    Notebook,
}

fn escape(text: &str) -> String {
    let mut escaped = String::new();
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '\n' | '\r' => escaped.push(' '),
            '*' | '_' | '[' | ']' | '`' | '#' | '\\' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn fence(text: &str, language: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let marker = "`".repeat(3.max(longest + 1));
    format!("{marker}{language}\n{text}\n{marker}")
}

fn details(label: &str, content: &str, language: &str) -> String {
    format!(
        "<details>\n<summary>{}</summary>\n\n{}\n\n</details>",
        label
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;"),
        fence(content, language)
    )
}

fn block_markdown(block: &Block) -> String {
    match block {
        Block::Text { text } => text.clone(),
        Block::Reasoning { text, .. } => details("Reasoning", text, "text"),
        Block::ToolCall { name, args, .. } => details(
            &format!("Tool: {name}"),
            &serde_json::to_string_pretty(args).unwrap_or_default(),
            "json",
        ),
        Block::ToolResult {
            content,
            truncated,
            is_error,
            ..
        } => details(
            &format!(
                "Tool result{}{}",
                if *is_error { " (error)" } else { "" },
                if *truncated { " (truncated)" } else { "" }
            ),
            content,
            "text",
        ),
        Block::Image { blob_id, mime, .. } => {
            let extension = match mime.as_str() {
                "image/png" => "png",
                "image/jpeg" => "jpg",
                "image/webp" => "webp",
                "image/gif" => "gif",
                _ => "bin",
            };
            format!("Image: {}.{extension}", escape(blob_id))
        }
        Block::Attachment { name, .. } => format!("Attachment: {}", escape(name)),
        Block::Context { items } => format!(
            "Context: {}",
            items
                .iter()
                .map(|item| escape(&item.label))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Block::Error { message } => format!("Error: {}", escape(message)),
    }
}

fn date(timestamp: i64) -> String {
    chrono::DateTime::from_timestamp_millis(timestamp)
        .unwrap_or_default()
        .to_rfc3339()
}

fn header(chat: &Conversation, connection: &str) -> String {
    format!("# {}\n\n- **Connection:** {}\n- **Database:** {}\n- **Schema:** {}\n- **Created:** {}\n\n> This export is not encrypted. Images and attachments are referenced by filename; their files are not included.",
        escape(&chat.title), escape(connection), escape(chat.database.as_deref().unwrap_or("—")),
        escape(chat.schema.as_deref().unwrap_or("—")), date(chat.created_at))
}

fn role(role: &Role) -> &'static str {
    match role {
        Role::User => "User",
        Role::Assistant => "Assistant",
        Role::Tool => "Tool",
    }
}

pub(super) fn conversation_markdown(chat: &Conversation, connection: &str) -> String {
    let mut sections = vec![header(chat, connection)];
    for message in &chat.messages {
        sections.push(format!(
            "## {} · {}",
            role(&message.role),
            date(message.created_at)
        ));
        sections.extend(message.blocks.iter().map(block_markdown));
    }
    sections.join("\n\n") + "\n"
}

fn markdown_cell(content: String) -> NotebookCellExport {
    NotebookCellExport {
        cell_type: "markdown".into(),
        content,
        name: None,
        schema: None,
    }
}

// Only explicit SQL fences become executable cells. Other fences stay intact.
fn text_cells(
    text: &str,
    database: Option<&str>,
    query_index: &mut usize,
) -> Vec<NotebookCellExport> {
    let lines: Vec<&str> = text.lines().collect();
    let mut cells = Vec::new();
    let mut pending = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start_matches(' ');
        let indent = line.len() - trimmed.len();
        let marker = trimmed.chars().next().unwrap_or(' ');
        let count = trimmed.chars().take_while(|ch| *ch == marker).count();
        if indent <= 3 && matches!(marker, '`' | '~') && count >= 3 {
            let language = trimmed[count..].trim().to_ascii_lowercase();
            let mut end = index + 1;
            while end < lines.len() {
                let closing = lines[end].trim_start_matches(' ');
                let closing_indent = lines[end].len() - closing.len();
                let closing_count = closing.chars().take_while(|ch| *ch == marker).count();
                if closing_indent <= 3
                    && closing_count >= count
                    && closing[closing_count..].trim().is_empty()
                {
                    break;
                }
                end += 1;
            }
            let sql = matches!(
                language.as_str(),
                "sql" | "postgresql" | "postgres" | "mysql" | "sqlite" | "tsql" | "plsql"
            );
            if sql {
                let prose = pending.join("\n");
                if !prose.trim().is_empty() {
                    cells.push(markdown_cell(prose));
                }
                pending.clear();
                let content = lines[index + 1..end].join("\n");
                if !content.trim().is_empty() {
                    cells.push(NotebookCellExport {
                        cell_type: "sql".into(),
                        name: Some(derive_cell_name(&content, *query_index)),
                        content,
                        schema: database.map(str::to_owned),
                    });
                    *query_index += 1;
                }
            } else {
                pending.extend_from_slice(&lines[index..(end + 1).min(lines.len())]);
            }
            index = end + 1;
        } else {
            pending.push(line);
            index += 1;
        }
    }
    let prose = pending.join("\n");
    if !prose.trim().is_empty() {
        cells.push(markdown_cell(prose));
    }
    cells
}

pub(super) fn conversation_notebook(chat: &Conversation, connection: &str) -> NotebookExport {
    let mut cells = vec![markdown_cell(header(chat, connection))];
    let mut query_index = 1;
    for message in &chat.messages {
        cells.push(markdown_cell(format!(
            "## {} · {}",
            role(&message.role),
            date(message.created_at)
        )));
        for block in &message.blocks {
            if let Block::Text { text } = block {
                cells.extend(text_cells(text, chat.database.as_deref(), &mut query_index));
            } else {
                cells.push(markdown_cell(block_markdown(block)));
            }
        }
    }
    NotebookExport {
        version: 1,
        title: chat.title.clone(),
        created_at: date(chat.created_at),
        cells,
    }
}

#[tauri::command]
pub async fn ai_chat_export(
    connection_id: String,
    id: String,
    format: ExportFormat,
) -> Result<String, StoreError> {
    tokio::task::spawn_blocking(move || {
        let base = crate::paths::get_app_config_dir();
        let chat = ConversationStore::new(&base).load(&connection_id, &id)?;
        let connections =
            crate::persistence::load_connections(&crate::paths::resolve_connections_path(&base))
                .map_err(|_| StoreError::Io)?;
        let connection = connections
            .iter()
            .find(|connection| connection.id == connection_id)
            .map(|connection| connection.name.as_str())
            .unwrap_or(&connection_id);
        match format {
            ExportFormat::Markdown => Ok(conversation_markdown(&chat, connection)),
            ExportFormat::Notebook => {
                serde_json::to_string_pretty(&conversation_notebook(&chat, connection))
                    .map_err(|_| StoreError::Io)
            }
        }
    })
    .await
    .map_err(|_| StoreError::Io)?
}

#[cfg(test)]
#[path = "tests/exports.rs"]
mod tests;
