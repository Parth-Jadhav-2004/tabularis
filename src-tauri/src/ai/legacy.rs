use crate::config;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

// --- Data Structures ---

#[derive(Serialize, Deserialize, Debug)]
pub struct AiGenerateRequest {
    pub provider: String,
    pub model: String,
    pub prompt: String,
    pub schema: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AiExplainRequest {
    pub provider: String,
    pub model: String,
    pub query: String,
    pub language: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AiCellNameRequest {
    pub provider: String,
    pub model: String,
    pub query: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AiTabRenameRequest {
    pub provider: String,
    pub model: String,
    pub query: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AiSuggestTableNameRequest {
    pub provider: String,
    pub model: String,
    pub headers: Vec<String>,
    pub sample_rows: Vec<Vec<String>>,
}

#[derive(Deserialize, Debug)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Deserialize, Debug)]
struct OllamaModel {
    name: String,
}

#[derive(Deserialize, Debug)]
struct OpenAiModelList {
    data: Vec<OpenAiModel>,
}

#[derive(Deserialize, Debug)]
struct OpenAiModel {
    id: String,
}

#[derive(Deserialize, Debug)]
struct AnthropicModelList {
    data: Vec<AnthropicModel>,
}

#[derive(Deserialize, Debug)]
struct AnthropicModel {
    id: String,
}

#[derive(Deserialize, Debug)]
struct OpenRouterModelList {
    data: Vec<OpenRouterModel>,
}

#[derive(Deserialize, Debug)]
struct OpenRouterModel {
    id: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct AiModelsCache {
    last_updated: u64,
    models: HashMap<String, Vec<String>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MiniMaxEndpoint {
    region: &'static str,
    pub(super) openai_base_url: &'static str,
}

pub(super) const MINIMAX_ENDPOINTS: [MiniMaxEndpoint; 2] = [
    MiniMaxEndpoint {
        region: "global",
        openai_base_url: "https://api.minimax.io/v1",
    },
    MiniMaxEndpoint {
        region: "cn_zh",
        openai_base_url: "https://api.minimaxi.com/v1",
    },
];

pub(super) static MINIMAX_PREFERRED_ENDPOINT: AtomicUsize = AtomicUsize::new(0);

pub(super) fn minimax_endpoint_order(preferred: usize) -> [usize; 2] {
    if preferred == 1 {
        [1, 0]
    } else {
        [0, 1]
    }
}

// --- Helper Functions ---

fn load_default_models() -> HashMap<String, Vec<String>> {
    super::catalog::default_models()
}

fn get_cache_path(_app: &AppHandle) -> Option<std::path::PathBuf> {
    Some(crate::paths::get_app_config_dir().join("ai_models_cache.json"))
}

fn load_cache(app: &AppHandle) -> Option<AiModelsCache> {
    let path = get_cache_path(app)?;
    if path.exists() {
        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    } else {
        None
    }
}

fn save_cache(app: &AppHandle, models: &HashMap<String, Vec<String>>) {
    if let Some(path) = get_cache_path(app) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let cache = AiModelsCache {
            last_updated: timestamp,
            models: models.clone(),
        };

        if let Ok(content) = serde_json::to_string(&cache) {
            let _ = fs::write(path, content);
        }
    }
}

// --- Fetchers ---

pub(super) fn ai_client_for(provider: &str) -> Client {
    crate::proxy::ai_http_client(provider).unwrap_or_else(|e| {
        log::warn!("Failed to build proxied AI client for {provider}: {e}");
        Client::new()
    })
}

/// Local Ollama is always on loopback — never send it through a remote proxy.
pub(super) fn ollama_http_client() -> Client {
    crate::proxy::direct_http_client().unwrap_or_else(|e| {
        log::warn!("Failed to build direct Ollama HTTP client: {e}");
        Client::new()
    })
}

async fn fetch_ollama_models(port: u16) -> Vec<String> {
    let client = ollama_http_client();
    let url = format!("http://localhost:{}/api/tags", port);
    match client.get(&url).send().await {
        Ok(res) => {
            if res.status().is_success() {
                if let Ok(json) = res.json::<OllamaTagsResponse>().await {
                    return json.models.into_iter().map(|m| m.name).collect();
                }
            }
            Vec::new()
        }
        Err(_) => Vec::new(),
    }
}

async fn fetch_openai_models(api_key: &str) -> Vec<String> {
    if api_key.is_empty() {
        return Vec::new();
    }
    let client = ai_client_for("openai");
    match client
        .get("https://api.openai.com/v1/models")
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
    {
        Ok(res) => {
            if res.status().is_success() {
                if let Ok(json) = res.json::<OpenAiModelList>().await {
                    return json
                        .data
                        .into_iter()
                        .map(|m| m.id)
                        .filter(|id| id.starts_with("gpt") || id.starts_with("o1"))
                        .collect();
                }
            }
            Vec::new()
        }
        Err(_) => Vec::new(),
    }
}

async fn fetch_anthropic_models(api_key: &str) -> Vec<String> {
    if api_key.is_empty() {
        return Vec::new();
    }
    let client = ai_client_for("anthropic");
    match client
        .get("https://api.anthropic.com/v1/models")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
    {
        Ok(res) => {
            if res.status().is_success() {
                if let Ok(json) = res.json::<AnthropicModelList>().await {
                    return json.data.into_iter().map(|m| m.id).collect();
                }
            }
            Vec::new()
        }
        Err(_) => Vec::new(),
    }
}

async fn fetch_minimax_models(api_key: &str) -> Vec<String> {
    if api_key.is_empty() {
        return Vec::new();
    }
    let client = ai_client_for("minimax");
    let preferred = MINIMAX_PREFERRED_ENDPOINT.load(Ordering::Relaxed);
    for index in minimax_endpoint_order(preferred) {
        let endpoint = MINIMAX_ENDPOINTS[index];
        if let Ok(res) = client
            .get(format!("{}/models", endpoint.openai_base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
        {
            if res.status().is_success() {
                if let Ok(json) = res.json::<OpenAiModelList>().await {
                    MINIMAX_PREFERRED_ENDPOINT.store(index, Ordering::Relaxed);
                    return json.data.into_iter().map(|m| m.id).collect();
                }
            }
        }
    }
    Vec::new()
}

async fn fetch_openrouter_models() -> Vec<String> {
    let client = ai_client_for("openrouter");
    match client
        .get("https://openrouter.ai/api/v1/models")
        .send()
        .await
    {
        Ok(res) => {
            if res.status().is_success() {
                if let Ok(json) = res.json::<OpenRouterModelList>().await {
                    return json.data.into_iter().map(|m| m.id).collect();
                }
            }
            Vec::new()
        }
        Err(_) => Vec::new(),
    }
}

/// Build an API endpoint URL from a user-provided base_url.
///
/// Handles various formats the user might input:
/// - Full path (e.g., `.../v1/chat/completions`) → used as-is
/// - Base with version (e.g., `.../v1` or `.../v4`) → appends only the endpoint
/// - Base with trailing slash → trimmed, then appends endpoint
/// - Plain base URL → appends endpoint directly
///
/// This avoids hardcoding `/v1` so that providers using different
/// version paths (e.g., Zhipu GLM's `/v4`) work correctly.
pub(super) fn build_api_url(base_url: &str, endpoint: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');

    // User already provided the full path
    if trimmed.ends_with(endpoint) {
        return trimmed.to_string();
    }

    format!("{trimmed}{endpoint}")
}

async fn fetch_custom_openai_models(base_url: &str, api_key: &str) -> Vec<String> {
    if base_url.is_empty() || api_key.is_empty() {
        return Vec::new();
    }

    let client = ai_client_for("custom-openai");

    let url = build_api_url(base_url, "/models");

    match client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
    {
        Ok(res) => {
            if res.status().is_success() {
                if let Ok(json) = res.json::<OpenAiModelList>().await {
                    return json.data.into_iter().map(|m| m.id).collect();
                }
            }
            Vec::new()
        }
        Err(_) => Vec::new(),
    }
}

// --- Commands ---

#[tauri::command]
pub fn clear_ai_models_cache(app: AppHandle) -> Result<(), String> {
    if let Some(path) = get_cache_path(&app) {
        if path.exists() {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_ai_models(
    app: AppHandle,
    force_refresh: bool,
) -> Result<HashMap<String, Vec<String>>, String> {
    // Load config to get Ollama port
    let app_config = config::load_config_internal(&app);
    let ollama_port = app_config.ai_ollama_port.unwrap_or(11434);

    // 1. Check Cache (if not forced)
    if !force_refresh {
        if let Some(cache) = load_cache(&app) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            // 24 hours = 86400 seconds
            if now - cache.last_updated < 86400 {
                let mut cached_models = cache.models;

                // Always refresh Ollama as it is local and fast
                let ollama_models = fetch_ollama_models(ollama_port).await;
                // Replace or insert ollama entry
                if !ollama_models.is_empty() {
                    cached_models.insert("ollama".to_string(), ollama_models);
                } else {
                    cached_models.insert("ollama".to_string(), vec![]);
                }

                // Always refresh custom-openai as it depends on user configuration
                if let (Some(base_url), Ok(api_key)) = (
                    app_config.ai_custom_openai_url.clone(),
                    config::get_ai_api_key(&app, "custom-openai"),
                ) {
                    if !base_url.is_empty() && !api_key.is_empty() {
                        let custom_models = fetch_custom_openai_models(&base_url, &api_key).await;
                        if !custom_models.is_empty() {
                            cached_models.insert("custom-openai".to_string(), custom_models);
                        } else {
                            cached_models.insert("custom-openai".to_string(), vec![]);
                        }
                    }
                }

                // Always refresh Anthropic if API key is present
                if let Ok(key) = config::get_ai_api_key(&app, "anthropic") {
                    let anthropic_models = fetch_anthropic_models(&key).await;
                    if !anthropic_models.is_empty() {
                        cached_models.insert("anthropic".to_string(), anthropic_models);
                    }
                }

                // Always refresh MiniMax if API key is present
                if let Ok(key) = config::get_ai_api_key(&app, "minimax") {
                    let minimax_models = fetch_minimax_models(&key).await;
                    if !minimax_models.is_empty() {
                        cached_models.insert("minimax".to_string(), minimax_models);
                    }
                }

                return Ok(cached_models);
            }
        }
    }

    let mut models = load_default_models();

    // 1. Ollama (Dynamic)
    let ollama_models = fetch_ollama_models(ollama_port).await;
    if !ollama_models.is_empty() {
        models.insert("ollama".to_string(), ollama_models);
    }

    // 2. OpenAI (Dynamic if key exists)
    if let Ok(key) = config::get_ai_api_key(&app, "openai") {
        let remote_models = fetch_openai_models(&key).await;
        if !remote_models.is_empty() {
            if let Some(static_list) = models.get_mut("openai") {
                let mut set: HashSet<String> = static_list.iter().cloned().collect();
                set.extend(remote_models);
                *static_list = set.into_iter().collect();
                static_list.sort();
            }
        }
    }

    // 3. Anthropic (Dynamic if key exists)
    if let Ok(key) = config::get_ai_api_key(&app, "anthropic") {
        let remote_models = fetch_anthropic_models(&key).await;
        if !remote_models.is_empty() {
            if let Some(static_list) = models.get_mut("anthropic") {
                let mut set: HashSet<String> = static_list.iter().cloned().collect();
                set.extend(remote_models);
                *static_list = set.into_iter().collect();
                static_list.sort();
            }
        }
    }

    // 4. MiniMax (Dynamic if key exists)
    if let Ok(key) = config::get_ai_api_key(&app, "minimax") {
        let remote_models = fetch_minimax_models(&key).await;
        if !remote_models.is_empty() {
            if let Some(static_list) = models.get_mut("minimax") {
                let mut set: HashSet<String> = static_list.iter().cloned().collect();
                set.extend(remote_models);
                *static_list = set.into_iter().collect();
                static_list.sort();
            }
        }
    }

    // 5. OpenRouter (Dynamic public)
    let openrouter_models = fetch_openrouter_models().await;
    if !openrouter_models.is_empty() {
        if let Some(static_list) = models.get_mut("openrouter") {
            let favorites: HashSet<String> = static_list.iter().cloned().collect();
            let mut new_list = static_list.clone();

            for m in openrouter_models {
                if !favorites.contains(&m) {
                    new_list.push(m);
                }
            }
            *static_list = new_list;
        }
    }

    // 6. Custom OpenAI (Dynamic if configured)
    if let (Some(base_url), Ok(api_key)) = (
        app_config.ai_custom_openai_url,
        config::get_ai_api_key(&app, "custom-openai"),
    ) {
        if !base_url.is_empty() && !api_key.is_empty() {
            let custom_models = fetch_custom_openai_models(&base_url, &api_key).await;
            if !custom_models.is_empty() {
                models.insert("custom-openai".to_string(), custom_models);
            } else {
                models.insert("custom-openai".to_string(), vec![]);
            }
        } else {
            models.insert("custom-openai".to_string(), vec![]);
        }
    } else {
        models.insert("custom-openai".to_string(), vec![]);
    }

    // Save to Cache
    save_cache(&app, &models);

    Ok(models)
}

#[tauri::command]
pub async fn generate_ai_query(app: AppHandle, req: AiGenerateRequest) -> Result<String, String> {
    generate_query(app, req).await
}

#[tauri::command]
pub async fn explain_ai_query(app: AppHandle, req: AiExplainRequest) -> Result<String, String> {
    explain_query(app, req).await
}

#[tauri::command]
pub async fn analyze_ai_explain_plan(
    app: AppHandle,
    req: AiExplainRequest,
) -> Result<String, String> {
    analyze_explain_plan(app, req).await
}

#[tauri::command]
pub async fn generate_cell_name(app: AppHandle, req: AiCellNameRequest) -> Result<String, String> {
    generate_cellname(app, req).await
}

// --- Shared helpers ---

pub(super) async fn resolve_model(
    provider: &str,
    model: &str,
    app_config: &config::AppConfig,
    ollama_port: u16,
) -> Result<String, String> {
    if !model.is_empty() {
        return Ok(model.to_string());
    }
    match provider {
        "ollama" => {
            let models = fetch_ollama_models(ollama_port).await;
            models
                .first()
                .cloned()
                .ok_or_else(|| "No Ollama models found. Is Ollama running?".to_string())
        }
        "custom-openai" => app_config
            .ai_custom_openai_model
            .as_ref()
            .filter(|m| !m.is_empty())
            .cloned()
            .ok_or_else(|| "No model specified for custom OpenAI provider.".to_string()),
        _ => {
            let models = load_default_models();
            models
                .get(provider)
                .and_then(|m| m.first())
                .cloned()
                .ok_or_else(|| format!("No models found for provider {}", provider))
        }
    }
}

async fn dispatch_provider(
    app: &AppHandle,
    app_config: &config::AppConfig,
    gen_req: &AiGenerateRequest,
    system_prompt: &str,
    ollama_port: u16,
) -> Result<String, String> {
    use super::provider::ChatProvider;
    let mut req = super::provider::ChatRequest {
        provider: gen_req.provider.clone(),
        model: gen_req.model.clone(),
        system: system_prompt.to_string(),
        messages: vec![super::message::Message::user(&gen_req.prompt)],
        tools: Vec::new(),
        max_output_tokens: (gen_req.provider == "anthropic").then_some(1024),
        reasoning: None,
        resolved_images: HashMap::new(),
    };
    let _ = (app_config, ollama_port);
    let cancel = tokio_util::sync::CancellationToken::new();
    let provider = super::providers::configured_provider(app, &mut req, &cancel)
        .await
        .map_err(|error| error.to_string())?;
    let stream = provider
        .stream(req, cancel)
        .await
        .map_err(|error| error.to_string())?;
    collect_legacy_response(stream).await
}

// --- Logic Implementation ---

pub async fn generate_query(app: AppHandle, mut req: AiGenerateRequest) -> Result<String, String> {
    log::info!("Generating AI query using provider: {}", req.provider);

    let app_config = config::load_config_internal(&app);
    let ollama_port = app_config.ai_ollama_port.unwrap_or(11434);
    req.model = resolve_model(&req.provider, &req.model, &app_config, ollama_port).await?;

    let raw_prompt = config::get_system_prompt(app.clone());
    let system_prompt = raw_prompt.replace("{{SCHEMA}}", &req.schema);

    let result = dispatch_provider(&app, &app_config, &req, &system_prompt, ollama_port).await;

    match &result {
        Ok(_) => log::info!("AI query generated successfully using {}", req.model),
        Err(e) => log::error!("AI query generation failed: {}", e),
    }

    result
}

pub async fn explain_query(app: AppHandle, mut req: AiExplainRequest) -> Result<String, String> {
    log::info!("Explaining query using AI provider: {}", req.provider);

    let app_config = config::load_config_internal(&app);
    let ollama_port = app_config.ai_ollama_port.unwrap_or(11434);
    req.model = resolve_model(&req.provider, &req.model, &app_config, ollama_port).await?;

    let raw_prompt = config::get_explain_prompt(app.clone());
    let system_prompt = raw_prompt.replace("{{LANGUAGE}}", &req.language);

    let gen_req = AiGenerateRequest {
        provider: req.provider.clone(),
        model: req.model.clone(),
        prompt: format!("Query:\n{}\n", req.query),
        schema: String::new(),
    };

    let result = dispatch_provider(&app, &app_config, &gen_req, &system_prompt, ollama_port).await;

    match &result {
        Ok(_) => log::info!(
            "Query explanation generated successfully using {}",
            req.model
        ),
        Err(e) => log::error!("Query explanation generation failed: {}", e),
    }

    result
}

pub async fn analyze_explain_plan(
    app: AppHandle,
    mut req: AiExplainRequest,
) -> Result<String, String> {
    log::info!("Analyzing explain plan using AI provider: {}", req.provider);

    let app_config = config::load_config_internal(&app);
    let ollama_port = app_config.ai_ollama_port.unwrap_or(11434);
    req.model = resolve_model(&req.provider, &req.model, &app_config, ollama_port).await?;

    let raw_prompt = config::get_explainplan_prompt(app.clone());
    let system_prompt = raw_prompt.replace("{{LANGUAGE}}", &req.language);

    let gen_req = AiGenerateRequest {
        provider: req.provider.clone(),
        model: req.model.clone(),
        prompt: req.query.clone(),
        schema: String::new(),
    };

    let result = dispatch_provider(&app, &app_config, &gen_req, &system_prompt, ollama_port).await;

    match &result {
        Ok(_) => log::info!(
            "Explain plan analysis generated successfully using {}",
            req.model
        ),
        Err(e) => log::error!("Explain plan analysis failed: {}", e),
    }

    result
}

async fn generate_with_simple_prompt(
    app: AppHandle,
    provider: String,
    model: String,
    query: String,
    get_prompt: fn(AppHandle) -> String,
    label: &str,
) -> Result<String, String> {
    log::info!("Generating {} using AI provider: {}", label, provider);

    let app_config = config::load_config_internal(&app);
    let ollama_port = app_config.ai_ollama_port.unwrap_or(11434);
    let resolved_model = resolve_model(&provider, &model, &app_config, ollama_port).await?;

    let system_prompt = get_prompt(app.clone());

    let gen_req = AiGenerateRequest {
        provider: provider.clone(),
        model: resolved_model.clone(),
        prompt: query,
        schema: String::new(),
    };

    let result = dispatch_provider(&app, &app_config, &gen_req, &system_prompt, ollama_port).await;

    match &result {
        Ok(v) => log::info!("{} generated: {}", label, v),
        Err(e) => log::error!("{} generation failed: {}", label, e),
    }

    result
}

pub async fn generate_cellname(app: AppHandle, req: AiCellNameRequest) -> Result<String, String> {
    generate_with_simple_prompt(
        app,
        req.provider,
        req.model,
        req.query,
        config::get_cellname_prompt,
        "Cell name",
    )
    .await
}

#[tauri::command]
pub async fn suggest_table_name(
    app: AppHandle,
    req: AiSuggestTableNameRequest,
) -> Result<String, String> {
    log::info!("Suggesting table name using AI provider: {}", req.provider);

    let app_config = config::load_config_internal(&app);
    let ollama_port = app_config.ai_ollama_port.unwrap_or(11434);
    let resolved_model = resolve_model(&req.provider, &req.model, &app_config, ollama_port).await?;

    let sample_preview = req
        .sample_rows
        .iter()
        .take(3)
        .map(|r| r.join(", "))
        .collect::<Vec<_>>()
        .join(" | ");

    let prompt = format!(
        "Given these column names: [{}] and sample data: [{}], suggest a concise snake_case table name that describes this data. Reply with only the table name, nothing else.",
        req.headers.join(", "),
        sample_preview
    );

    let gen_req = AiGenerateRequest {
        provider: req.provider.clone(),
        model: resolved_model,
        prompt,
        schema: String::new(),
    };

    let system_prompt = "You are a database naming expert. Reply with only a snake_case table name, no explanation.";
    let result = dispatch_provider(&app, &app_config, &gen_req, system_prompt, ollama_port).await;

    match &result {
        Ok(name) => {
            let cleaned = name
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_lowercase()
                .replace(|c: char| !c.is_alphanumeric() && c != '_', "_")
                .trim_matches('_')
                .to_string();
            log::info!("Table name suggested: {}", cleaned);
            Ok(cleaned)
        }
        Err(e) => {
            log::error!("Table name suggestion failed: {}", e);
            Err(e.clone())
        }
    }
}

#[tauri::command]
pub async fn generate_tab_rename(
    app: AppHandle,
    req: AiTabRenameRequest,
) -> Result<String, String> {
    generate_with_simple_prompt(
        app,
        req.provider,
        req.model,
        req.query,
        config::get_tabrename_prompt,
        "Tab name",
    )
    .await
}

/// Collect text only; reasoning and protocol metadata never reach the legacy modals.
pub(super) async fn collect_legacy_response(
    mut stream: futures::stream::BoxStream<'static, super::provider::ChatEvent>,
) -> Result<String, String> {
    use super::provider::{ChatEvent, FinishReason};
    use futures::StreamExt;
    let mut text = String::new();
    while let Some(event) = stream.next().await {
        match event {
            ChatEvent::TextDelta { text: delta } => text.push_str(&delta),
            ChatEvent::Error { error } => return Err(error.to_string()),
            ChatEvent::Done {
                finish: FinishReason::Stop | FinishReason::Length,
            } => return Ok(clean_response(&text)),
            ChatEvent::Done { .. } => {
                return Err("The AI provider could not complete the request.".to_string())
            }
            _ => {}
        }
    }
    Err("The AI provider returned an incomplete response.".to_string())
}

fn clean_response(text: &str) -> String {
    let text = text.trim();
    if text.starts_with("```") {
        let mut lines = text.lines();
        lines.next(); // Skip first line
        let mut result = Vec::new();
        for line in lines {
            if line.trim() == "```" {
                break;
            }
            result.push(line);
        }
        return result.join("\n").trim().to_string();
    }
    text.to_string()
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
