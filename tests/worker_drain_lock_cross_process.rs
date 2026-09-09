//! Cross-process regression for the worker drain lock (`axon_rust-x4gxr.2/.3`).
//!
//! The in-process unit tests in `axon-services` verify SQLite's same-process
//! lock bookkeeping, but the feature's real requirement is *cross-process*
//! mutual exclusion: exactly one `axon jobs worker` may hold the lock at a time,
//! so a second worker (auto-spawned or manual) exits immediately while a server
//! or another worker is alive. sqlx defaults SQLite to WAL, where a read-only
//! `BEGIN EXCLUSIVE` does not take a cross-process lock — this test would fail
//! under that default and passes with the rollback-journal fix.
//!
//! It spawns a real `axon jobs worker` as a managed child (kept alive for the
//! test's duration), waits for it to acquire the lock, then runs a second
//! worker and asserts it refuses.

use std::process::Command;
use std::time::{Duration, Instant};

use axon_services::runtime::{WorkerDrainLock, drain_lock_path};

struct ManagedWorker(std::process::Child);

impl Drop for ManagedWorker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn axon_bin() -> &'static str {
    env!("CARGO_BIN_EXE_axon")
}

#[test]
fn second_worker_refuses_while_first_holds_the_lock() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let data_dir = tmp.path();
    std::fs::write(data_dir.join("config.toml"), "").expect("isolated config");

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("probe runtime");
    let lock_path = drain_lock_path(&data_dir.join("jobs.db"));
    let holder_log = data_dir.join("holder.stderr");
    // Own cleanup even if a readiness or refusal assertion fails.
    let mut holder = ManagedWorker(
        worker_command(data_dir)
            .args(["jobs", "worker", "--idle-exit-secs", "0"])
            .env("AXON_DATA_DIR", data_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::fs::File::create(&holder_log).expect("holder log"))
            .spawn()
            .expect("spawn holder worker"),
    );

    // Probe real cross-process ownership instead of guessing startup duration.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let exited = holder.0.try_wait().expect("holder status");
        assert!(
            exited.is_none(),
            "holder exited: {exited:?}; stderr: {}",
            std::fs::read_to_string(&holder_log).unwrap_or_default()
        );
        if lock_path.exists()
            && runtime
                .block_on(WorkerDrainLock::is_held(&lock_path))
                .expect("probe lock")
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "holder never acquired queue lock; stderr: {}",
            std::fs::read_to_string(&holder_log).unwrap_or_default()
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    // Second worker: must detect the held lock and exit immediately. It refuses
    // at `try_hold`, before building any runtime, so `.output()` returns fast.
    let second = worker_command(data_dir)
        .args(["jobs", "worker", "--idle-exit-secs", "1", "--json"])
        .env("AXON_DATA_DIR", data_dir)
        .output()
        .expect("run second worker");

    let stdout = String::from_utf8_lossy(&second.stdout);
    let stderr = String::from_utf8_lossy(&second.stderr);

    assert!(
        stderr.contains("jobs.worker_already_active"),
        "second worker must report queue ownership refusal; stdout: {stdout}; stderr: {stderr}"
    );
    assert!(
        !second.status.success(),
        "a worker that did not acquire the queue must not report success: {:?}",
        second.status
    );
    assert!(holder.0.try_wait().expect("holder status").is_none());
    assert!(
        runtime
            .block_on(WorkerDrainLock::is_held(&lock_path))
            .expect("holder retains lock")
    );
    drop(holder);
    let _reacquired = runtime
        .block_on(WorkerDrainLock::try_hold(&lock_path))
        .expect("reacquire released lock")
        .expect("holder death releases queue ownership");
}

/// Keep startup on temporary state and dummy service endpoints. The empty
/// queue needs no provider work, and the losing worker fails before runtime
/// construction.
fn worker_command(data_dir: &std::path::Path) -> Command {
    let mut cmd = Command::new(axon_bin());
    cmd.env("QDRANT_URL", "http://127.0.0.1:1")
        .env("TEI_URL", "http://127.0.0.1:1")
        .env("AXON_ALLOW_INCOMPATIBLE_STORE_STARTUP", "1")
        // Explicit paths keep both workers on this test's queue and prevent
        // repo/user dotenv files from supplying live configuration.
        .env("AXON_SQLITE_PATH", data_dir.join("jobs.db"))
        .env("AXON_CONFIG_PATH", data_dir.join("config.toml"))
        .env("AXON_ENV_FILE", data_dir.join("absent.env"));
    cmd
}
