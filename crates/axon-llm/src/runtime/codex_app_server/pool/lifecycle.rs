use std::io;
use tokio::task::JoinHandle;

pub(super) fn spawn_idle_reaper(pool: &std::sync::Arc<super::CodexPool>) {
    let weak = std::sync::Arc::downgrade(pool);
    let period = pool.idle_ttl.max(std::time::Duration::from_millis(1));
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(period).await;
            let Some(pool) = weak.upgrade() else {
                return;
            };
            let expired = {
                let mut idle = pool.idle.lock().await;
                let mut expired = Vec::new();
                let mut index = 0;
                while index < idle.len() {
                    if idle[index].is_stale(pool.idle_ttl) {
                        expired.push(idle.swap_remove(index));
                    } else {
                        index += 1;
                    }
                }
                expired
            };
            // Never hold the idle queue lock while awaiting child reaping.
            for slot in expired {
                super::discard_slot(slot, "idle TTL expired").await;
            }
            let key = format!(
                "{}\0{}",
                pool.backend.codex_cmd,
                pool.backend.codex_model.as_deref().unwrap_or("")
            );
            super::POOL_MAP.remove_if(&key, |_, candidate| {
                std::sync::Arc::ptr_eq(&pool, candidate)
                    && std::sync::Arc::strong_count(candidate) == 2
                    && pool.permits.available_permits() == pool.size
                    && pool.idle.try_lock().is_ok_and(|idle| idle.is_empty())
            });
        }
    });
}

/// Own cancellation cleanup before initialization has produced a usable slot.
/// The child remains unreaped while this guard is armed, so its group identity
/// cannot be recycled. Normal cleanup disarms immediately after awaiting reaping.
pub(super) struct LifecycleGuard {
    pid: Option<u32>,
    pub(super) stderr: Option<JoinHandle<Result<Vec<u8>, io::Error>>>,
}

impl LifecycleGuard {
    pub(super) fn new(pid: Option<u32>) -> Self {
        Self { pid, stderr: None }
    }

    pub(super) fn disarm_group(&mut self) {
        self.pid = None;
    }
}

impl Drop for LifecycleGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(pid) = self.pid.take() {
            let _ = super::super::kill_process_group(pid);
        }
        if let Some(task) = self.stderr.take() {
            task.abort();
        }
        // Child::kill_on_drop remains responsible for direct-child reaping.
    }
}
