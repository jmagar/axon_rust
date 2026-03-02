use super::*;
use httpmock::prelude::*;

#[tokio::test]
async fn refresh_url_304_not_modified() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(304);
    });
    let url = format!("{}/page", server.base_url());
    let prev = RefreshTargetState {
        etag: Some("\"abc\"".into()),
        last_modified: None,
        content_hash: Some("oldhash".into()),
    };
    let client = reqwest::Client::new();
    let result = fetch_and_process_url(&client, &url, Some(&prev))
        .await
        .unwrap();
    assert!(result.not_modified);
    assert!(!result.changed);
    assert_eq!(result.status_code, 304);
    assert_eq!(result.content_hash.as_deref(), Some("oldhash"));
}

#[tokio::test]
async fn refresh_url_200_matching_hash() {
    let body = "<html><body><p>Hello World</p></body></html>";
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(200)
            .header("content-type", "text/html")
            .body(body);
    });
    let url = format!("{}/page", server.base_url());

    let client = reqwest::Client::new();
    let first = fetch_and_process_url(&client, &url, None).await.unwrap();
    assert!(first.changed);
    let hash = first.content_hash.clone().unwrap();

    let prev = RefreshTargetState {
        etag: None,
        last_modified: None,
        content_hash: Some(hash),
    };
    let result = fetch_and_process_url(&client, &url, Some(&prev))
        .await
        .unwrap();
    assert!(!result.changed);
    assert!(!result.not_modified);
    assert_eq!(result.status_code, 200);
}

#[tokio::test]
async fn refresh_url_200_new_content() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(200)
            .header("content-type", "text/html")
            .header("etag", "\"new-etag\"")
            .body("<html><body><p>New content here</p></body></html>");
    });
    let url = format!("{}/page", server.base_url());
    let prev = RefreshTargetState {
        etag: Some("\"old-etag\"".into()),
        last_modified: None,
        content_hash: Some("stale-hash-that-wont-match".into()),
    };
    let client = reqwest::Client::new();
    let result = fetch_and_process_url(&client, &url, Some(&prev))
        .await
        .unwrap();
    assert!(result.changed);
    assert!(!result.not_modified);
    assert_eq!(result.status_code, 200);
    assert!(result.markdown.is_some());
    assert_eq!(result.etag.as_deref(), Some("\"new-etag\""));
}

#[tokio::test]
async fn refresh_url_404_not_changed() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/gone");
        then.status(404);
    });
    let url = format!("{}/gone", server.base_url());
    let client = reqwest::Client::new();
    let result = fetch_and_process_url(&client, &url, None).await.unwrap();
    assert!(!result.changed);
    assert!(!result.not_modified);
    assert_eq!(result.status_code, 404);
    assert!(result.markdown.is_none());
}

#[tokio::test]
async fn refresh_url_first_time_fetch() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/new");
        then.status(200)
            .header("content-type", "text/html")
            .header("last-modified", "Wed, 25 Feb 2026 00:00:00 GMT")
            .body("<html><body><p>Brand new page</p></body></html>");
    });
    let url = format!("{}/new", server.base_url());
    let client = reqwest::Client::new();
    let result = fetch_and_process_url(&client, &url, None).await.unwrap();
    assert!(result.changed);
    assert!(!result.not_modified);
    assert_eq!(result.status_code, 200);
    assert!(result.content_hash.is_some());
    assert!(result.markdown.is_some());
    assert_eq!(
        result.last_modified.as_deref(),
        Some("Wed, 25 Feb 2026 00:00:00 GMT")
    );
}
