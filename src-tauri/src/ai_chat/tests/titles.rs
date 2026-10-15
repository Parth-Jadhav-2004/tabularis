use super::*;
use crate::secure_store::testing::TestDirectory;

#[path = "fixtures.rs"]
mod fixtures;

#[test]
fn title_request_uses_first_reply_model_and_only_bounded_untrusted_text() {
    let mut chat = fixtures::conversation();
    chat.messages[0].blocks = vec![
        fixtures::text(&"🦊".repeat(5000)),
        Block::Reasoning {
            text: "private-reasoning".into(),
            signature: Some("signature".into()),
        },
    ];
    chat.messages.push(Message::user("later-message"));
    let request = title_request(&chat, "custom prompt".into()).unwrap();
    assert_eq!(request.provider, "openai");
    assert_eq!(request.model, "reply-model");
    assert_eq!(request.system, "custom prompt");
    assert_eq!(request.max_output_tokens, Some(128));
    assert!(request.tools.is_empty());
    let Block::Text { text } = &request.messages[0].blocks[0] else {
        panic!()
    };
    let value: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(
        value["untrusted_conversation_excerpt"]["user"]
            .as_str()
            .unwrap()
            .chars()
            .count(),
        4000
    );
    assert!(!text.contains("private-reasoning"));
    assert!(!text.contains("later-message"));
    chat.title_source = TitleSource::User;
    assert!(title_request(&chat, String::new()).is_none());
    chat.title_source = TitleSource::Generated;
    assert!(title_request(&chat, String::new()).is_none());
    chat.title_source = TitleSource::FirstMessage;
    chat.messages[1].blocks.clear();
    assert!(title_request(&chat, String::new()).is_none());
}

#[tokio::test]
async fn title_collector_bounds_output_and_rejects_partial_or_filtered_streams() {
    let stream = |events| Box::pin(futures::stream::iter(events)) as BoxStream<'static, ChatEvent>;
    let text = |value: &str| ChatEvent::TextDelta { text: value.into() };
    let done = |finish| ChatEvent::Done { finish };
    assert_eq!(
        collect_title(stream(vec![
            text("\"One two three four five six seven\""),
            done(FinishReason::Stop)
        ]))
        .await
        .unwrap(),
        "One two three four five six"
    );
    assert!(collect_title(stream(vec![text("incomplete")]))
        .await
        .is_err());
    assert!(collect_title(stream(vec![
        text("filtered"),
        done(FinishReason::ContentFilter)
    ]))
    .await
    .is_err());
    assert!(collect_title(stream(vec![
        text(&"x".repeat(4097)),
        done(FinishReason::Stop)
    ]))
    .await
    .is_err());
    assert!(clean_title(" \n ").is_none());
    assert!(clean_title("<script>").is_none());
    assert_eq!(clean_title(&"🦊".repeat(100)).unwrap().chars().count(), 80);
}

#[test]
fn generated_titles_preserve_newer_messages_and_survive_stale_saves() {
    let dir = TestDirectory::new().unwrap();
    let store = ConversationStore::new(dir.path());
    let snapshot = fixtures::conversation();
    store.save(snapshot.clone()).unwrap();
    let mut newer = snapshot.clone();
    newer.messages.push(Message::user("Next question"));
    store.save(newer.clone()).unwrap();
    assert!(store
        .apply_generated_title(&snapshot, "Generated title".into())
        .unwrap()
        .is_some());
    let loaded = store.load("connection", "chat").unwrap();
    assert_eq!(loaded.messages.len(), 3);
    assert_eq!(loaded.title_source, TitleSource::Generated);
    store.save(newer.clone()).unwrap();
    assert_eq!(
        store.load("connection", "chat").unwrap().title,
        "Generated title"
    );
    store
        .rename("connection", "chat", "Manual title".into())
        .unwrap();
    assert!(store
        .apply_generated_title(&snapshot, "Late title".into())
        .unwrap()
        .is_none());
    store.save(newer).unwrap();
    assert_eq!(
        store.load("connection", "chat").unwrap().title,
        "Manual title"
    );
    let sealed = std::fs::read_to_string(dir.path().join("ai_chats/connection/chat.chat")).unwrap();
    assert!(!sealed.contains("Manual title"));
    assert!(!sealed.contains("Generated title"));
}

#[test]
fn late_generated_titles_do_not_recreate_deleted_or_replaced_chats() {
    let dir = TestDirectory::new().unwrap();
    let store = ConversationStore::new(dir.path());
    let snapshot = fixtures::conversation();
    store.save(snapshot.clone()).unwrap();
    store.delete("connection", "chat").unwrap();
    assert!(store
        .apply_generated_title(&snapshot, "Late title".into())
        .is_err());
    assert!(store.list(Some("connection")).unwrap().is_empty());
    let mut replacement = snapshot.clone();
    replacement.created_at += 1;
    store.save(replacement).unwrap();
    assert!(store
        .apply_generated_title(&snapshot, "Late title".into())
        .unwrap()
        .is_none());
}
