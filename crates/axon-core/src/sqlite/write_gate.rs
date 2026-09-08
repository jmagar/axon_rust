use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, OwnedMutexGuard};

#[derive(Debug, Default)]
struct Inner {
    mutex: Arc<Mutex<()>>,
    next_holder_id: AtomicU64,
    holder: StdMutex<Option<(u64, &'static std::panic::Location<'static>)>>,
}

#[derive(Debug, Clone, Default)]
pub struct SqliteWriteGate(Arc<Inner>);

pub struct SqliteWriteGuard {
    guard: Option<OwnedMutexGuard<()>>,
    inner: Arc<Inner>,
    holder_id: u64,
}

impl SqliteWriteGate {
    #[track_caller]
    pub fn lock(&self) -> impl Future<Output = SqliteWriteGuard> + '_ {
        let caller = std::panic::Location::caller();
        async move {
            let started = Instant::now();
            let lock = Arc::clone(&self.0.mutex).lock_owned();
            tokio::pin!(lock);
            let guard = tokio::select! {
                guard = &mut lock => guard,
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    let holder = self.0.holder.lock()
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
            let holder_id = self.0.next_holder_id.fetch_add(1, Ordering::Relaxed);
            *self
                .0
                .holder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((holder_id, caller));
            SqliteWriteGuard {
                guard: Some(guard),
                inner: Arc::clone(&self.0),
                holder_id,
            }
        }
    }

    #[track_caller]
    pub fn try_lock(&self) -> Option<SqliteWriteGuard> {
        let guard = Arc::clone(&self.0.mutex).try_lock_owned().ok()?;
        let holder_id = self.0.next_holder_id.fetch_add(1, Ordering::Relaxed);
        *self
            .0
            .holder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some((holder_id, std::panic::Location::caller()));
        Some(SqliteWriteGuard {
            guard: Some(guard),
            inner: Arc::clone(&self.0),
            holder_id,
        })
    }
}

impl Drop for SqliteWriteGuard {
    fn drop(&mut self) {
        drop(self.guard.take());
        let mut holder = self
            .inner
            .holder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if holder.is_some_and(|(id, _)| id == self.holder_id) {
            *holder = None;
        }
    }
}

#[cfg(test)]
#[path = "write_gate_tests.rs"]
mod tests;
