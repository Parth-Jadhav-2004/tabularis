use super::*;
use crate::secure_store::testing::TestDirectory;

#[test]
fn missing_key_on_read_does_not_generate_a_replacement() {
    let dir = TestDirectory::new().unwrap();
    let result = with_store_in(dir.path(), false, |_| Ok(()));
    assert_eq!(result, Err(StoreError::KeyUnavailable));
    assert!(dir.backend.key.lock().unwrap().is_none());
}

#[test]
fn first_write_creates_verifies_and_caches_a_32_byte_key() {
    let dir = TestDirectory::new().unwrap();
    let sealed = with_store_in(dir.path(), true, |store| store.seal(b"secret", "a")).unwrap();
    let encoded = dir.backend.key.lock().unwrap().clone().unwrap();
    assert_eq!(BASE64.decode(encoded).unwrap().len(), 32);
    dir.backend
        .denied
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        with_store_in(dir.path(), false, |store| store.open(&sealed, "a")).unwrap(),
        b"secret"
    );
}

#[test]
fn denied_keychain_never_creates_plaintext_or_caches_failure() {
    let dir = TestDirectory::new().unwrap();
    dir.backend
        .denied
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        with_store_in(dir.path(), true, |store| store.write_sealed(
            &dir.path().join("item"),
            b"private",
            "a"
        )),
        Err(StoreError::KeyUnavailable)
    );
    assert!(!dir.path().join("item").exists());
    dir.backend
        .denied
        .store(false, std::sync::atomic::Ordering::SeqCst);
    assert!(with_store_in(dir.path(), true, |store| store.seal(b"private", "a")).is_ok());
}

#[test]
fn lost_key_is_not_recreated_after_cache_eviction() {
    let dir = TestDirectory::new().unwrap();
    with_store_in(dir.path(), true, |store| store.seal(b"secret", "a")).unwrap();
    invalidate(dir.path());
    *dir.backend.key.lock().unwrap() = None;
    assert_eq!(
        with_store_in(dir.path(), true, |_| Ok(())),
        Err(StoreError::KeyUnavailable)
    );
    assert!(dir.backend.key.lock().unwrap().is_none());
}

#[test]
fn orphaned_encrypted_files_prevent_silent_key_replacement() {
    let dir = TestDirectory::new().unwrap();
    fs::create_dir_all(dir.path().join("ai_chats/c")).unwrap();
    fs::write(
        dir.path().join("ai_chats/c/id.chat"),
        "unreadable encrypted history",
    )
    .unwrap();
    assert_eq!(
        with_store_in(dir.path(), true, |_| Ok(())),
        Err(StoreError::KeyUnavailable)
    );
}

#[test]
fn invalid_keychain_contents_are_not_replaced() {
    let dir = TestDirectory::new().unwrap();
    for key in ["not base64".to_string(), BASE64.encode([1; 16])] {
        *dir.backend.key.lock().unwrap() = Some(key.clone());
        assert_eq!(
            with_store_in(dir.path(), true, |_| Ok(())),
            Err(StoreError::KeyUnavailable)
        );
        assert_eq!(
            dir.backend.key.lock().unwrap().as_deref(),
            Some(key.as_str())
        );
    }
}

#[test]
fn reset_marker_invalidates_stale_process_cache() {
    let dir = TestDirectory::new().unwrap();
    with_store_in(dir.path(), true, |_| Ok(())).unwrap();
    let old_marker = fs::read_to_string(dir.path().join(MARKER)).unwrap();
    // Model another process resetting the key while this process retains its cache.
    *dir.backend.key.lock().unwrap() = None;
    fs::remove_file(dir.path().join(MARKER)).unwrap();
    with_store_in(dir.path(), true, |_| Ok(())).unwrap();
    assert_ne!(
        fs::read_to_string(dir.path().join(MARKER)).unwrap(),
        old_marker
    );
}

#[test]
fn concurrent_first_use_shares_one_key() {
    let dir = TestDirectory::new().unwrap();
    let path = dir.path().to_path_buf();
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let path = path.clone();
            std::thread::spawn(move || {
                with_store_in(&path, true, |store| store.seal(b"secret", "a")).unwrap()
            })
        })
        .collect();
    for worker in workers {
        let bytes = worker.join().unwrap();
        assert_eq!(
            with_store_in(dir.path(), false, |store| store.open(&bytes, "a")).unwrap(),
            b"secret"
        );
    }
}

#[test]
fn deleting_key_clears_cache_and_marker() {
    let dir = TestDirectory::new().unwrap();
    with_store_in(dir.path(), true, |_| Ok(())).unwrap();
    exclusive_in(dir.path(), || delete_key_locked(dir.path())).unwrap();
    assert!(dir.backend.key.lock().unwrap().is_none());
    assert!(!dir.path().join(MARKER).exists());
    assert_eq!(
        with_store_in(dir.path(), false, |_| Ok(())),
        Err(StoreError::KeyUnavailable)
    );
}
