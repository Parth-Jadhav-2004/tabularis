use super::{exclusive_in, key::delete_key_locked, migration::activity_paths, StoreError};
use std::fs;
use std::path::Path;

/// Destructive recovery is explicit and does not need the old encryption key.
pub fn reset_in(base: &Path, confirmed: bool) -> Result<(), StoreError> {
    if !confirmed {
        return Err(StoreError::InvalidInput);
    }
    exclusive_in(base, || {
        let chats = base.join("ai_chats");
        let approvals = base.join("pending_approvals");
        super::check_directory(&chats)?;
        super::check_directory(&approvals)?;
        let mut files: Vec<_> = activity_paths(base)
            .into_iter()
            .chain(std::iter::once(base.join(".mcp_session_state.json")))
            .collect();
        if approvals.exists() {
            for entry in fs::read_dir(&approvals)? {
                files.push(entry?.path());
            }
        }
        for entry in fs::read_dir(base)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(".ai-store-") || name.starts_with(".ai-migration-") {
                files.push(entry.path());
            }
        }
        // Validate every managed path before starting destructive recovery.
        for path in &files {
            super::check_file(path)?;
        }
        if chats.exists() {
            fs::remove_dir_all(chats)?;
        }
        for path in files {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(StoreError::Io),
            }
        }
        // Keep the approval directory inode: notify is attached to it.
        delete_key_locked(base)
    })
}

#[cfg(test)]
#[path = "tests/reset.rs"]
mod tests;
