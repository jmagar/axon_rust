use super::*;

#[test]
fn registering_a_retry_preserves_the_old_attempt_token() {
    let shutdown = CancellationToken::new();
    let job_id = JobId::new(uuid::Uuid::new_v4());
    let old = register(job_id, 1, &shutdown);
    let fresh = register(job_id, 2, &shutdown);

    assert!(cancel_attempt(job_id, 1));
    assert!(old.is_cancelled());
    assert!(!fresh.is_cancelled());

    unregister(job_id, 1);
    assert!(cancel_attempt(job_id, 2));
    assert!(fresh.is_cancelled());
    unregister(job_id, 2);
}
