//! Bounded, Unicode-safe model output. MCP's legacy encoding is unaffected.
use serde_json::Value;

use super::{RowsPreview, ToolOutput};
use crate::models::QueryResult;

pub const MAX_RESULT_CHARS: usize = 12_000;
pub const MAX_CELL_CHARS: usize = 200;
pub const DEFAULT_ROW_LIMIT: u32 = 50;
pub const MAX_ROW_LIMIT: u32 = 1_000;

/// The marker counts toward the limit, including for very small limits.
pub fn text(value: &str, limit: usize) -> (String, bool) {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= limit {
        return (value.to_string(), false);
    }
    let marker = "\n… [truncated] …\n";
    let marker_len = marker.chars().count();
    if limit < marker_len {
        return (chars.into_iter().take(limit).collect(), true);
    }
    let remaining = limit - marker_len;
    let head = remaining.div_ceil(2);
    let tail = remaining / 2;
    let content = chars[..head]
        .iter()
        .copied()
        .chain(marker.chars())
        .chain(chars[chars.len() - tail..].iter().copied())
        .collect();
    (content, true)
}

pub fn output(value: &str) -> ToolOutput {
    let (content, truncated) = text(value, MAX_RESULT_CHARS);
    ToolOutput {
        content,
        truncated,
        rows: None,
    }
}

fn cell(value: Value) -> (Value, bool) {
    let rendered = match &value {
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    };
    let (short, truncated) = text(&rendered, MAX_CELL_CHARS);
    if truncated {
        (Value::String(short), true)
    } else {
        (value, false)
    }
}

/// Preserve a head and tail sample, including when the driver ignores limits.
pub fn query_result(result: QueryResult, limit: u32) -> ToolOutput {
    let limit = limit.clamp(1, MAX_ROW_LIMIT) as usize;
    let mut truncated = result.truncated
        || result.pagination.as_ref().is_some_and(|page| page.has_more)
        || result
            .additional_results
            .as_ref()
            .is_some_and(|sets| !sets.is_empty());
    let mut columns = Vec::with_capacity(result.columns.len());
    for column in result.columns {
        let (column, cut) = text(&column, MAX_CELL_CHARS);
        truncated |= cut;
        columns.push(column);
    }
    let count = result.rows.len();
    let head = limit.div_ceil(2);
    let tail = limit / 2;
    let mut rows = Vec::new();
    truncated |= count > limit;
    for (index, row) in result.rows.into_iter().enumerate() {
        if count > limit && index >= head && index < count - tail {
            continue;
        }
        truncated |= row.len() > columns.len();
        let mut values = Vec::new();
        for value in row.into_iter().take(columns.len()) {
            let (value, cut) = cell(value);
            truncated |= cut;
            values.push(value);
        }
        rows.push(values);
    }
    let mut preview = RowsPreview {
        columns,
        rows,
        affected_rows: result.affected_rows,
    };
    loop {
        // These fields contain only JSON values, so serialization cannot fail.
        let content = serde_json::to_string(&preview).expect("JSON row preview must serialize");
        if content.chars().count() <= MAX_RESULT_CHARS {
            return ToolOutput {
                content,
                truncated,
                rows: Some(preview),
            };
        }
        truncated = true;
        if preview.rows.len() > 2 {
            preview.rows.remove(preview.rows.len() / 2);
        } else if !preview.columns.is_empty() {
            preview.columns.pop();
            for row in &mut preview.rows {
                row.truncate(preview.columns.len());
            }
        } else {
            preview.rows.pop();
        }
    }
}

#[cfg(test)]
mod tests;
