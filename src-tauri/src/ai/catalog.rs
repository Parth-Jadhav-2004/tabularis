use super::provider::Capabilities;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
#[serde(untagged)]
enum ModelEntry {
    Name(String),
    Detailed {
        id: String,
        #[serde(default)]
        capabilities: Capabilities,
    },
}

fn catalog() -> HashMap<String, Vec<ModelEntry>> {
    serde_yaml::from_str(include_str!("../ai_models.yaml")).unwrap_or_default()
}

pub(super) fn default_models() -> HashMap<String, Vec<String>> {
    catalog()
        .into_iter()
        .map(|(provider, entries)| {
            (
                provider,
                entries
                    .into_iter()
                    .map(|entry| match entry {
                        ModelEntry::Name(id) | ModelEntry::Detailed { id, .. } => id,
                    })
                    .collect(),
            )
        })
        .collect()
}

pub(super) fn capabilities(provider: &str, model: &str) -> Capabilities {
    if let Some(entries) = catalog().get(provider) {
        for entry in entries {
            if let ModelEntry::Detailed { id, capabilities } = entry {
                if id == model {
                    return *capabilities;
                }
            }
        }
    }
    match provider {
        "anthropic" => Capabilities {
            tools: true,
            images: true,
            reasoning: true,
            context_window: None,
        },
        "openai" => Capabilities {
            tools: true,
            images: true,
            reasoning: false,
            context_window: None,
        },
        "openrouter" | "custom-openai" | "minimax" => Capabilities {
            tools: true,
            ..Capabilities::default()
        },
        _ => Capabilities::default(),
    }
}
