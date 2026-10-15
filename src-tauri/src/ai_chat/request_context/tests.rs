use super::*;
use crate::ai::message::Message;
use crate::drivers::sqlite::SqliteDriver;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

#[path = "../tests/fixtures.rs"]
mod fixtures;

fn spec() -> ContextSpec {
    serde_json::from_value(serde_json::json!({ "connection_id":"fixture", "database":null, "schema":null, "language":"it", "editor":{"query":"SELECT * FROM users"} })).unwrap()
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
async fn rust_builds_scoped_context_and_attaches_only_names_and_sizes() {
    let (_directory, params, driver) = fixture().await;
    let built = resolve_with_driver(&spec(), &params, &driver, true, &SchemaCache::default())
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
    )
    .await
    .unwrap();
    assert!(built.items.is_empty());
    assert!(!built.system.contains("private-selection-marker"));
    assert!(!built.system.contains("private-cursor-marker"));
    selected.tables = vec!["users".into()];
    let built = resolve_with_driver(&selected, &params, &driver, false, &SchemaCache::default())
        .await
        .unwrap();
    assert!(built
        .items
        .iter()
        .any(|item| item.kind == "table" && item.id == "users"));
    selected.excluded_tables = vec!["users".into()];
    let built = resolve_with_driver(&selected, &params, &driver, false, &SchemaCache::default())
        .await
        .unwrap();
    assert!(!built.items.iter().any(|item| item.kind == "table"));
}

#[tokio::test]
async fn backend_refresh_changes_context_while_identical_requests_share_metadata() {
    let (_directory, params, driver) = fixture().await;
    let cache = SchemaCache::default();
    let first = resolve_with_driver(&spec(), &params, &driver, false, &cache)
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
    let cached = resolve_with_driver(&spec(), &params, &driver, false, &cache)
        .await
        .unwrap();
    assert_eq!(first.system, cached.system);
    cache.invalidate_connection("fixture");
    let refreshed = resolve_with_driver(&spec(), &params, &driver, false, &cache)
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
    let built = resolve_with_driver(&spec(), &params, &driver, false, &SchemaCache::default())
        .await
        .unwrap();
    assert!(apply(&mut req, built).is_err());
    req.messages[0].role = Role::User;
    req.messages.push(req.messages[0].clone());
    let built = resolve_with_driver(&spec(), &params, &driver, false, &SchemaCache::default())
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
            kind: "table".into(),
            id: "private-table-marker".into(),
            label: "private-table-marker".into(),
            chars: Some(123),
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
