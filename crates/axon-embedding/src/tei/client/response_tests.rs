use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// Valid JSON with excessive whitespace proves that rejection precedes decoding,
// including when the endpoint omits Content-Length.
#[tokio::test]
async fn oversized_success_is_rejected_with_content_length() {
    oversized_success(true).await;
}

#[tokio::test]
async fn oversized_success_is_rejected_without_content_length() {
    oversized_success(false).await;
}

async fn oversized_success(declared_length: bool) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            request.push(socket.read_u8().await.unwrap());
        }
        let padding = 17 * 1024 * 1024;
        let mut headers = "HTTP/1.1 200 OK\r\nConnection: close\r\n".to_owned();
        if declared_length {
            headers.push_str(&format!("Content-Length: {}\r\n", padding + 7));
        }
        headers.push_str("\r\n");
        if socket.write_all(headers.as_bytes()).await.is_err() {
            return;
        }
        for _ in 0..padding / 4096 {
            if socket.write_all(&[b' '; 4096]).await.is_err() {
                return;
            }
        }
        let _ = socket.write_all(b"[[0.5]]").await;
    });
    let result = send_with_body(
        reqwest::Client::new()
            .get(url)
            .timeout(std::time::Duration::from_secs(10)),
    )
    .await;
    server.await.unwrap();
    assert!(
        matches!(result, Err(ResponseError::TooLarge)),
        "oversized valid JSON must be rejected (Content-Length={declared_length})"
    );
}

fn client(endpoint: String) -> super::super::TeiClient {
    super::super::TeiClient::new(super::super::TeiClientParams {
        endpoint,
        provider_id: "bounded-response".into(),
        max_batch_inputs: 2,
        max_input_tokens: 8192,
        max_batch_tokens: 16384,
        max_concurrent_requests: 1,
        max_in_flight_inputs: 2,
        max_attempts: 3,
        request_timeout: std::time::Duration::from_secs(10),
        retry_backoff_base_ms: 1,
    })
    .unwrap()
}

#[tokio::test]
async fn oversized_info_is_rejected_before_decoding() {
    let server = httpmock::MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method("GET").path("/info");
            then.status(200).body(format!(
                "{}{{\"model_id\":\"model\"}}",
                " ".repeat(2 * 1024 * 1024)
            ));
        })
        .await;
    let error = client(server.base_url()).fetch_info().await.unwrap_err();
    assert_eq!(error.code, "embedding.tei.response_too_large".into());
    assert!(!error.retryable);
}

#[tokio::test]
async fn oversized_batches_split_without_rejecting_valid_vectors() {
    let server = httpmock::MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method("POST")
                .path("/embed")
                .json_body(serde_json::json!({"inputs":["a", "b"],"truncate":false}));
            then.status(200)
                .body(format!("{}[[0.5],[0.7]]", " ".repeat(17 * 1024 * 1024)));
        })
        .await;
    for (input, value) in [("a", 0.5), ("b", 0.7)] {
        server
            .mock_async(|when, then| {
                when.method("POST")
                    .path("/embed")
                    .json_body(serde_json::json!({"inputs":[input],"truncate":false}));
                then.status(200).json_body(serde_json::json!([[value]]));
            })
            .await;
    }
    let result = client(server.base_url())
        .embed_all(&["a".into(), "b".into()])
        .await
        .unwrap();
    assert_eq!(result.vectors, vec![vec![0.5], vec![0.7]]);
    assert_eq!(
        result.requests, 3,
        "oversized batches must split into bounded requests"
    );
}
