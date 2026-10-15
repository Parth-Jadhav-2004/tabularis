use super::*;
use crate::ai::message::Message;
use crate::ai_chat::{
    store::ConversationStore,
    types::{ChatMode, Conversation, TitleSource},
};
use crate::secure_store::testing::TestDirectory;

fn chat(id: &str) -> Conversation {
    let mut message = Message::user("image");
    message.blocks.push(Block::Image {
        blob_id: id.into(),
        mime: "image/png".into(),
        width: 1,
        height: 1,
    });
    Conversation {
        schema_version: 1,
        id: "one".into(),
        connection_id: "c".into(),
        title: "image".into(),
        title_source: TitleSource::FirstMessage,
        database: None,
        schema: None,
        mode: ChatMode::Ask,
        target: None,
        provider: None,
        model: None,
        created_at: 0,
        updated_at: 0,
        messages: vec![message],
    }
}

#[test]
fn blobs_round_trip_deduplicate_and_hide_bytes_and_mime() {
    let dir = TestDirectory::new().unwrap();
    let store = BlobStore::new(dir.path());
    let id = store.put(b"private-blob-marker", "text/plain").unwrap();
    assert_eq!(id, store.put(b"private-blob-marker", "text/plain").unwrap());
    let blob = store.get(&id).unwrap();
    assert_eq!(blob.bytes, b"private-blob-marker");
    assert_eq!(blob.mime, "text/plain");
    let disk = fs::read_to_string(store.path(&id).unwrap()).unwrap();
    assert!(!disk.contains("private-blob-marker") && !disk.contains("text/plain"));
    assert_eq!(fs::read_dir(store.dir().unwrap()).unwrap().count(), 1);
}

#[test]
fn invalid_blob_ids_mime_and_size_are_rejected() {
    let dir = TestDirectory::new().unwrap();
    let store = BlobStore::new(dir.path());
    for id in ["../outside", "", "ABC", &"f".repeat(65)] {
        assert!(store.get(id).is_err());
    }
    for mime in ["", "invalid", "image/png\nprivate", "image/\0png"] {
        assert!(store.put(b"bytes", mime).is_err());
    }
    assert_eq!(
        store.put(&vec![0; MAX_BLOB_BYTES + 1], "image/png"),
        Err(StoreError::TooLarge)
    );
}

#[test]
fn copied_blob_and_tampered_blob_fail_closed() {
    let dir = TestDirectory::new().unwrap();
    let store = BlobStore::new(dir.path());
    let id = store.put(b"one", "text/plain").unwrap();
    let other = "0".repeat(64);
    fs::copy(store.path(&id).unwrap(), store.path(&other).unwrap()).unwrap();
    assert!(matches!(store.get(&other), Err(StoreError::WrongContext)));
    fs::write(store.path(&id).unwrap(), "corrupted").unwrap();
    assert!(store.get(&id).is_err());
}

#[test]
fn gc_retains_referenced_blobs_and_new_uploads_then_collects_orphans() {
    let dir = TestDirectory::new().unwrap();
    let blobs = BlobStore::new(dir.path());
    let referenced = blobs.put(b"referenced", "image/png").unwrap();
    let orphan = blobs.put(b"orphan", "text/plain").unwrap();
    ConversationStore::new(dir.path())
        .save(chat(&referenced))
        .unwrap();
    assert_eq!(blobs.gc(Duration::from_secs(3600)).unwrap(), 0);
    assert_eq!(blobs.gc(Duration::ZERO).unwrap(), 1);
    assert!(blobs.get(&referenced).is_ok());
    assert!(blobs.get(&orphan).is_err());
}

#[test]
fn gc_does_not_remove_any_blob_if_a_conversation_is_unreadable() {
    let dir = TestDirectory::new().unwrap();
    let blobs = BlobStore::new(dir.path());
    let orphan = blobs.put(b"orphan", "text/plain").unwrap();
    ConversationStore::new(dir.path())
        .save(chat(&orphan))
        .unwrap();
    fs::write(dir.path().join("ai_chats/c/one.chat"), "corrupted").unwrap();
    assert!(blobs.gc(Duration::ZERO).is_err());
    assert!(blobs.get(&orphan).is_ok());
}
