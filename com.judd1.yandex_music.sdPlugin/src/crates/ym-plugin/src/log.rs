use std::collections::VecDeque;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use file_rotate::{compression::Compression, suffix::AppendCount, ContentLimit, FileRotate};
use tracing_subscriber::fmt::writer::MakeWriter;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_BACKUPS: usize = 3;
const RING_CAP: usize = 500;

pub type LogRing = Arc<Mutex<VecDeque<String>>>;

type Rotor = FileRotate<AppendCount>;

#[derive(Clone)]
pub struct RingMaker(LogRing, Arc<AtomicBool>);

pub struct RingGuard(LogRing, Arc<AtomicBool>);

impl Write for RingGuard {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.1.load(Ordering::Acquire) {
            return Ok(buf.len());
        }
        if let Ok(s) = std::str::from_utf8(buf) {
            let line = s.trim_end_matches(['\n', '\r']).to_owned();
            if !line.is_empty() {
                let mut g = self.0.lock().expect("ring mutex");
                g.push_back(line);
                while g.len() > RING_CAP {
                    g.pop_front();
                }
            }
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for RingMaker {
    type Writer = RingGuard;
    fn make_writer(&'a self) -> Self::Writer {
        RingGuard(self.0.clone(), self.1.clone())
    }
}

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

pub fn default_log_dir() -> PathBuf {
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

fn build_subscriber(
    log_dir: &Path,
    ring: LogRing,
    enabled: Arc<AtomicBool>,
) -> impl tracing::Subscriber + Send + Sync + use<> {
    let filter = EnvFilter::try_from_env("YM_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_writer(file_maker_at(log_dir));
    let stderr_layer = fmt::layer().with_ansi(false).with_writer(io::stderr);
    let ring_layer = fmt::layer().with_ansi(false).with_target(false).with_writer(RingMaker(ring, enabled));
    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stderr_layer)
        .with(ring_layer)
}

pub type LogHandle = (LogRing, Arc<AtomicBool>);

pub fn init() -> LogHandle {
    init_at(default_log_dir())
}

pub fn init_at(log_dir: PathBuf) -> LogHandle {
    let ring: LogRing = Arc::new(Mutex::new(VecDeque::with_capacity(RING_CAP)));
    let enabled = Arc::new(AtomicBool::new(true));
    let _ = build_subscriber(&log_dir, ring.clone(), enabled.clone()).try_init();
    (ring, enabled)
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
        let ring: LogRing = Arc::new(Mutex::new(VecDeque::new()));
        let subscriber = build_subscriber(dir.path(), ring, Arc::new(AtomicBool::new(true)));
        with_default(subscriber, || {
            tracing::info!("hello-marker-98765");
        });
        let content = fs::read_to_string(dir.path().join("plugin.log")).unwrap();
        assert!(content.contains("hello-marker-98765"), "лог не попал в файл: {content:?}");
    }

    #[test]
    fn events_land_in_ring_buffer() {
        let dir = tempfile::tempdir().unwrap();
        let ring: LogRing = Arc::new(Mutex::new(VecDeque::new()));
        let subscriber = build_subscriber(dir.path(), ring.clone(), Arc::new(AtomicBool::new(true)));
        with_default(subscriber, || {
            tracing::info!("ring-marker-abc");
        });
        let lines = ring.lock().unwrap();
        assert!(lines.iter().any(|l| l.contains("ring-marker-abc")), "лог не попал в ring: {lines:?}");
    }

    #[test]
    fn real_ring_line_parses_into_at_level_text() {
        let dir = tempfile::tempdir().unwrap();
        let ring: LogRing = Arc::new(Mutex::new(VecDeque::new()));
        let subscriber = build_subscriber(dir.path(), ring.clone(), Arc::new(AtomicBool::new(true)));
        with_default(subscriber, || {
            tracing::info!("parse-me-please");
            tracing::warn!("warn-me-too");
        });
        let lines: Vec<String> = ring.lock().unwrap().iter().cloned().collect();
        let info = lines.iter().find(|l| l.contains("parse-me-please")).unwrap();
        let p = ym_core::log_lines::parse_ring_line(info, 0);
        assert_eq!(p.level.as_deref(), Some("info"), "строка из ринга: {info:?}");
        assert_eq!(p.text, "parse-me-please");
        assert!(p.at.is_some());
        let warn = lines.iter().find(|l| l.contains("warn-me-too")).unwrap();
        let w = ym_core::log_lines::parse_ring_line(warn, 0);
        assert_eq!(w.level.as_deref(), Some("warn"));
        assert_eq!(w.text, "warn-me-too");
    }

    #[test]
    fn ring_gating_respects_flag() {
        let dir = tempfile::tempdir().unwrap();
        let ring: LogRing = Arc::new(Mutex::new(VecDeque::new()));
        let flag = Arc::new(AtomicBool::new(false));
        let subscriber = build_subscriber(dir.path(), ring.clone(), flag.clone());
        with_default(subscriber, || {
            tracing::info!("off-line-hidden");
            assert!(ring.lock().unwrap().is_empty(), "при выключенном debug ring пуст");
            flag.store(true, Ordering::Release);
            tracing::info!("on-line-xyz");
        });
        let lines = ring.lock().unwrap();
        assert!(lines.iter().any(|l| l.contains("on-line-xyz")));
        assert!(!lines.iter().any(|l| l.contains("off-line-hidden")));
    }
}
