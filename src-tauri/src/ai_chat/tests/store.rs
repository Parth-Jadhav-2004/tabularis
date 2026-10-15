use super::*;
use crate::ai::message::{Block, Message};
use crate::ai_chat::types::{ChatMode, ChatTarget};
use crate::secure_store::testing::TestDirectory;

pub(crate) fn conversation(connection: &str, id: &str) -> Conversation {
    Conversation {
        schema_version: 1,
        id: id.into(),
        connection_id: connection.into(),
        database: Some("analytics".into()),
        schema: Some("public".into()),
        title: "private-title-marker".into(),
        title_source: TitleSource::FirstMessage,
        mode: ChatMode::Ask,
        target: Some(ChatTarget::Notebook {
            notebook_id: "notebook-1".into(),
        }),
        provider: Some("anthropic".into()),
        model: Some("model".into()),
        created_at: 123,
        updated_at: 123,
        messages: vec![Message::user("private-message-marker")],
    }
}

#[test]
fn write_list_load_and_rename_round_trip_with_sealed_index() {
    let dir = TestDirectory::new().unwrap();
    let store = ConversationStore::new(dir.path());
    let chat = conversation("c1", "one");
    assert!(!store.save(chat.clone()).unwrap().large);
    let loaded = store.load("c1", "one").unwrap();
    assert_eq!(loaded.messages, chat.messages);
    assert_eq!(loaded.target, chat.target);
    let list = store.list(Some("c1")).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].message_count, 1);
    store
        .rename("c1", "one", "renamed-private-marker".into())
        .unwrap();
    assert_eq!(
        store.load("c1", "one").unwrap().title_source,
        TitleSource::User
    );
    for name in ["one.chat", "index.chat"] {
        let text = fs::read_to_string(dir.path().join("ai_chats/c1").join(name)).unwrap();
        assert!(!text.contains("private-marker"));
        assert!(!text.contains("private-message-marker"));
        assert!(!text.contains("private-title-marker"));
    }
}

#[test]
fn listing_all_connections_and_deleting_are_scoped() {
    let dir = TestDirectory::new().unwrap();
    let store = ConversationStore::new(dir.path());
    assert!(store.list(None).unwrap().is_empty());
    store.save(conversation("c1", "same-id")).unwrap();
    store.save(conversation("c2", "same-id")).unwrap();
    assert_eq!(store.list(None).unwrap().len(), 2);
    store.delete("c1", "same-id").unwrap();
    assert!(store.list(Some("c1")).unwrap().is_empty());
    assert!(store.load("c2", "same-id").is_ok());
    store.delete_connection("c2").unwrap();
    assert!(store.list(None).unwrap().is_empty());
    store.delete_all().unwrap();
    assert!(!dir.path().join("ai_chats").exists());
}

#[test]
fn missing_corrupted_and_stale_indexes_rebuild_from_conversations() {
    let dir = TestDirectory::new().unwrap();
    let store = ConversationStore::new(dir.path());
    store.save(conversation("c", "one")).unwrap();
    let index = dir.path().join("ai_chats/c/index.chat");
    fs::remove_file(&index).unwrap();
    assert_eq!(store.list(Some("c")).unwrap().len(), 1);
    fs::write(&index, "corrupted").unwrap();
    assert_eq!(store.list(Some("c")).unwrap().len(), 1);
    let old_index = fs::read(&index).unwrap();
    store.save(conversation("c", "two")).unwrap();
    fs::write(&index, old_index).unwrap();
    assert_eq!(store.list(Some("c")).unwrap().len(), 2);
}

#[test]
fn copied_conversations_fail_with_wrong_context() {
    let dir = TestDirectory::new().unwrap();
    let store = ConversationStore::new(dir.path());
    store.save(conversation("c", "one")).unwrap();
    fs::copy(
        dir.path().join("ai_chats/c/one.chat"),
        dir.path().join("ai_chats/c/two.chat"),
    )
    .unwrap();
    assert_eq!(store.load("c", "two"), Err(StoreError::WrongContext));
}

#[test]
fn traversal_reserved_ids_and_blank_titles_are_rejected() {
    let dir = TestDirectory::new().unwrap();
    let store = ConversationStore::new(dir.path());
    for id in [
        "../outside",
        "",
        "/tmp/outside",
        "index",
        "blobs",
        "a.b",
        "a\\b",
    ] {
        assert!(store.save(conversation("c", id)).is_err());
        assert!(store.list(Some(id)).is_err());
    }
    let mut chat = conversation("c", "one");
    chat.title = "  ".into();
    assert!(store.save(chat).is_err());
}

#[test]
fn unknown_blocks_survive_loading_without_losing_supported_blocks() {
    let dir = TestDirectory::new().unwrap();
    let chat = conversation("c", "one");
    let store = ConversationStore::new(dir.path());
    store.save(chat.clone()).unwrap();
    let mut value = serde_json::to_value(chat).unwrap();
    value["messages"][0]["blocks"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"type":"future_block"}));
    with_store_in(dir.path(), false, |cipher| {
        cipher.write_sealed(
            &dir.path().join("ai_chats/c/one.chat"),
            &serde_json::to_vec(&value).unwrap(),
            "chat:c:one",
        )
    })
    .unwrap();
    assert_eq!(store.load("c", "one").unwrap().messages[0].blocks.len(), 1);
}

#[test]
fn newer_versions_and_corrupt_history_are_never_overwritten() {
    let dir = TestDirectory::new().unwrap();
    let store = ConversationStore::new(dir.path());
    let chat = conversation("c", "one");
    store.save(chat.clone()).unwrap();
    let path = dir.path().join("ai_chats/c/one.chat");
    let mut newer = chat.clone();
    newer.schema_version = 2;
    assert!(store.save(newer.clone()).is_err());
    with_store_in(dir.path(), false, |cipher| {
        cipher.write_sealed(&path, &serde_json::to_vec(&newer).unwrap(), "chat:c:one")
    })
    .unwrap();
    assert_eq!(store.load("c", "one"), Err(StoreError::UnsupportedVersion));
    assert!(store.save(chat.clone()).is_err());
    fs::write(&path, "corrupted history").unwrap();
    assert!(store.save(chat).is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), "corrupted history");
}

#[test]
fn large_conversations_report_warning_and_oversized_ones_fail() {
    let dir = TestDirectory::new().unwrap();
    let store = ConversationStore::new(dir.path());
    let mut chat = conversation("c", "large");
    chat.messages[0].blocks = vec![Block::Text {
        text: "x".repeat(2 * 1024 * 1024),
    }];
    assert!(store.save(chat.clone()).unwrap().large);
    chat.id = "oversized".into();
    chat.messages[0].blocks = vec![Block::Text {
        text: "x".repeat(16 * 1024 * 1024),
    }];
    assert!(matches!(store.save(chat), Err(StoreError::TooLarge)));
    assert!(!dir.path().join("ai_chats/c/oversized.chat").exists());
}

#[test]
fn simultaneous_saves_preserve_both_index_entries() {
    let dir = TestDirectory::new().unwrap();
    let base = dir.path().to_path_buf();
    let workers: Vec<_> = ["one", "two"]
        .into_iter()
        .map(|id| {
            let base = base.clone();
            std::thread::spawn(move || {
                ConversationStore::new(&base)
                    .save(conversation("c", id))
                    .unwrap()
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(
        ConversationStore::new(dir.path())
            .list(Some("c"))
            .unwrap()
            .len(),
        2
    );
}

#[cfg(unix)]
#[test]
fn symlinked_connection_directories_are_rejected() {
    let dir = TestDirectory::new().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("ai_chats")).unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join("ai_chats/c")).unwrap();
    assert!(ConversationStore::new(dir.path())
        .save(conversation("c", "one"))
        .is_err());
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[test]
fn late_saves_cannot_recreate_history_after_connection_deletion() {
    let dir = TestDirectory::new().unwrap();
    let store = ConversationStore::new(dir.path());
    let connection = crate::models::SavedConnection {
        id: "c".into(),
        name: "Test".into(),
        params: crate::models::ConnectionParams::default(),
        group_id: None,
        sort_order: None,
        detect_json_in_text_columns: None,
        appearance: None,
        tag_ids: None,
        environment: None,
    };
    let path = dir.path().join("connections.json");
    fs::write(&path, serde_json::to_vec(&vec![connection]).unwrap()).unwrap();
    store
        .save_for_existing_connection(conversation("c", "one"))
        .unwrap();
    fs::write(&path, "[]").unwrap();
    store.delete_connection("c").unwrap();
    assert!(matches!(
        store.save_for_existing_connection(conversation("c", "one")),
        Err(StoreError::InvalidInput)
    ));
    assert!(!dir.path().join("ai_chats/c").exists());
}

#[test]
fn saving_a_loaded_future_block_does_not_discard_unknown_data() {
    let dir = TestDirectory::new().unwrap();
    let store = ConversationStore::new(dir.path());
    let chat = conversation("c", "one");
    store.save(chat.clone()).unwrap();
    let path = dir.path().join("ai_chats/c/one.chat");
    let mut raw = serde_json::to_value(&chat).unwrap();
    raw["messages"][0]["blocks"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"type":"future_block", "blob_id":"future"}));
    with_store_in(dir.path(), false, |cipher| {
        cipher.write_sealed(&path, &serde_json::to_vec(&raw).unwrap(), "chat:c:one")
    })
    .unwrap();
    let original = fs::read(&path).unwrap();
    assert!(matches!(
        store.save(store.load("c", "one").unwrap()),
        Err(StoreError::UnsupportedVersion)
    ));
    assert_eq!(fs::read(path).unwrap(), original);
}
