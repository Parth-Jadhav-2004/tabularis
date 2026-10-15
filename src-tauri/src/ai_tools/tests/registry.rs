use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use crate::ai_approval::{ApprovalDecision, PendingApproval, PollOutcome};
use crate::ai_chat::types::ChatMode;
use crate::ai_tools::approval::{ApprovalGate, ApprovalQueueError};
use crate::ai_tools::{database, safety, ToolContext, ToolError, ToolOrigin, ToolRegistry};
use crate::config::AppConfig;
use crate::drivers::driver_trait::DatabaseDriver;
use crate::drivers::sqlite::SqliteDriver;
use crate::models::{ConnectionParams, DatabaseSelection, SavedConnection};

#[derive(Default)]
enum Decision {
    #[default]
    Approve,
    Deny,
    Edited(String),
    Mismatch,
    Timeout,
    Unavailable,
    Error,
}

#[derive(Default)]
struct RecordingGate {
    requests: Mutex<Vec<PendingApproval>>,
    decision: Decision,
}

#[async_trait]
impl ApprovalGate for RecordingGate {
    async fn request(
        &self,
        request: &PendingApproval,
        _: u64,
    ) -> Result<PollOutcome, ApprovalQueueError> {
        self.requests.lock().unwrap().push(request.clone());
        match &self.decision {
            Decision::Timeout => return Ok(PollOutcome::TimedOut),
            Decision::Unavailable => return Ok(PollOutcome::HostUnavailable),
            Decision::Error => {
                return Err(ApprovalQueueError::Poll("Fixture approval error".into()))
            }
            _ => {}
        }
        Ok(PollOutcome::Decided(ApprovalDecision {
            approval_id: if matches!(self.decision, Decision::Mismatch) {
                "another-request".into()
            } else {
                request.id.clone()
            },
            decided_at: crate::ai_activity::now_iso8601(),
            decision: if matches!(self.decision, Decision::Deny) {
                "deny"
            } else {
                "approve"
            }
            .into(),
            reason: None,
            edited_query: match &self.decision {
                Decision::Edited(query) => Some(query.clone()),
                _ => None,
            },
        }))
    }
}

async fn fixture(
    mode: ChatMode,
    readonly: bool,
    gate: Arc<RecordingGate>,
) -> (ToolContext, tempfile::NamedTempFile) {
    let file = tempfile::NamedTempFile::new().unwrap();
    let pool = SqlitePoolOptions::new()
        .connect_with(SqliteConnectOptions::new().filename(file.path()))
        .await
        .unwrap();
    for query in [
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)",
        "CREATE INDEX users_name ON users(name)",
        "INSERT INTO users (name) VALUES ('Alice'), ('Bob'), ('Carol')",
        "CREATE TABLE \"odd\"\"name;--\" (value TEXT)",
        "INSERT INTO \"odd\"\"name;--\" VALUES ('quoted identifier')",
    ] {
        sqlx::query(query).execute(&pool).await.unwrap();
    }
    pool.close().await;
    let params = ConnectionParams {
        driver: "sqlite".into(),
        database: DatabaseSelection::Single(file.path().to_str().unwrap().into()),
        ..Default::default()
    };
    let connection = SavedConnection {
        id: "fixture-connection".into(),
        name: "Fixture connection".into(),
        params: params.clone(),
        group_id: None,
        sort_order: None,
        detect_json_in_text_columns: None,
        ai_instructions: None,
        appearance: None,
        tag_ids: None,
        environment: None,
    };
    (
        ToolContext {
            connection,
            params,
            driver: Arc::new(SqliteDriver::new()),
            config: AppConfig {
                ai_audit_enabled: Some(false),
                mcp_approval_mode: Some("off".into()),
                ..Default::default()
            },
            database: None,
            schema: None,
            readonly,
            mode,
            origin: ToolOrigin::Chat {
                conversation_id: "conversation".into(),
                run_id: "run".into(),
            },
            approval: gate,
        },
        file,
    )
}

async fn names(context: &ToolContext) -> Vec<Value> {
    ToolRegistry
        .run(
            "run_query",
            context,
            json!({"query":"SELECT name FROM users ORDER BY id"}),
        )
        .await
        .unwrap()
        .rows
        .unwrap()
        .rows
        .into_iter()
        .map(|row| row[0].clone())
        .collect()
}

#[tokio::test]
async fn metadata_tools_share_driver_operations_and_omit_credentials() {
    let (mut context, _file) = fixture(ChatMode::Ask, false, Arc::default()).await;
    context.connection.params.password = Some("private-password-marker".into());
    context.connection.params.connection_uri = Some("private-uri-marker".into());
    let output = ToolRegistry
        .run("list_connections", &context, json!({}))
        .await
        .unwrap();
    let connections: Value = serde_json::from_str(&output.content).unwrap();
    assert_eq!(connections[0]["id"], context.connection_id());
    assert!(!output.content.contains("private-"));
    let output = ToolRegistry
        .run("list_databases", &context, json!({}))
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&output.content).unwrap(),
        database::list_databases(context.driver.as_ref(), &context.params)
            .await
            .unwrap()
    );
    let output = ToolRegistry
        .run("list_tables", &context, json!({}))
        .await
        .unwrap();
    assert!(serde_json::from_str::<Vec<String>>(&output.content)
        .unwrap()
        .contains(&"users".into()));
    let output = ToolRegistry
        .run("describe_table", &context, json!({"table_name":"users"}))
        .await
        .unwrap();
    let description: Value = serde_json::from_str(&output.content).unwrap();
    assert_eq!(description["table"], "users");
    assert_eq!(description["columns"].as_array().unwrap().len(), 2);
    assert!(!description["indexes"].as_array().unwrap().is_empty());
    assert_eq!(
        description,
        database::describe_table(context.driver.as_ref(), &context.params, "users", None)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn sample_rows_handles_quoted_identifiers_and_limits() {
    let (context, _file) = fixture(ChatMode::Ask, false, Arc::default()).await;
    let output = ToolRegistry
        .run(
            "sample_rows",
            &context,
            json!({"table_name":"users", "limit":2}),
        )
        .await
        .unwrap();
    assert_eq!(output.rows.unwrap().rows.len(), 2);
    let output = ToolRegistry
        .run(
            "sample_rows",
            &context,
            json!({"table_name":"odd\"name;--"}),
        )
        .await
        .unwrap();
    assert_eq!(output.rows.unwrap().rows[0][0], "quoted identifier");
    assert_eq!(names(&context).await.len(), 3);
}

#[tokio::test]
async fn read_queries_reject_mutations_and_stacked_statements_without_approval() {
    let gate = Arc::new(RecordingGate::default());
    let (context, _file) = fixture(ChatMode::Agent, false, gate.clone()).await;
    for query in [
        "DELETE FROM users",
        "UPDATE users SET name='changed'",
        "DROP TABLE users",
        "WITH q AS (SELECT 1) DELETE FROM users",
        "WITH q AS (DELETE FROM users RETURNING *) SELECT * FROM q",
        "SELECT 1; DELETE FROM users",
        "PRAGMA query_only=0",
        "BEGIN",
        "SELECT 1 INTO stolen",
    ] {
        assert!(
            matches!(
                ToolRegistry
                    .run("run_query", &context, json!({"query":query}))
                    .await,
                Err(ToolError::Safety(_))
            ),
            "{query}"
        );
    }
    assert_eq!(
        names(&context).await,
        vec![json!("Alice"), json!("Bob"), json!("Carol")]
    );
    assert!(gate.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn definitions_and_execution_enforce_ask_and_readonly_policy() {
    let gate = Arc::new(RecordingGate::default());
    let (mut context, _file) = fixture(ChatMode::Ask, false, gate.clone()).await;
    assert_eq!(ToolRegistry.tools_for(ChatMode::Ask, &context).len(), 7);
    assert_eq!(ToolRegistry.tools_for(ChatMode::Agent, &context).len(), 8);
    assert_eq!(
        ToolRegistry
            .run(
                "run_statement",
                &context,
                json!({"query":"DELETE FROM users"})
            )
            .await,
        Err(ToolError::NotAvailable)
    );
    context.mode = ChatMode::Agent;
    context.readonly = true;
    assert_eq!(ToolRegistry.tools_for(ChatMode::Agent, &context).len(), 7);
    assert_eq!(
        ToolRegistry
            .run(
                "run_statement",
                &context,
                json!({"query":"DELETE FROM users"})
            )
            .await,
        Err(ToolError::NotAvailable)
    );
    context.readonly = false;
    context.config.mcp_readonly_connections = Some(vec![context.connection.id.clone()]);
    assert_eq!(ToolRegistry.tools_for(ChatMode::Agent, &context).len(), 7);
    assert!(gate.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn model_arguments_cannot_override_scope_or_bypass_validation() {
    let (context, _file) = fixture(ChatMode::Agent, false, Arc::default()).await;
    for args in [
        json!({"query":"SELECT 1", "connection_id":"other"}),
        json!({"query":"SELECT 1", "database":"other"}),
        json!({"query":"SELECT 1", "schema":"other"}),
        json!({"query":"SELECT 1", "limit":-1}),
        json!({"query":"SELECT 1", "limit":0}),
        json!({"query":"SELECT 1", "limit":1.5}),
        json!({"query":"SELECT 1", "limit":"100"}),
        json!({"query":"SELECT 1", "limit":4294967296u64}),
        json!({}),
        json!([]),
    ] {
        assert_eq!(
            ToolRegistry.run("run_query", &context, args).await,
            Err(ToolError::InvalidArguments)
        );
    }
    assert_eq!(
        ToolRegistry.run("nonexistent", &context, json!({})).await,
        Err(ToolError::NotAvailable)
    );
}

#[tokio::test]
async fn queries_apply_default_and_hard_row_caps() {
    let (context, _file) = fixture(ChatMode::Ask, false, Arc::default()).await;
    let query = "WITH RECURSIVE seq(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM seq WHERE x<1205) SELECT x FROM seq";
    for (limit, expected) in [(None, 50), (Some(12), 12), (Some(5000), 1000)] {
        let mut args = json!({"query":query});
        if let Some(limit) = limit {
            args["limit"] = json!(limit);
        }
        let output = ToolRegistry.run("run_query", &context, args).await.unwrap();
        assert_eq!(output.rows.unwrap().rows.len(), expected);
        assert!(output.truncated);
    }
}

#[tokio::test]
async fn explain_is_planning_only_and_does_not_mutate_the_fixture() {
    let (context, _file) = fixture(ChatMode::Ask, false, Arc::default()).await;
    let output = ToolRegistry
        .run(
            "explain_query",
            &context,
            json!({"query":"SELECT name FROM users"}),
        )
        .await
        .unwrap();
    assert!(!output.content.is_empty());
    for query in [
        "DELETE FROM users",
        "EXPLAIN ANALYZE DELETE FROM users",
        "SELECT 1; DELETE FROM users",
    ] {
        assert!(matches!(
            ToolRegistry
                .run("explain_query", &context, json!({"query":query}))
                .await,
            Err(ToolError::Safety(_))
        ));
    }
    assert_eq!(names(&context).await.len(), 3);
}

#[tokio::test]
async fn each_write_requires_fresh_approval_bound_to_sql_and_target_even_when_mcp_is_off() {
    let gate = Arc::new(RecordingGate::default());
    let (mut context, _file) = fixture(ChatMode::Agent, false, gate.clone()).await;
    context.schema = Some("main".into());
    for query in [
        "UPDATE users SET name='Approved' WHERE id=1",
        "UPDATE users SET name='Again' WHERE id=1",
    ] {
        let output = ToolRegistry
            .run("run_statement", &context, json!({"query":query}))
            .await
            .unwrap();
        assert_eq!(output.rows.unwrap().affected_rows, 1);
    }
    assert_eq!(names(&context).await[0], "Again");
    let requests = gate.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_ne!(requests[0].id, requests[1].id);
    assert_eq!(requests[0].connection_id, context.connection.id);
    assert_eq!(requests[0].schema.as_deref(), Some("main"));
    assert_eq!(
        requests[0].database.as_deref(),
        context.params.database.as_vec().first().map(String::as_str)
    );
    assert_eq!(
        requests[0].query,
        "UPDATE users SET name='Approved' WHERE id=1"
    );
}

#[tokio::test]
async fn denial_timeouts_mismatches_and_transport_errors_never_execute() {
    for decision in [
        Decision::Deny,
        Decision::Timeout,
        Decision::Unavailable,
        Decision::Mismatch,
        Decision::Error,
    ] {
        let gate = Arc::new(RecordingGate {
            decision,
            ..Default::default()
        });
        let (context, _file) = fixture(ChatMode::Agent, false, gate.clone()).await;
        assert!(matches!(
            ToolRegistry
                .run(
                    "run_statement",
                    &context,
                    json!({"query":"DELETE FROM users"})
                )
                .await,
            Err(ToolError::Approval { .. })
        ));
        assert_eq!(names(&context).await.len(), 3);
        assert_eq!(gate.requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn approved_edits_are_revalidated_and_only_the_approved_sql_is_executed() {
    for (edited, allowed) in [
        ("UPDATE users SET name='Edited' WHERE id=2", true),
        ("DELETE FROM users; DROP TABLE users", false),
        ("BEGIN", false),
    ] {
        let gate = Arc::new(RecordingGate {
            decision: Decision::Edited(edited.into()),
            ..Default::default()
        });
        let (context, _file) = fixture(ChatMode::Agent, false, gate).await;
        let result = ToolRegistry
            .run(
                "run_statement",
                &context,
                json!({"query":"DELETE FROM users"}),
            )
            .await;
        assert_eq!(result.is_ok(), allowed);
        let names = names(&context).await;
        assert_eq!(names.len(), 3);
        assert_eq!(names[1], if allowed { "Edited" } else { "Bob" });
    }
}

#[tokio::test]
async fn unsupported_or_multiple_statements_never_reach_the_gate() {
    let gate = Arc::new(RecordingGate::default());
    let (context, _file) = fixture(ChatMode::Agent, false, gate.clone()).await;
    for query in [
        "BEGIN",
        "VACUUM",
        "DELETE FROM users; DROP TABLE users",
        "SELECT 1",
    ] {
        assert!(ToolRegistry
            .run("run_statement", &context, json!({"query":query}))
            .await
            .is_err());
    }
    assert!(gate.requests.lock().unwrap().is_empty());
    assert_eq!(names(&context).await.len(), 3);
}

#[tokio::test]
async fn shared_mcp_results_keep_their_existing_json_shape() {
    let (context, _file) = fixture(ChatMode::Ask, false, Arc::default()).await;
    let result = database::execute(
        context.driver.as_ref(),
        &context.params,
        "SELECT name FROM users",
        100,
        None,
        false,
    )
    .await
    .unwrap();
    let encoded =
        super::super::output::tool_result(&result, super::super::output::ToolOutputFormat::Json)
            .unwrap();
    assert_eq!(
        encoded["content"][0]["text"],
        serde_json::to_string_pretty(&result).unwrap()
    );
    assert_eq!(result.rows.len(), 3);
    assert_eq!(safety::require_read("SELECT name FROM users"), Ok(()));
}
