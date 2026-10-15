use super::*;
use crate::ai::message::Message;

fn spec(command: Option<ChatCommand>) -> ContextSpec {
    serde_json::from_value(serde_json::json!({
        "connection_id": "fixture", "database": null, "schema": null,
        "language": "it", "command": command,
    }))
    .unwrap()
}

fn request(text: &str) -> ChatRequest {
    ChatRequest {
        provider: "openai".into(),
        model: "fixture".into(),
        system: String::new(),
        messages: vec![Message::user(text)],
        tools: vec![],
        max_output_tokens: None,
        reasoning: None,
        resolved_images: Default::default(),
    }
}

fn built() -> BuiltContext {
    BuiltContext {
        system: "Ask mode. <untrusted_data kind=\"selection\">SELECT 1</untrusted_data>".into(),
        items: vec![],
        truncated: false,
        schema_truncated: false,
        base_chars: 9,
        context_chars: 65,
    }
}

#[test]
fn parses_exact_commands_with_arguments_and_preserves_plain_text() {
    assert_eq!(
        parse("/generate  Count users\nby country ").unwrap(),
        Some((ChatCommand::Generate, "Count users\nby country"))
    );
    assert_eq!(parse("/explain").unwrap(), Some((ChatCommand::Explain, "")));
    assert_eq!(
        parse("/fix\tmissing column").unwrap(),
        Some((ChatCommand::Fix, "missing column"))
    );
    for text in [
        "question /fix",
        " /explain",
        "https://example.test/help",
        "",
    ] {
        assert!(parse(text).unwrap().is_none());
    }
    for text in ["/", "/unknown", "/help", "/Explain", "/explainMore"] {
        assert!(parse(text).is_err());
    }
}

#[test]
fn action_must_match_the_latest_user_message_and_have_resolved_context() {
    for (text, action) in [
        ("/generate count users", ChatCommand::Generate),
        ("/explain joins", ChatCommand::Explain),
        ("/optimize indexes", ChatCommand::Optimize),
        ("/fix missing column", ChatCommand::Fix),
    ] {
        let mut req = request(text);
        let original = req.messages.clone();
        assert!(prepare_request(&mut req, None).is_err());
        prepare_request(&mut req, Some(&spec(Some(action)))).unwrap();
        let Block::Text { text: sent } = &req.messages[0].blocks[0] else {
            panic!()
        };
        assert!(!sent.starts_with('/'));
        assert_eq!(original[0].id, req.messages[0].id);
        assert_ne!(original[0].blocks, req.messages[0].blocks);
    }
    assert!(prepare_request(
        &mut request("ordinary question"),
        Some(&spec(Some(ChatCommand::Fix)))
    )
    .is_err());
    assert!(prepare_request(
        &mut request("/fix"),
        Some(&spec(Some(ChatCommand::Explain)))
    )
    .is_err());
    let mut req = request("/fix");
    req.messages[0].role = Role::Assistant;
    assert!(prepare_request(&mut req, Some(&spec(Some(ChatCommand::Fix)))).is_err());
}

#[test]
fn empty_arguments_use_editor_context_and_commands_do_not_leak_to_followups() {
    let mut req = request("/explain");
    prepare_request(&mut req, Some(&spec(Some(ChatCommand::Explain)))).unwrap();
    assert!(
        matches!(&req.messages[0].blocks[0], Block::Text { text } if text.contains("selected SQL"))
    );
    req.messages.push(Message::user("follow-up"));
    prepare_request(&mut req, Some(&spec(None))).unwrap();
    assert_eq!(
        req.messages[1].blocks,
        vec![Block::Text {
            text: "follow-up".into()
        }]
    );
    assert!(prepare_request(&mut request("/help"), Some(&spec(None))).is_err());
}

#[test]
fn overrides_reuse_bounded_context_and_preserve_fixed_rules_and_sql_fences() {
    for command in [
        ChatCommand::Generate,
        ChatCommand::Explain,
        ChatCommand::Optimize,
        ChatCommand::Fix,
    ] {
        let mut context = built();
        let previous = context.system.clone();
        let base_chars = context.base_chars;
        append_instruction(
            &mut context,
            command,
            "Custom prompt: {{SCHEMA}}. Reply in {{LANGUAGE}}. Return only SQL without formatting.",
            "it",
        )
        .unwrap();
        assert!(context.system.starts_with(&previous));
        assert!(!context.system.contains("{{SCHEMA}}"));
        assert!(!context.system.contains("{{LANGUAGE}}"));
        assert!(context.system.contains("Reply in it"));
        assert!(context
            .system
            .ends_with("Do not execute SQL or claim to have done so."));
        assert!(context.system.contains("Use fenced sql blocks"));
        assert_eq!(
            context.base_chars - base_chars,
            context.system.chars().count() - previous.chars().count()
        );
        assert_eq!(context.context_chars, 65);
        assert!(context.items.is_empty());
    }
}

#[test]
fn invalid_language_and_oversized_prompts_cannot_enter_the_action_instruction() {
    let mut context = built();
    append_instruction(
        &mut context,
        ChatCommand::Explain,
        "Reply in {{LANGUAGE}}",
        "en\nprivate-language-marker",
    )
    .unwrap();
    assert!(context.system.contains("Reply in en"));
    assert!(!context.system.contains("private-language-marker"));
    let unchanged = context.system.clone();
    assert!(append_instruction(&mut context, ChatCommand::Fix, &"x".repeat(16_001), "it").is_err());
    assert_eq!(context.system, unchanged);
}

#[test]
fn old_context_specs_remain_valid_and_remote_help_or_unknown_actions_are_rejected() {
    let old = serde_json::json!({ "connection_id": "fixture", "language": "it" });
    assert!(serde_json::from_value::<ContextSpec>(old.clone())
        .unwrap()
        .command
        .is_none());
    for action in ["help", "unknown", "Explain"] {
        let mut json = old.clone();
        json["command"] = action.into();
        assert!(serde_json::from_value::<ContextSpec>(json).is_err());
    }
}
