use super::*;
use std::sync::Arc;

#[tokio::test]
async fn interactive_completion_precedes_queued_background_without_exceeding_capacity() {
    use axon_api::source::JobPriority;
    let key = CompletionKey::OpenAi {
        base_url: "http://priority-regression".into(),
        model: "unique".into(),
    };
    let held = acquire_completion_permit_for_key(key.clone(), 1)
        .await
        .unwrap();
    let background = acquire_completion_permit_for_key(key.clone(), 1);
    tokio::pin!(background);
    assert!(futures_util::poll!(&mut background).is_pending());
    let interactive = crate::reservation::with_priority(
        JobPriority::Interactive,
        acquire_completion_permit_for_key(key.clone(), 1),
    );
    tokio::pin!(interactive);
    assert!(futures_util::poll!(&mut interactive).is_pending());
    drop(held);
    let interactive_permit = match futures_util::poll!(&mut interactive) {
        std::task::Poll::Ready(Ok(permit)) => permit,
        _ => panic!("interactive work must get the next execution slot"),
    };
    assert!(futures_util::poll!(&mut background).is_pending());
    drop(interactive_permit);
    assert!(futures_util::poll!(&mut background).is_ready());
}

#[test]
fn completion_concurrency_defaults_to_four() {
    assert_eq!(parse_completion_concurrency_limit(None), 4);
}

#[test]
fn completion_concurrency_rejects_zero() {
    assert_eq!(parse_completion_concurrency_limit(Some("0")), 4);
}

#[test]
fn completion_concurrency_clamps_to_semaphore_max() {
    let huge = (Semaphore::MAX_PERMITS + 1).to_string();
    assert_eq!(
        parse_completion_concurrency_limit(Some(&huge)),
        Semaphore::MAX_PERMITS
    );
}

#[tokio::test]
async fn completion_limiter_is_keyed_by_backend_identity_only() {
    let openai_key = CompletionKey::OpenAi {
        base_url: "http://one".to_string(),
        model: "gpt".to_string(),
    };
    let first = acquire_completion_permit_for_key(openai_key.clone(), 1)
        .await
        .expect("first permit");

    assert_eq!(
        available_permits_for_key(&openai_key),
        Some(0),
        "first permit should saturate the one-permit limiter"
    );

    let same_key_limit_one = completion_semaphore_for_key_for_tests(openai_key.clone(), 1);
    let same_key_limit_two = completion_semaphore_for_key_for_tests(openai_key.clone(), 2);
    assert!(
        Arc::ptr_eq(&same_key_limit_one, &same_key_limit_two),
        "changing the limit must not create a bypass bucket for the same backend",
    );

    let gemini_key = CompletionKey::Gemini {
        cmd: "gemini".to_string(),
        model: "flash".to_string(),
    };
    let second_different_backend = acquire_completion_permit_for_key(gemini_key, 1).await;
    assert!(
        second_different_backend.is_ok(),
        "different backend key should use an independent limiter"
    );

    drop(first);
}

#[tokio::test]
async fn interactive_burst_preserves_background_progress() {
    use axon_api::source::JobPriority;
    let key = CompletionKey::OpenAi {
        base_url: "http://priority-fairness".into(),
        model: "unique".into(),
    };
    let mut held = acquire_completion_permit_for_key(key.clone(), 1)
        .await
        .unwrap();
    let background = acquire_completion_permit_for_key(key.clone(), 1);
    tokio::pin!(background);
    assert!(futures_util::poll!(&mut background).is_pending());
    let mut interactive = (0..9)
        .map(|_| {
            Box::pin(crate::reservation::with_priority(
                JobPriority::Interactive,
                acquire_completion_permit_for_key(key.clone(), 1),
            ))
        })
        .collect::<Vec<_>>();
    for pending in &mut interactive {
        assert!(futures_util::poll!(pending.as_mut()).is_pending());
    }
    for pending in interactive.iter_mut().take(8) {
        drop(held);
        held = match futures_util::poll!(pending.as_mut()) {
            std::task::Poll::Ready(Ok(permit)) => permit,
            _ => panic!("interactive waiter must get its fair turn"),
        };
        assert!(futures_util::poll!(&mut background).is_pending());
    }
    drop(held);
    let background_permit = match futures_util::poll!(&mut background) {
        std::task::Poll::Ready(Ok(permit)) => permit,
        _ => panic!("background waiter must progress after the bounded burst"),
    };
    assert!(futures_util::poll!(interactive[8].as_mut()).is_pending());
    drop(background_permit);
    assert!(futures_util::poll!(interactive[8].as_mut()).is_ready());
}

#[tokio::test]
async fn canceling_queued_and_granted_waiters_reclaims_execution_capacity() {
    let key = CompletionKey::OpenAi {
        base_url: "http://priority-cancel".into(),
        model: "unique".into(),
    };
    let held = acquire_completion_permit_for_key(key.clone(), 1)
        .await
        .unwrap();
    for _ in 0..20 {
        let pending = acquire_completion_permit_for_key(key.clone(), 1);
        tokio::pin!(pending);
        assert!(futures_util::poll!(&mut pending).is_pending());
    }
    let mut granted = Box::pin(acquire_completion_permit_for_key(key.clone(), 1));
    assert!(futures_util::poll!(granted.as_mut()).is_pending());
    drop(held);
    // Cancel after dispatch grants capacity but before the caller polls it.
    drop(granted);
    assert_eq!(available_permits_for_key(&key), Some(1));
    let next = acquire_completion_permit_for_key(key, 1);
    tokio::pin!(next);
    assert!(futures_util::poll!(&mut next).is_ready());
}
