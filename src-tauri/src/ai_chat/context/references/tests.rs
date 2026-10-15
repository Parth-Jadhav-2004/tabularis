use super::*;

fn catalog(names: &[&str]) -> Vec<TableInfo> {
    names
        .iter()
        .map(|name| TableInfo {
            name: (*name).into(),
            comment: None,
        })
        .collect()
}

fn scan(sql: &str) -> TableReferences {
    extract_referenced_tables(
        sql,
        &catalog(&[
            "users",
            "orders",
            "teams",
            "odd\"name",
            "odd`name",
            "odd]name",
            "表",
        ]),
        Some("app"),
        Some("public"),
    )
}

#[test]
fn joins_aliases_comma_lists_and_nested_queries_keep_discovery_order() {
    let result = scan("SELECT u.id FROM users u, teams t LEFT JOIN orders o ON o.user_id=u.id WHERE EXISTS (SELECT 1 FROM users)");
    assert_eq!(result.tables, ["users", "teams", "orders"]);
    assert!(result.missing.is_empty());
    assert!(!result.truncated);
    assert_eq!(
        scan("SELECT * FROM (SELECT * FROM users) u JOIN orders o ON true").tables,
        ["users", "orders"]
    );
}

#[test]
fn quoted_identifiers_and_qualified_names_respect_the_selected_scope() {
    assert_eq!(
        scan("SELECT * FROM public.\"users\" JOIN app.public.orders o ON true").tables,
        ["users", "orders"]
    );
    assert_eq!(
        scan("SELECT * FROM \"odd\"\"name\", `odd``name`, [odd]]name], 表").tables,
        ["odd\"name", "odd`name", "odd]name", "表"]
    );
    let result = scan("SELECT * FROM other.users JOIN elsewhere.public.orders o ON true");
    assert!(result.tables.is_empty());
    assert_eq!(result.missing, ["other.users", "elsewhere.public.orders"]);
}

#[test]
fn mysql_database_qualification_and_literal_dots_are_distinct() {
    let result = extract_referenced_tables(
        "SELECT * FROM app.users, `odd.name`",
        &catalog(&["users", "odd.name"]),
        Some("app"),
        None,
    );
    assert_eq!(result.tables, ["users", "odd.name"]);
    assert!(result.missing.is_empty());
}

#[test]
fn strings_comments_and_function_from_clauses_do_not_select_tables() {
    let result = scan("SELECT 'FROM teams', $$ JOIN orders $$, $tag$ UPDATE teams $tag$, EXTRACT(YEAR FROM created_at), TRIM('x' FROM name) FROM users -- JOIN orders\n /* nested /* FROM teams */ JOIN orders */ # FROM teams\n");
    assert_eq!(result.tables, ["users"]);
    assert!(result.missing.is_empty());
    assert_eq!(
        scan("SELECT * FROM generate_series(1, 10) g JOIN users ON true").tables,
        ["users"]
    );
}

#[test]
fn ctes_and_recursive_aliases_are_excluded_but_their_source_tables_are_kept() {
    let result = scan("WITH RECURSIVE recent(id) AS NOT MATERIALIZED (SELECT id FROM users UNION ALL SELECT id FROM recent), detail AS (SELECT * FROM orders) SELECT * FROM recent JOIN detail ON true");
    assert_eq!(result.tables, ["users", "orders"]);
    assert!(result.missing.is_empty());
    let result = scan("SELECT * FROM users WHERE EXISTS (WITH users AS (SELECT * FROM teams) SELECT * FROM users); SELECT * FROM users");
    assert_eq!(result.tables, ["users", "teams"]);
    assert!(result.missing.is_empty());
}

#[test]
fn nonrecursive_cte_can_read_a_physical_table_with_the_same_name() {
    let result = scan("WITH users AS (SELECT * FROM users) SELECT * FROM users");
    assert_eq!(result.tables, ["users"]);
    assert!(result.missing.is_empty());
}

#[test]
fn write_targets_are_context_references_without_becoming_execution_policy() {
    assert_eq!(scan("UPDATE ONLY users SET name='FROM teams'; INSERT INTO orders(user_id) SELECT id FROM users; DELETE FROM teams").tables, ["users", "orders", "teams"]);
    assert_eq!(
        scan("SELECT * FROM users FOR UPDATE SKIP LOCKED").tables,
        ["users"]
    );
    assert!(scan("SELECT * FROM users FOR UPDATE SKIP LOCKED")
        .missing
        .is_empty());
    assert!(
        scan("INSERT INTO users(id) VALUES (1) ON CONFLICT DO UPDATE SET id=1")
            .missing
            .is_empty()
    );
}

#[test]
fn unknown_and_ambiguous_names_are_reported_without_guessing() {
    assert_eq!(
        scan("SELECT * FROM absent JOIN absent a ON true").missing,
        ["absent"]
    );
    let tables = catalog(&["Users", "USERS"]);
    let result = extract_referenced_tables("SELECT * FROM users", &tables, None, None);
    assert!(result.tables.is_empty());
    assert_eq!(result.missing, ["users"]);
    assert_eq!(
        extract_referenced_tables("SELECT * FROM \"Users\"", &tables, None, None).tables,
        ["Users"]
    );
    assert_eq!(scan("SELECT * FROM \"USERS\"").missing, ["USERS"]);
}

#[test]
fn incomplete_quotes_do_not_invent_references_and_scan_limits_are_explicit() {
    assert!(scan("SELECT * FROM \"users").tables.is_empty());
    let sql = format!(
        "SELECT * FROM users; {} SELECT * FROM teams",
        "界".repeat(100_000)
    );
    let result = scan(&sql);
    assert_eq!(result.tables, ["users"]);
    assert!(result.truncated);
    assert!(scan(&"( ".repeat(40_000)).truncated);
}

#[test]
fn cte_lookahead_is_bounded_and_postgres_json_operators_are_not_comments() {
    assert!(scan(&format!("SELECT * FROM users {}", "WITH ".repeat(129))).truncated);
    let result = scan("SELECT profile #>> '{name}' FROM users");
    assert_eq!(result.tables, ["users"]);
    assert!(result.missing.is_empty());
}
