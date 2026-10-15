use crate::config::{self, AiKeyStatus, AppConfig};
use serde::Serialize;
use std::collections::HashSet;
use tauri::AppHandle;

#[derive(Debug, Serialize)]
pub struct AvailableProvider {
    provider: &'static str,
    label: &'static str,
    source: &'static str,
    models_cached: bool,
    usable: bool,
    reason: Option<&'static str>,
}

pub(super) fn configured_keyless_url(url: Option<&str>) -> bool {
    url.and_then(|value| reqwest::Url::parse(value.trim()).ok())
        .is_some_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
}

fn available_providers(
    config: &AppConfig,
    cached: &HashSet<String>,
    key_status: impl Fn(&str) -> AiKeyStatus,
) -> Vec<AvailableProvider> {
    let mut providers = Vec::new();
    for (provider, label) in [
        ("openai", "OpenAI"),
        ("anthropic", "Anthropic"),
        ("minimax", "MiniMax"),
        ("openrouter", "OpenRouter"),
        ("ollama", "Ollama"),
        ("custom-openai", "OpenAI Compatible"),
    ] {
        let source = if provider == "ollama" {
            if config.ai_ollama_port.is_some_and(|port| port > 0) {
                Some("keyless")
            } else {
                None
            }
        } else if provider == "custom-openai"
            && !configured_keyless_url(config.ai_custom_openai_url.as_deref())
        {
            None
        } else {
            let status = key_status(provider);
            if status.configured {
                Some(if status.from_env { "env" } else { "keychain" })
            } else if provider == "custom-openai" {
                Some("keyless")
            } else {
                None
            }
        };
        if let Some(source) = source {
            providers.push(AvailableProvider {
                provider,
                label,
                source,
                models_cached: cached.contains(provider),
                usable: true,
                reason: None,
            });
        }
    }
    providers
}

#[tauri::command]
pub async fn get_ai_available_providers(app: AppHandle) -> Result<Vec<AvailableProvider>, String> {
    let (mut providers, ollama_port) = tauri::async_runtime::spawn_blocking(move || {
        let config = config::load_config_internal(&app);
        if config.ai_enabled != Some(true) {
            return (Vec::new(), None);
        }
        let cached = std::fs::read(crate::paths::get_app_config_dir().join("ai_models_cache.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|value| value.get("models")?.as_object().cloned())
            .map(|models| models.into_iter().map(|(provider, _)| provider).collect())
            .unwrap_or_default();
        let providers = available_providers(&config, &cached, |provider| {
            config::get_ai_api_key_status(&app, provider)
        });
        (providers, config.ai_ollama_port)
    })
    .await
    .map_err(|_| "Could not check AI provider availability.".to_string())?;
    if let Some(provider) = providers
        .iter_mut()
        .find(|provider| provider.provider == "ollama")
    {
        provider.reason = ollama_unavailable_reason(ollama_port.unwrap_or(11434)).await;
        provider.usable = provider.reason.is_none();
    }
    Ok(providers)
}

async fn ollama_unavailable_reason(port: u16) -> Option<&'static str> {
    let response = super::legacy::ollama_http_client()
        .get(format!("http://localhost:{port}/api/tags"))
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await;
    let response = match response {
        Ok(response) if response.status().is_success() => response,
        _ => return Some("unreachable"),
    };
    let value = match response.json::<serde_json::Value>().await {
        Ok(value) => value,
        Err(_) => return Some("unreachable"),
    };
    if value["models"].as_array().is_some_and(|models| {
        models
            .iter()
            .any(|model| model["name"].as_str().is_some_and(|name| !name.is_empty()))
    }) {
        None
    } else {
        Some("no_models")
    }
}

#[cfg(test)]
#[path = "availability_tests.rs"]
mod tests;
