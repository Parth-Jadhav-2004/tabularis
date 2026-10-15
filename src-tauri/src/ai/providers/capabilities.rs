use crate::ai::{
    catalog,
    provider::{Capabilities, ChatRequest, ProviderError},
};
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

type ProbeCache = HashMap<(u16, String), (Instant, Capabilities)>;
static OLLAMA_CAPABILITIES: OnceLock<Mutex<ProbeCache>> = OnceLock::new();

pub(super) async fn probe_ollama(
    client: &Client,
    port: u16,
    model: &str,
    cancel: &CancellationToken,
) -> Capabilities {
    let key = (port, model.to_string());
    let cache = OLLAMA_CAPABILITIES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some((_, caps)) = cache
        .lock()
        .unwrap()
        .get(&key)
        .filter(|(at, _)| at.elapsed() < Duration::from_secs(86400))
    {
        return *caps;
    }
    let response = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Capabilities::default(),
        response = client.post(format!("http://localhost:{port}/api/show")).timeout(Duration::from_secs(5)).json(&json!({"model": model})).send() => response,
    };
    let value = match response {
        Ok(response) if response.status().is_success() => tokio::select! {
            biased;
            _ = cancel.cancelled() => return Capabilities::default(),
            value = response.json::<Value>() => value.ok(),
        },
        _ => None,
    };
    if let Some(value) = value {
        let caps = from_show(&value);
        let mut cache = cache.lock().unwrap();
        cache.retain(|_, (at, _)| at.elapsed() < Duration::from_secs(86400));
        cache.insert(key, (Instant::now(), caps));
        caps
    } else {
        Capabilities::default()
    }
}

pub(super) fn from_show(value: &Value) -> Capabilities {
    let supports = |name| {
        value["capabilities"]
            .as_array()
            .is_some_and(|caps| caps.iter().any(|cap| cap == name))
    };
    let context_window = value["model_info"]
        .as_object()
        .and_then(|info| {
            info.iter()
                .find(|(key, _)| key.ends_with(".context_length"))
        })
        .and_then(|(_, value)| value.as_u64())
        .and_then(|n| n.try_into().ok());
    Capabilities {
        tools: supports("tools"),
        images: supports("vision"),
        reasoning: supports("thinking"),
        context_window,
    }
}

#[tauri::command]
pub async fn get_ai_model_capabilities(
    app: tauri::AppHandle,
    provider: String,
    model: String,
) -> Result<Capabilities, ProviderError> {
    if provider != "ollama" {
        return Ok(catalog::capabilities(&provider, &model));
    }
    let mut req = ChatRequest {
        provider,
        model,
        system: String::new(),
        messages: Vec::new(),
        tools: Vec::new(),
        max_output_tokens: None,
        reasoning: None,
        resolved_images: HashMap::new(),
    };
    let adapter = super::configured_provider(&app, &mut req, &CancellationToken::new()).await?;
    Ok(adapter.caps)
}
