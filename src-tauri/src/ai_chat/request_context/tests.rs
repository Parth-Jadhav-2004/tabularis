use super::*;
use crate::ai::message::Message;
use crate::drivers::sqlite::SqliteDriver;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

#[path = "../tests/fixtures.rs"]
mod fixtures;

fn spec() -> ContextSpec {
    serde_json::from_value(serde_json::json!({ "connection_id":"fixture", "database":null, "schema":null, "language":"it", "editor":{"query":"SELECT * FROM users"} })).unwrap()
}

fn notebook_spec() -> ContextSpec {
    let mut selected = spec();
    selected.editor.query = Some("private-excluded-editor-marker".into());
    selected.notebook = Some(serde_json::from_value(serde_json::json!({
        "id":"notebook", "title":"Revenue", "cell_count":2, "stop_on_error":true, "params":[],
        "outline":[{"id":"focused", "index":1, "type":"sql", "name":null, "schema":null,
            "content":"SELECT * FROM users", "truncated":false}],
        "focused":{"id":"focused", "index":1, "type":"sql", "name":null, "schema":null,
            "content":"SELECT * FROM users", "truncated":false},
        "neighbours":[], "mentioned":[{"id":"mentioned", "index":2, "type":"sql", "name":null, "schema":null,
            "content":"SELECT * FROM missing", "truncated":false}],
        "errors":[], "results":[{"cell_id":"focused", "columns":["name"], "row_count":1,
            "rows":[["private-result-marker"]], "truncated":false}], "truncated":false,
    })).unwrap());
    selected
}

#[tokio::test]
async fn notebook_context_resolves_focused_and_mentioned_tables_and_persists_only_metadata() {
    let (_directory, params, driver) = fixture().await;
    let selected = notebook_spec();
    let built = resolve_with_driver(
        &selected,
        &params,
        &driver,
        false,
        &SchemaCache::default(),
        None,
        None,
    )
    .await
    .unwrap();
    assert!(built
        .items
        .iter()
        .any(|item| item.kind == "table" && item.id == "users"));
    assert!(built
        .system
        .contains("Table \"missing\" referenced but not found"));
    assert!(!built.system.contains("private-result-marker"));
    assert!(!built.system.contains("private-excluded-editor-marker"));
    assert!(built.items.iter().any(|item| item.kind == "notebook"));
    let mut request = request();
    apply(&mut request, built).unwrap();
    let user = serde_json::to_string(&request.messages.last().unwrap()).unwrap();
    assert!(!user.contains("SELECT"));
    assert!(!user.contains("private-"));
    assert!(user.contains("notebook"));
}

#[tokio::test]
async fn notebook_removal_and_result_choices_are_enforced_in_rust() {
    let (_directory, params, driver) = fixture().await;
    let mut selected = notebook_spec();
    selected.include.notebook_results = true;
    let built = resolve_with_driver(
        &selected,
        &params,
        &driver,
        false,
        &SchemaCache::default(),
        None,
        None,
    )
    .await
    .unwrap();
    assert!(built.system.contains("private-result-marker"));
    selected.include.notebook = false;
    selected.editor = Default::default();
    let built = resolve_with_driver(
        &selected,
        &params,
        &driver,
        false,
        &SchemaCache::default(),
        None,
        None,
    )
    .await
    .unwrap();
    assert!(built
        .items
        .iter()
        .all(|item| !item.kind.starts_with("notebook")));
    assert!(!built.system.contains("private-result-marker"));
    assert!(!built.system.contains("SELECT * FROM users"));
}

#[test]
fn notebook_requests_keep_legacy_defaults_and_reject_invalid_snapshots() {
    let old = spec();
    assert!(old.include.notebook);
    assert!(!old.include.notebook_results);
    let mut selected = notebook_spec();
    assert!(selected.validate().is_ok());
    selected.notebook.as_mut().unwrap().results[0]
        .rows
        .push(vec!["界".repeat(201)]);
    assert!(selected.validate().is_err());
}

async fn fixture() -> (tempfile::TempDir, ConnectionParams, SqliteDriver) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("context.sqlite");
    let pool = SqlitePoolOptions::new()
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    (
        directory,
        ConnectionParams {
            driver: "sqlite".into(),
            database: DatabaseSelection::Single(path.to_string_lossy().into()),
            password: Some("private-password-marker".into()),
            ..Default::default()
        },
        SqliteDriver::new(),
    )
}

fn request() -> ChatRequest {
    ChatRequest {
        provider: "openai".into(),
        model: "fixture-model".into(),
        system: "client-system-marker".into(),
        messages: vec![Message::user("question")],
        tools: vec![crate::ai::provider::ToolDef {
            name: "client-tool".into(),
            description: "fixture".into(),
            parameters: serde_json::json!({}),
        }],
        max_output_tokens: None,
        reasoning: None,
        resolved_images: Default::default(),
    }
}

#[tokio::test]
async fn selection_only_actions_resolve_tables_and_gaps_without_the_excluded_query() {
    let (_directory, params, driver) = fixture().await;
    let mut selected = spec();
    selected.command = Some(super::super::slash::ChatCommand::Explain);
    selected.include.query = false;
    selected.editor.query = Some("SELECT * FROM private-excluded-query-marker".into());
    selected.editor.selection =
        Some("SELECT * FROM users JOIN missing ON users.id = missing.id".into());
    let built = resolve_with_driver(
        &selected,
        &params,
        &driver,
        false,
        &SchemaCache::default(),
        None,
        None,
    )
    .await
    .unwrap();
    assert!(built
        .items
        .iter()
        .any(|item| item.kind == "table" && item.label == "users"));
    assert!(built.items.iter().any(|item| item.kind == "gaps"));
    assert!(built.items.iter().any(|item| item.kind == "selection"));
    assert!(!built.items.iter().any(|item| item.kind == "query"));
    assert!(built.system.contains("missing"));
    assert!(!built.system.contains("private-excluded-query-marker"));
}

#[tokio::test]
async fn saved_instructions_are_fresh_independent_removable_and_metadata_only() {
    let (_directory, params, driver) = fixture().await;
    let mut selected = spec();
    selected.include.connection = false;
    let cache = SchemaCache::default();
    for text in [
        "Prefer explicit JOINs. private-instruction-marker",
        "Use aliases. updated-marker",
    ] {
        let built =
            resolve_with_driver(&selected, &params, &driver, false, &cache, None, Some(text))
                .await
                .unwrap();
        assert!(built.system.contains(text));
        assert!(built
            .system
            .contains("Connection instructions are user preferences"));
        let preview = super::super::preview::ContextPreview::from(
            resolve_with_driver(&selected, &params, &driver, false, &cache, None, Some(text))
                .await
                .unwrap(),
        );
        let mut req = request();
        apply(&mut req, built).unwrap();
        let sent = req.messages[0]
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Context { items } => Some(items),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            serde_json::to_value(sent).unwrap(),
            serde_json::to_value(&preview.items).unwrap()
        );
        let item = preview
            .items
            .iter()
            .find(|item| item.kind == "instructions")
            .unwrap();
        assert!(item.chars.is_some_and(|size| size > text.chars().count()));
        let metadata = serde_json::to_string(&preview).unwrap();
        assert!(!metadata.contains(text));
        assert!(!metadata.contains("-marker"));
    }
    selected.include.instructions = false;
    let built = resolve_with_driver(
        &selected,
        &params,
        &driver,
        false,
        &cache,
        None,
        Some("excluded-marker"),
    )
    .await
    .unwrap();
    assert!(!built.system.contains("excluded-marker"));
    assert!(!built.items.iter().any(|item| item.kind == "instructions"));
    selected.include.instructions = true;
    let built = resolve_with_driver(&selected, &params, &driver, false, &cache, None, None)
        .await
        .unwrap();
    assert!(!built.items.iter().any(|item| item.kind == "instructions"));
}

#[test]
fn old_context_choices_default_instructions_on_and_client_text_is_rejected() {
    let old = serde_json::json!({"connection_id":"fixture", "database":null, "schema":null,
        "language":"en", "include":{"connection":false}});
    let selected: ContextSpec = serde_json::from_value(old.clone()).unwrap();
    assert!(selected.include.instructions);
    let mut spoofed = old;
    spoofed["ai_instructions"] = serde_json::json!("client-marker");
    assert!(serde_json::from_value::<ContextSpec>(spoofed).is_err());
}

#[tokio::test]
async fn rust_builds_scoped_context_and_attaches_only_names_and_sizes() {
    let (_directory, params, driver) = fixture().await;
    let built = resolve_with_driver(
        &spec(),
        &params,
        &driver,
        true,
        &SchemaCache::default(),
        None,
        None,
    )
    .await
    .unwrap();
    assert!(built.system.contains("SELECT * FROM users"));
    assert!(built.system.contains("The connection is read-only"));
    assert!(built.system.contains("\"name\":\"id\""));
    assert!(!built.system.contains("private-password-marker"));
    let mut req = request();
    let event = apply(&mut req, built).unwrap();
    assert!(!req.system.contains("client-system-marker"));
    assert!(req.tools.is_empty());
    assert!(matches!(
        req.messages[0].blocks.last(),
        Some(Block::Context { .. })
    ));
    let metadata = serde_json::to_string(&event).unwrap();
    assert!(metadata.contains("\"chars\":"));
    assert!(metadata.contains("\"id\":\"users\""));
    assert!(!metadata.contains("SELECT"));
    assert!(!metadata.contains("private-password-marker"));
    assert!(!metadata.contains("PRIMARY"));
}

#[tokio::test]
async fn excluded_context_is_not_loaded_or_sent_and_explicit_tables_take_priority() {
    let (_directory, params, driver) = fixture().await;
    let mut selected = spec();
    selected.include.query = false;
    selected.include.schema = false;
    selected.include.connection = false;
    selected.editor.selection = Some("private-selection-marker".into());
    selected.include.selection = false;
    selected.editor.cursor_statement = Some("private-cursor-marker".into());
    selected.include.cursor_statement = false;
    let invalid_params = ConnectionParams {
        driver: "sqlite".into(),
        database: DatabaseSelection::Single("/missing/context.sqlite".into()),
        ..Default::default()
    };
    let built = resolve_with_driver(
        &selected,
        &invalid_params,
        &driver,
        false,
        &SchemaCache::default(),
        None,
        None,
    )
    .await
    .unwrap();
    assert!(built.items.is_empty());
    assert!(!built.system.contains("private-selection-marker"));
    assert!(!built.system.contains("private-cursor-marker"));
    selected.tables = vec!["users".into()];
    let built = resolve_with_driver(
        &selected,
        &params,
        &driver,
        false,
        &SchemaCache::default(),
        None,
        None,
    )
    .await
    .unwrap();
    assert!(built
        .items
        .iter()
        .any(|item| item.kind == "table" && item.id == "users"));
    selected.excluded_tables = vec!["users".into()];
    let built = resolve_with_driver(
        &selected,
        &params,
        &driver,
        false,
        &SchemaCache::default(),
        None,
        None,
    )
    .await
    .unwrap();
    assert!(!built.items.iter().any(|item| item.kind == "table"));
}

#[tokio::test]
async fn backend_refresh_changes_context_while_identical_requests_share_metadata() {
    let (_directory, params, driver) = fixture().await;
    let cache = SchemaCache::default();
    let first = resolve_with_driver(&spec(), &params, &driver, false, &cache, None, None)
        .await
        .unwrap();
    let path = params.database.primary();
    let pool = SqlitePoolOptions::new()
        .connect_with(SqliteConnectOptions::new().filename(path))
        .await
        .unwrap();
    sqlx::query("ALTER TABLE users ADD COLUMN refreshed_column TEXT")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    let cached = resolve_with_driver(&spec(), &params, &driver, false, &cache, None, None)
        .await
        .unwrap();
    assert_eq!(first.system, cached.system);
    cache.invalidate_connection("fixture");
    let refreshed = resolve_with_driver(&spec(), &params, &driver, false, &cache, None, None)
        .await
        .unwrap();
    assert!(refreshed.system.contains("refreshed_column"));
}

#[test]
fn context_wire_rejects_unbounded_fields_and_model_controlled_mode_or_credentials() {
    let mut selected = spec();
    selected.tables = vec!["users".into(); 101];
    assert!(selected.validate().is_err());
    selected.tables.clear();
    selected.editor.query = Some("x".repeat(1_048_577));
    assert!(selected.validate().is_err());
    for extra in ["mode", "readonly", "password", "driver", "system"] {
        let mut value = serde_json::to_value(spec()).unwrap();
        value[extra] = serde_json::json!("untrusted-value");
        assert!(serde_json::from_value::<ContextSpec>(value).is_err());
    }
}

#[tokio::test]
async fn context_cannot_be_attached_to_an_assistant_or_ambiguous_message_id() {
    let (_directory, params, driver) = fixture().await;
    let mut req = request();
    req.messages[0].role = Role::Assistant;
    let built = resolve_with_driver(
        &spec(),
        &params,
        &driver,
        false,
        &SchemaCache::default(),
        None,
        None,
    )
    .await
    .unwrap();
    assert!(apply(&mut req, built).is_err());
    req.messages[0].role = Role::User;
    req.messages.push(req.messages[0].clone());
    let built = resolve_with_driver(
        &spec(),
        &params,
        &driver,
        false,
        &SchemaCache::default(),
        None,
        None,
    )
    .await
    .unwrap();
    assert!(apply(&mut req, built).is_err());
}

#[test]
fn older_context_blocks_without_sizes_still_deserialize() {
    let block: Block = serde_json::from_value(serde_json::json!({"type":"context","items":[{"kind":"table","id":"users","label":"users"}]})).unwrap();
    let Block::Context { items } = block else {
        panic!("context block expected");
    };
    assert_eq!(items[0].chars, None);
    assert!(!items[0].truncated);
}

#[test]
fn sent_context_metadata_round_trips_through_encrypted_conversation_storage() {
    let directory = crate::secure_store::testing::TestDirectory::new().unwrap();
    let store = crate::ai_chat::store::ConversationStore::new(directory.path());
    let mut conversation = fixtures::conversation();
    conversation.messages[0].blocks.push(Block::Context {
        items: vec![ContextItemRef {
            view_count: None,

            kind: "table".into(),
            id: "private-table-marker".into(),
            label: "private-table-marker".into(),
            chars: Some(123),
            table_count: None,
            truncated: true,
        }],
    });
    store.save(conversation.clone()).unwrap();
    assert_eq!(
        store
            .load(&conversation.connection_id, &conversation.id)
            .unwrap()
            .messages,
        conversation.messages
    );
    let sealed =
        std::fs::read_to_string(directory.path().join("ai_chats/connection/chat.chat")).unwrap();
    assert!(!sealed.contains("private-table-marker"));
}

#[tokio::test]
async fn preview_matches_sent_summaries_without_prompt_or_editor_contents() {
    let (_directory, params, driver) = fixture().await;
    let cache = SchemaCache::default();
    let mut selected = spec();
    selected.editor.query = Some("SELECT * FROM users -- private-editor-marker 界".into());
    let built = resolve_with_driver(&selected, &params, &driver, false, &cache, None, None)
        .await
        .unwrap();
    let expected_size = built.system.chars().count();
    let preview = super::super::preview::ContextPreview::from(built);
    assert_eq!(preview.total_chars, expected_size);
    assert_eq!(
        preview
            .items
            .iter()
            .find(|item| item.kind == "schema")
            .unwrap()
            .table_count,
        Some(1)
    );
    let mut req = request();
    let built = resolve_with_driver(&selected, &params, &driver, false, &cache, None, None)
        .await
        .unwrap();
    let ContextEvent::Context { items, .. } = apply(&mut req, built).unwrap();
    assert_eq!(preview.items, items);
    let wire = serde_json::to_string(&preview).unwrap();
    for excluded in [
        "SELECT",
        "private-editor-marker",
        "private-password-marker",
        "PRIMARY",
        "untrusted_data",
        "system",
    ] {
        assert!(!wire.contains(excluded));
    }
}

#[tokio::test]
async fn preview_exposes_budget_truncation_and_respects_table_and_schema_exclusion() {
    let (_directory, params, driver) = fixture().await;
    let mut selected = spec();
    selected.editor.query = Some(format!("SELECT * FROM users; -- {}", "界".repeat(60_000)));
    selected.excluded_tables = vec!["users".into()];
    let built = resolve_with_driver(
        &selected,
        &params,
        &driver,
        false,
        &SchemaCache::default(),
        None,
        None,
    )
    .await
    .unwrap();
    let preview = super::super::preview::ContextPreview::from(built);
    assert!(preview.truncated);
    assert!(preview
        .items
        .iter()
        .any(|item| item.kind == "query" && item.truncated));
    assert!(!preview.items.iter().any(|item| item.kind == "table"));
    selected.include.schema = false;
    let built = resolve_with_driver(
        &selected,
        &params,
        &driver,
        false,
        &SchemaCache::default(),
        None,
        None,
    )
    .await
    .unwrap();
    assert!(built.items.iter().all(|item| item.table_count.is_none()));
}

#[tokio::test]
async fn view_references_mentions_exclusions_and_saved_query_summaries_reach_the_same_resolver() {
    let (_directory, params, driver) = fixture().await;
    let pool = SqlitePoolOptions::new()
        .connect_with(SqliteConnectOptions::new().filename(params.database.primary()))
        .await
        .unwrap();
    sqlx::query("CREATE VIEW v_users AS SELECT id, name FROM users")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    let mut selected = spec();
    selected.editor.query = Some("SELECT * FROM v_users".into());
    let cache = SchemaCache::default();
    let built = resolve_with_driver(&selected, &params, &driver, false, &cache, None, None)
        .await
        .unwrap();
    assert!(built
        .items
        .iter()
        .any(|item| item.kind == "view" && item.id == "v_users"));
    assert_eq!(
        built
            .items
            .iter()
            .find(|item| item.kind == "schema")
            .unwrap()
            .view_count,
        Some(1)
    );
    selected.excluded_views = vec!["v_users".into()];
    let built = resolve_with_driver(&selected, &params, &driver, false, &cache, None, None)
        .await
        .unwrap();
    assert!(!built.items.iter().any(|item| item.kind == "view"));
    selected.include.schema = false;
    selected.include.query = false;
    selected.excluded_views.clear();
    selected.views = vec!["v_users".into()];
    let saved = SavedQueryContexts {
        queries: vec![super::super::context::SavedQueryContext {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Revenue".into(),
            sql: "SELECT 'private-saved-marker'".into(),
            truncated: false,
        }],
        unavailable: Vec::new(),
    };
    let built = resolve_with_driver(
        &selected,
        &params,
        &driver,
        false,
        &cache,
        Some(&saved),
        None,
    )
    .await
    .unwrap();
    assert!(built.items.iter().any(|item| item.kind == "view"));
    assert!(!built
        .items
        .iter()
        .any(|item| item.kind == "schema" || item.kind == "query"));
    let preview = super::super::preview::ContextPreview::from(built);
    assert!(preview
        .items
        .iter()
        .any(|item| item.kind == "saved_query" && item.label == "Revenue"));
    assert!(!serde_json::to_string(&preview)
        .unwrap()
        .contains("private-saved-marker"));
}

#[test]
fn mention_limits_and_effective_databases_match_flat_and_schema_based_scopes() {
    let mut selected = spec();
    selected.saved_queries = vec![uuid::Uuid::new_v4().to_string(); 21];
    assert!(selected.validate().is_err());
    selected.saved_queries = vec!["../private".into()];
    assert!(selected.validate().is_err());
    selected.saved_queries.clear();
    selected.tables = vec!["users".into(); 100];
    selected.views = vec!["v_users".into()];
    assert!(selected.validate().is_err());
    selected.database = Some("app".into());
    selected.schema = Some("analytics".into());
    let params = ConnectionParams::default();
    let mut capabilities = crate::drivers::driver_trait::DriverCapabilities::default();
    assert_eq!(
        selection::effective_database(&selected, &params, &capabilities),
        Some("analytics")
    );
    capabilities.schemas = true;
    assert_eq!(
        selection::effective_database(&selected, &params, &capabilities),
        Some("app")
    );
    capabilities.schemas = false;
    capabilities.file_based = true;
    assert_eq!(
        selection::effective_database(&selected, &params, &capabilities),
        Some("app")
    );
}

#[test]
fn typed_mentions_are_deduplicated_prioritized_and_excluded_independently() {
    use crate::ai_schema_context::detailed::{ObjectKind, ObjectRef, SchemaCatalog};
    use crate::models::TableInfo;
    let mut selected = spec();
    selected.tables = vec!["users".into(), "users".into()];
    selected.views = vec!["users".into(), "missing_view".into()];
    let catalog = SchemaCatalog {
        tables: vec![TableInfo {
            name: "users".into(),
            comment: None,
        }],
        views: vec![
            TableInfo {
                name: "users".into(),
                comment: None,
            },
            TableInfo {
                name: "v_users".into(),
                comment: None,
            },
        ],
        ..Default::default()
    };
    let objects =
        selection::selected_objects(&selected, &catalog, "SELECT * FROM v_users", None, None);
    assert_eq!(
        objects,
        vec![
            ObjectRef {
                kind: ObjectKind::Table,
                name: "users".into()
            },
            ObjectRef {
                kind: ObjectKind::View,
                name: "users".into()
            },
            ObjectRef {
                kind: ObjectKind::View,
                name: "missing_view".into()
            },
            ObjectRef {
                kind: ObjectKind::View,
                name: "v_users".into()
            }
        ]
    );
    selected.excluded_views = vec!["users".into(), "v_users".into()];
    let objects =
        selection::selected_objects(&selected, &catalog, "SELECT * FROM v_users", None, None);
    assert_eq!(objects[0].kind, ObjectKind::Table);
    assert_eq!(objects.len(), 2);
}
