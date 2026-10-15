use super::*;

#[test]
fn reads_allow_comments_literals_ctes_and_one_terminal_semicolon() {
    for sql in [
        "SELECT 1; -- trailing comment",
        "SELECT '; DELETE FROM users'",
        "WITH q AS (SELECT 1) SELECT * FROM q",
        "SELECT $$value; still a string$$",
        "SELECT E'escaped\\\'; value'",
        "PRAGMA table_info(users)",
        "EXPLAIN QUERY PLAN SELECT 1",
    ] {
        assert_eq!(require_read(sql), Ok(()), "{sql}");
    }
}

#[test]
fn read_tools_reject_writes_ddl_cte_mutations_and_side_effects() {
    for sql in [
        "DELETE FROM users",
        "UPDATE users SET active = 0",
        "DROP TABLE users",
        "WITH q AS (DELETE FROM users RETURNING *) SELECT * FROM q",
        "WITH q AS (SELECT 1) DELETE FROM users",
        "SELECT 1 INTO stolen_copy",
        "SELECT * FROM users INTO OUTFILE '/tmp/stolen'",
        "SELECT lo_export(1, '/tmp/stolen')",
        "SELECT pg_catalog.\"lo_export\"(1, '/tmp/stolen')",
        "EXPLAIN ANALYZE DELETE FROM users",
        "PRAGMA query_only = 0",
        "SELECT * FROM users FOR UPDATE",
    ] {
        assert_eq!(require_read(sql), Err(SafetyError::ReadRequired), "{sql}");
    }
}

#[test]
fn multiple_statements_are_rejected_for_writable_connections_too() {
    for sql in [
        "SELECT 1; DELETE FROM users",
        "UPDATE users SET active = 0; SELECT 1",
        "SELECT '\\''; DROP TABLE users",
        "SELECT 1; /* hidden */ DROP TABLE users",
        "BEGIN; DELETE FROM users; COMMIT",
    ] {
        assert_eq!(
            enforce_connection_policy(sql, false),
            Err(SafetyError::MultipleStatements),
            "{sql}"
        );
    }
}

#[test]
fn transactions_and_unknown_statements_fail_closed() {
    for sql in ["BEGIN", "COMMIT", "ROLLBACK", "SAVEPOINT q"] {
        assert_eq!(classify_statement(sql), Ok(StatementKind::Transaction));
        assert_eq!(
            enforce_connection_policy(sql, false),
            Err(SafetyError::UnsupportedStatement)
        );
    }
    for sql in ["EXEC sp_something", "VACUUM", "nonsense"] {
        assert_eq!(classify_statement(sql), Ok(StatementKind::Unknown));
        assert_eq!(require_read(sql), Err(SafetyError::ReadRequired));
    }
}

#[test]
fn connection_policy_never_allows_a_write_on_a_readonly_connection() {
    assert_eq!(
        enforce_connection_policy("SELECT 1", true),
        Ok(StatementKind::Read)
    );
    for sql in ["DELETE FROM users", "DROP TABLE users", "BEGIN", "unknown"] {
        assert_eq!(
            enforce_connection_policy(sql, true),
            Err(SafetyError::ConnectionReadOnly)
        );
    }
    assert_eq!(
        enforce_connection_policy("DELETE FROM users", false),
        Ok(StatementKind::Write)
    );
    assert_eq!(
        enforce_connection_policy("DROP TABLE users", false),
        Ok(StatementKind::Ddl)
    );
}

#[test]
fn invalid_and_oversized_input_is_rejected_without_echoing_it() {
    for sql in [
        "",
        "  ",
        "-- comment only",
        "/* comment */",
        "SELECT\0secret-marker",
    ] {
        let error = classify_statement(sql).unwrap_err();
        assert_eq!(error, SafetyError::InvalidQuery);
        assert!(!error.to_string().contains("secret-marker"));
    }
    assert_eq!(
        classify_statement(&"x".repeat(MAX_QUERY_BYTES + 1)),
        Err(SafetyError::TooLarge)
    );
}

#[test]
fn extracted_mcp_policy_preserves_defaults_and_fails_closed_on_unknown_kinds() {
    for kind in [
        "select",
        "write",
        "ddl",
        "unknown",
        "transaction",
        "unexpected",
    ] {
        assert!(!blocked_readonly(false, kind));
        assert_eq!(blocked_readonly(true, kind), kind != "select");
        assert!(!needs_approval("off", kind));
        assert!(needs_approval("all", kind));
        for mode in ["writes_only", "", "future-mode"] {
            assert_eq!(needs_approval(mode, kind), kind != "select");
        }
    }
}
