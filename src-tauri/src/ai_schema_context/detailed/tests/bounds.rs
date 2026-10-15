use super::*;

#[tokio::test]
async fn detailed_metadata_hard_cap_does_not_expand_with_a_larger_request() {
    let directory = tempfile::tempdir().unwrap();
    let params = ConnectionParams {
        driver: "sqlite".into(),
        database: DatabaseSelection::Single(
            directory
                .path()
                .join("missing/context.sqlite")
                .to_string_lossy()
                .into(),
        ),
        ..Default::default()
    };
    let catalog = SchemaCatalog {
        tables: (0..101)
            .map(|index| TableInfo {
                name: format!("table_{index}"),
                comment: None,
            })
            .collect(),
        total_table_count: 101,
        truncated: false,
        ..Default::default()
    };
    let names = catalog
        .tables
        .iter()
        .map(|table| table.name.clone())
        .collect::<Vec<_>>();
    let context = load_detailed_from_driver(
        &SqliteDriver::new(),
        &params,
        None,
        &catalog,
        &names,
        usize::MAX,
    )
    .await;
    assert_eq!(context.tables.len(), 100);
    assert_eq!(context.omitted_tables, ["table_100"]);
    assert!(context.truncated);
}

#[tokio::test]
async fn real_metadata_flows_from_query_selection_to_a_bounded_prompt() {
    use crate::ai_chat::context::{
        build_context, references::extract_referenced_tables, ConnectionContext, ContextBudgets,
        ContextInput, EditorContext,
    };
    use crate::ai_chat::types::ChatMode;
    let (_directory, params, driver) = fixture().await;
    let catalog = load_catalog(&driver, &params, None).await.unwrap();
    let query = "SELECT u.name, t.id FROM users u JOIN teams t ON t.id = u.team_id";
    let references = extract_referenced_tables(query, &catalog.tables, None, None);
    let detailed =
        load_detailed_from_driver(&driver, &params, None, &catalog, &references.tables, 20).await;
    let input = ContextInput {
        saved_queries: None,

        mode: ChatMode::Ask,
        language: "en",
        connection: Some(ConnectionContext {
            driver: "sqlite",
            dialect: "sqlite",
            identifier_quote: "\"",
            database: None,
            schema: None,
            server_version: None,
            readonly: false,
        }),
        instructions: None,
        catalog: Some(&catalog),
        detailed: Some(&detailed),
        editor: EditorContext {
            query: Some(query),
            ..Default::default()
        },
        last_error: None,
    };
    let result = build_context(&input, ContextBudgets::default());
    assert_eq!(
        result
            .items
            .iter()
            .filter(|item| item.kind == "table")
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["users", "teams"]
    );
    assert!(!result.items.iter().any(|item| item.kind == "gaps"));
    assert!(result.system.contains("users_name"));
    assert!(result.system.contains(query));
    assert!(result.context_chars <= ContextBudgets::default().total());
    assert!(!result.truncated);
}
