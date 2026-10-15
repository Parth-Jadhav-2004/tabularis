use super::{
    anthropic,
    capabilities::from_show,
    decoder::{Decoder, Protocol},
    history::call_id,
    ollama, openai_compat, HttpProvider,
};
use crate::ai::message::{Block, Message, Role, ToolStatus};
use crate::ai::provider::{
    Capabilities, ChatEvent, ChatProvider, ChatRequest, DropReason, ErrorKind, FinishReason,
    ImageData, ToolDef,
};
use futures::StreamExt;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

fn request(provider: &str) -> ChatRequest {
    let mut message = Message::user("count users");
    message.id = "m1".into();
    ChatRequest {
        provider: provider.into(),
        model: "model".into(),
        system: "system".into(),
        messages: vec![message],
        tools: vec![],
        max_output_tokens: None,
        reasoning: None,
        resolved_images: HashMap::new(),
    }
}

fn all_caps() -> Capabilities {
    Capabilities {
        tools: true,
        images: true,
        reasoning: true,
        context_window: None,
    }
}

fn recorded_history() -> ChatRequest {
    let mut req = request("anthropic");
    req.messages[0].blocks.push(Block::Image {
        blob_id: "blob".into(),
        mime: "image/png".into(),
        width: 1,
        height: 1,
    });
    req.resolved_images.insert(
        "blob".into(),
        ImageData {
            mime: "image/png".into(),
            base64: "cGljdHVyZQ==".into(),
        },
    );
    let mut assistant = Message::user("");
    assistant.id = "m2".into();
    assistant.role = Role::Assistant;
    assistant.provider = Some("anthropic".into());
    assistant.blocks = vec![
        Block::Reasoning {
            text: "thinking".into(),
            signature: Some("opaque-signature".into()),
        },
        Block::ToolCall {
            id: "toolu_vendor+id".into(),
            name: "list_tables".into(),
            args: json!({"schema":"public"}),
            status: ToolStatus::Completed,
        },
    ];
    req.messages.push(assistant);
    let mut result = Message::user("");
    result.id = "m3".into();
    result.role = Role::Tool;
    result.blocks = vec![Block::ToolResult {
        call_id: "toolu_vendor+id".into(),
        content: "users".into(),
        truncated: false,
        is_error: false,
    }];
    req.messages.push(result);
    req.tools = vec![ToolDef {
        name: "list_tables".into(),
        description: "List tables".into(),
        parameters: json!({"type":"object", "properties":{"schema":{"type":"string"}}}),
    }];
    req
}

#[test]
fn text_request_serialization_matches_each_protocol() {
    let req = request("custom-openai");
    assert_eq!(
        openai_compat::request(&req, Capabilities::default()).0,
        json!({"model":"model", "messages":[{"role":"system", "content":"system"}, {"role":"user", "content":"count users"}], "temperature":0.0, "stream":true, "stream_options":{"include_usage":true}})
    );
    assert_eq!(
        anthropic::request(&req, all_caps()).0,
        json!({"model":"model", "system":"system", "messages":[{"role":"user", "content":[{"type":"text", "text":"count users"}]}], "stream":true, "max_tokens":4096})
    );
    assert_eq!(
        ollama::request(&req, Capabilities::default()).0,
        json!({"model":"model", "messages":[{"role":"system", "content":"system"}, {"role":"user", "content":"count users"}], "stream":true, "options":{"temperature":0.0}})
    );
}

#[test]
fn cross_provider_history_serializes_tools_images_and_drops_reasoning() {
    let mut req = recorded_history();
    req.provider = "openai".into();
    let (body, notices) = openai_compat::request(&req, all_caps());
    let id = call_id("toolu_vendor+id");
    assert_eq!(
        body["messages"][1]["content"][1]["image_url"]["url"],
        "data:image/png;base64,cGljdHVyZQ=="
    );
    assert_eq!(
        body["messages"][2]["tool_calls"][0],
        json!({"id": id, "type":"function", "function":{"name":"list_tables", "arguments":"{\"schema\":\"public\"}"}})
    );
    assert_eq!(
        body["messages"][3],
        json!({"role":"tool", "tool_call_id":id, "content":"users"})
    );
    assert_eq!(
        notices,
        vec![ChatEvent::Notice {
            message_id: Some("m2".into()),
            reason: DropReason::ReasoningOmitted
        }]
    );
    assert_eq!(
        body["tools"][0]["function"]["parameters"],
        req.tools[0].parameters
    );

    req.provider = "ollama".into();
    let (body, notices) = ollama::request(&req, all_caps());
    assert_eq!(body["messages"][1]["images"], json!(["cGljdHVyZQ=="]));
    assert_eq!(
        body["messages"][2]["tool_calls"][0]["function"]["arguments"],
        json!({"schema":"public"})
    );
    assert_eq!(
        body["messages"][3],
        json!({"role":"tool", "tool_name":"list_tables", "content":"users"})
    );
    assert_eq!(notices.len(), 1);
}

#[test]
fn anthropic_echoes_only_its_own_reasoning_signatures_and_remaps_ids() {
    let mut req = recorded_history();
    let (body, notices) = anthropic::request(&req, all_caps());
    assert!(notices.is_empty());
    assert_eq!(
        body["messages"][1]["content"][0],
        json!({"type":"thinking", "thinking":"thinking", "signature":"opaque-signature"})
    );
    assert_eq!(
        body["messages"][1]["content"][1]["id"],
        call_id("toolu_vendor+id")
    );
    assert_eq!(
        body["messages"][2]["content"][0]["tool_use_id"],
        call_id("toolu_vendor+id")
    );
    assert_eq!(
        body["messages"][0]["content"][1]["source"]["media_type"],
        "image/png"
    );
    assert_eq!(body["tools"][0]["input_schema"], req.tools[0].parameters);
    req.messages[1].provider = Some("openai".into());
    let (body, notices) = anthropic::request(&req, all_caps());
    assert_eq!(body["messages"][1]["content"][0]["type"], "tool_use");
    assert_eq!(notices.len(), 1);
    assert!(!body.to_string().contains("opaque-signature"));
}

#[test]
fn unsupported_images_are_dropped_and_tools_flattened_with_notices() {
    let req = recorded_history();
    for (body, notices) in [
        openai_compat::request(&req, Capabilities::default()),
        anthropic::request(&req, Capabilities::default()),
        ollama::request(&req, Capabilities::default()),
    ] {
        assert!(body.get("tools").is_none());
        assert!(!body.to_string().contains("cGljdHVyZQ=="));
        assert!(body.to_string().contains("untrusted_tool_result"));
        assert!(notices.iter().any(|e| matches!(
            e,
            ChatEvent::Notice {
                reason: DropReason::ImagesUnsupported,
                ..
            }
        )));
        assert_eq!(
            notices
                .iter()
                .filter(|e| matches!(
                    e,
                    ChatEvent::Notice {
                        reason: DropReason::ToolsFlattened,
                        ..
                    }
                ))
                .count(),
            2
        );
    }
}

#[test]
fn unresolved_images_are_reported_without_serializing_blob_ids() {
    let mut req = recorded_history();
    req.resolved_images.clear();
    for (body, notices) in [
        openai_compat::request(&req, all_caps()),
        anthropic::request(&req, all_caps()),
        ollama::request(&req, all_caps()),
    ] {
        assert!(notices.iter().any(|e| matches!(
            e,
            ChatEvent::Notice {
                reason: DropReason::ImageUnavailable,
                ..
            }
        )));
        assert!(!body.to_string().contains("blob"));
    }
}

#[test]
fn tool_id_normalization_is_stable_and_provider_safe() {
    let id = call_id("id with spaces & punctuation");
    assert_eq!(call_id(&id), id);
    assert_eq!(id.len(), 37);
    assert!(id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
    assert_ne!(call_id("one"), call_id("two"));
}

#[test]
fn model_limits_and_reasoning_settings_are_mapped() {
    let mut req = request("openai");
    req.max_output_tokens = Some(2048);
    req.reasoning = Some(crate::ai::provider::ReasoningLevel::Low);
    let body = openai_compat::request(&req, all_caps()).0;
    assert_eq!(body["max_completion_tokens"], 2048);
    assert_eq!(body["reasoning_effort"], "low");
    assert!(body.get("temperature").is_none());
    let body = anthropic::request(&req, all_caps()).0;
    assert_eq!(
        body["thinking"],
        json!({"type":"enabled", "budget_tokens":1024})
    );
    assert!(body.get("temperature").is_none());
    let body = ollama::request(&req, all_caps()).0;
    assert_eq!(body["options"]["num_predict"], 2048);
    assert_eq!(body["think"], true);
}

#[test]
fn ollama_show_maps_capabilities_and_context_window() {
    let caps = from_show(
        &json!({"capabilities":["completion", "tools", "vision", "thinking"], "model_info":{"llama.context_length":32768}}),
    );
    assert_eq!(
        caps,
        Capabilities {
            context_window: Some(32768),
            ..all_caps()
        }
    );
    assert_eq!(from_show(&json!({})), Capabilities::default());
}

#[test]
fn unsupported_tools_reasoning_and_foreign_signatures_are_reported() {
    let mut req = recorded_history();
    req.reasoning = Some(crate::ai::provider::ReasoningLevel::Medium);
    let (body, notices) = anthropic::request(&req, Capabilities::default());
    assert!(!body.to_string().contains("opaque-signature"));
    assert!(notices.contains(&ChatEvent::Notice {
        message_id: None,
        reason: DropReason::ToolsUnsupported
    }));
    assert!(notices.contains(&ChatEvent::Notice {
        message_id: None,
        reason: DropReason::ReasoningUnsupported
    }));
}

#[tokio::test]
async fn ollama_probes_are_cached_after_success_and_retry_after_failure() {
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let cancel = CancellationToken::new();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        for body in ["not json", "{\"capabilities\":[\"tools\",\"vision\"]}"] {
            let (mut socket, _) = listener.accept().await.unwrap();
            let req = read_request(&mut socket).await;
            assert!(req.starts_with("POST /api/show "));
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        }
    });
    assert_eq!(
        super::capabilities::probe_ollama(&client, port, "test-cached-model", &cancel).await,
        Capabilities::default()
    );
    let caps = super::capabilities::probe_ollama(&client, port, "test-cached-model", &cancel).await;
    assert!(caps.tools && caps.images);
    server.await.unwrap();
    // The listener is closed: this call must be served by the cache.
    assert_eq!(
        super::capabilities::probe_ollama(&client, port, "test-cached-model", &cancel).await,
        caps
    );
}

#[tokio::test]
async fn connection_failures_are_typed_network_errors() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let result = adapter("openai", vec![endpoint])
        .stream(request("openai"), CancellationToken::new())
        .await;
    assert!(matches!(result, Err(error) if error.kind == ErrorKind::Network));
}

fn decode(decoder: &mut Decoder, value: Value) -> Vec<ChatEvent> {
    decoder.frame(&value.to_string()).unwrap()
}

#[test]
fn openai_assembles_interleaved_tool_arguments_and_trailing_usage() {
    let mut decoder = Decoder::new(Protocol::OpenAi);
    let events = decode(
        &mut decoder,
        json!({"choices":[{"index":0, "delta":{"tool_calls":[{"index":1, "id":"second", "function":{"name":"describe_table", "arguments":"{\"table\":"}}, {"index":0, "id":"first", "function":{"name":"list_tables", "arguments":"{"}}]}}]}),
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, ChatEvent::ToolCallStart { .. }))
            .count(),
        2
    );
    decode(
        &mut decoder,
        json!({"choices":[{"index":0, "delta":{"tool_calls":[{"index":0, "function":{"arguments":"}"}}, {"index":1, "function":{"arguments":"\"users\"}"}}]}}]}),
    );
    let events = decode(
        &mut decoder,
        json!({"choices":[{"index":0, "delta":{}, "finish_reason":"tool_calls"}]}),
    );
    assert_eq!(
        events,
        vec![
            ChatEvent::ToolCallEnd {
                id: call_id("first"),
                args: json!({})
            },
            ChatEvent::ToolCallEnd {
                id: call_id("second"),
                args: json!({"table":"users"})
            }
        ]
    );
    let events = decode(
        &mut decoder,
        json!({"choices":[], "usage":{"prompt_tokens":10, "completion_tokens":5, "prompt_tokens_details":{"cached_tokens":2}}}),
    );
    assert!(
        matches!(&events[0], ChatEvent::Usage { usage } if usage.input_tokens == 10 && usage.output_tokens == 5 && usage.cache_read_tokens == 2)
    );
    assert_eq!(
        decoder.frame("[DONE]").unwrap(),
        vec![ChatEvent::Done {
            finish: FinishReason::ToolUse
        }]
    );
    assert!(decoder.eof().unwrap().is_empty());
}

#[test]
fn anthropic_assembles_json_signatures_and_cumulative_usage() {
    let mut decoder = Decoder::new(Protocol::Anthropic);
    decode(
        &mut decoder,
        json!({"type":"message_start", "message":{"usage":{"input_tokens":12, "output_tokens":1, "cache_read_input_tokens":3}}}),
    );
    let events = decode(
        &mut decoder,
        json!({"type":"content_block_delta", "index":0, "delta":{"type":"signature_delta", "signature":"opaque"}}),
    );
    assert_eq!(
        events,
        vec![ChatEvent::ReasoningSignature {
            signature: "opaque".into()
        }]
    );
    decode(
        &mut decoder,
        json!({"type":"content_block_start", "index":1, "content_block":{"type":"tool_use", "id":"toolu_1", "name":"query", "input":{}}}),
    );
    decode(
        &mut decoder,
        json!({"type":"content_block_delta", "index":1, "delta":{"type":"input_json_delta", "partial_json":"{\"sql\":"}}),
    );
    decode(
        &mut decoder,
        json!({"type":"content_block_delta", "index":1, "delta":{"type":"input_json_delta", "partial_json":"\"SELECT 1\"}"}}),
    );
    assert_eq!(
        decode(
            &mut decoder,
            json!({"type":"content_block_stop", "index":1})
        ),
        vec![ChatEvent::ToolCallEnd {
            id: call_id("toolu_1"),
            args: json!({"sql":"SELECT 1"})
        }]
    );
    let usage = decode(
        &mut decoder,
        json!({"type":"message_delta", "delta":{"stop_reason":"tool_use"}, "usage":{"output_tokens":25}}),
    );
    assert!(
        matches!(&usage[0], ChatEvent::Usage { usage } if usage.input_tokens == 12 && usage.output_tokens == 25 && usage.cache_read_tokens == 3)
    );
    assert_eq!(
        decode(&mut decoder, json!({"type":"message_stop"})),
        vec![ChatEvent::Done {
            finish: FinishReason::ToolUse
        }]
    );
}

#[test]
fn ollama_decodes_text_thinking_tools_and_done_reason() {
    let mut decoder = Decoder::new(Protocol::Ollama);
    assert_eq!(
        decode(
            &mut decoder,
            json!({"message":{"content":"hello", "thinking":"reason"}, "done":false})
        ),
        vec![
            ChatEvent::TextDelta {
                text: "hello".into()
            },
            ChatEvent::ReasoningDelta {
                text: "reason".into()
            }
        ]
    );
    let events = decode(
        &mut decoder,
        json!({"message":{"tool_calls":[{"function":{"name":"list_tables", "arguments":{}}}]}, "done":false}),
    );
    assert!(matches!(&events[0], ChatEvent::ToolCallStart { name, .. } if name == "list_tables"));
    let events = decode(
        &mut decoder,
        json!({"done":true, "done_reason":"stop", "prompt_eval_count":8, "eval_count":4}),
    );
    assert_eq!(
        events.last(),
        Some(&ChatEvent::Done {
            finish: FinishReason::ToolUse
        })
    );
}

#[test]
fn malformed_json_tool_args_and_early_eof_fail_closed() {
    for protocol in [Protocol::OpenAi, Protocol::Anthropic, Protocol::Ollama] {
        let mut decoder = Decoder::new(protocol);
        assert!(decoder.frame("malformed").is_err());
        assert!(decoder.eof().is_err());
    }
    let mut decoder = Decoder::new(Protocol::OpenAi);
    decode(
        &mut decoder,
        json!({"choices":[{"delta":{"tool_calls":[{"index":0, "id":"call", "function":{"name":"tool", "arguments":"{"}}]}}]}),
    );
    assert!(decoder
        .frame(&json!({"choices":[{"delta":{}, "finish_reason":"tool_calls"}]}).to_string())
        .is_err());
}

#[test]
fn unknown_anthropic_events_are_ignored_and_stream_errors_typed() {
    assert!(Decoder::new(Protocol::Anthropic)
        .frame("{\"type\":\"future_event\"}")
        .unwrap()
        .is_empty());
    let error = Decoder::new(Protocol::Anthropic).frame("{\"type\":\"error\", \"error\":{\"type\":\"overloaded_error\", \"message\":\"private prompt\"}}").unwrap_err();
    assert_eq!(error.kind, ErrorKind::RateLimit);
    assert!(!error.message.contains("private prompt"));
}

async fn read_request(socket: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut chunk = [0; 1024];
    loop {
        let size = socket.read(&mut chunk).await.unwrap();
        assert!(size > 0);
        bytes.extend_from_slice(&chunk[..size]);
        if let Some(headers_end) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..headers_end]);
            let length = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|s| s.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            if bytes.len() >= headers_end + 4 + length {
                return String::from_utf8(bytes).unwrap();
            }
        }
    }
}

async fn serve(body: &'static str, status: u16) -> (String, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = read_request(&mut socket).await;
        socket.write_all(format!("HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
        request
    });
    (url, task)
}

fn adapter(provider: &str, endpoints: Vec<String>) -> HttpProvider {
    HttpProvider {
        provider: provider.into(),
        client: reqwest::Client::builder().no_proxy().build().unwrap(),
        api_key: "test-key".into(),
        endpoints,
        caps: all_caps(),
    }
}

#[tokio::test]
async fn http_fixtures_stream_all_protocols_and_collect_legacy_response() {
    for (provider, body) in [
        ("openai", "data: {\"choices\":[{\"delta\":{\"content\":\"```sql\\nSELECT 1;\\n```\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n"),
        ("anthropic", "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"```sql\\nSELECT 1;\\n```\"}}\n\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":5}}\n\ndata: {\"type\":\"message_stop\"}\n\n"),
        ("ollama", "{\"message\":{\"content\":\"```sql\\nSELECT 1;\\n```\"},\"done\":false}\n{\"done\":true}\n"),
    ] {
        let (url, server) = serve(body, 200).await;
        let stream = adapter(provider, vec![url]).stream(request(provider), CancellationToken::new()).await.unwrap();
        assert_eq!(crate::ai::legacy::collect_legacy_response(stream).await.unwrap(), "SELECT 1;");
        let request = server.await.unwrap();
        assert!(request.contains("\"stream\":true"));
        assert!(request.contains("test-key"));
    }
}

#[tokio::test]
async fn http_error_is_typed_and_minimax_fails_over_before_streaming() {
    let (bad, bad_server) = serve("{\"error\":{\"message\":\"private prompt\"}}", 401).await;
    let provider = adapter("openai", vec![bad.clone()]);
    let error = match provider
        .stream(request("openai"), CancellationToken::new())
        .await
    {
        Err(error) => error,
        Ok(_) => panic!("expected auth error"),
    };
    assert_eq!(error.kind, ErrorKind::Auth);
    bad_server.await.unwrap();
    let (bad, bad_server) = serve("busy", 503).await;
    let (good, good_server) = serve("data: {\"choices\":[{\"delta\":{\"content\":\"ok\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n", 200).await;
    let stream = adapter("minimax", vec![bad, good])
        .stream(request("minimax"), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(
        crate::ai::legacy::collect_legacy_response(stream)
            .await
            .unwrap(),
        "ok"
    );
    bad_server.await.unwrap();
    good_server.await.unwrap();
}

#[tokio::test]
async fn cancellation_interrupts_waiting_for_headers_and_body() {
    for send_headers in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let cancel = CancellationToken::new();
        let server_cancel = cancel.clone();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            read_request(&mut socket).await;
            if send_headers {
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 999999\r\n\r\n")
                    .await
                    .unwrap();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
            server_cancel.cancel();
            tokio::time::sleep(Duration::from_millis(50)).await;
        });
        let task = async {
            let stream = adapter("openai", vec![url])
                .stream(request("openai"), cancel)
                .await
                .unwrap();
            let events: Vec<_> = stream.collect().await;
            assert_eq!(
                events,
                vec![ChatEvent::Done {
                    finish: FinishReason::Cancelled
                }]
            );
        };
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap();
        server.await.unwrap();
    }
}

#[tokio::test]
async fn truncated_http_stream_emits_error_and_exactly_one_done() {
    let (url, server) = serve(
        "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n",
        200,
    )
    .await;
    let events: Vec<_> = adapter("openai", vec![url])
        .stream(request("openai"), CancellationToken::new())
        .await
        .unwrap()
        .collect()
        .await;
    assert!(matches!(&events[0], ChatEvent::TextDelta { text } if text == "partial"));
    assert!(matches!(&events[1], ChatEvent::Error { .. }));
    assert_eq!(
        events.last(),
        Some(&ChatEvent::Done {
            finish: FinishReason::Error
        })
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, ChatEvent::Done { .. }))
            .count(),
        1
    );
    server.await.unwrap();
}
