//! Driver-facing operations shared by the typed tools and MCP adapters.
use serde_json::{json, Value};

use crate::drivers::driver_trait::{DatabaseDriver, SqlDialect};
use crate::models::{
    ConnectionParams, QueryResult, SavedConnection, TableQueryTemplateKind,
    TableQueryTemplateRequest,
};

pub(crate) fn connection_summaries(connections: &[SavedConnection]) -> Value {
    json!(connections
        .iter()
        .map(|connection| json!({
            "id": connection.id,
            "name": connection.name,
            "driver": connection.params.driver,
            "host": connection.params.host,
            "database": connection.params.database.as_vec(),
        }))
        .collect::<Vec<_>>())
}

pub(crate) async fn list_databases(
    driver: &dyn DatabaseDriver,
    params: &ConnectionParams,
) -> Result<Value, String> {
    driver
        .get_databases(params)
        .await
        .map(|databases| json!(databases))
}

pub(crate) async fn list_tables(
    driver: &dyn DatabaseDriver,
    params: &ConnectionParams,
    schema: Option<&str>,
) -> Result<Value, String> {
    let tables = driver.get_tables(params, schema).await?;
    Ok(json!(tables
        .iter()
        .map(|table| &table.name)
        .collect::<Vec<_>>()))
}

pub(crate) async fn describe_table(
    driver: &dyn DatabaseDriver,
    params: &ConnectionParams,
    table: &str,
    schema: Option<&str>,
) -> Result<Value, String> {
    let (columns, foreign_keys, indexes) = tokio::join!(
        driver.get_columns(params, table, schema),
        driver.get_foreign_keys(params, table, schema),
        driver.get_indexes(params, table, schema),
    );
    Ok(
        json!({ "table": table, "columns": columns?, "foreign_keys": foreign_keys?, "indexes": indexes? }),
    )
}

pub(crate) async fn execute(
    driver: &dyn DatabaseDriver,
    params: &ConnectionParams,
    query: &str,
    limit: u32,
    schema: Option<&str>,
    readonly: bool,
) -> Result<QueryResult, String> {
    if readonly {
        driver
            .execute_read_only_query(params, query, Some(limit), 1, schema)
            .await
    } else {
        driver
            .execute_query(params, query, Some(limit), 1, schema)
            .await
    }
}

fn quote_identifier(name: &str, quote: &str) -> Result<String, String> {
    if name.is_empty() || name.len() > 512 || name.contains('\0') {
        return Err("Invalid table or schema name.".into());
    }
    match quote {
        "\"" | "`" => Ok(format!(
            "{quote}{}{quote}",
            name.replace(quote, &quote.repeat(2))
        )),
        "[" => Ok(format!("[{}]", name.replace(']', "]]"))),
        _ => Err("The driver does not declare a supported identifier quote.".into()),
    }
}

/// No free SQL is accepted by sample_rows; plugin templates still pass policy.
pub(crate) async fn sample_query(
    driver: &dyn DatabaseDriver,
    params: &ConnectionParams,
    table: &str,
    schema: Option<&str>,
    limit: u32,
) -> Result<String, String> {
    let capabilities = &driver.manifest().capabilities;
    let quoted_table = quote_identifier(table, &capabilities.identifier_quote)?;
    let qualified = if let Some(schema) = schema {
        format!(
            "{}.{}",
            quote_identifier(schema, &capabilities.identifier_quote)?,
            quoted_table
        )
    } else {
        quoted_table
    };
    if capabilities.table_query_templates {
        let request = TableQueryTemplateRequest {
            table: table.into(),
            schema: schema.map(str::to_string),
            kind: TableQueryTemplateKind::Select,
            columns: Vec::new(),
            limit: Some(limit),
        };
        if let Some(query) = driver.get_table_query_template(params, &request).await? {
            return Ok(query);
        }
    }
    Ok(match capabilities.sql_dialect {
        Some(SqlDialect::Mssql) => format!("SELECT TOP ({limit}) * FROM {qualified}"),
        Some(SqlDialect::Oracle) => {
            format!("SELECT * FROM {qualified} FETCH FIRST {limit} ROWS ONLY")
        }
        _ => format!("SELECT * FROM {qualified} LIMIT {limit}"),
    })
}

#[cfg(test)]
mod tests;
