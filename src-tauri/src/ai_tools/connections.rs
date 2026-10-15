use super::cooldown::{DISABLE_CHECK_COOLDOWN, RELOAD_COOLDOWN};
use crate::drivers::driver_trait::{DatabaseDriver, SqlDialect};
use crate::drivers::registry as driver_registry;
use crate::mcp::protocol::JsonRpcError;
use crate::models::{ConnectionParams, DatabaseSelection, K8sConnection, SshConnection};
use crate::{commands, credential_cache, paths, persistence, plugins};
use serde_json::Value;
use std::sync::Arc;

/// MCP-mode equivalent of `expand_ssh_connection_params` — no AppHandle needed.
/// Loads SSH credentials from the config file and keychain directly.
async fn expand_ssh_params_for_mcp(
    params: &ConnectionParams,
) -> Result<ConnectionParams, JsonRpcError> {
    let mut expanded = params.clone();

    if !params.ssh_enabled.unwrap_or(false) {
        return Ok(expanded);
    }

    let ssh_id = match &params.ssh_connection_id {
        Some(id) => id.clone(),
        None => return Ok(expanded), // legacy inline SSH fields already present
    };

    let ssh_path = paths::get_app_config_dir().join("ssh_connections.json");
    if !ssh_path.exists() {
        return Err(JsonRpcError {
            code: -32000,
            message: format!("SSH connection {} not found", ssh_id),
            data: None,
        });
    }

    let content = tokio::task::spawn_blocking({
        let p = ssh_path.clone();
        move || std::fs::read_to_string(p).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| JsonRpcError {
        code: -32000,
        message: e.to_string(),
        data: None,
    })?
    .map_err(|e| JsonRpcError {
        code: -32000,
        message: e,
        data: None,
    })?;

    let mut ssh: SshConnection = serde_json::from_str::<Vec<SshConnection>>(&content)
        .unwrap_or_default()
        .into_iter()
        .find(|s| s.id == ssh_id)
        .ok_or_else(|| JsonRpcError {
            code: -32000,
            message: format!("SSH connection {} not found", ssh_id),
            data: None,
        })?;

    if ssh.auth_type.is_none() {
        ssh.auth_type = Some(
            if ssh
                .key_file
                .as_ref()
                .map_or(false, |k| !k.trim().is_empty())
            {
                "ssh_key".to_string()
            } else {
                "password".to_string()
            },
        );
    }

    if ssh.save_in_keychain.unwrap_or(false) {
        let cache = std::sync::Arc::new(credential_cache::CredentialCache::default());
        let id = ssh.id.clone();
        let (pwd_r, pass_r) = tokio::task::spawn_blocking(move || {
            let pwd = credential_cache::get_ssh_password_cached(&cache, &id);
            let pass = credential_cache::get_ssh_key_passphrase_cached(&cache, &id);
            (pwd, pass)
        })
        .await
        .map_err(|e| JsonRpcError {
            code: -32000,
            message: e.to_string(),
            data: None,
        })?;

        if let Ok(v) = pwd_r {
            if !v.trim().is_empty() {
                ssh.password = Some(v);
            }
        }
        if let Ok(v) = pass_r {
            if !v.trim().is_empty() {
                ssh.key_passphrase = Some(v);
            }
        }
    }

    expanded.ssh_host = Some(ssh.host);
    expanded.ssh_port = Some(ssh.port);
    expanded.ssh_user = Some(ssh.user);
    expanded.ssh_password = ssh.password;
    expanded.ssh_key_file = ssh.key_file;
    expanded.ssh_key_passphrase = ssh.key_passphrase;

    Ok(expanded)
}

/// MCP-mode equivalent of K8s saved-connection expansion.
async fn expand_k8s_params_for_mcp(
    params: &ConnectionParams,
) -> Result<ConnectionParams, JsonRpcError> {
    let mut expanded = params.clone();

    if !params.k8s_enabled.unwrap_or(false) {
        return Ok(expanded);
    }

    let k8s_id = match &params.k8s_connection_id {
        Some(id) => id.clone(),
        None => return Ok(expanded),
    };

    let k8s_path = paths::get_app_config_dir().join("k8s_connections.json");
    if !k8s_path.exists() {
        return Err(JsonRpcError {
            code: -32000,
            message: format!("K8s connection {} not found", k8s_id),
            data: None,
        });
    }

    let content = tokio::task::spawn_blocking({
        let p = k8s_path.clone();
        move || std::fs::read_to_string(p).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| JsonRpcError {
        code: -32000,
        message: e.to_string(),
        data: None,
    })?
    .map_err(|e| JsonRpcError {
        code: -32000,
        message: e,
        data: None,
    })?;

    let k8s: K8sConnection = serde_json::from_str::<Vec<K8sConnection>>(&content)
        .unwrap_or_default()
        .into_iter()
        .find(|k| k.id == k8s_id)
        .ok_or_else(|| JsonRpcError {
            code: -32000,
            message: format!("K8s connection {} not found", k8s_id),
            data: None,
        })?;

    expanded.k8s_context = Some(k8s.context);
    expanded.k8s_namespace = Some(k8s.namespace);
    expanded.k8s_resource_type = Some(k8s.resource_type);
    expanded.k8s_resource_name = Some(k8s.resource_name);
    expanded.k8s_port = Some(k8s.port);
    expanded.k8s_kubectl_path = k8s.kubectl_path;
    expanded.k8s_kubeconfig_path = k8s.kubeconfig_path;

    Ok(expanded)
}

fn find_connection(conn_id: &str) -> Result<crate::models::SavedConnection, JsonRpcError> {
    let config_path = paths::resolve_connections_path(&paths::get_app_config_dir());
    let connections = persistence::load_connections(&config_path).map_err(|e| JsonRpcError {
        code: -32000,
        message: e,
        data: None,
    })?;

    connections
        .into_iter()
        .find(|c| c.id == conn_id || c.name.eq_ignore_ascii_case(conn_id))
        .ok_or_else(|| JsonRpcError {
            code: -32000,
            message: format!("Connection not found: {}", conn_id),
            data: None,
        })
}

/// Full connection resolution for MCP: DB password + SSH expansion + tunnel setup.
async fn resolve_db_params(
    conn_id: &str,
) -> Result<(crate::models::SavedConnection, ConnectionParams), JsonRpcError> {
    let mut conn = find_connection(conn_id)?;

    // Load DB password from keychain unless the connection uses IAM auth,
    // whose 15-min tokens must come from the `password` field on every
    // connect — never from the keychain, where a stale token would survive.
    let iam_auth = conn.params.use_iam_auth.unwrap_or(false);
    if !iam_auth && conn.params.save_in_keychain.unwrap_or(false) {
        let cache = std::sync::Arc::new(credential_cache::CredentialCache::default());
        let id = conn.id.clone();
        let pwd = tokio::task::spawn_blocking(move || {
            credential_cache::get_db_password_cached(&cache, &id)
        })
        .await
        .map_err(|e| JsonRpcError {
            code: -32000,
            message: e.to_string(),
            data: None,
        })?;

        if let Ok(p) = pwd {
            if !p.trim().is_empty() {
                conn.params.password = Some(p);
            }
        }
    } else if iam_auth && conn.params.save_in_keychain.unwrap_or(false) {
        log::warn!(
            "MCP: connection {} has use_iam_auth=true; ignoring any password stored in the keychain. A fresh RDS auth token must be supplied on every connect.",
            conn_id
        );
        conn.params.password = None;
    }

    // connections.json never holds the URI, so a passthrough connection that
    // works in the app would reach the driver without its endpoint and
    // credentials here unless it is restored the same way. Kept out of the
    // password branches above: the URI has its own keychain entry and its own
    // marker, so it must not depend on how the password happens to be stored.
    if conn.params.connection_uri_in_keychain.unwrap_or(false) {
        let cache = std::sync::Arc::new(credential_cache::CredentialCache::default());
        let id = conn.id.clone();
        let uri = tokio::task::spawn_blocking(move || {
            credential_cache::get_connection_uri_cached(&cache, &id, true)
        })
        .await
        .map_err(|e| JsonRpcError {
            code: -32000,
            message: e.to_string(),
            data: None,
        })?;
        if let Ok(Some(value)) = uri {
            conn.params.connection_uri = Some(value);
        }
    }

    let expanded = expand_ssh_params_for_mcp(&conn.params).await?;
    let expanded = expand_k8s_params_for_mcp(&expanded).await?;
    let db_params = commands::resolve_connection_params(&expanded).map_err(|e| JsonRpcError {
        code: -32000,
        message: e,
        data: None,
    })?;
    Ok((conn, db_params))
}

/// Resolve the driver for an MCP-known connection. Returns the connection,
/// the resolved DB params, and the registered driver. Errors with a JSON-RPC
/// payload: see [`resolve_driver_for_params`] for how a registry miss (e.g.
/// the plugin failed to load, or was never installed) versus a registered
/// driver's own resolution failure are each surfaced.
///
/// `active_ids` is the caller's already-loaded `active_external_drivers`
/// (see [`resolve_driver_for_params`]) — passed through rather than
/// re-reading it here, since every caller already has an `AppConfig` in hand
/// or loads one anyway.
pub(crate) async fn resolve_db_driver(
    conn_id: &str,
    active_ids: Option<&[String]>,
) -> Result<
    (
        crate::models::SavedConnection,
        ConnectionParams,
        Arc<dyn DatabaseDriver>,
    ),
    JsonRpcError,
> {
    let (conn, db_params) = resolve_db_params(conn_id).await?;
    let driver = resolve_driver_for_params(&db_params, active_ids).await?;
    Ok((conn, db_params, driver))
}

/// Resolve the registered driver for `db_params`, rescanning installed
/// plugins against the latest on-disk config once and retrying on a
/// registry miss. Split out from [`resolve_db_driver`] so this retry logic
/// is testable against a bare [`ConnectionParams`], without needing a
/// resolvable [`crate::models::SavedConnection`].
///
/// `active_ids` is the caller's `active_external_drivers` (`None` means "no
/// explicit preference saved" — see [`plugins::manager::load_plugins`]'s doc
/// comment). Taken as a parameter rather than read from disk here so this
/// function stays a pure function of its inputs plus the driver registry —
/// callers already have an `AppConfig` in hand (or load one anyway), so this
/// avoids a redundant disk read on every call.
pub(crate) async fn resolve_driver_for_params(
    db_params: &ConnectionParams,
    active_ids: Option<&[String]>,
) -> Result<Arc<dyn DatabaseDriver>, JsonRpcError> {
    // Reconciles the registry against `active_ids` before every lookup —
    // not just on a miss, unlike the rescan-on-miss below. A driver that was
    // disabled or uninstalled elsewhere keeps resolving successfully, so it
    // never produces the miss that rescan relies on to notice anything
    // (issue #787). Gated by its own cooldown, independent of
    // `RELOAD_COOLDOWN` below: sharing one timer would let a busy reconcile
    // check consume the cooldown budget a genuine registry-miss rescan needs
    // to stay responsive.
    if DISABLE_CHECK_COOLDOWN.elapsed() {
        driver_registry::reconcile_active_drivers(active_ids).await;
    }

    match driver_registry::get_connection_driver(db_params).await {
        Ok(driver) => return Ok(driver),
        // The id has a registered driver, so this failure came from
        // `for_connection` itself (e.g. a `get_connection_metadata` RPC
        // error) rather than a registry miss — rescanning plugins on disk
        // wouldn't change that, so surface the error directly instead of
        // paying for a pointless rescan and a repeat of the same failing
        // RPC call.
        Err(message)
            if driver_registry::get_driver(&db_params.driver)
                .await
                .is_some() =>
        {
            return Err(JsonRpcError {
                code: -32000,
                message,
                data: None,
            });
        }
        Err(_) => {}
    }

    // Not found in the in-memory registry populated at startup: this
    // subprocess may have been running since before the plugin was
    // installed/enabled, or before a connection was migrated to it (issue
    // #783). The GUI hot-registers such changes in-process, but this
    // standalone subprocess has no way to observe them — rescan installed
    // plugins against the latest on-disk config once and retry before
    // giving up. Rate-limited so a connection whose driver id is genuinely
    // wrong (typo, never-installed plugin) can't force a filesystem scan on
    // every single call from a tight retry loop.
    if RELOAD_COOLDOWN.elapsed() {
        plugins::manager::reload_plugins_from_disk_config().await;
    }
    driver_registry::get_connection_driver(db_params)
        .await
        .map_err(|message| JsonRpcError {
            code: -32000,
            message,
            data: None,
        })
}

/// Resolves the schema to pass to a metadata fetch, defaulting to `"public"`
/// on postgres-dialect drivers when the caller didn't supply one.
///
/// Capability-driven, not driver-id-driven (issue #614): checks the
/// resolved driver's declared `sql_dialect` rather than comparing
/// `driver == "postgres"` literally, so a postgres-compatible driver
/// registered under a different id (e.g. the standalone PostgreSQL plugin,
/// id `"postgresql"`) gets the same `"public"` default the builtin driver
/// always did. Any other dialect passes `requested_schema` through
/// unchanged.
pub(crate) fn resolve_default_schema<'a>(
    driver: &Arc<dyn DatabaseDriver>,
    requested_schema: Option<&'a str>,
) -> Option<&'a str> {
    if driver.manifest().capabilities.sql_dialect == Some(SqlDialect::Postgres) {
        Some(requested_schema.unwrap_or("public"))
    } else {
        requested_schema
    }
}

/// The `database` argument of a tool call, if the caller named one.
pub(crate) fn database_arg(args: &serde_json::Map<String, Value>) -> Option<&str> {
    args.get("database")
        .and_then(|v| v.as_str())
        .filter(|d| !d.is_empty())
}

/// Point `params` at `database` when the caller names one, the same way the
/// GUI's query commands apply their `database` argument. Without it a
/// connection with several databases selected always runs against the first.
pub(crate) fn apply_database_override(params: &mut ConnectionParams, database: Option<&str>) {
    if let Some(db) = database {
        params.database = DatabaseSelection::Single(db.to_string());
    }
}
