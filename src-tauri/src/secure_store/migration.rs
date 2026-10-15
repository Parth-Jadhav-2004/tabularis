use super::{envelope::is_sealed, with_store_in, CipherStore, StoreError};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct MigrationReport {
    pub files: usize,
    pub lines: usize,
}

pub(crate) fn activity_paths(base: &Path) -> Vec<PathBuf> {
    std::iter::once(base.join("ai_activity.jsonl"))
        .chain((1..=5).map(|i| base.join(format!("ai_activity.{i}.jsonl"))))
        .collect()
}

fn json_paths(base: &Path) -> Result<Vec<(PathBuf, String)>, StoreError> {
    let mut paths = vec![(base.join(".mcp_session_state.json"), "mcp_session".into())];
    let approvals = base.join("pending_approvals");
    if approvals.exists() {
        for entry in fs::read_dir(approvals)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            for phase in ["pending", "decision"] {
                if let Some(id) = name.strip_suffix(&format!(".{phase}.json")) {
                    if id.is_empty()
                        || id.len() > 128
                        || !id
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                    {
                        return Err(StoreError::InvalidInput);
                    }
                    paths.push((entry.path(), format!("approval:{id}:{phase}")));
                }
            }
        }
    }
    Ok(paths)
}

fn read_regular(path: &Path) -> Result<Vec<u8>, StoreError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(StoreError::InvalidInput);
    }
    fs::read(path).map_err(Into::into)
}

fn validate_json(bytes: &[u8], aad: &str) -> Result<(), StoreError> {
    if aad == "mcp_session" {
        serde_json::from_slice::<crate::ai_activity::SessionState>(bytes)
            .map_err(|_| StoreError::Corrupted)?;
    } else if let Some(id) = aad
        .strip_prefix("approval:")
        .and_then(|s| s.strip_suffix(":pending"))
    {
        let pending: crate::ai_approval::PendingApproval =
            serde_json::from_slice(bytes).map_err(|_| StoreError::Corrupted)?;
        if pending.id != id {
            return Err(StoreError::WrongContext);
        }
    } else if let Some(id) = aad
        .strip_prefix("approval:")
        .and_then(|s| s.strip_suffix(":decision"))
    {
        let decision: crate::ai_approval::ApprovalDecision =
            serde_json::from_slice(bytes).map_err(|_| StoreError::Corrupted)?;
        if decision.approval_id != id || !matches!(decision.decision.as_str(), "approve" | "deny") {
            return Err(StoreError::WrongContext);
        }
    } else {
        return Err(StoreError::InvalidInput);
    }
    Ok(())
}

/// Verify a private ciphertext staging file before replacing the original.
fn replace_verified(
    path: &Path,
    sealed: &[u8],
    verify: impl FnOnce(&[u8]) -> Result<(), StoreError>,
) -> Result<(), StoreError> {
    let staging = tempfile::NamedTempFile::new_in(path.parent().ok_or(StoreError::InvalidInput)?)?
        .into_temp_path();
    super::io::write_private_atomic(&staging, sealed)?;
    verify(&fs::read(&staging)?)?;
    staging.persist(path).map_err(|_| StoreError::Io)?;
    Ok(())
}

fn migrate_lines(path: &Path, store: &CipherStore) -> Result<usize, StoreError> {
    let original = read_regular(path)?;
    let text = std::str::from_utf8(&original).map_err(|_| StoreError::Corrupted)?;
    let mut migrated = 0;
    let mut output = String::new();
    let mut expected = Vec::new();
    for (index, line) in text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
    {
        if BASE64
            .decode(line.trim())
            .is_ok_and(|bytes| is_sealed(&bytes))
        {
            output.push_str(line.trim());
        } else {
            // Preserve malformed legacy entries encrypted; the reader counts/skips them.
            output.push_str(&store.seal_line(line.as_bytes(), "activity")?);
            expected.push((index, line.as_bytes()));
            migrated += 1;
        }
        output.push('\n');
    }
    if migrated > 0 {
        replace_verified(path, output.as_bytes(), |bytes| {
            let text = std::str::from_utf8(bytes).map_err(|_| StoreError::Corrupted)?;
            if text != output {
                return Err(StoreError::Corrupted);
            }
            let lines: Vec<_> = text.lines().collect();
            for (index, original) in expected {
                if store.open_line(lines.get(index).ok_or(StoreError::Corrupted)?, "activity")?
                    != original
                {
                    return Err(StoreError::Corrupted);
                }
            }
            Ok(())
        })?;
    }
    Ok(migrated)
}

pub fn migrate_in(base: &Path) -> Result<MigrationReport, StoreError> {
    let logs: Vec<_> = activity_paths(base)
        .into_iter()
        .filter(|p| p.exists())
        .collect();
    let json: Vec<_> = json_paths(base)?
        .into_iter()
        .filter(|(p, _)| p.exists())
        .collect();
    if logs.is_empty() && json.is_empty() {
        return Ok(MigrationReport::default());
    }
    with_store_in(base, true, |store| {
        let mut report = MigrationReport::default();
        for path in logs {
            if !path.exists() {
                continue;
            }
            let lines = migrate_lines(&path, store)?;
            if lines > 0 {
                report.files += 1;
                report.lines += lines;
            }
        }
        for (path, aad) in json {
            if !path.exists() {
                continue;
            }
            let bytes = read_regular(&path)?;
            if is_sealed(&bytes) {
                validate_json(&store.open(&bytes, &aad)?, &aad)?;
                continue;
            }
            validate_json(&bytes, &aad)?;
            let sealed = store.seal(&bytes, &aad)?;
            replace_verified(&path, &sealed, |sealed| {
                if store.open(sealed, &aad)? != bytes {
                    return Err(StoreError::Corrupted);
                }
                Ok(())
            })?;
            report.files += 1;
        }
        Ok(report)
    })
}

#[cfg(test)]
#[path = "tests/migration.rs"]
mod tests;
