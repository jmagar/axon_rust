use std::future::Future;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, OwnedMutexGuard};

#[derive(Debug, Default)]
struct Inner {
    mutex: Arc<Mutex<()>>,
    holder: StdMutex<Option<&'static std::panic::Location<'static>>>,
}

#[derive(Debug, Clone, Default)]
pub struct SqliteWriteGate(Arc<Inner>);

pub struct SqliteWriteGuard {
    _guard: OwnedMutexGuard<()>,
    inner: Arc<Inner>,
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
                    let holder = *self.0.holder.lock()
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
                inner: Arc::clone(&self.0),
            }
        }
    }

    #[track_caller]
    pub fn try_lock(&self) -> Option<SqliteWriteGuard> {
        let guard = Arc::clone(&self.0.mutex).try_lock_owned().ok()?;
        *self
            .0
            .holder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(std::panic::Location::caller());
        Some(SqliteWriteGuard {
            _guard: guard,
            inner: Arc::clone(&self.0),
        })
    }
}

impl Drop for SqliteWriteGuard {
    fn drop(&mut self) {
        // Keep writer admission until its diagnostic ownership has been cleared.
        *self
            .inner
            .holder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
}

#[cfg(test)]
#[path = "write_gate_tests.rs"]
mod tests;
