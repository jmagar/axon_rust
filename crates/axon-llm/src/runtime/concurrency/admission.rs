//! Non-preemptive execution admission: prefer interactive waiters, but grant a
//! queued background caller after at most eight consecutive interactive grants.
//! One queue owns capacity; cancellation releases both queued and granted work.

use axon_api::source::JobPriority;
use std::collections::{HashSet, VecDeque};
use std::error::Error;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::oneshot;

pub(super) struct Admission {
    limit: usize,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    next_id: u64,
    active: HashSet<u64>,
    waiting: VecDeque<Waiter>,
    interactive_streak: usize,
}

struct Waiter {
    id: u64,
    interactive: bool,
    ready: oneshot::Sender<()>,
}

/// An execution slot, released automatically even if its caller is cancelled.
pub struct CompletionPermit {
    admission: Arc<Admission>,
    id: u64,
}

impl Admission {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            limit,
            state: Mutex::new(State::default()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(super) async fn acquire(
        self: Arc<Self>,
        priority: JobPriority,
    ) -> Result<CompletionPermit, Box<dyn Error + Send + Sync>> {
        let (ready, receive) = oneshot::channel();
        let permit = {
            let mut state = self.lock();
            let id = state.next_id;
            state.next_id = id
                .checked_add(1)
                .ok_or("LLM admission identifier exhausted")?;
            let permit = CompletionPermit {
                admission: self.clone(),
                id,
            };
            state.waiting.push_back(Waiter {
                id,
                interactive: priority == JobPriority::Interactive,
                ready,
            });
            self.dispatch(&mut state);
            permit
        };
        receive
            .await
            .map_err(|_| "LLM execution admission closed")?;
        Ok(permit)
    }

    fn dispatch(&self, state: &mut State) {
        while state.active.len() < self.limit && !state.waiting.is_empty() {
            let prefer_interactive = state.interactive_streak < 8;
            let index = state
                .waiting
                .iter()
                .position(|waiter| waiter.interactive == prefer_interactive)
                .unwrap_or(0);
            let waiter = state.waiting.remove(index).expect("queued waiter exists");
            if waiter.ready.send(()).is_err() {
                continue;
            }
            state.active.insert(waiter.id);
            state.interactive_streak = if waiter.interactive {
                state.interactive_streak.saturating_add(1)
            } else {
                0
            };
        }
    }

    #[cfg(test)]
    pub(super) fn available_permits(&self) -> usize {
        self.limit - self.lock().active.len()
    }
}

impl Drop for CompletionPermit {
    fn drop(&mut self) {
        let mut state = self.admission.lock();
        state.active.remove(&self.id);
        state.waiting.retain(|waiter| waiter.id != self.id);
        self.admission.dispatch(&mut state);
    }
}
