//! Process-local admission for mutations sharing the SQLite writer boundary.

use std::future::Future;
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
    holder: StdMutex<Option<&'static std::panic::Location<'static>>>,
}

#[derive(Debug, Clone, Default)]
pub struct SqliteWriteGate(Arc<SqliteWriteGateInner>);

pub struct SqliteWriteGuard<'a> {
    _guard: tokio::sync::MutexGuard<'a, ()>,
    inner: &'a SqliteWriteGateInner,
}

impl Drop for SqliteWriteGuard<'_> {
    fn drop(&mut self) {
        *self
            .inner
            .holder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
}

/// Backward-compatible scheduler-facing name for the shared SQLite writer gate.
pub type SchedulerWriteGate = SqliteWriteGate;

impl SqliteWriteGate {
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
                    let holder = *self
                        .0
                        .holder
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    tracing::warn!(
                        waited_ms = started.elapsed().as_millis() as u64,
                        caller = %caller,
                        holder = holder.map(ToString::to_string).as_deref().unwrap_or("unknown"),
                        "sqlite writer admission blocked"
                    );
                    lock.await
                }
            };
            *self
                .0
                .holder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(caller);
            SqliteWriteGuard {
                _guard: guard,
                inner: &self.0,
            }
        }
    }

    /// Attempt admission without parking behind another SQLite writer.
    #[track_caller]
    pub fn try_lock(&self) -> Option<SqliteWriteGuard<'_>> {
        let guard = self.0.mutex.try_lock().ok()?;
        *self
            .0
            .holder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(std::panic::Location::caller());
        Some(SqliteWriteGuard {
            _guard: guard,
            inner: &self.0,
        })
    }
}
