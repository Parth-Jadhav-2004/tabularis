use super::*;
use serde_json::json;

fn result(columns: Vec<String>, rows: Vec<Vec<Value>>) -> QueryResult {
    QueryResult {
        columns,
        rows,
        affected_rows: 0,
        truncated: false,
        pagination: None,
        additional_results: None,
        server_time_ms: None,
        fetch_time_ms: None,
    }
}

#[test]
fn unicode_text_preserves_head_tail_and_exact_caps() {
    let value = format!("HEAD{}TAIL", "🦀é".repeat(10_000));
    for limit in [0, 1, 15, 16, 17, 20, 201, MAX_RESULT_CHARS] {
        let (output, truncated) = text(&value, limit);
        assert!(truncated);
        assert_eq!(output.chars().count(), limit);
        if limit >= 24 {
            assert!(output.starts_with("HEAD"));
            assert!(output.ends_with("TAIL"));
        }
    }
    assert_eq!(text("", 0), (String::new(), false));
    assert_eq!(text("🦀é", 2), ("🦀é".into(), false));
    assert_eq!(output("small").truncated, false);
    assert!(output(&value).truncated);
}

#[test]
fn small_results_preserve_types_and_affected_rows() {
    let mut input = result(
        vec!["integer".into(), "null".into(), "bool".into()],
        vec![vec![json!(42), Value::Null, json!(true)]],
    );
    input.affected_rows = 3;
    let output = query_result(input, DEFAULT_ROW_LIMIT);
    assert!(!output.truncated);
    let preview = output.rows.unwrap();
    assert_eq!(preview.rows[0], vec![json!(42), Value::Null, json!(true)]);
    assert_eq!(preview.affected_rows, 3);
    assert_eq!(serde_json::to_string(&preview).unwrap(), output.content);
}

#[test]
fn driver_rows_are_sampled_at_both_ends_and_hard_capped() {
    for limit in [1, 5, 50, 1000, 5000] {
        let input = result(
            vec!["id".into()],
            (0..2000).map(|i| vec![json!(i)]).collect(),
        );
        let output = query_result(input, limit);
        assert!(output.truncated);
        let preview = output.rows.unwrap();
        assert!(preview.rows.len() <= limit.min(MAX_ROW_LIMIT) as usize);
        assert_eq!(preview.rows[0][0], 0);
        if limit > 1 {
            assert_eq!(preview.rows.last().unwrap()[0], 1999);
        }
        assert!(output.content.chars().count() <= MAX_RESULT_CHARS);
    }
}

#[test]
fn cells_including_nested_json_are_bounded() {
    let input = result(
        vec!["text".into(), "object".into()],
        vec![vec![
            json!("🦀".repeat(1000)),
            json!({"long":"x".repeat(1000)}),
        ]],
    );
    let output = query_result(input, 50);
    assert!(output.truncated);
    for cell in &output.rows.unwrap().rows[0] {
        assert!(cell.as_str().unwrap().chars().count() <= MAX_CELL_CHARS);
    }
}

#[test]
fn wide_rows_and_escaped_characters_fit_both_content_and_preview_budget() {
    for column_count in [4, 1000] {
        let input = result(
            (0..column_count)
                .map(|i| format!("column-{i}{}", "x".repeat(200)))
                .collect(),
            (0..50)
                .map(|_| vec![json!("\n\"\\".repeat(100)); column_count])
                .collect(),
        );
        let output = query_result(input, 50);
        assert!(output.truncated);
        assert!(output.content.chars().count() <= MAX_RESULT_CHARS);
        let preview = output.rows.unwrap();
        assert!(serde_json::to_string(&preview).unwrap().chars().count() <= MAX_RESULT_CHARS);
        assert!(preview
            .rows
            .iter()
            .all(|row| row.len() == preview.columns.len()));
    }
}

#[test]
fn driver_truncation_pagination_and_extra_result_sets_are_reported() {
    for reason in 0..3 {
        let mut input = result(vec![], vec![]);
        match reason {
            0 => input.truncated = true,
            1 => {
                input.pagination = Some(crate::models::Pagination {
                    page: 1,
                    page_size: 50,
                    total_rows: None,
                    has_more: true,
                })
            }
            _ => input.additional_results = Some(vec![result(vec![], vec![])]),
        }
        assert!(query_result(input, 50).truncated);
    }
    assert!(!query_result(result(vec![], vec![]), 50).truncated);
}

#[test]
fn excess_driver_cells_are_dropped_and_reported_as_truncated() {
    let input = result(vec!["visible".into()], vec![vec![json!(1), json!("extra")]]);
    let output = query_result(input, 50);
    assert!(output.truncated);
    assert_eq!(output.rows.unwrap().rows[0], vec![json!(1)]);
}
