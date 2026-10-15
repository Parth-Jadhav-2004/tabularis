use super::{CipherStore, StoreError};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json::{json, Value};
use zeroize::Zeroizing;

fn cipher(key: u8) -> CipherStore {
    CipherStore::from_key(Zeroizing::new([key; 32]))
}

#[test]
fn round_trip_including_binary_unicode_and_empty_payloads() {
    let store = cipher(7);
    for bytes in [
        b"".as_slice(),
        "private caffè ☕".as_bytes(),
        &[0, 255, 17, 0],
    ] {
        let sealed = store.seal(bytes, "chat:connection:id").unwrap();
        assert_eq!(store.open(&sealed, "chat:connection:id").unwrap(), bytes);
    }
}

#[test]
fn fresh_nonces_hide_repeated_plaintext() {
    let store = cipher(7);
    let first = store.seal(b"private-marker", "activity").unwrap();
    let second = store.seal(b"private-marker", "activity").unwrap();
    assert_ne!(first, second);
    assert!(!String::from_utf8(first).unwrap().contains("private-marker"));
}

#[test]
fn copied_file_rejects_a_different_context() {
    let sealed = cipher(7).seal(b"secret", "chat:c:one").unwrap();
    assert_eq!(
        cipher(7).open(&sealed, "chat:c:two"),
        Err(StoreError::WrongContext)
    );
}

#[test]
fn wrong_key_and_tampered_ciphertext_fail_authentication() {
    let sealed = cipher(7).seal(b"secret", "activity").unwrap();
    assert_eq!(
        cipher(8).open(&sealed, "activity"),
        Err(StoreError::Corrupted)
    );
    let mut value: Value = serde_json::from_slice(&sealed).unwrap();
    let mut ct = BASE64.decode(value["ct"].as_str().unwrap()).unwrap();
    ct[0] ^= 1;
    value["ct"] = json!(BASE64.encode(ct));
    assert_eq!(
        cipher(7).open(&serde_json::to_vec(&value).unwrap(), "activity"),
        Err(StoreError::Corrupted)
    );
}

#[test]
fn replacing_context_digest_does_not_bypass_aad_authentication() {
    let store = cipher(7);
    let mut sealed: Value = serde_json::from_slice(&store.seal(b"secret", "one").unwrap()).unwrap();
    let other: Value = serde_json::from_slice(&store.seal(b"", "two").unwrap()).unwrap();
    sealed["ctx"] = other["ctx"].clone();
    assert_eq!(
        store.open(&serde_json::to_vec(&sealed).unwrap(), "two"),
        Err(StoreError::Corrupted)
    );
}

#[test]
fn malformed_envelopes_never_panic_or_accept_plaintext() {
    let store = cipher(7);
    for bytes in [b"plain secret".as_slice(), b"{}", b"[]"] {
        assert_eq!(store.open(bytes, "a"), Err(StoreError::Corrupted));
    }
    let original: Value = serde_json::from_slice(&store.seal(b"secret", "a").unwrap()).unwrap();
    for (field, value, error) in [
        ("nonce", json!("AA=="), StoreError::Corrupted),
        ("ct", json!("not base64"), StoreError::Corrupted),
        ("v", json!(2), StoreError::UnsupportedVersion),
        ("alg", json!("none"), StoreError::UnsupportedVersion),
    ] {
        let mut envelope = original.clone();
        envelope[field] = value;
        assert_eq!(
            store.open(&serde_json::to_vec(&envelope).unwrap(), "a"),
            Err(error)
        );
    }
}

#[test]
fn sealed_lines_have_no_newlines_and_round_trip() {
    let store = cipher(7);
    let line = store.seal_line(b"private\nquery", "activity").unwrap();
    assert!(!line.contains(['\n', '\r']));
    assert_eq!(
        store.open_line(&line, "activity").unwrap(),
        b"private\nquery"
    );
    assert_eq!(
        store.open_line("invalid", "activity"),
        Err(StoreError::Corrupted)
    );
}

#[test]
fn atomic_file_replace_keeps_only_ciphertext_and_private_permissions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/test.chat");
    let store = cipher(7);
    store
        .write_sealed(&path, b"first-private-marker", "chat:c:id")
        .unwrap();
    store
        .write_sealed(&path, b"second-private-marker", "chat:c:id")
        .unwrap();
    assert_eq!(
        store.read_sealed(&path, "chat:c:id").unwrap(),
        b"second-private-marker"
    );
    assert!(!std::fs::read_to_string(&path)
        .unwrap()
        .contains("private-marker"));
    assert_eq!(
        std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
        1
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn failed_atomic_replace_preserves_the_existing_destination() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("destination");
    std::fs::create_dir(&path).unwrap();
    assert_eq!(
        cipher(7).write_sealed(&path, b"private", "a"),
        Err(StoreError::Io)
    );
    assert!(path.is_dir());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
