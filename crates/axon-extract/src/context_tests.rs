use super::*;
use reqwest::Client;

#[test]
fn credentials_are_explicit_capabilities_not_ambient_state() {
    let context = VerticalContext::new(None, Vec::new(), Client::new());
    assert!(context.github_token().is_none());
    assert!(context.huggingface_token().is_none());
    assert!(context.reddit_credentials().is_none());
}

#[test]
fn vertical_context_is_constructed_from_public_capabilities_only() {
    let context = VerticalContext::new(
        Some("public-agent".to_string()),
        vec!["amazon".to_string()],
        Client::new(),
    );
    assert_eq!(context.ua(), "public-agent");
    assert!(context.auto_dispatch_skipped("amazon"));
    assert!(!context.auto_dispatch_skipped("docs_rs"));
}

#[tokio::test]
async fn vertical_context_uses_the_injected_http_provider() {
    let server = httpmock::MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method("GET")
                .path("/injected")
                .header("user-agent", "adapter-owned-client");
            then.status(200).body("injected client");
        })
        .await;
    let client = Client::builder()
        .user_agent("adapter-owned-client")
        .build()
        .unwrap();
    let context = VerticalContext::new(None, Vec::new(), client);
    // Client defaults are applied when sending, not to an unsent Request.
    let response = context
        .http_client()
        .get(server.url("/injected"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(response.text().await.unwrap(), "injected client");
}
