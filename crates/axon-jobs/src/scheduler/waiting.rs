//! Settle abandoned waiters and hand their capacity to queued successors.

use super::*;

impl ProviderScheduler {
    pub(super) async fn wait_for_grant(
        &self,
        reservation_id: String,
        wait_timeout: Option<Duration>,
    ) -> Result<ReservationGrant, SchedulerError> {
        let started = Instant::now();
        loop {
            let capacity_changed = self.dispatch_signal.changed.notified();
            tokio::pin!(capacity_changed);
            capacity_changed.as_mut().enable();
            let grant = self.reservation_grant(&reservation_id).await?;
            if grant.is_granted() {
                return Ok(grant);
            }
            if wait_timeout.is_some_and(|timeout| started.elapsed() >= timeout) {
                let changed = self
                    .settle_waiting_and_dispatch(&reservation_id, None, "queue_timeout")
                    .await?;
                if changed > 0 {
                    return Err(SchedulerError::WaitTimeout);
                }
                continue;
            }
            // Normal in-process capacity changes are event-driven. The slow
            // timeout remains solely as cross-process/crash recovery because
            // another process cannot signal this Notify.
            if let Some(_claim) = self.dispatch_signal.try_claim_recovery() {
                let recovery_due = tokio::select! {
                    _ = &mut capacity_changed => false,
                    _ = tokio::time::sleep(RECOVERY_POLL) => true,
                };
                if recovery_due {
                    self.dispatch_queued().await?;
                    self.dispatch_signal.changed.notify_waiters();
                }
            } else {
                capacity_changed.await;
            }
        }
    }

    pub(super) async fn cancel_waiting(
        &self,
        reservation_id: &str,
        fence: &str,
        reason: &str,
    ) -> Result<(), SchedulerError> {
        self.settle_waiting_and_dispatch(reservation_id, Some(fence), reason)
            .await?;
        Ok(())
    }

    /// A timeout may expire only a queued row. Drop cleanup additionally owns
    /// a fence and may cancel a grant the waiter has not observed yet.
    pub(super) async fn settle_waiting_and_dispatch(
        &self,
        reservation_id: &str,
        cancel_fence: Option<&str>,
        reason: &str,
    ) -> Result<u64, SchedulerError> {
        let _write_permit = self.write_gate.lock().await;
        let mut connection = self.pool.acquire().await?;
        begin_immediate(&mut connection).await?;
        let result = async {
            let query = match cancel_fence {
                Some(fence) => sqlx::query(
                    "UPDATE provider_reservations SET status = 'canceled', granted_units = 0,
                     terminal_reason = ?, updated_at = datetime('now')
                     WHERE reservation_id = ? AND authority_id = ? AND fence = ?
                       AND status IN ('queued','granted')",
                )
                .bind(reason)
                .bind(reservation_id)
                .bind(&self.domain.authority_id)
                .bind(fence),
                None => sqlx::query(
                    "UPDATE provider_reservations SET status = 'expired', granted_units = 0,
                     terminal_reason = ?, updated_at = datetime('now')
                     WHERE reservation_id = ? AND authority_id = ? AND status = 'queued'",
                )
                .bind(reason)
                .bind(reservation_id)
                .bind(&self.domain.authority_id),
            };
            let changed = query.execute(&mut *connection).await?.rows_affected();
            if changed > 0 {
                let domain = domain_name(self.domain.kind)?;
                while self.grant_head_locked(&mut connection, &domain).await? {}
            }
            Ok(changed)
        }
        .await;
        match result {
            Ok(changed) => {
                sqlx::query("COMMIT").execute(&mut *connection).await?;
                if changed > 0 {
                    self.dispatch_signal.changed.notify_waiters();
                }
                Ok(changed)
            }
            Err(error) => Err(rollback_after_error(&mut connection, error).await),
        }
    }
}
