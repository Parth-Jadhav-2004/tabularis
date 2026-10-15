use super::*;

#[test]
fn identifier_quoting_handles_supported_delimiters_and_rejects_invalid_inputs() {
    for (name, quote, expected) in [
        ("users", "\"", "\"users\""),
        ("a\"b", "\"", "\"a\"\"b\""),
        ("a`b", "`", "`a``b`"),
        ("a]b", "[", "[a]]b]"),
        (
            "x; DELETE FROM users;--",
            "\"",
            "\"x; DELETE FROM users;--\"",
        ),
    ] {
        assert_eq!(quote_identifier(name, quote).unwrap(), expected);
    }
    for name in ["", "a\0b", &"a".repeat(513)] {
        assert!(quote_identifier(name, "\"").is_err());
    }
    assert!(quote_identifier("users", "'").is_err());
    assert!(quote_identifier("users", "").is_err());
}

#[tokio::test]
async fn builtin_sample_templates_quote_scope_and_use_driver_dialects() {
    use crate::drivers::{mysql::MysqlDriver, postgres::PostgresDriver, sqlite::SqliteDriver};
    let params = ConnectionParams::default();
    let query = sample_query(&SqliteDriver::new(), &params, "a\"b", Some("main"), 7)
        .await
        .unwrap();
    assert_eq!(query, "SELECT * FROM \"main\".\"a\"\"b\" LIMIT 7");
    let query = sample_query(&MysqlDriver::new(), &params, "a`b", Some("db"), 8)
        .await
        .unwrap();
    assert_eq!(query, "SELECT * FROM `db`.`a``b` LIMIT 8");
    let query = sample_query(&PostgresDriver::new(), &params, "users", Some("public"), 9)
        .await
        .unwrap();
    assert_eq!(query, "SELECT * FROM \"public\".\"users\" LIMIT 9");
    for query in [
        query,
        "SELECT * FROM \"x; DELETE FROM users;--\" LIMIT 3".into(),
    ] {
        assert_eq!(crate::ai_tools::safety::require_read(&query), Ok(()));
    }
}
