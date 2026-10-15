//! User-written connection preferences, stored as ordinary configuration.
use serde::{Deserialize, Deserializer};

pub(crate) const MAX_INSTRUCTION_CHARS: usize = 2_000;

pub(crate) fn normalize_instructions(value: Option<String>) -> Result<Option<String>, String> {
    match value {
        Some(text) if text.chars().count() > MAX_INSTRUCTION_CHARS => {
            Err("AI instructions must contain at most 2000 characters.".into())
        }
        Some(text) if text.trim().is_empty() => Ok(None),
        value => Ok(value),
    }
}

pub(crate) fn deserialize_instructions<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    normalize_instructions(Option::<String>::deserialize(deserializer)?)
        .map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests;
