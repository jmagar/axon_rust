use super::*;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::Notify;

type Hook = (JobId, Arc<Notify>, Arc<Notify>);
static HOOK: OnceLock<Mutex<Option<Hook>>> = OnceLock::new();

pub(crate) fn install(job_id: JobId) -> (Arc<Notify>, Arc<Notify>) {
    let entered = Arc::new(Notify::new());
    let resume = Arc::new(Notify::new());
    *HOOK.get_or_init(Default::default).lock().unwrap() =
        Some((job_id, entered.clone(), resume.clone()));
    (entered, resume)
}

pub(super) async fn pause_once(job_id: JobId) {
    let hook = {
        let mut hook = HOOK.get_or_init(Default::default).lock().unwrap();
        if hook.as_ref().is_some_and(|entry| entry.0 == job_id) {
            hook.take()
        } else {
            None
        }
    };
    if let Some((_, entered, resume)) = hook {
        entered.notify_one();
        resume.notified().await;
    }
}
