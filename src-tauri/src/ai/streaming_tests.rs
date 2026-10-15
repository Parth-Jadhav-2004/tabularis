use super::{
    message::{Block, Message},
    provider::{ChatEvent, ErrorKind, FinishReason, ProviderError},
    sse::{FrameParser, Framing},
};
use serde_json::json;

#[test]
fn sse_preserves_utf8_at_every_chunk_boundary() {
    let input =
        ": heartbeat\r\nevent: token\r\ndata: {\"text\":\"caffè ☕\"}\r\n\r\ndata: [DONE]\n\n"
            .as_bytes();
    for split in 0..=input.len() {
        let mut parser = FrameParser::new(Framing::Sse);
        let mut frames = parser.push(&input[..split]).unwrap();
        frames.extend(parser.push(&input[split..]).unwrap());
        frames.extend(parser.finish().unwrap());
        assert_eq!(frames, ["{\"text\":\"caffè ☕\"}", "[DONE]"]);
    }
}

#[test]
fn sse_multiline_cr_and_unterminated_last_event() {
    let mut parser = FrameParser::new(Framing::Sse);
    assert_eq!(
        parser.push(b"data: one\rdata:two\r\rdata: final").unwrap(),
        ["one\ntwo"]
    );
    assert_eq!(parser.finish().unwrap(), ["final"]);
}

#[test]
fn ndjson_handles_split_lines_empty_lines_and_eof() {
    let mut parser = FrameParser::new(Framing::Ndjson);
    assert!(parser.push(b"{\"done\": fa").unwrap().is_empty());
    assert_eq!(
        parser.push(b"lse}\r\n\n{\"done\": true}").unwrap(),
        ["{\"done\": false}"]
    );
    assert_eq!(parser.finish().unwrap(), ["{\"done\": true}"]);
}

#[test]
fn parser_rejects_invalid_utf8_and_oversized_frames() {
    assert!(FrameParser::new(Framing::Sse)
        .push(b"data: \xff\n")
        .is_err());
    assert!(FrameParser::new(Framing::Ndjson)
        .push(&vec![b'x'; 8 * 1024 * 1024 + 1])
        .is_err());
}

#[test]
fn message_skips_unknown_blocks_but_rejects_malformed_known_blocks() {
    let mut value = json!({"id":"m1", "role":"user", "created_at":0, "blocks":[{"type":"future", "secret":"do not log"}, {"type":"text", "text":"hello"}]});
    let message: Message = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        message.blocks,
        vec![Block::Text {
            text: "hello".into()
        }]
    );
    value["blocks"] = json!([{"type":"text"}]);
    assert!(serde_json::from_value::<Message>(value).is_err());
}

#[test]
fn wire_format_is_snake_case_and_tagged() {
    assert_eq!(
        serde_json::to_value(ChatEvent::Done {
            finish: FinishReason::ToolUse
        })
        .unwrap(),
        json!({"type":"done", "finish":"tool_use"})
    );
    let event = ChatEvent::Error {
        error: ProviderError::new(ErrorKind::RateLimit, "Try later"),
    };
    assert_eq!(
        serde_json::to_value(event).unwrap(),
        json!({"type":"error", "error":{"kind":"rate_limit", "message":"Try later"}})
    );
}

#[test]
fn errors_are_typed_and_never_echo_upstream_bodies() {
    for (status, body, kind) in [
        (401, "secret", ErrorKind::Auth),
        (429, "prompt", ErrorKind::RateLimit),
        (
            400,
            "context_length_exceeded: private query",
            ErrorKind::ContextLength,
        ),
        (413, "private", ErrorKind::ContextLength),
        (500, "credential", ErrorKind::Other),
    ] {
        let error = ProviderError::from_status(status, body);
        assert_eq!(error.kind, kind);
        assert!(!error.message.contains(body));
    }
}

#[test]
fn catalog_supports_string_and_detailed_entries_without_changing_model_lists() {
    let models = super::catalog::default_models();
    assert_eq!(models["openai"][0], "gpt-5.5");
    assert!(super::catalog::capabilities("openai", "gpt-5.5").reasoning);
    assert!(!super::catalog::capabilities("openai", "gpt-oss-20B").images);
    assert!(super::catalog::capabilities("anthropic", "new-live-model").tools);
    assert!(!super::catalog::capabilities("unknown", "new-model").tools);
}
