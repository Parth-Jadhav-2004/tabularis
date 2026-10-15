use super::*;
use crate::models::{ExportPayload, SavedConnection};

fn connection(instructions: serde_json::Value) -> SavedConnection {
    serde_json::from_value(serde_json::json!({
        "id": "fixture", "name": "Fixture", "params": {"driver": "sqlite", "database": "fixture.db"},
        "ai_instructions": instructions,
    })).unwrap()
}

#[test]
fn unicode_limit_is_shared_by_validation_and_deserialization() {
    let text = "😀".repeat(MAX_INSTRUCTION_CHARS);
    assert_eq!(
        normalize_instructions(Some(text.clone())).unwrap(),
        Some(text.clone())
    );
    assert_eq!(
        connection(serde_json::json!(text))
            .ai_instructions
            .as_deref(),
        Some(text.as_str())
    );
    let too_long = format!("{text}a");
    assert!(normalize_instructions(Some(too_long.clone())).is_err());
    let mut value = serde_json::to_value(connection(serde_json::Value::Null)).unwrap();
    value["ai_instructions"] = serde_json::json!(too_long);
    assert!(serde_json::from_value::<SavedConnection>(value).is_err());
}

#[test]
fn old_connections_and_empty_preferences_remain_compatible() {
    for value in [
        serde_json::Value::Null,
        serde_json::json!(""),
        serde_json::json!(" \n\t "),
    ] {
        let conn = connection(value);
        assert!(conn.ai_instructions.is_none());
        assert!(serde_json::to_value(conn)
            .unwrap()
            .get("ai_instructions")
            .is_none());
    }
    let value = serde_json::to_value(connection(serde_json::Value::Null)).unwrap();
    assert!(serde_json::from_value::<SavedConnection>(value)
        .unwrap()
        .ai_instructions
        .is_none());
    let text = "  Prefer explicit JOINs.\n保留别名 😀  ";
    assert_eq!(
        connection(serde_json::json!(text))
            .ai_instructions
            .as_deref(),
        Some(text)
    );
}

#[test]
fn native_export_import_and_persistence_preserve_and_clear_preferences() {
    let text = "Prefer explicit joins.\nUse qualified table names.";
    let conn = connection(serde_json::json!(text));
    let payload = ExportPayload {
        version: 1,
        groups: vec![],
        connections: vec![conn.clone()],
        ssh_connections: vec![],
        k8s_connections: vec![],
        tags: vec![],
    };
    let exported = serde_json::to_string(&payload).unwrap();
    let decoded: ExportPayload = serde_json::from_str(&exported).unwrap();
    let imported = crate::connection_import::tabularis::apply(
        &decoded,
        &[crate::connection_import::convert::ImportResolution {
            index: 0,
            action: "import".into(),
            replace_existing_id: None,
            group_id: None,
            new_group_name: None,
            new_group_parent_id: None,
        }],
        &[],
    );
    assert_eq!(
        imported.connections[0].ai_instructions.as_deref(),
        Some(text)
    );
    assert_ne!(imported.connections[0].id, conn.id);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("connections.json");
    crate::persistence::save_connections(&path, &[conn]).unwrap();
    let mut file = crate::persistence::load_connections_file(&path).unwrap();
    assert_eq!(file.connections[0].ai_instructions.as_deref(), Some(text));
    file.connections[0].ai_instructions = None;
    crate::persistence::save_connections_file(&path, &file).unwrap();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(!saved.contains("ai_instructions"));
    assert!(!saved.contains(text));
}
