use std::fs;
use std::path::{Path, PathBuf};

const LOCK_NAME: &str = "ym_streamdeck_plugin.pid";

pub fn lock_path() -> PathBuf {
    if let Ok(p) = std::env::var("YM_LOCK_PATH") {
        return PathBuf::from(p);
    }
    std::env::temp_dir().join(LOCK_NAME)
}

pub fn read_pid(path: &Path) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse::<u32>().ok()
}

pub fn write_pid(path: &Path, pid: u32) {
    if let Err(e) = fs::write(path, pid.to_string()) {
        tracing::warn!("не удалось записать lock-файл {}: {e}", path.display());
    }
}

pub fn ensure_single_instance() {
    ensure_single_instance_at(&lock_path());
}

pub fn ensure_single_instance_at(path: &Path) {
    let me = std::process::id();
    if let Some(old) = read_pid(path)
        && old != me
    {
        tracing::info!("найден прежний экземпляр (PID {old}), завершаю его");
        kill_stale(old);
    }
    write_pid(path, me);
}

fn kill_stale(pid: u32) {
    use sysinfo::{Pid, ProcessesToUpdate, Signal, System};
    let spid = Pid::from_u32(pid);
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::Some(&[spid]), true);
    if let Some(proc_) = sys.process(spid)
        && proc_.kill_with(Signal::Term).is_none()
    {
        proc_.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_then_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("lock.pid");
        write_pid(&p, 4242);
        assert_eq!(read_pid(&p), Some(4242));
    }

    #[test]
    fn read_garbage_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("lock.pid");
        fs::write(&p, "not-a-pid").unwrap();
        assert_eq!(read_pid(&p), None);
        assert_eq!(read_pid(&dir.path().join("missing")), None);
    }

    #[test]
    fn ensure_writes_own_pid() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("lock.pid");
        ensure_single_instance_at(&p);
        assert_eq!(read_pid(&p), Some(std::process::id()));
    }

    #[test]
    fn lock_path_uses_temp_dir() {
        assert_eq!(lock_path().file_name().unwrap(), LOCK_NAME);
        assert!(lock_path().starts_with(std::env::temp_dir()));
    }
}
