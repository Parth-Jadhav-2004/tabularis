use super::{
    anthropic, capabilities,
    decoder::{Decoder, Protocol},
    ollama, openai_compat,
};
use crate::ai::provider::{
    Capabilities, ChatEvent, ChatProvider, ChatRequest, ErrorKind, FinishReason, ProviderError,
};
use crate::ai::sse::{FrameParser, Framing};
use crate::ai::{catalog, legacy};
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::{stream, StreamExt};
use reqwest::{Client, Response};
use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;

pub struct HttpProvider {
    pub(super) client: Client,
    pub(super) api_key: String,
    pub(super) endpoints: Vec<String>,
    pub(super) provider: String,
    pub(super) caps: Capabilities,
}

pub async fn configured_provider(
    app: &AppHandle,
    req: &mut ChatRequest,
    cancel: &CancellationToken,
) -> Result<HttpProvider, ProviderError> {
    let config = crate::config::load_config_internal(app);
    let port = config.ai_ollama_port.unwrap_or(11434);
    req.model = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(ProviderError::new(ErrorKind::Other, "The AI request was cancelled.")),
        model = legacy::resolve_model(&req.provider, &req.model, &config, port) => model.map_err(|message| ProviderError::new(ErrorKind::Other, message))?,
    };
    let key = if req.provider == "ollama" {
        String::new()
    } else {
        crate::config::get_ai_api_key(app, &req.provider).map_err(|_| {
            ProviderError::new(
                ErrorKind::Auth,
                "No API key is available for this AI provider.",
            )
        })?
    };
    let endpoints = match req.provider.as_str() {
        "openai" => vec!["https://api.openai.com/v1/chat/completions".to_string()],
        "anthropic" => vec!["https://api.anthropic.com/v1/messages".to_string()],
        "openrouter" => vec!["https://openrouter.ai/api/v1/chat/completions".to_string()],
        "ollama" => vec![format!("http://localhost:{port}/api/chat")],
        "custom-openai" => {
            let url = config
                .ai_custom_openai_url
                .as_deref()
                .filter(|u| !u.is_empty())
                .ok_or_else(|| {
                    ProviderError::new(ErrorKind::Other, "Custom OpenAI URL not configured.")
                })?;
            vec![legacy::build_api_url(url, "/chat/completions")]
        }
        "minimax" => legacy::minimax_endpoint_order(
            legacy::MINIMAX_PREFERRED_ENDPOINT.load(Ordering::Relaxed),
        )
        .iter()
        .map(|&index| {
            format!(
                "{}/chat/completions",
                legacy::MINIMAX_ENDPOINTS[index].openai_base_url
            )
        })
        .collect(),
        _ => {
            return Err(ProviderError::new(
                ErrorKind::Other,
                "Unsupported AI provider.",
            ))
        }
    };
    let client = if req.provider == "ollama" {
        legacy::ollama_http_client()
    } else {
        legacy::ai_client_for(&req.provider)
    };
    let caps = if req.provider == "ollama" {
        capabilities::probe_ollama(&client, port, &req.model, cancel).await
    } else {
        catalog::capabilities(&req.provider, &req.model)
    };
    Ok(HttpProvider {
        client,
        api_key: key,
        endpoints,
        provider: req.provider.clone(),
        caps,
    })
}

#[async_trait]
impl ChatProvider for HttpProvider {
    fn capabilities(&self, _model: &str) -> Capabilities {
        self.caps
    }

    async fn stream(
        &self,
        req: ChatRequest,
        cancel: CancellationToken,
    ) -> Result<BoxStream<'static, ChatEvent>, ProviderError> {
        let protocol = match self.provider.as_str() {
            "anthropic" => Protocol::Anthropic,
            "ollama" => Protocol::Ollama,
            _ => Protocol::OpenAi,
        };
        let (body, notices) = match protocol {
            Protocol::OpenAi => openai_compat::request(&req, self.caps),
            Protocol::Anthropic => anthropic::request(&req, self.caps),
            Protocol::Ollama => ollama::request(&req, self.caps),
        };
        let mut error = ProviderError::network();
        for endpoint in &self.endpoints {
            let mut request = self
                .client
                .post(endpoint)
                .timeout(Duration::from_secs(300))
                .json(&body);
            if protocol == Protocol::Anthropic {
                request = request
                    .header("x-api-key", &self.api_key)
                    .header("anthropic-version", "2023-06-01");
            } else if !self.api_key.is_empty() {
                request = request.bearer_auth(&self.api_key);
            }
            if self.provider == "openrouter" {
                request = request
                    .header("HTTP-Referer", "https://github.com/TabularisDB/tabularis")
                    .header("X-Title", "Tabularis");
            }
            let response = tokio::select! {
                biased;
                _ = cancel.cancelled() => return Ok(stream::once(async { ChatEvent::Done { finish: FinishReason::Cancelled } }).boxed()),
                response = request.send() => response,
            };
            match response {
                Ok(response) if response.status().is_success() => {
                    if self.provider == "minimax" {
                        if let Some(index) = legacy::MINIMAX_ENDPOINTS
                            .iter()
                            .position(|entry| endpoint.starts_with(entry.openai_base_url))
                        {
                            legacy::MINIMAX_PREFERRED_ENDPOINT.store(index, Ordering::Relaxed);
                        }
                    }
                    return Ok(response_stream(response, protocol, notices, cancel));
                }
                Ok(mut response) => {
                    let status = response.status().as_u16();
                    let mut bytes = Vec::new();
                    while bytes.len() < 16 * 1024 {
                        let chunk = tokio::select! {
                            biased;
                            _ = cancel.cancelled() => return Ok(stream::once(async { ChatEvent::Done { finish: FinishReason::Cancelled } }).boxed()),
                            chunk = response.chunk() => chunk,
                        };
                        match chunk {
                            Ok(Some(chunk)) => bytes.extend_from_slice(
                                &chunk[..chunk.len().min(16 * 1024 - bytes.len())],
                            ),
                            _ => break,
                        }
                    }
                    error = ProviderError::from_status(status, &String::from_utf8_lossy(&bytes));
                }
                Err(_) => error = ProviderError::network(),
            }
        }
        Err(error)
    }
}

struct ResponseStream {
    response: Response,
    parser: FrameParser,
    decoder: Decoder,
    pending: VecDeque<ChatEvent>,
    terminal: bool,
    cancel: CancellationToken,
}

impl ResponseStream {
    fn fail(&mut self, error: ProviderError) {
        self.pending.push_back(ChatEvent::Error { error });
        self.pending.push_back(ChatEvent::Done {
            finish: FinishReason::Error,
        });
        self.terminal = true;
    }

    fn frames(&mut self, frames: Result<Vec<String>, ProviderError>) {
        match frames {
            Err(error) => self.fail(error),
            Ok(frames) => {
                for frame in frames {
                    match self.decoder.frame(&frame) {
                        Ok(events) => self.pending.extend(events),
                        Err(error) => {
                            self.fail(error);
                            break;
                        }
                    }
                    if self.decoder.done {
                        self.terminal = true;
                        break;
                    }
                }
            }
        }
    }
}

fn response_stream(
    response: Response,
    protocol: Protocol,
    notices: Vec<ChatEvent>,
    cancel: CancellationToken,
) -> BoxStream<'static, ChatEvent> {
    let state = ResponseStream {
        response,
        parser: FrameParser::new(if protocol == Protocol::Ollama {
            Framing::Ndjson
        } else {
            Framing::Sse
        }),
        decoder: Decoder::new(protocol),
        pending: notices.into(),
        terminal: false,
        cancel,
    };
    stream::unfold(state, |mut state| async move {
        loop {
            if state.cancel.is_cancelled() && !state.terminal {
                state.pending.clear();
                state.pending.push_back(ChatEvent::Done {
                    finish: FinishReason::Cancelled,
                });
                state.terminal = true;
            }
            if let Some(event) = state.pending.pop_front() {
                return Some((event, state));
            }
            if state.terminal {
                return None;
            }
            let chunk = tokio::select! {
                biased;
                _ = state.cancel.cancelled() => continue,
                chunk = state.response.chunk() => chunk,
            };
            match chunk {
                Ok(Some(bytes)) => {
                    let frames = state.parser.push(&bytes);
                    state.frames(frames);
                }
                Ok(None) => {
                    let frames = state.parser.finish();
                    state.frames(frames);
                    if !state.terminal {
                        match state.decoder.eof() {
                            Ok(events) => {
                                state.pending.extend(events);
                                state.terminal = true;
                            }
                            Err(error) => state.fail(error),
                        }
                    }
                }
                Err(_) => state.fail(ProviderError::network()),
            }
        }
    })
    .boxed()
}
