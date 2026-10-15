//! A conservative identifier scan for context selection, never SQL execution.
//! Strings, comments and function arguments do not become table references.
mod tokens;

use crate::models::TableInfo;
use std::collections::HashSet;
use tokens::{tokenize, Identifier, Token};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct TableReferences {
    pub tables: Vec<String>,
    pub missing: Vec<String>,
    pub truncated: bool,
}

struct Cte {
    name: Identifier,
    start: usize,
    end: usize,
}

fn closing_paren(tokens: &[Token], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        match token {
            Token::Open => depth += 1,
            Token::Close => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn ctes(tokens: &[Token]) -> (Vec<Cte>, bool) {
    let mut result = Vec::new();
    let mut with_count = 0;
    for (start, token) in tokens.iter().enumerate() {
        if !token.keyword("with") {
            continue;
        }
        with_count += 1;
        // Bound look-ahead work as well as input size for deeply nested or
        // malformed SQL. This scan is context discovery, not a full parser.
        if with_count > 128 {
            return (result, true);
        }
        let mut end = tokens.len();
        let mut depth = 0usize;
        for (index, token) in tokens.iter().enumerate().skip(start + 1) {
            match token {
                Token::Open => depth += 1,
                Token::Close if depth > 0 => depth -= 1,
                Token::Close | Token::Semicolon if depth == 0 => {
                    end = index;
                    break;
                }
                _ => {}
            }
        }
        let mut index = start + 1;
        let recursive = tokens
            .get(index)
            .is_some_and(|token| token.keyword("recursive"));
        if recursive {
            index += 1;
        }
        loop {
            let Some(Token::Identifier(name)) = tokens.get(index) else {
                break;
            };
            let name = name.clone();
            index += 1;
            if matches!(tokens.get(index), Some(Token::Open)) {
                let Some(close) = closing_paren(tokens, index) else {
                    break;
                };
                index = close + 1;
            }
            if !tokens.get(index).is_some_and(|token| token.keyword("as")) {
                break;
            }
            index += 1;
            if tokens.get(index).is_some_and(|token| token.keyword("not")) {
                index += 1;
            }
            if tokens
                .get(index)
                .is_some_and(|token| token.keyword("materialized"))
            {
                index += 1;
            }
            if !matches!(tokens.get(index), Some(Token::Open)) {
                break;
            }
            let Some(close) = closing_paren(tokens, index) else {
                break;
            };
            result.push(Cte {
                name,
                start: if recursive { start } else { close + 1 },
                end,
            });
            index = close + 1;
            if !matches!(tokens.get(index), Some(Token::Comma)) {
                break;
            }
            index += 1;
        }
    }
    (result, false)
}

fn matches_name(identifier: &Identifier, name: &str) -> bool {
    identifier.name == name || (!identifier.quoted && identifier.name.eq_ignore_ascii_case(name))
}

fn resolve_name<'a>(
    parts: &[Identifier],
    tables: &'a [TableInfo],
    database: Option<&str>,
    schema: Option<&str>,
) -> Option<&'a str> {
    let last = parts.last()?;
    // A catalog may itself use qualified names. An explicitly quoted dot stays
    // part of the identifier instead of becoming a scope separator.
    let joined = parts
        .iter()
        .map(|part| part.name.as_str())
        .collect::<Vec<_>>()
        .join(".");
    if let Some(table) = tables.iter().find(|table| table.name == joined) {
        return Some(&table.name);
    }
    let in_scope = match parts {
        [_] => true,
        [qualifier, _] => schema
            .or(database)
            .is_some_and(|scope| matches_name(qualifier, scope)),
        [db, owner, _] => {
            database.is_some_and(|scope| matches_name(db, scope))
                && schema.is_some_and(|scope| matches_name(owner, scope))
        }
        _ => false,
    };
    if !in_scope {
        return None;
    }
    let mut matches = tables
        .iter()
        .filter(|table| matches_name(last, &table.name));
    let first = matches.next()?;
    // Case-insensitive fallback must not choose one of two ambiguous objects.
    if matches.next().is_some() {
        return None;
    }
    Some(&first.name)
}

#[derive(Default)]
struct QueryScope {
    query: bool,
    insert: bool,
    from: bool,
}

/// Match FROM/JOIN/UPDATE/INTO references against the selected schema's catalog.
/// Unknown, ambiguous and cross-scope names are gaps, never guessed tables.
pub fn extract_referenced_tables(
    sql: &str,
    known_tables: &[TableInfo],
    database: Option<&str>,
    schema: Option<&str>,
) -> TableReferences {
    let (tokens, truncated) = tokenize(sql);
    let (ctes, ctes_truncated) = ctes(&tokens);
    let mut result = TableReferences {
        truncated: truncated || ctes_truncated,
        ..Default::default()
    };
    let mut seen = HashSet::new();
    let mut scopes = vec![QueryScope::default()];
    for (index, token) in tokens.iter().enumerate() {
        match token {
            Token::Open => {
                scopes.push(QueryScope::default());
                continue;
            }
            Token::Close => {
                if scopes.len() > 1 {
                    scopes.pop();
                }
                continue;
            }
            Token::Semicolon => {
                scopes = vec![QueryScope::default()];
                continue;
            }
            _ => {}
        }
        let scope = scopes.last_mut().unwrap();
        if token.keyword("select") || token.keyword("delete") {
            scope.query = true;
        }
        if token.keyword("insert") || token.keyword("merge") {
            scope.insert = true;
        }
        if [
            "where",
            "group",
            "order",
            "having",
            "limit",
            "offset",
            "returning",
            "union",
            "except",
            "intersect",
            "on",
            "set",
            "values",
        ]
        .iter()
        .any(|keyword| token.keyword(keyword))
        {
            scope.from = false;
        }
        let from = token.keyword("from") && scope.query;
        let join = token.keyword("join") && scope.query;
        let update = token.keyword("update")
            && !index
                .checked_sub(1)
                .and_then(|i| tokens.get(i))
                .is_some_and(|prev| {
                    prev.keyword("for") || prev.keyword("do") || prev.keyword("key")
                });
        let into = token.keyword("into") && (scope.insert || scope.query);
        let comma = matches!(token, Token::Comma) && scope.from;
        if !(from || join || update || into || comma) {
            continue;
        }
        if from || join {
            scope.from = true;
        }
        let mut next = index + 1;
        while tokens.get(next).is_some_and(|token| {
            ["only", "lateral", "low_priority", "ignore"]
                .iter()
                .any(|keyword| token.keyword(keyword))
        }) {
            next += 1;
        }
        let Some(Token::Identifier(first)) = tokens.get(next) else {
            continue;
        };
        if tokens[next].keyword("set") {
            continue;
        }
        let mut parts = vec![first.clone()];
        while matches!(tokens.get(next + 1), Some(Token::Dot)) {
            let Some(Token::Identifier(part)) = tokens.get(next + 2) else {
                break;
            };
            parts.push(part.clone());
            next += 2;
        }
        // Table-valued functions are not catalog tables. Their arguments are
        // scanned independently, so SELECT subqueries inside them still work.
        if matches!(tokens.get(next + 1), Some(Token::Open)) && !into {
            continue;
        }
        if parts.len() == 1
            && ctes.iter().any(|cte| {
                index >= cte.start && index < cte.end && matches_name(first, &cte.name.name)
            })
        {
            continue;
        }
        let label = parts
            .iter()
            .map(|part| part.name.as_str())
            .collect::<Vec<_>>()
            .join(".");
        match resolve_name(&parts, known_tables, database, schema) {
            Some(name) if seen.insert((true, name.to_string())) => result.tables.push(name.into()),
            None if seen.insert((false, label.clone())) => result.missing.push(label),
            _ => {}
        }
    }
    result
}

#[cfg(test)]
mod tests;
