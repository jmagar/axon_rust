use super::*;

fn config(endpoint: &str) -> TeiEmbeddingConfig {
    TeiEmbeddingConfig {
        endpoint: endpoint.to_string(),
        model: "test-model".to_string(),
        dimensions: 2,
        timeout: Duration::from_secs(1),
        max_batch_inputs: 64,
        max_concurrent_requests: 3,
        max_in_flight_inputs: 128,
        max_input_tokens: 8192,
        max_batch_tokens: 131_072,
        instruction_support: InstructionSupport::None,
        retry_backoff_ms: 1,
        max_attempts: 1,
    }
}

#[test]
fn providers_with_the_same_admission_profile_share_process_gates() {
    let first = TeiEmbeddingProvider::new(config("http://tei-shared.test"));
    let second = TeiEmbeddingProvider::new(config("http://tei-shared.test/"));
    let other = TeiEmbeddingProvider::new(config("http://tei-other.test"));

    assert!(Arc::ptr_eq(&first.admission, &second.admission));
    assert!(!Arc::ptr_eq(&first.admission, &other.admission));
}

#[test]
fn endpoint_identity_and_live_owner_define_the_shared_capacity() {
    let first = TeiEmbeddingProvider::new(config("http://TEI-NORMALIZED.test:80/base/"));
    let mut larger = config("http://tei-normalized.test/base");
    larger.max_concurrent_requests = 20;
    larger.max_in_flight_inputs = 1024;
    let second = TeiEmbeddingProvider::new(larger.clone());
    assert!(Arc::ptr_eq(&first.admission, &second.admission));
    assert_eq!(second.admission.max_concurrent_requests, 3);
    assert_eq!(second.admission.max_in_flight_inputs, 128);
    drop(first);
    drop(second);
    let replacement = TeiEmbeddingProvider::new(larger);
    assert_eq!(replacement.admission.max_concurrent_requests, 20);
    assert_eq!(replacement.admission.max_in_flight_inputs, 1024);
}
