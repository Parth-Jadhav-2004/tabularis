use super::*;
use crate::ai::provider::ErrorKind;
use crate::ai_schema_context::detailed::{DetailedTable, MetadataKind};
use crate::models::TableInfo;
use std::sync::atomic::{AtomicUsize, Ordering};

fn key(connection: &str) -> ScopeKey {
    ScopeKey {
        connection_id: connection.into(),
        database: Some("app".into()),
        schema: Some("public".into()),
        driver: "postgres".into(),
    }
}

fn catalog(name: &str) -> SchemaCatalog {
    SchemaCatalog {
        tables: vec![TableInfo {
            name: name.into(),
            comment: None,
        }],
        total_table_count: 1,
        truncated: false,
        ..Default::default()
    }
}

#[test]
fn scope_expiry_detaches_old_inflight_batches_before_a_new_request() {
    let cache = SchemaCache {
        ttl: Duration::ZERO,
        ..Default::default()
    };
    let old = cache.scope(key("one"));
    assert!(!Arc::ptr_eq(&old, &cache.scope(key("one"))));
}

#[test]
fn scopes_are_isolated_and_invalidation_detaches_every_matching_scope() {
    let cache = SchemaCache::default();
    let original = cache.scope(key("one"));
    assert!(Arc::ptr_eq(&original, &cache.scope(key("one"))));
    let other_connection = cache.scope(key("two"));
    let mut other_key = key("one");
    other_key.database = Some("other".into());
    let other_database = cache.scope(other_key.clone());
    assert!(!Arc::ptr_eq(&original, &other_database));
    let mut schema_key = key("one");
    schema_key.schema = Some("private".into());
    assert!(!Arc::ptr_eq(&original, &cache.scope(schema_key)));
    let mut driver_key = key("one");
    driver_key.driver = "other-driver".into();
    assert!(!Arc::ptr_eq(&original, &cache.scope(driver_key)));
    cache.invalidate_connection("one");
    assert!(!Arc::ptr_eq(&original, &cache.scope(key("one"))));
    assert!(!Arc::ptr_eq(&other_database, &cache.scope(other_key)));
    assert!(Arc::ptr_eq(&other_connection, &cache.scope(key("two"))));
    cache.invalidate_all();
    assert!(!Arc::ptr_eq(&other_connection, &cache.scope(key("two"))));
}

#[tokio::test]
async fn concurrent_catalog_requests_share_one_successful_load() {
    let scope = ScopeCache::new(TTL);
    let calls = AtomicUsize::new(0);
    let loader = || async {
        calls.fetch_add(1, Ordering::SeqCst);
        tokio::task::yield_now().await;
        Ok(catalog("users"))
    };
    let (left, right) = tokio::join!(scope.catalog(loader), scope.catalog(loader));
    assert!(Arc::ptr_eq(&left.unwrap(), &right.unwrap()));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn expired_catalog_reloads_and_discards_details_from_the_previous_catalog() {
    let scope = ScopeCache::new(TTL);
    scope
        .catalog(|| async { Ok(catalog("old")) })
        .await
        .unwrap();
    scope
        .details(&["old".into()], || async {
            DetailedSchemaContext::default()
        })
        .await;
    scope
        .data
        .lock()
        .unwrap()
        .catalog
        .as_mut()
        .unwrap()
        .loaded_at = Instant::now() - TTL;
    let current = scope
        .catalog(|| async { Ok(catalog("new")) })
        .await
        .unwrap();
    assert_eq!(current.tables[0].name, "new");
    assert!(scope.data.lock().unwrap().batches.is_empty());
}

#[tokio::test]
async fn failed_catalog_loads_and_cancelled_initializers_can_be_retried() {
    let scope = Arc::new(ScopeCache::new(TTL));
    assert!(scope
        .catalog(|| async { Err(ProviderError::new(ErrorKind::Other, "fixture")) })
        .await
        .is_err());
    let (started, ready) = tokio::sync::oneshot::channel();
    let pending = scope.clone();
    let task = tokio::spawn(async move {
        pending
            .catalog(|| async {
                started.send(()).unwrap();
                std::future::pending::<Result<SchemaCatalog, ProviderError>>().await
            })
            .await
    });
    ready.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(
        scope
            .catalog(|| async { Ok(catalog("retry")) })
            .await
            .unwrap()
            .tables[0]
            .name,
        "retry"
    );
}

#[tokio::test]
async fn refresh_during_a_load_cannot_repopulate_the_new_entry() {
    let cache = SchemaCache::default();
    let old = cache.scope(key("one"));
    let (started, ready) = tokio::sync::oneshot::channel();
    let (finish, wait) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        old.catalog(|| async {
            started.send(()).unwrap();
            wait.await.unwrap();
            Ok(catalog("stale"))
        })
        .await
    });
    ready.await.unwrap();
    cache.invalidate_connection("one");
    let fresh = cache.scope(key("one"));
    fresh
        .catalog(|| async { Ok(catalog("fresh")) })
        .await
        .unwrap();
    finish.send(()).unwrap();
    assert_eq!(task.await.unwrap().unwrap().tables[0].name, "stale");
    assert_eq!(
        fresh
            .catalog(|| async { panic!("fresh catalog must stay cached") })
            .await
            .unwrap()
            .tables[0]
            .name,
        "fresh"
    );
}

#[tokio::test]
async fn detail_keys_preserve_priority_and_unavailable_metadata_is_retried() {
    let scope = ScopeCache::new(TTL);
    let names = vec!["users".into(), "teams".into()];
    let first = scope
        .details(&names, || async { DetailedSchemaContext::default() })
        .await;
    let cached = scope
        .details(&names, || async {
            panic!("identical details must stay cached")
        })
        .await;
    assert!(Arc::ptr_eq(&first, &cached));
    let reversed = scope
        .details(&["teams".into(), "users".into()], || async {
            DetailedSchemaContext::default()
        })
        .await;
    assert!(!Arc::ptr_eq(&first, &reversed));
    let calls = AtomicUsize::new(0);
    for _ in 0..2 {
        scope
            .details(&["missing_indexes".into()], || async {
                calls.fetch_add(1, Ordering::SeqCst);
                DetailedSchemaContext {
                    tables: vec![DetailedTable {
                        kind: Default::default(),
                        definition: None,

                        name: "missing_indexes".into(),
                        comment: None,
                        columns: vec![],
                        foreign_keys: vec![],
                        indexes: vec![],
                        unavailable: vec![MetadataKind::Indexes],
                    }],
                    ..Default::default()
                }
            })
            .await;
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn cache_caps_and_zero_ttl_bound_retained_scopes_and_detail_batches() {
    let cache = SchemaCache::default();
    let oldest = cache.scope(key("oldest"));
    for index in 0..MAX_SCOPES {
        cache.scope(key(&format!("connection_{index}")));
    }
    assert_eq!(cache.scopes.lock().unwrap().len(), MAX_SCOPES);
    assert!(!Arc::ptr_eq(&oldest, &cache.scope(key("oldest"))));
    let scope = ScopeCache::new(TTL);
    for index in 0..MAX_BATCHES + 1 {
        scope
            .details(&[format!("table_{index}")], || async {
                DetailedSchemaContext::default()
            })
            .await;
    }
    assert_eq!(scope.data.lock().unwrap().batches.len(), MAX_BATCHES);
    assert!(scope.fresh_details(&["table_0".into()]).is_none());
    let expired = ScopeCache::new(Duration::ZERO);
    let first = expired
        .catalog(|| async { Ok(catalog("first")) })
        .await
        .unwrap();
    let second = expired
        .catalog(|| async { Ok(catalog("second")) })
        .await
        .unwrap();
    assert!(!Arc::ptr_eq(&first, &second));
}

#[tokio::test]
async fn unavailable_view_catalogs_are_retried_and_typed_detail_keys_do_not_collide() {
    let cache = SchemaCache::default();
    let scope = cache.scope(key("one"));
    let loads = AtomicUsize::new(0);
    for _ in 0..2 {
        scope
            .catalog(|| async {
                loads.fetch_add(1, Ordering::SeqCst);
                Ok(SchemaCatalog {
                    views_unavailable: true,
                    ..Default::default()
                })
            })
            .await
            .unwrap();
    }
    assert_eq!(loads.load(Ordering::SeqCst), 2);
    for kind in ["table:users", "view:users"] {
        scope
            .details(&[kind.into()], || async {
                loads.fetch_add(1, Ordering::SeqCst);
                DetailedSchemaContext::default()
            })
            .await;
    }
    assert_eq!(loads.load(Ordering::SeqCst), 4);
    scope
        .details(&["view:users".into()], || async {
            panic!("typed batch should be cached")
        })
        .await;
}
