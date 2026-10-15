use super::*;

#[test]
fn test_load_default_models() {
    let models = load_default_models();
    assert!(models.contains_key("openai"));
    assert!(models.contains_key("anthropic"));
    assert!(models.contains_key("openrouter"));
    assert!(models.contains_key("minimax"));

    // Check for new futuristic models from yaml
    let openai = models.get("openai").unwrap();
    assert!(openai.contains(&"gpt-5.5".to_string()));

    // Anthropic should include the flagship Fable 5 and current Opus/Sonnet/Haiku
    let anthropic = models.get("anthropic").unwrap();
    assert!(anthropic.contains(&"claude-fable-5".to_string()));
    assert!(anthropic.contains(&"claude-opus-4-8".to_string()));
    assert!(anthropic.contains(&"claude-sonnet-4-6".to_string()));
    assert!(anthropic.contains(&"claude-haiku-4-5".to_string()));

    // Check MiniMax models
    let minimax = models.get("minimax").unwrap();
    assert!(minimax.contains(&"MiniMax-M3".to_string()));
    assert!(minimax.contains(&"MiniMax-M2.7".to_string()));
    assert!(minimax.contains(&"MiniMax-M2.7-highspeed".to_string()));
    // M3 should be listed first so it is selected as the default model
    assert_eq!(minimax.first().map(String::as_str), Some("MiniMax-M3"));

    // Ollama is not in yaml, so it shouldn't be here yet
    assert!(!models.contains_key("ollama"));
}

#[test]
fn test_minimax_endpoint_order_prefers_last_successful_region() {
    let global_first = minimax_endpoint_order(0);
    assert_eq!(MINIMAX_ENDPOINTS[global_first[0]].region, "global");
    assert_eq!(MINIMAX_ENDPOINTS[global_first[1]].region, "cn_zh");

    let cn_first = minimax_endpoint_order(1);
    assert_eq!(MINIMAX_ENDPOINTS[cn_first[0]].region, "cn_zh");
    assert_eq!(MINIMAX_ENDPOINTS[cn_first[1]].region, "global");

    assert_eq!(minimax_endpoint_order(usize::MAX), [0, 1]);
}

#[test]
fn test_anthropic_request_body_omits_temperature() {
    let req = crate::ai::provider::ChatRequest {
        provider: "anthropic".to_string(),
        model: "claude-sonnet-5-5".to_string(),
        system: "system".to_string(),
        messages: vec![crate::ai::message::Message::user("count users")],
        tools: vec![],
        max_output_tokens: Some(1024),
        reasoning: None,
        resolved_images: HashMap::new(),
    };
    let (body, _) = crate::ai::providers::legacy_anthropic_body(&req);
    assert!(body.get("temperature").is_none());
    assert_eq!(body["model"], "claude-sonnet-5-5");
    assert_eq!(body["system"], "system");
    assert_eq!(body["messages"][0]["content"][0]["text"], "count users");
    assert_eq!(body["max_tokens"], 1024);
}

#[test]
fn test_clean_response() {
    let input = "```sql\nSELECT * FROM users;\n```";
    let output = clean_response(input);
    assert_eq!(output, "SELECT * FROM users;");

    let input_no_code = "SELECT * FROM users;";
    let output_no_code = clean_response(input_no_code);
    assert_eq!(output_no_code, "SELECT * FROM users;");

    let input_whitespace = "   ```sql\nSELECT 1;\n```   ";
    let output_whitespace = clean_response(input_whitespace);
    assert_eq!(output_whitespace, "SELECT 1;");
}
