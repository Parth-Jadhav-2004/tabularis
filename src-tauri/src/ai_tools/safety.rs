//! Shared SQL classification and conservative database tool policy.

/// MCP policy remains compatible; unknown statement kinds always count as writes.
pub fn blocked_readonly(readonly: bool, kind: &str) -> bool {
    readonly && kind != "select"
}

pub fn needs_approval(mode: &str, kind: &str) -> bool {
    match mode {
        "off" => false,
        "all" => true,
        _ => kind != "select",
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatementKind {
    Read,
    Write,
    Ddl,
    Transaction,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafetyError {
    InvalidQuery,
    TooLarge,
    MultipleStatements,
    ReadRequired,
    ConnectionReadOnly,
    UnsupportedStatement,
}

impl std::fmt::Display for SafetyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidQuery => "The query is empty or invalid.",
            Self::TooLarge => "The query exceeds the tool input limit.",
            Self::MultipleStatements => "Database tools accept one SQL statement per call.",
            Self::ReadRequired => "This tool only accepts read queries.",
            Self::ConnectionReadOnly => "This connection is read-only.",
            Self::UnsupportedStatement => "This statement is not supported by database tools.",
        })
    }
}

impl std::error::Error for SafetyError {}

const MAX_QUERY_BYTES: usize = 128 * 1024;

/// Strict chat/tool policy, independent of the legacy audit classification API.
/// Check both supported escaping interpretations before considering a read safe.
pub fn classify_statement(sql: &str) -> Result<StatementKind, SafetyError> {
    if sql.len() > MAX_QUERY_BYTES {
        return Err(SafetyError::TooLarge);
    }
    if sql.trim().is_empty() || sql.contains('\0') {
        return Err(SafetyError::InvalidQuery);
    }
    for backslash_escapes in [false, true] {
        let stripped = strip_impl(sql, backslash_escapes);
        if stripped.trim().is_empty() {
            return Err(SafetyError::InvalidQuery);
        }
        if has_trailing_statements(&stripped) {
            return Err(SafetyError::MultipleStatements);
        }
    }
    Ok(match classify_query_kind(sql) {
        "select" => StatementKind::Read,
        "write" => StatementKind::Write,
        "ddl" => StatementKind::Ddl,
        _ => {
            let stripped = strip_strings_and_comments(sql);
            let keyword = first_keyword(&stripped.trim_start().to_uppercase());
            match keyword.as_str() {
                "BEGIN" | "COMMIT" | "ROLLBACK" | "SAVEPOINT" | "RELEASE" | "START" => {
                    StatementKind::Transaction
                }
                _ => StatementKind::Unknown,
            }
        }
    })
}

pub fn require_read(sql: &str) -> Result<(), SafetyError> {
    match classify_statement(sql)? {
        StatementKind::Read => Ok(()),
        _ => Err(SafetyError::ReadRequired),
    }
}

/// Transaction control is excluded: tools own their read-only transaction scope.
pub fn enforce_connection_policy(sql: &str, readonly: bool) -> Result<StatementKind, SafetyError> {
    let kind = classify_statement(sql)?;
    if readonly && kind != StatementKind::Read {
        return Err(SafetyError::ConnectionReadOnly);
    }
    match kind {
        StatementKind::Read | StatementKind::Write | StatementKind::Ddl => Ok(kind),
        _ => Err(SafetyError::UnsupportedStatement),
    }
}

/// Classify the kind of SQL: `"select"`, `"write"`, `"ddl"`, or `"unknown"`.
///
/// Conservative on purpose: any input we cannot confidently identify as a
/// read returns `"unknown"`, so callers can fail closed when enforcing
/// safety policies (read-only mode, approval gates).
pub fn classify_query_kind(sql: &str) -> &'static str {
    let stripped = strip_strings_and_comments(sql);
    let trimmed = stripped.trim_start();
    if trimmed.is_empty() {
        return "unknown";
    }
    let trimmed_upper = trimmed.to_uppercase();

    if is_create_compound_routine(&trimmed_upper) {
        return "ddl";
    }

    // Fail closed for multi-statement payloads: a leading `SELECT 1; DROP …`
    // must NOT be tagged as a clean read just because the first keyword is
    // SELECT — the read-only and approval gates rely on this classification.
    //
    // SQL dialects disagree on backslash escaping inside string literals:
    // MySQL/MariaDB treat `\'` as an escaped quote by default, while
    // PostgreSQL standard-conforming strings treat `\` as a literal byte.
    // That disagreement shifts where a string literal ends, and therefore
    // where a `;` becomes a visible statement separator — e.g. MySQL reads
    // `SELECT '\''; DROP TABLE users` as two statements, the standard reading
    // as one. We strip under both interpretations and fail closed if EITHER
    // reveals a trailing statement, so a payload cannot hide an injected
    // separator by exploiting whichever dialect we happen not to assume.
    if has_trailing_statements(&stripped) || has_trailing_statements(&strip_impl(sql, true)) {
        return "unknown";
    }
    // Peel any leading `(` (and surrounding whitespace) before reading the
    // first keyword. A parenthesized query expression such as
    // `(SELECT ...) UNION ALL (SELECT ...)` — the shape you get when each
    // UNION branch needs its own ORDER BY / LIMIT — starts with `(`, so the
    // first keyword would otherwise come back empty and fall through to
    // "unknown", needlessly tripping the read-only and approval gates for a
    // pure read. Multi-statement payloads are already rejected above, so the
    // inner leading keyword is a safe basis for classification.
    let peeled_upper = trimmed
        .trim_start_matches(|c: char| c == '(' || c.is_whitespace())
        .to_uppercase();
    let first = first_keyword(&peeled_upper);

    let kind = match first.as_str() {
        // EXPLAIN needs option-aware handling: plain EXPLAIN only plans, but
        // EXPLAIN ANALYZE (and EXPLAIN (ANALYZE ...)) *executes* the wrapped
        // statement. Classifying it blindly as a read would let
        // `EXPLAIN ANALYZE DELETE …` run past read-only mode and the approval
        // gate. See `classify_explain`.
        "EXPLAIN" => classify_explain(&peeled_upper),
        // A leading SELECT is not proof of a read: `SELECT … INTO` creates a
        // table (PostgreSQL) or writes a file (MySQL `INTO OUTFILE`), locking
        // clauses take row locks, and some functions act on the server.
        "SELECT" | "VALUES" => classify_select_side_effects(&peeled_upper),
        // `PRAGMA x = …` and `PRAGMA x(…)` change database settings (SQLite).
        "PRAGMA" => classify_pragma(&peeled_upper),
        "SHOW" | "DESCRIBE" | "DESC" => "select",
        "INSERT" | "UPDATE" | "DELETE" | "MERGE" | "REPLACE" => "write",
        "CREATE" | "DROP" | "ALTER" | "TRUNCATE" | "RENAME" | "GRANT" | "REVOKE" | "COMMENT" => {
            "ddl"
        }
        "WITH" => classify_cte(&peeled_upper),
        _ => "unknown",
    };

    // A statement classified as a clean read must not reach a side-effecting
    // function through a quoted name, which the denylist (matching bare names)
    // would miss. Fail closed: the engine read-only backstop does not stop
    // every such function (e.g. `lo_export` writes a file even inside a
    // read-only transaction), so the classifier is the last line of defence.
    if kind == "select" && has_quoted_function_call(sql) {
        return "unknown";
    }
    kind
}

fn is_create_compound_routine(upper_sql: &str) -> bool {
    if first_keyword(upper_sql) != "CREATE" {
        return false;
    }

    let header = upper_sql.split("BEGIN").next().unwrap_or(upper_sql);
    let is_routine = ["PROCEDURE", "FUNCTION", "TRIGGER", "EVENT"]
        .iter()
        .any(|keyword| contains_keyword(header, keyword));
    if !is_routine {
        return false;
    }

    let without_terminal_semicolon = upper_sql.trim_end().trim_end_matches(';').trim_end();
    if !without_terminal_semicolon.ends_with("END") {
        return false;
    }

    !has_complete_end_before_terminal_statement(without_terminal_semicolon)
}

fn has_complete_end_before_terminal_statement(upper_sql: &str) -> bool {
    let terminal_end = match upper_sql.rfind("END") {
        Some(index) => index,
        None => return false,
    };
    let before_terminal = &upper_sql[..terminal_end];
    let bytes = before_terminal.as_bytes();
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';

    for i in 0..bytes.len().saturating_sub(2) {
        if &bytes[i..i + 3] != b"END" {
            continue;
        }
        let prev_ok = i == 0 || !is_word(bytes[i - 1]);
        let next_ok = i + 3 == bytes.len() || !is_word(bytes[i + 3]);
        if !prev_ok || !next_ok {
            continue;
        }

        let mut cursor = i + 3;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= bytes.len() || bytes[cursor] != b';' {
            continue;
        }
        cursor += 1;
        while cursor < bytes.len() && (bytes[cursor].is_ascii_whitespace() || bytes[cursor] == b';')
        {
            cursor += 1;
        }
        if cursor < bytes.len() && bytes[cursor].is_ascii_alphabetic() {
            return true;
        }
    }

    false
}

/// Returns true when `stripped` contains a semicolon followed by additional
/// non-whitespace SQL content — i.e., more than one statement.
///
/// The input must already have been run through `strip_impl`, which replaces
/// string-literal, comment, and quoted-identifier bytes with whitespace, so
/// any `;` that survives here is a real statement terminator under the
/// escaping interpretation used to produce `stripped`.
pub(crate) fn has_trailing_statements(stripped: &str) -> bool {
    let mut found_semi = false;
    for c in stripped.chars() {
        if found_semi && !c.is_whitespace() {
            return true;
        }
        if c == ';' {
            found_semi = true;
        }
    }
    false
}

fn first_keyword(upper: &str) -> String {
    upper
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect()
}

fn classify_cte(stripped_upper: &str) -> &'static str {
    let ddl = ["CREATE", "DROP", "ALTER", "TRUNCATE", "RENAME"];
    let dml = ["INSERT", "UPDATE", "DELETE", "MERGE", "REPLACE"];
    for kw in &ddl {
        if contains_keyword(stripped_upper, kw) {
            return "ddl";
        }
    }
    for kw in &dml {
        if contains_keyword(stripped_upper, kw) {
            return "write";
        }
    }
    classify_select_side_effects(stripped_upper)
}

/// PostgreSQL functions that act outside the current transaction (signal
/// other sessions, reload config, touch server files, open a second
/// connection). A read-only transaction does not stop them, so a query that
/// calls one is never a clean read.
const SIDE_EFFECT_FUNCTIONS: &[&str] = &[
    "PG_TERMINATE_BACKEND",
    "PG_CANCEL_BACKEND",
    "PG_RELOAD_CONF",
    "PG_ROTATE_LOGFILE",
    "PG_SWITCH_WAL",
    "PG_PROMOTE",
    "PG_CREATE_RESTORE_POINT",
    "PG_LOG_BACKEND_MEMORY_CONTEXTS",
    "LO_IMPORT",
    "LO_EXPORT",
    "LO_UNLINK",
    "LO_CREATE",
    "LO_CREAT",
    "LO_PUT",
    "LO_FROM_BYTEA",
    "LO_TRUNCATE",
    "LOWRITE",
    "DBLINK",
    "DBLINK_EXEC",
    "DBLINK_CONNECT",
    "DBLINK_CONNECT_U",
    "DBLINK_SEND_QUERY",
    "SETVAL",
    "NEXTVAL",
];

/// Classify a statement that reads like a query (`SELECT …`, `VALUES …`, or
/// the final statement of a CTE) by looking for clauses that make it write.
///
/// `stripped_upper` must have string literals, quoted identifiers and
/// comments removed, so a column alias or literal cannot trigger a match.
fn classify_select_side_effects(stripped_upper: &str) -> &'static str {
    // `SELECT … INTO new_table` (PostgreSQL), `INTO OUTFILE` / `INTO DUMPFILE`
    // (MySQL). `INSERT … SELECT` never gets here: it starts with INSERT.
    if contains_keyword(stripped_upper, "INTO") {
        return "write";
    }
    let words: Vec<&str> = stripped_upper
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .collect();
    let has_locking_clause = words
        .windows(2)
        .any(|pair| pair[0] == "FOR" && matches!(pair[1], "UPDATE" | "SHARE" | "NO" | "KEY"))
        || words
            .windows(4)
            .any(|w| w == ["LOCK", "IN", "SHARE", "MODE"]);
    if has_locking_clause {
        return "write";
    }
    if SIDE_EFFECT_FUNCTIONS
        .iter()
        .any(|name| calls_function(stripped_upper, name))
    {
        return "write";
    }
    "select"
}

/// Whether `haystack` contains a call to `name`: the whole word followed,
/// after optional whitespace, by `(`.
fn calls_function(haystack: &str, name: &str) -> bool {
    let mut offset = 0;
    while let Some(index) = keyword_index(&haystack[offset..], name) {
        let after = &haystack[offset + index + name.len()..];
        if after.trim_start().starts_with('(') {
            return true;
        }
        offset += index + name.len();
    }
    false
}

/// SQLite pragmas that only report information, even when given an argument
/// (`PRAGMA table_info(t)`). Any other pragma called with an argument sets a
/// value.
const INTROSPECTION_PRAGMAS: &[&str] = &[
    "TABLE_INFO",
    "TABLE_XINFO",
    "TABLE_LIST",
    "INDEX_LIST",
    "INDEX_INFO",
    "INDEX_XINFO",
    "FOREIGN_KEY_LIST",
    "FOREIGN_KEY_CHECK",
    "INTEGRITY_CHECK",
    "QUICK_CHECK",
];

/// SQLite pragmas that do work even without an argument.
const ACTION_PRAGMAS: &[&str] = &[
    "OPTIMIZE",
    "WAL_CHECKPOINT",
    "INCREMENTAL_VACUUM",
    "SHRINK_MEMORY",
];

/// Classify a `PRAGMA` statement. Reading a pragma (`PRAGMA user_version`) or
/// calling an introspection pragma (`PRAGMA table_info(t)`) is a read;
/// assigning one (`PRAGMA user_version = 42`), passing a value to any other
/// pragma (`PRAGMA journal_mode(DELETE)`) or running an action pragma
/// (`PRAGMA optimize`) is a write.
///
/// `pragma_upper` must be the stripped, upper-cased SQL whose first keyword
/// is `PRAGMA`.
fn classify_pragma(pragma_upper: &str) -> &'static str {
    let body = pragma_upper["PRAGMA".len()..].trim();
    let body = body.trim_end_matches(';').trim_end();
    if body.contains('=') {
        return "write";
    }
    // Drop an optional `schema.` prefix.
    let name_part = body.split('(').next().unwrap_or("").trim();
    let name = name_part.rsplit('.').next().unwrap_or("").trim();
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return "unknown";
    }
    if ACTION_PRAGMAS.contains(&name) {
        return "write";
    }
    if body.contains('(') && !INTROSPECTION_PRAGMAS.contains(&name) {
        return "write";
    }
    "select"
}

fn contains_keyword(haystack: &str, needle: &str) -> bool {
    keyword_index(haystack, needle).is_some()
}

/// Byte offset of the first whole-word occurrence of `needle` in `haystack`,
/// or `None`. "Whole word" means the match is not flanked by an identifier
/// byte on either side, so `ANALYZE` does not match inside `analyze_runs`.
fn keyword_index(haystack: &str, needle: &str) -> Option<usize> {
    let bytes = haystack.as_bytes();
    let nbytes = needle.as_bytes();
    let nlen = nbytes.len();
    if nlen == 0 || bytes.len() < nlen {
        return None;
    }
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    for i in 0..=bytes.len() - nlen {
        if &bytes[i..i + nlen] == nbytes {
            let prev_ok = i == 0 || !is_word(bytes[i - 1]);
            let next_ok = i + nlen == bytes.len() || !is_word(bytes[i + nlen]);
            if prev_ok && next_ok {
                return Some(i);
            }
        }
    }
    None
}

/// Classify an `EXPLAIN …` statement, whose kind depends on its options.
///
/// A plain `EXPLAIN` only produces a plan and never touches data, so it is a
/// read. But `EXPLAIN ANALYZE …` (legacy syntax) and `EXPLAIN (ANALYZE …) …`
/// (parenthesized options) *run* the wrapped statement to gather actual timing
/// — so an `ANALYZE` option makes the whole thing as dangerous as the wrapped
/// statement itself. We therefore detect an `ANALYZE` option and, when present,
/// classify the wrapped statement (`write`/`ddl`/`unknown`) instead of `select`.
///
/// `explain_upper` must be the stripped, upper-cased SQL whose first keyword is
/// `EXPLAIN`.
fn classify_explain(explain_upper: &str) -> &'static str {
    let after = explain_upper["EXPLAIN".len()..].trim_start();
    let (options, wrapped) = split_explain_options(after);

    // Only ANALYZE causes execution; every other option (VERBOSE, BUFFERS,
    // FORMAT …, SQLite's QUERY PLAN, MySQL's FORMAT=JSON) is plan-only.
    if !contains_keyword(options, "ANALYZE") {
        return "select";
    }
    classify_wrapped_statement(wrapped)
}

/// Split the text following `EXPLAIN` into `(options, wrapped_statement)`.
///
/// Handles both the parenthesized form `(ANALYZE, BUFFERS) SELECT …` and the
/// bare form `ANALYZE VERBOSE SELECT …`. For the bare form the option list runs
/// up to the wrapped statement's first keyword. Fails closed (everything is
/// "options", wrapped is empty) when a leading `(` is never closed.
fn split_explain_options(after: &str) -> (&str, &str) {
    if after.starts_with('(') {
        let bytes = after.as_bytes();
        let mut depth = 0usize;
        for (i, &b) in bytes.iter().enumerate() {
            match b {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return (&after[1..i], after[i + 1..].trim_start());
                    }
                }
                _ => {}
            }
        }
        return (after, "");
    }
    match first_wrapped_statement_index(after) {
        Some(idx) => (&after[..idx], &after[idx..]),
        None => (after, ""),
    }
}

/// Byte offset of the earliest statement keyword that could begin the target of
/// an `EXPLAIN`, used to find where a bare option list ends.
fn first_wrapped_statement_index(upper: &str) -> Option<usize> {
    const KEYWORDS: &[&str] = &[
        "SELECT", "INSERT", "UPDATE", "DELETE", "MERGE", "REPLACE", "VALUES", "TABLE", "WITH",
        "CREATE", "DROP", "ALTER", "TRUNCATE", "RENAME",
    ];
    KEYWORDS
        .iter()
        .filter_map(|kw| keyword_index(upper, kw))
        .min()
}

/// Classify the statement wrapped by an executing `EXPLAIN ANALYZE`, as if it
/// had been submitted directly. Unrecognized/empty input is `"unknown"` so the
/// safety gates fail closed.
fn classify_wrapped_statement(wrapped_upper: &str) -> &'static str {
    let peeled = wrapped_upper.trim_start_matches(|c: char| c == '(' || c.is_whitespace());
    match first_keyword(peeled).as_str() {
        "SELECT" | "VALUES" => classify_select_side_effects(peeled),
        "PRAGMA" => classify_pragma(peeled),
        "SHOW" | "DESCRIBE" | "DESC" | "TABLE" => "select",
        "INSERT" | "UPDATE" | "DELETE" | "MERGE" | "REPLACE" => "write",
        "CREATE" | "DROP" | "ALTER" | "TRUNCATE" | "RENAME" | "GRANT" | "REVOKE" | "COMMENT" => {
            "ddl"
        }
        "WITH" => classify_cte(peeled),
        _ => "unknown",
    }
}

/// Replace string literals and SQL comments with whitespace so keyword
/// scanning cannot be fooled by tokens that live inside a value, comment,
/// or quoted identifier.
///
/// Uses the SQL-standard reading of single-quoted strings (only `''` escapes
/// a quote). For the backslash-aware reading used to harden multi-statement
/// detection against MySQL-style escaping, see [`strip_impl`].
pub fn strip_strings_and_comments(sql: &str) -> String {
    strip_impl(sql, false)
}

/// Backing implementation for [`strip_strings_and_comments`].
///
/// When `backslash_escapes` is true, a `\` inside a single-quoted string
/// escapes the following byte (MySQL/MariaDB default). When false, backslash
/// is an ordinary literal byte (SQL standard / PostgreSQL standard strings).
/// Only single-quoted string scanning honours the flag — comments and quoted
/// identifiers are dialect-independent here.
fn strip_impl(sql: &str, backslash_escapes: bool) -> String {
    strip_impl_opts(sql, backslash_escapes, false)
}

/// Backing implementation for [`strip_impl`] and [`strip_identifiers_marked`].
///
/// When `mark_identifiers` is true, each quoted identifier is replaced by the
/// marker byte [`IDENT_MARK`] instead of spaces, so a later pass can tell a
/// quoted identifier apart from a blanked string literal — needed to spot a
/// function invoked through a quoted name (`"lo_export"(…)`).
fn strip_impl_opts(sql: &str, backslash_escapes: bool, mark_identifiers: bool) -> String {
    let ident_fill = if mark_identifiers { IDENT_MARK } else { b' ' };
    let bytes = sql.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        // Line comment
        if c == b'-' && i + 1 < bytes.len() && bytes[i + 1] == b'-' {
            while i < bytes.len() && bytes[i] != b'\n' {
                out.push(b' ');
                i += 1;
            }
            continue;
        }
        // Block comment (non-nesting; sufficient for classification)
        if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            out.push(b' ');
            out.push(b' ');
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                out.push(b' ');
                i += 1;
            }
            if i + 1 < bytes.len() {
                out.push(b' ');
                out.push(b' ');
                i += 2;
            } else {
                while i < bytes.len() {
                    out.push(b' ');
                    i += 1;
                }
            }
            continue;
        }
        // Escape string constant (`E'…'` / `e'…'`, PostgreSQL): a backslash
        // always escapes the next byte here, regardless of the dialect flag.
        // Without this, `E'\''` desynchronises the scanner under the standard
        // reading — the `\` is treated as literal, the following `''` reads as
        // an escaped quote, and the string never closes — masking whatever
        // comes after (e.g. a side-effecting `lo_export(…)` call).
        if (c == b'E' || c == b'e')
            && i + 1 < bytes.len()
            && bytes[i + 1] == b'\''
            && (i == 0 || !is_ident_byte(bytes[i - 1]))
        {
            out.push(b' ');
            out.push(b' ');
            i += 2;
            while i < bytes.len() {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    out.push(b' ');
                    out.push(b' ');
                    i += 2;
                } else if bytes[i] == b'\'' && i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    out.push(b' ');
                    out.push(b' ');
                    i += 2;
                } else if bytes[i] == b'\'' {
                    out.push(b' ');
                    i += 1;
                    break;
                } else {
                    out.push(b' ');
                    i += 1;
                }
            }
            continue;
        }
        // Dollar-quoted string constant (`$tag$ … $tag$`, PostgreSQL). The body
        // is wholly literal — no escapes — so it ends only at the matching
        // delimiter. Scanning it as text (rather than skipping it) is what let
        // `$$'$$, lo_export(…)` hide the call: the `'` inside was read as an
        // ordinary string opener and swallowed the rest of the statement.
        if c == b'$' {
            if let Some(tag_len) = dollar_quote_tag_len(&bytes[i..]) {
                let delim = &bytes[i..i + tag_len];
                for _ in 0..tag_len {
                    out.push(b' ');
                }
                i += tag_len;
                while i < bytes.len() {
                    if bytes[i] == b'$' && bytes[i..].starts_with(delim) {
                        for _ in 0..tag_len {
                            out.push(b' ');
                        }
                        i += tag_len;
                        break;
                    }
                    out.push(b' ');
                    i += 1;
                }
                continue;
            }
        }
        // Single-quoted string literal (SQL-style escaped quote: '')
        if c == b'\'' {
            out.push(b' ');
            i += 1;
            while i < bytes.len() {
                if backslash_escapes && bytes[i] == b'\\' && i + 1 < bytes.len() {
                    // MySQL-style backslash escape: the next byte is part of
                    // the string regardless of what it is (including `'`).
                    out.push(b' ');
                    out.push(b' ');
                    i += 2;
                } else if bytes[i] == b'\'' && i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    out.push(b' ');
                    out.push(b' ');
                    i += 2;
                } else if bytes[i] == b'\'' {
                    out.push(b' ');
                    i += 1;
                    break;
                } else {
                    out.push(b' ');
                    i += 1;
                }
            }
            continue;
        }
        // Double-quoted identifier (PostgreSQL / ANSI)
        if c == b'"' {
            out.push(ident_fill);
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                out.push(ident_fill);
                i += 1;
            }
            if i < bytes.len() {
                out.push(ident_fill);
                i += 1;
            }
            continue;
        }
        // Backtick-quoted identifier (MySQL)
        if c == b'`' {
            out.push(ident_fill);
            i += 1;
            while i < bytes.len() && bytes[i] != b'`' {
                out.push(ident_fill);
                i += 1;
            }
            if i < bytes.len() {
                out.push(ident_fill);
                i += 1;
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_default()
}

/// Marker byte that [`strip_identifiers_marked`] leaves in place of a quoted
/// identifier. `0x01` cannot occur in the stripped output otherwise (every
/// other byte is either copied source or a blanking space), so a run of it
/// unambiguously marks where a quoted identifier stood.
const IDENT_MARK: u8 = 0x01;

/// Whether `b` can appear inside an unquoted identifier. Used to tell an
/// `E'…'` escape string apart from an identifier that merely ends in `E`.
fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// If `bytes` starts with a PostgreSQL dollar-quote opening delimiter
/// (`$$` or `$tag$`, where `tag` is an identifier), return its length in
/// bytes; otherwise `None` (e.g. a `$1` positional parameter).
fn dollar_quote_tag_len(bytes: &[u8]) -> Option<usize> {
    debug_assert_eq!(bytes.first(), Some(&b'$'));
    let mut j = 1;
    if j < bytes.len() && (bytes[j].is_ascii_alphabetic() || bytes[j] == b'_') {
        j += 1;
        while j < bytes.len() && is_ident_byte(bytes[j]) {
            j += 1;
        }
    }
    if j < bytes.len() && bytes[j] == b'$' {
        Some(j + 1)
    } else {
        None
    }
}

/// Like [`strip_strings_and_comments`], but quoted identifiers become runs of
/// [`IDENT_MARK`] instead of spaces so [`has_quoted_function_call`] can find a
/// function invoked through a quoted name.
fn strip_identifiers_marked(sql: &str) -> String {
    strip_impl_opts(sql, false, true)
}

/// Whether the statement calls a function through a *quoted* identifier, i.e. a
/// quoted identifier immediately followed by `(` — `"lo_export"(…)`,
/// `pg_catalog."lo_export"(…)`, `U&"lo_export"(…)`. The side-effect-function
/// denylist matches on the bare name, so quoting the name slips a write-shaped
/// call (file write via `lo_export`, backend signals, `dblink_exec`, …) past
/// it. A read-only transaction does not stop several of those functions, so a
/// query that calls one by a quoted name is never treated as a clean read.
fn has_quoted_function_call(sql: &str) -> bool {
    let marked = strip_identifiers_marked(sql);
    let bytes = marked.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == IDENT_MARK {
            while i < bytes.len() && bytes[i] == IDENT_MARK {
                i += 1;
            }
            let next = bytes[i..].iter().find(|b| !b.is_ascii_whitespace());
            if next == Some(&b'(') {
                return true;
            }
        } else {
            i += 1;
        }
    }
    false
}

#[cfg(test)]
#[path = "safety/tests.rs"]
mod tests;
