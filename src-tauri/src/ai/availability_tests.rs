use super::*;

fn no_key(_: &str) -> AiKeyStatus {
    AiKeyStatus {
        configured: false,
        from_env: false,
    }
}

#[test]
fn unconfigured_providers_are_omitted_without_implicit_ollama() {
    assert!(available_providers(&AppConfig::default(), &HashSet::new(), no_key).is_empty());
}

#[test]
fn configured_keys_report_their_source_and_cached_models() {
    let cached = HashSet::from(["openai".to_string()]);
    let providers = available_providers(&AppConfig::default(), &cached, |provider| AiKeyStatus {
        configured: matches!(provider, "openai" | "anthropic"),
        from_env: provider == "anthropic",
    });
    assert_eq!(providers.len(), 2);
    assert_eq!(providers[0].provider, "openai");
    assert_eq!(providers[0].source, "keychain");
    assert!(providers[0].models_cached);
    assert_eq!(providers[1].source, "env");
    assert!(!providers[1].models_cached);
}

#[test]
fn keyless_providers_require_an_explicit_valid_endpoint() {
    let config = AppConfig {
        ai_ollama_port: Some(11434),
        ai_custom_openai_url: Some("http://localhost:8080/v1".to_string()),
        ..AppConfig::default()
    };
    let providers = available_providers(&config, &HashSet::new(), no_key);
    assert_eq!(providers.len(), 2);
    assert_eq!(providers[0].provider, "ollama");
    assert_eq!(providers[1].provider, "custom-openai");
    assert!(providers
        .iter()
        .all(|provider| provider.source == "keyless"));
    let invalid = AppConfig {
        ai_ollama_port: Some(0),
        ai_custom_openai_url: Some("not a URL".to_string()),
        ..AppConfig::default()
    };
    assert!(available_providers(&invalid, &HashSet::new(), no_key).is_empty());
}

#[test]
fn custom_provider_needs_a_url_even_with_a_key_and_preserves_key_precedence() {
    let has_key = |_: &str| AiKeyStatus {
        configured: true,
        from_env: false,
    };
    assert!(
        !available_providers(&AppConfig::default(), &HashSet::new(), has_key)
            .iter()
            .any(|provider| provider.provider == "custom-openai")
    );
    let config = AppConfig {
        ai_custom_openai_url: Some("https://example.test/v1".to_string()),
        ..AppConfig::default()
    };
    let providers = available_providers(&config, &HashSet::new(), has_key);
    assert_eq!(providers.last().unwrap().source, "keychain");
}

#[test]
fn keyless_urls_only_accept_http_endpoints() {
    for url in [
        None,
        Some(""),
        Some("file:///tmp/api"),
        Some("ftp://example.test"),
        Some("http://"),
    ] {
        assert!(!configured_keyless_url(url));
    }
    assert!(configured_keyless_url(Some("https://example.test/api")));
}

#[tokio::test]
async fn ollama_checks_distinguish_available_empty_and_unreachable_instances() {
    use std::io::{Read, Write};
    for (status, body, expected) in [
        (200, r#"{"models":[{"name":"local-model"}]}"#, None),
        (200, r#"{"models":[]}"#, Some("no_models")),
        (200, r#"{"models":[{"name":""}]}"#, Some("no_models")),
        (200, "not JSON", Some("unreachable")),
        (503, "{}", Some("unreachable")),
    ] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut buffer = [0; 4096];
            let _ = stream.read(&mut buffer).unwrap();
            write!(stream, "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        });
        assert_eq!(ollama_unavailable_reason(port).await, expected);
        server.join().unwrap();
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    assert_eq!(ollama_unavailable_reason(port).await, Some("unreachable"));
}
