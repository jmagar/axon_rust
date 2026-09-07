//! Process-local admission for mutations sharing the SQLite writer boundary.

use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Process-local admission gate for scheduler mutations sharing one SQLite DB.
///
/// SQLite admits one writer at a time. Without this gate, concurrent provider
/// calls can park every SQLx connection worker inside SQLite's busy handler,
/// starving unrelated job heartbeats and control-plane reads of a pool slot.
///
/// The gate is intentionally process-local even though the DB is shared with
/// short-lived CLI processes: cross-process writers are serialized by SQLite's
/// own write lock, so the accepted bound is that a gate holder may stall up to
/// the busy timeout behind an external writer while in-process writers queue
/// behind the gate.
#[derive(Debug, Default)]
struct SqliteWriteGateInner {
    mutex: Mutex<()>,
    next_holder_id: AtomicU64,
    holder: StdMutex<Option<(u64, &'static std::panic::Location<'static>)>>,
}

#[derive(Debug, Clone, Default)]
pub struct SqliteWriteGate(Arc<SqliteWriteGateInner>);

pub struct SqliteWriteGuard<'a> {
    guard: Option<tokio::sync::MutexGuard<'a, ()>>,
    inner: &'a SqliteWriteGateInner,
    holder_id: u64,
}

impl Drop for SqliteWriteGuard<'_> {
    fn drop(&mut self) {
        drop(self.guard.take());
        let mut holder = self
            .inner
            .holder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if holder.is_some_and(|(holder_id, _)| holder_id == self.holder_id) {
            *holder = None;
        }
    }
}

/// Backward-compatible scheduler-facing name for the shared SQLite writer gate.
pub type SchedulerWriteGate = SqliteWriteGate;

impl SqliteWriteGate {
    fn record_holder(&self, caller: &'static std::panic::Location<'static>) -> u64 {
        let holder_id = self.0.next_holder_id.fetch_add(1, Ordering::Relaxed);
        *self
            .0
            .holder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((holder_id, caller));
        holder_id
    }

    #[doc(hidden)]
    #[track_caller]
    pub fn lock(&self) -> impl Future<Output = SqliteWriteGuard<'_>> + '_ {
        let caller = std::panic::Location::caller();
        async move {
            let started = Instant::now();
            let lock = self.0.mutex.lock();
            tokio::pin!(lock);
            let guard = tokio::select! {
                guard = &mut lock => guard,
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    let holder = self
                        .0
                        .holder
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .map(|(_, location)| location);
                    tracing::warn!(
                        waited_ms = started.elapsed().as_millis() as u64,
                        caller = %caller,
                        holder = holder.map(ToString::to_string).as_deref().unwrap_or("unknown"),
                        "sqlite writer admission blocked"
                    );
                    lock.await
                }
            };
            let holder_id = self.record_holder(caller);
            SqliteWriteGuard {
                guard: Some(guard),
                inner: &self.0,
                holder_id,
            }
        }
    }

    /// Attempt admission without parking behind another SQLite writer.
    #[track_caller]
    pub fn try_lock(&self) -> Option<SqliteWriteGuard<'_>> {
        let guard = self.0.mutex.try_lock().ok()?;
        let holder_id = self.record_holder(std::panic::Location::caller());
        Some(SqliteWriteGuard {
            guard: Some(guard),
            inner: &self.0,
            holder_id,
        })
    }
}

#[cfg(test)]
#[path = "write_gate_tests.rs"]
mod tests;
