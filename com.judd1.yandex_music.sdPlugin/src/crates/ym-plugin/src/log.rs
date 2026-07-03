use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use file_rotate::{compression::Compression, suffix::AppendCount, ContentLimit, FileRotate};
use tracing_subscriber::fmt::writer::MakeWriter;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_BACKUPS: usize = 3;

type Rotor = FileRotate<AppendCount>;

#[derive(Clone)]
pub struct FileMaker(Arc<Mutex<Rotor>>);

pub struct FileGuard(Arc<Mutex<Rotor>>);

impl Write for FileGuard {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().expect("log mutex").write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().expect("log mutex").flush()
    }
}

impl<'a> MakeWriter<'a> for FileMaker {
    type Writer = FileGuard;
    fn make_writer(&'a self) -> Self::Writer {
        FileGuard(self.0.clone())
    }
}

fn file_maker_at(log_dir: &Path) -> FileMaker {
    let _ = fs::create_dir_all(log_dir);
    let log_file = log_dir.join("plugin.log");
    #[cfg(unix)]
    let rotor = FileRotate::new(
        log_file,
        AppendCount::new(MAX_BACKUPS),
        ContentLimit::Bytes(MAX_BYTES),
        Compression::None,
        None,
    );
    #[cfg(not(unix))]
    let rotor = FileRotate::new(
        log_file,
        AppendCount::new(MAX_BACKUPS),
        ContentLimit::Bytes(MAX_BYTES),
        Compression::None,
    );
    FileMaker(Arc::new(Mutex::new(rotor)))
}

fn default_log_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("YM_LOG_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(bin) = exe.parent()
    {
        let root = bin.parent().unwrap_or(bin);
        return root.join("logs");
    }
    std::env::temp_dir().join("ym-plugin-logs")
}

fn build_subscriber(log_dir: &Path) -> impl tracing::Subscriber + Send + Sync + use<> {
    let filter = EnvFilter::try_from_env("YM_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_writer(file_maker_at(log_dir));
    let stderr_layer = fmt::layer().with_ansi(false).with_writer(io::stderr);
    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stderr_layer)
}

pub fn init() {
    init_at(default_log_dir());
}

pub fn init_at(log_dir: PathBuf) {
    let _ = build_subscriber(&log_dir).try_init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::subscriber::with_default;

    #[test]
    fn file_writer_appends_to_plugin_log() {
        let dir = tempfile::tempdir().unwrap();
        let maker = file_maker_at(dir.path());
        let mut w = maker.make_writer();
        w.write_all(b"line-xyz\n").unwrap();
        w.flush().unwrap();
        let content = fs::read_to_string(dir.path().join("plugin.log")).unwrap();
        assert!(content.contains("line-xyz"));
    }

    #[test]
    fn events_land_in_file_via_subscriber() {
        let dir = tempfile::tempdir().unwrap();
        let subscriber = build_subscriber(dir.path());
        with_default(subscriber, || {
            tracing::info!("hello-marker-98765");
        });
        let content = fs::read_to_string(dir.path().join("plugin.log")).unwrap();
        assert!(content.contains("hello-marker-98765"), "лог не попал в файл: {content:?}");
    }
}
