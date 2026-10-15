use super::*;
use crate::drivers::sqlite::SqliteDriver;
use crate::models::DatabaseSelection;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

mod bounds;

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
    for query in [
        "CREATE TABLE teams (id INTEGER PRIMARY KEY)",
        "CREATE TABLE users (id INTEGER PRIMARY KEY, team_id INTEGER NOT NULL DEFAULT 1 REFERENCES teams(id), name TEXT)",
        "CREATE UNIQUE INDEX users_name ON users(name)",
    ] {
        sqlx::query(query).execute(&pool).await.unwrap();
    }
    pool.close().await;
    let params = ConnectionParams {
        driver: "sqlite".into(),
        database: DatabaseSelection::Single(path.to_string_lossy().into()),
        ..Default::default()
    };
    (directory, params, SqliteDriver::new())
}

#[tokio::test]
async fn catalog_is_sorted_and_details_include_indexes_columns_and_foreign_keys() {
    let (_directory, params, driver) = fixture().await;
    let mut catalog = load_catalog(&driver, &params, None).await.unwrap();
    assert_eq!(
        catalog
            .tables
            .iter()
            .map(|table| table.name.as_str())
            .collect::<Vec<_>>(),
        ["teams", "users"]
    );
    assert_eq!(catalog.total_table_count, 2);
    assert!(!catalog.truncated);
    catalog.tables[1].comment = Some("Customer accounts".into());
    let context =
        load_detailed_from_driver(&driver, &params, None, &catalog, &["users".into()], 20).await;
    assert_eq!(context.tables.len(), 1);
    let table = &context.tables[0];
    assert_eq!(table.comment.as_deref(), Some("Customer accounts"));
    assert!(table
        .columns
        .iter()
        .any(|column| column.name == "id" && column.is_pk));
    assert!(table.columns.iter().any(|column| column.name == "team_id"
        && !column.is_nullable
        && column.default_value.as_deref() == Some("1")));
    assert!(table
        .foreign_keys
        .iter()
        .any(|key| key.column_name == "team_id" && key.ref_table == "teams"));
    assert!(table
        .indexes
        .iter()
        .any(|index| index.name == "users_name" && index.is_unique));
    assert!(table.unavailable.is_empty());
    assert!(!context.truncated);
}

#[tokio::test]
async fn selected_order_duplicates_missing_names_and_zero_limit_are_preserved() {
    let (_directory, params, driver) = fixture().await;
    let catalog = load_catalog(&driver, &params, None).await.unwrap();
    let names = vec![
        "users".into(),
        "missing".into(),
        "teams".into(),
        "users".into(),
    ];
    let context = load_detailed_from_driver(&driver, &params, None, &catalog, &names, 1).await;
    assert_eq!(context.tables[0].name, "users");
    assert_eq!(context.missing_tables, ["missing"]);
    assert_eq!(context.omitted_tables, ["teams"]);
    assert!(context.truncated);
    let context = load_detailed_from_driver(&driver, &params, None, &catalog, &names, 0).await;
    assert!(context.tables.is_empty());
    assert_eq!(context.omitted_tables, ["users", "teams"]);
    let context = load_detailed_from_driver(&driver, &params, None, &catalog, &[], 20).await;
    assert!(context.tables.is_empty());
    assert!(!context.truncated);
}

#[tokio::test]
async fn failed_calls_are_marked_without_including_database_errors() {
    let directory = tempfile::tempdir().unwrap();
    let params = ConnectionParams {
        driver: "sqlite".into(),
        database: DatabaseSelection::Single(
            directory
                .path()
                .join("missing/private-context.sqlite")
                .to_string_lossy()
                .into(),
        ),
        ..Default::default()
    };
    let driver = SqliteDriver::new();
    let catalog = SchemaCatalog {
        tables: vec![TableInfo {
            name: "users".into(),
            comment: None,
        }],
        total_table_count: 1,
        truncated: false,
        ..Default::default()
    };
    let context = load_detailed_from_driver(
        &driver,
        &params,
        None,
        &catalog,
        &["users".into(), "not_in_catalog".into()],
        20,
    )
    .await;
    assert_eq!(
        context.tables[0].unavailable,
        [
            MetadataKind::Columns,
            MetadataKind::ForeignKeys,
            MetadataKind::Indexes
        ]
    );
    assert_eq!(context.missing_tables, ["not_in_catalog"]);
    let json = serde_json::to_string(&context).unwrap();
    assert!(!json.contains("private-context"));
    assert!(!json.contains("unable to open"));
}

#[tokio::test]
async fn views_have_their_own_metadata_calls_and_share_the_selected_object_limit() {
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
    let catalog = load_catalog(&driver, &params, None).await.unwrap();
    assert_eq!(catalog.total_view_count, 1);
    assert_eq!(catalog.views[0].name, "v_users");
    assert!(!catalog.views_unavailable);
    let objects = vec![
        ObjectRef {
            kind: ObjectKind::View,
            name: "v_users".into(),
        },
        ObjectRef {
            kind: ObjectKind::Table,
            name: "users".into(),
        },
        ObjectRef {
            kind: ObjectKind::View,
            name: "absent".into(),
        },
    ];
    let context =
        load_detailed_objects_from_driver(&driver, &params, None, &catalog, &objects, 1).await;
    assert_eq!(context.tables.len(), 1);
    assert_eq!(context.tables[0].kind, ObjectKind::View);
    assert!(context.tables[0]
        .columns
        .iter()
        .any(|column| column.name == "name"));
    assert!(context.tables[0]
        .definition
        .as_deref()
        .unwrap()
        .contains("FROM users"));
    assert!(context.tables[0].unavailable.is_empty());
    assert_eq!(context.omitted_tables, ["users"]);
    assert_eq!(context.missing_views, ["absent"]);
    assert!(context.truncated);
    let known = reference_catalog(&catalog);
    assert!(known.iter().any(|item| item.name == "v_users"));
    assert!(known.iter().any(|item| item.name == "users"));
}

#[tokio::test]
async fn failed_view_columns_and_definition_are_explicit_without_driver_errors() {
    let dir = tempfile::tempdir().unwrap();
    let params = ConnectionParams {
        driver: "sqlite".into(),
        database: DatabaseSelection::Single(
            dir.path()
                .join("missing/private-marker.sqlite")
                .to_string_lossy()
                .into(),
        ),
        ..Default::default()
    };
    let catalog = SchemaCatalog {
        views: vec![TableInfo {
            name: "v_users".into(),
            comment: None,
        }],
        total_view_count: 1,
        ..Default::default()
    };
    let context = load_detailed_objects_from_driver(
        &SqliteDriver::new(),
        &params,
        None,
        &catalog,
        &[ObjectRef {
            kind: ObjectKind::View,
            name: "v_users".into(),
        }],
        20,
    )
    .await;
    assert_eq!(
        context.tables[0].unavailable,
        [MetadataKind::Columns, MetadataKind::Definition]
    );
    assert!(!serde_json::to_string(&context)
        .unwrap()
        .contains("private-marker"));
}
