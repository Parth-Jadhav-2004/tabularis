//! Read only explicitly selected saved SQL, with scope and bounded-file checks.
//! No saved-query SQL or raw filesystem error is returned in IPC summaries.
use crate::ai_chat::context::{SavedQueryContext, SavedQueryContexts};
use crate::saved_queries::SavedQueryMeta;
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::path::Path;

const META_LIMIT: usize = 2 * 1024 * 1024;
const SQL_LIMIT: usize = 256 * 1024;

fn read_bounded(path: &Path, limit: usize) -> Result<(String, bool), ()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| ())?;
    if !metadata.is_file() {
        return Err(());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path).map_err(|_| ())?;
    if !file.metadata().map_err(|_| ())?.is_file() {
        return Err(());
    }
    let mut bytes = Vec::new();
    file.take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    let truncated = bytes.len() > limit;
    bytes.truncate(limit);
    if let Err(error) = std::str::from_utf8(&bytes) {
        if truncated && error.error_len().is_none() {
            bytes.truncate(error.valid_up_to());
        } else {
            return Err(());
        }
    }
    Ok((String::from_utf8(bytes).map_err(|_| ())?, truncated))
}

pub(super) fn load_from_dir(
    dir: &Path,
    connection_id: &str,
    database: Option<&str>,
    ids: &[String],
) -> SavedQueryContexts {
    if ids.is_empty() {
        return SavedQueryContexts::default();
    }
    let unavailable = || SavedQueryContexts {
        queries: Vec::new(),
        unavailable: ids.to_vec(),
    };
    if !fs::symlink_metadata(dir).is_ok_and(|entry| entry.is_dir()) {
        return unavailable();
    }
    let Ok((json, false)) = read_bounded(&dir.join("meta.json"), META_LIMIT) else {
        return unavailable();
    };
    let Ok(metadata) = serde_json::from_str::<Vec<SavedQueryMeta>>(&json) else {
        return unavailable();
    };
    let mut result = SavedQueryContexts::default();
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            continue;
        }
        if !uuid::Uuid::parse_str(id).is_ok_and(|parsed| parsed.to_string() == *id) {
            result.unavailable.push(id.clone());
            continue;
        }
        let mut matches = metadata.iter().filter(|entry| entry.id == *id);
        let Some(entry) = matches.next() else {
            result.unavailable.push(id.clone());
            continue;
        };
        if matches.next().is_some()
            || entry.connection_id != connection_id
            || entry
                .database
                .as_deref()
                .is_some_and(|saved| Some(saved) != database)
            || entry.filename != format!("{id}.sql")
        {
            result.unavailable.push(id.clone());
            continue;
        }
        match read_bounded(&dir.join(&entry.filename), SQL_LIMIT) {
            Ok((sql, truncated)) if !sql.trim().is_empty() => {
                result.queries.push(SavedQueryContext {
                    id: id.clone(),
                    name: entry.name.chars().take(256).collect(),
                    sql,
                    truncated: truncated || entry.name.chars().count() > 256,
                })
            }
            _ => result.unavailable.push(id.clone()),
        }
    }
    result
}

#[cfg(test)]
mod tests;
