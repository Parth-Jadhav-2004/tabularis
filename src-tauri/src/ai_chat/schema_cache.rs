//! Short-lived metadata only. Invalidation detaches in-flight entries so their
//! completions cannot repopulate a refreshed connection or a different scope.
use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::ai::provider::ProviderError;
use crate::ai_schema_context::detailed::{DetailedSchemaContext, SchemaCatalog};

const TTL: Duration = Duration::from_secs(30);
const MAX_SCOPES: usize = 16;
const MAX_BATCHES: usize = 8;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ScopeKey {
    pub connection_id: String,
    pub database: Option<String>,
    pub schema: Option<String>,
    pub driver: String,
}

struct Timed<T> {
    loaded_at: Instant,
    value: Arc<T>,
}

#[derive(Default)]
struct ScopeData {
    catalog: Option<Timed<SchemaCatalog>>,
    batches: VecDeque<(Vec<String>, Timed<DetailedSchemaContext>)>,
}

pub(crate) struct ScopeCache {
    ttl: Duration,
    created_at: Instant,
    load: tokio::sync::Mutex<()>,
    data: Mutex<ScopeData>,
}

impl ScopeCache {
    fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            created_at: Instant::now(),
            load: tokio::sync::Mutex::new(()),
            data: Mutex::default(),
        }
    }

    fn fresh_catalog(&self) -> Option<Arc<SchemaCatalog>> {
        self.data
            .lock()
            .unwrap()
            .catalog
            .as_ref()
            .filter(|entry| entry.loaded_at.elapsed() < self.ttl)
            .map(|entry| entry.value.clone())
    }

    pub(crate) async fn catalog<F, Fut>(
        &self,
        loader: F,
    ) -> Result<Arc<SchemaCatalog>, ProviderError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<SchemaCatalog, ProviderError>>,
    {
        if let Some(catalog) = self.fresh_catalog() {
            return Ok(catalog);
        }
        let _load = self.load.lock().await;
        if let Some(catalog) = self.fresh_catalog() {
            return Ok(catalog);
        }
        let value = Arc::new(loader().await?);
        if value.views_unavailable {
            return Ok(value);
        }
        let mut data = self.data.lock().unwrap();
        data.batches.clear();
        data.catalog = Some(Timed {
            loaded_at: Instant::now(),
            value: value.clone(),
        });
        Ok(value)
    }

    fn fresh_details(&self, names: &[String]) -> Option<Arc<DetailedSchemaContext>> {
        let mut data = self.data.lock().unwrap();
        let position = data
            .batches
            .iter()
            .position(|(key, entry)| key == names && entry.loaded_at.elapsed() < self.ttl)?;
        let entry = data.batches.remove(position)?;
        let value = entry.1.value.clone();
        data.batches.push_back(entry);
        Some(value)
    }

    pub(crate) async fn details<F, Fut>(
        &self,
        names: &[String],
        loader: F,
    ) -> Arc<DetailedSchemaContext>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = DetailedSchemaContext>,
    {
        if let Some(details) = self.fresh_details(names) {
            return details;
        }
        let _load = self.load.lock().await;
        if let Some(details) = self.fresh_details(names) {
            return details;
        }
        let value = Arc::new(loader().await);
        // Failed metadata should be retried on the next request, not cached as
        // a successful empty table or index list for the duration of the TTL.
        if value
            .tables
            .iter()
            .all(|table| table.unavailable.is_empty())
        {
            let mut data = self.data.lock().unwrap();
            data.batches.retain(|(key, _)| key != names);
            if data.batches.len() >= MAX_BATCHES {
                data.batches.pop_front();
            }
            data.batches.push_back((
                names.to_vec(),
                Timed {
                    loaded_at: Instant::now(),
                    value: value.clone(),
                },
            ));
        }
        value
    }
}

pub(crate) struct SchemaCache {
    ttl: Duration,
    scopes: Mutex<VecDeque<(ScopeKey, Arc<ScopeCache>)>>,
}

impl Default for SchemaCache {
    fn default() -> Self {
        Self {
            ttl: TTL,
            scopes: Mutex::default(),
        }
    }
}

impl SchemaCache {
    pub(crate) fn scope(&self, key: ScopeKey) -> Arc<ScopeCache> {
        let mut scopes = self.scopes.lock().unwrap();
        if let Some(position) = scopes.iter().position(|(existing, _)| existing == &key) {
            let entry = scopes.remove(position).unwrap();
            if entry.1.created_at.elapsed() < self.ttl {
                let scope = entry.1.clone();
                scopes.push_back(entry);
                return scope;
            }
        }
        let scope = Arc::new(ScopeCache::new(self.ttl));
        if scopes.len() >= MAX_SCOPES {
            scopes.pop_front();
        }
        scopes.push_back((key, scope.clone()));
        scope
    }

    pub(crate) fn invalidate_connection(&self, connection_id: &str) {
        self.scopes
            .lock()
            .unwrap()
            .retain(|(key, _)| key.connection_id != connection_id);
    }

    pub(crate) fn invalidate_all(&self) {
        self.scopes.lock().unwrap().clear();
    }
}

pub(crate) fn shared() -> &'static SchemaCache {
    static CACHE: OnceLock<SchemaCache> = OnceLock::new();
    CACHE.get_or_init(SchemaCache::default)
}

#[cfg(test)]
mod tests;
