use super::*;

#[test]
fn scheduler_flush_policy_covers_full_group_final_and_close_without_a_timer() {
    let pool = 512;
    assert!(!should_flush(pool, 1, pool, false, false));
    // Prepared work retains permits from a three-pool semaphore until flush.
    // Flush at two pools / two envelopes so one maximum-sized envelope always
    // has enough headroom to reach the receiver and cannot deadlock its sender.
    assert!(should_flush(pool * 2, 2, pool, false, false));
    assert!(should_flush(2, 2, pool, false, false));
    assert!(should_flush(1, 1, pool, true, false));
    assert!(should_flush(1, 1, pool, false, true));
}
