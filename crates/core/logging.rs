use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};
use tracing_subscriber::fmt::writer::MakeWriter;

#[derive(Debug)]
struct SizeRotatingFile {
    path: PathBuf,
    file: std::fs::File,
    current_size: u64,
    max_bytes: u64,
    max_files_total: usize,
}

impl SizeRotatingFile {
    fn new(path: PathBuf, max_bytes: u64, max_files_total: usize) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        let current_size = file.metadata().map(|m| m.len()).unwrap_or(0);
        Ok(Self {
            path,
            file,
            current_size,
            max_bytes,
            max_files_total: max_files_total.max(1),
        })
    }

    fn indexed_path(&self, idx: usize) -> PathBuf {
        let mut s = self.path.as_os_str().to_owned();
        s.push(format!(".{idx}"));
        PathBuf::from(s)
    }

    // TODO(PERF-MED-4): rotate_if_needed performs blocking fs::rename and File::create.
    // These run inside Mutex<SizeRotatingFile> via tracing's sync Write trait — NOT in an
    // async task. If tracing ever moves to an async writer, wrap these in spawn_blocking.
    // Risk: on slow NFS mounts, rotation could briefly block the async runtime thread that
    // happens to be logging. Mitigation: keep log files on local disk (the default).
    fn rotate_if_needed(&mut self, incoming_len: usize) -> io::Result<()> {
        if self.max_bytes == 0 {
            return Ok(());
        }
        if self.current_size.saturating_add(incoming_len as u64) <= self.max_bytes {
            return Ok(());
        }

        self.file.flush()?;
        let rotated_slots = self.max_files_total.saturating_sub(1);
        if rotated_slots == 0 {
            self.file = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&self.path)?;
            self.current_size = 0;
            return Ok(());
        }

        let oldest = self.indexed_path(rotated_slots);
        if oldest.exists() {
            let _ = std::fs::remove_file(oldest);
        }

        for idx in (1..rotated_slots).rev() {
            let src = self.indexed_path(idx);
            let dst = self.indexed_path(idx + 1);
            if src.exists() {
                std::fs::rename(&src, &dst)?;
            }
        }

        if self.path.exists() {
            std::fs::rename(&self.path, self.indexed_path(1))?;
        }

        self.file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        self.current_size = 0;
        Ok(())
    }
}

#[derive(Clone)]
struct SizeRotateMakeWriter {
    inner: Arc<Mutex<SizeRotatingFile>>,
}

struct SizeRotateWriterGuard {
    inner: Arc<Mutex<SizeRotatingFile>>,
}

impl<'a> MakeWriter<'a> for SizeRotateMakeWriter {
    type Writer = SizeRotateWriterGuard;

    fn make_writer(&'a self) -> Self::Writer {
        SizeRotateWriterGuard {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Write for SizeRotateWriterGuard {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| io::Error::other("log writer mutex poisoned"))?;
        inner.rotate_if_needed(buf.len())?;
        let written = inner.file.write(buf)?;
        inner.current_size = inner.current_size.saturating_add(written as u64);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| io::Error::other("log writer mutex poisoned"))?;
        inner.file.flush()
    }
}

pub fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::prelude::*;

    let json_log_file = std::env::var("AXON_LOG_FILE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "logs/axon.log".to_string());
    let max_bytes = std::env::var("AXON_LOG_MAX_BYTES")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10 * 1024 * 1024)
        .max(1024);
    let max_files_total = std::env::var("AXON_LOG_MAX_FILES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(3)
        .max(1);

    // Browserless (CDP proxy) sends non-standard session management frames over
    // the same WebSocket as standard CDP traffic. chromiumoxide's Message<T> is
    // an untagged enum with only two variants — Response (needs `id`) and Event
    // (needs `method`) — so any Browserless-specific frame fails serde and the
    // chromey library logs it at ERROR. The frames are gracefully dropped and
    // crawling succeeds; the error level is a library misclassification.
    // Suppress by default. Because this directive is added in code, same-target
    // RUST_LOG overrides are not guaranteed to win under tracing-subscriber 0.3.
    // To force-enable this target, change/remove this directive in code.
    const SUPPRESS_CDP_NOISE: &str = "chromiumoxide::conn::raw_ws::parse_errors=off";

    let console_filter = EnvFilter::try_from_default_env()
        .map(|f| {
            f.add_directive(
                SUPPRESS_CDP_NOISE
                    .parse()
                    .expect("hard-coded directive is valid"),
            )
        })
        .unwrap_or_else(|_| EnvFilter::new(format!("warn,{SUPPRESS_CDP_NOISE}")));

    let file_filter = EnvFilter::try_from_default_env()
        .map(|f| {
            f.add_directive(
                SUPPRESS_CDP_NOISE
                    .parse()
                    .expect("hard-coded directive is valid"),
            )
        })
        .unwrap_or_else(|_| EnvFilter::new(format!("info,{SUPPRESS_CDP_NOISE}")));

    let log_path = PathBuf::from(json_log_file);

    let console_layer = tracing_subscriber::fmt::layer()
        .with_writer(io::stderr)
        .with_filter(console_filter);

    match SizeRotatingFile::new(log_path.clone(), max_bytes, max_files_total) {
        Ok(rotating) => {
            let file_writer = SizeRotateMakeWriter {
                inner: Arc::new(Mutex::new(rotating)),
            };
            let json_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_ansi(false)
                .with_writer(file_writer)
                .with_filter(file_filter);
            tracing_subscriber::registry()
                .with(console_layer)
                .with(json_layer)
                .init();
        }
        Err(err) => {
            eprintln!(
                "warning: failed to initialize file logging at {}: {} (continuing with console logs only)",
                log_path.display(),
                err
            );
            tracing_subscriber::registry().with(console_layer).init();
        }
    }
}

pub fn log_info(msg: &str) {
    info!("{}", msg);
}

pub fn log_warn(msg: &str) {
    warn!("{}", msg);
}

pub fn log_done(msg: &str) {
    info!(status = "done", "{}", msg);
}

#[cfg(test)]
mod tests {
    use super::SizeRotatingFile;
    use std::io::Write;
    use tempfile::tempdir;

    fn write_event(rotating: &mut SizeRotatingFile, event: &str) {
        let bytes = event.as_bytes();
        rotating
            .rotate_if_needed(bytes.len())
            .expect("rotate check should succeed");
        rotating
            .file
            .write_all(bytes)
            .expect("write should succeed");
        rotating.current_size = rotating.current_size.saturating_add(bytes.len() as u64);
        rotating.file.flush().expect("flush should succeed");
    }

    #[test]
    fn size_rotation_keeps_max_three_files() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("axon.log");
        let mut rotating = SizeRotatingFile::new(path.clone(), 32, 3).expect("init");

        for i in 0..20 {
            write_event(&mut rotating, &format!("event-{i:02}-payload\n"));
        }

        let entries = std::fs::read_dir(dir.path()).expect("read_dir").count();
        assert_eq!(entries, 3);
        assert!(path.exists());
        assert!(dir.path().join("axon.log.1").exists());
        assert!(dir.path().join("axon.log.2").exists());
        assert!(!dir.path().join("axon.log.3").exists());
    }

    #[test]
    fn writer_guard_write_all_updates_size_counter() {
        use super::{SizeRotateMakeWriter, SizeRotatingFile};
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("axon.log");
        let rotating = SizeRotatingFile::new(path.clone(), 1024, 3).expect("init");
        let make_writer = SizeRotateMakeWriter {
            inner: std::sync::Arc::new(std::sync::Mutex::new(rotating)),
        };
        use tracing_subscriber::fmt::writer::MakeWriter;
        let mut guard = make_writer.make_writer();
        let payload = b"hello-guard\n";
        guard.write_all(payload).expect("write_all should succeed");
        guard.flush().expect("flush should succeed");
        // Verify write reached the file.
        let content = std::fs::read_to_string(&path).expect("read file");
        assert_eq!(content, "hello-guard\n");
        // Verify the size counter was incremented — drives the rotation trigger.
        drop(guard);
        let inner = make_writer.inner.lock().unwrap();
        assert_eq!(inner.current_size, payload.len() as u64);
    }

    #[test]
    fn size_rotation_preserves_newest_in_primary_file() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("axon.log");
        let mut rotating = SizeRotatingFile::new(path.clone(), 10, 3).expect("init");

        write_event(&mut rotating, "A12345\n");
        write_event(&mut rotating, "B12345\n");
        write_event(&mut rotating, "C12345\n");
        write_event(&mut rotating, "D12345\n");

        let primary = std::fs::read_to_string(&path).expect("read primary");
        let rotated_1 = std::fs::read_to_string(dir.path().join("axon.log.1")).expect("read .1");
        let rotated_2 = std::fs::read_to_string(dir.path().join("axon.log.2")).expect("read .2");

        assert_eq!(primary, "D12345\n");
        assert_eq!(rotated_1, "C12345\n");
        assert_eq!(rotated_2, "B12345\n");
    }
}
