use super::*;
use std::sync::Arc;

#[tokio::test]
async fn dropping_acquisition_aborts_the_task_and_releases_its_writer() {
    let gate = axon_core::sqlite::SqliteWriteGate::default();
    let writer_gate = gate.clone();
    let (ready, held) = tokio::sync::oneshot::channel();
    let task = independent_acquisition(async move {
        let _writer = writer_gate.lock().await;
        let _ = ready.send(());
        std::future::pending::<()>().await;
    });
    held.await.unwrap();
    drop(task);
    let _writer = tokio::time::timeout(Duration::from_secs(1), gate.lock())
        .await
        .expect("stream cancellation must release the provider writer");
}

#[tokio::test]
async fn buffered_acquisition_keeps_writer_moving_during_downstream_write() {
    let gate = axon_core::sqlite::SqliteWriteGate::default();
    let acquired = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let mut pending = stream::iter(0..2)
        .map(|index| {
            let gate = gate.clone();
            let acquired = acquired.clone();
            let release = release.clone();
            independent_acquisition(async move {
                if index == 0 {
                    acquired.notified().await;
                } else {
                    let _writer = gate.lock().await;
                    acquired.notify_one();
                    release.notified().await;
                }
                index
            })
        })
        .buffered(2);
    assert_eq!(pending.next().await.unwrap().unwrap(), 0);
    release.notify_one();
    let writer = tokio::time::timeout(Duration::from_secs(1), gate.lock())
        .await
        .expect("downstream writer must not depend on polling the paused acquisition stream");
    drop(writer);
    assert_eq!(pending.next().await.unwrap().unwrap(), 1);
}
