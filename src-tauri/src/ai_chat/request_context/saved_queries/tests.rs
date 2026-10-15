use super::*;
use serde_json::json;

fn fixture() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    fs::write(
        dir.path().join(format!("{id}.sql")),
        "SELECT 'private-sql-marker 界';",
    )
    .unwrap();
    fs::write(dir.path().join("meta.json"), serde_json::to_vec(&json!([{
        "id":id,"name":"Revenue","filename":format!("{id}.sql"),"connection_id":"one","database":"app"
    }])).unwrap()).unwrap();
    (dir, id)
}

#[test]
fn selected_queries_are_scoped_deduplicated_and_read_afresh() {
    let (dir, id) = fixture();
    let result = load_from_dir(dir.path(), "one", Some("app"), &[id.clone(), id.clone()]);
    assert_eq!(result.queries.len(), 1);
    assert!(result.queries[0].sql.contains("private-sql-marker"));
    assert!(result.unavailable.is_empty());
    assert_eq!(
        load_from_dir(dir.path(), "two", Some("app"), &[id.clone()]).unavailable,
        [id.clone()]
    );
    assert_eq!(
        load_from_dir(dir.path(), "one", Some("other"), &[id.clone()]).unavailable,
        [id.clone()]
    );
    fs::write(dir.path().join(format!("{id}.sql")), "SELECT 'updated';").unwrap();
    assert_eq!(
        load_from_dir(dir.path(), "one", Some("app"), &[id]).queries[0].sql,
        "SELECT 'updated';"
    );
}

#[test]
fn unscoped_queries_belong_to_their_connection_and_missing_files_are_explicit() {
    let (dir, id) = fixture();
    let path = dir.path().join("meta.json");
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    metadata[0]["database"] = serde_json::Value::Null;
    fs::write(&path, serde_json::to_vec(&metadata).unwrap()).unwrap();
    assert_eq!(
        load_from_dir(dir.path(), "one", Some("other"), &[id.clone()])
            .queries
            .len(),
        1
    );
    fs::remove_file(dir.path().join(format!("{id}.sql"))).unwrap();
    assert_eq!(
        load_from_dir(dir.path(), "one", None, &[id.clone()]).unavailable,
        [id]
    );
}

#[test]
fn file_limits_preserve_unicode_and_reject_invalid_utf8_or_oversized_metadata() {
    let (dir, id) = fixture();
    let file = dir.path().join(format!("{id}.sql"));
    fs::write(&file, "界".repeat(SQL_LIMIT)).unwrap();
    let result = load_from_dir(dir.path(), "one", Some("app"), &[id.clone()]);
    assert!(result.queries[0].truncated);
    assert!(result.queries[0].sql.len() <= SQL_LIMIT);
    assert!(!result.queries[0].sql.contains('\u{fffd}'));
    fs::write(&file, [255, 255]).unwrap();
    assert_eq!(
        load_from_dir(dir.path(), "one", Some("app"), &[id.clone()]).unavailable,
        [id.clone()]
    );
    fs::write(dir.path().join("meta.json"), "x".repeat(META_LIMIT + 1)).unwrap();
    assert!(load_from_dir(dir.path(), "one", Some("app"), &[id])
        .queries
        .is_empty());
}

#[test]
fn ambiguous_ids_and_metadata_path_redirection_are_unavailable() {
    let (dir, id) = fixture();
    let path = dir.path().join("meta.json");
    let mut metadata: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    metadata.push(metadata[0].clone());
    fs::write(&path, serde_json::to_vec(&metadata).unwrap()).unwrap();
    assert_eq!(
        load_from_dir(dir.path(), "one", Some("app"), &[id.clone()]).unavailable,
        [id.clone()]
    );
    metadata.truncate(1);
    metadata[0]["filename"] = json!("../private.sql");
    fs::write(&path, serde_json::to_vec(&metadata).unwrap()).unwrap();
    assert_eq!(
        load_from_dir(dir.path(), "one", Some("app"), &[id.clone()]).unavailable,
        [id]
    );
}

#[cfg(unix)]
#[test]
fn symlinked_files_and_directories_are_unavailable() {
    use std::os::unix::fs::symlink;
    let (dir, id) = fixture();
    let file = dir.path().join(format!("{id}.sql"));
    fs::remove_file(&file).unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    fs::write(outside.path(), "private-outside-marker").unwrap();
    symlink(outside.path(), &file).unwrap();
    assert!(load_from_dir(dir.path(), "one", Some("app"), &[id.clone()])
        .queries
        .is_empty());
    let parent = tempfile::tempdir().unwrap();
    let link = parent.path().join("queries");
    symlink(dir.path(), &link).unwrap();
    assert!(load_from_dir(&link, "one", Some("app"), &[id])
        .queries
        .is_empty());
}
