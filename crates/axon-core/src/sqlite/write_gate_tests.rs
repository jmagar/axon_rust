use super::*;

#[track_caller]
fn test_location() -> &'static std::panic::Location<'static> {
    std::panic::Location::caller()
}

#[tokio::test]
async fn stale_guard_cleanup_cannot_erase_a_new_holder() {
    let gate = SqliteWriteGate::default();
    let guard = gate.lock().await;
    let newer_id = guard.holder_id + 1;
    let newer_location = test_location();
    *gate
        .0
        .holder
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((newer_id, newer_location));
    drop(guard);
    assert_eq!(
        *gate
            .0
            .holder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        Some((newer_id, newer_location))
    );
    assert!(gate.try_lock().is_some());
}
