use crate::secure_store::StoreError;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub(super) const MAX_CHAT_BYTES: usize = 16 * 1024 * 1024;
const MAX_SEALED_BYTES: u64 = (MAX_CHAT_BYTES * 2) as u64;

pub(super) fn validate_id(id: &str) -> Result<(), StoreError> {
    if id.is_empty()
        || id.len() > 128
        || id == "index"
        || id == "blobs"
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(StoreError::InvalidInput);
    }
    Ok(())
}

pub(super) fn check_dir(path: &Path) -> Result<(), StoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(StoreError::InvalidInput),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(StoreError::Io),
    }
}

pub(super) fn connection_dir(base: &Path, connection: &str) -> Result<PathBuf, StoreError> {
    validate_id(connection)?;
    let root = base.join("ai_chats");
    check_dir(&root)?;
    let dir = root.join(connection);
    check_dir(&dir)?;
    Ok(dir)
}

pub(super) fn chat_path(base: &Path, connection: &str, id: &str) -> Result<PathBuf, StoreError> {
    validate_id(id)?;
    Ok(connection_dir(base, connection)?.join(format!("{id}.chat")))
}

pub(super) fn private_dir(path: &Path) -> Result<(), StoreError> {
    check_dir(path)?;
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

pub(super) fn read_bounded(path: &Path) -> Result<Vec<u8>, StoreError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(StoreError::InvalidInput);
    }
    if metadata.len() > MAX_SEALED_BYTES {
        return Err(StoreError::TooLarge);
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_SEALED_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_SEALED_BYTES {
        return Err(StoreError::TooLarge);
    }
    Ok(bytes)
}

pub(super) fn connections(base: &Path) -> Result<Vec<String>, StoreError> {
    let root = base.join("ai_chats");
    check_dir(&root)?;
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut connections = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if name == "blobs" {
            continue;
        }
        if validate_id(&name).is_ok() && entry.file_type()?.is_dir() {
            connections.push(name);
        }
    }
    connections.sort();
    Ok(connections)
}
