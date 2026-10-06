use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use sysinfo::{Pid, ProcessesToUpdate, Signal, System};

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
        match process_name(me) {
            Some(ours) => stop_previous(old, &ours),
            None => tracing::warn!("не удалось прочитать имя своего процесса — PID {old} из lock-файла не трогаю"),
        }
    }
    write_pid(path, me);
}

fn process_name(pid: u32) -> Option<OsString> {
    let spid = Pid::from_u32(pid);
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::Some(&[spid]), true);
    sys.process(spid).map(|p| p.name().to_os_string())
}

fn stop_previous(pid: u32, ours: &OsStr) {
    let spid = Pid::from_u32(pid);
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::Some(&[spid]), true);
    let Some(proc_) = sys.process(spid) else {
        tracing::debug!("lock-файл указывает на PID {pid}, такого процесса нет");
        return;
    };
    if proc_.name() != ours {
        tracing::info!("PID {pid} из lock-файла занят чужим процессом ({}), не трогаю", proc_.name().to_string_lossy());
        return;
    }
    tracing::info!("найден прежний экземпляр (PID {pid}), завершаю его");
    if proc_.kill_with(Signal::Term).is_none() {
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

    #[cfg(unix)]
    fn sleeper() -> std::process::Child {
        std::process::Command::new("sleep").arg("30").spawn().unwrap()
    }

    #[cfg(unix)]
    fn exited_within(child: &mut std::process::Child, wait: std::time::Duration) -> bool {
        let until = std::time::Instant::now() + wait;
        while std::time::Instant::now() < until {
            if child.try_wait().unwrap().is_some() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        false
    }

    #[cfg(unix)]
    #[test]
    fn foreign_process_behind_stale_pid_survives_startup() {
        let mut child = sleeper();
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("lock.pid");
        write_pid(&p, child.id());
        ensure_single_instance_at(&p);
        let alive = !exited_within(&mut child, std::time::Duration::from_millis(300));
        let _ = child.kill();
        let _ = child.wait();
        assert!(alive, "PID из lock-файла достался чужому процессу — запуск плагина не должен его убивать");
        assert_eq!(read_pid(&p), Some(std::process::id()), "lock перезаписан своим PID");
    }

    #[test]
    fn previous_instance_helper() {
        if std::env::var_os("YM_PREVIOUS_INSTANCE_HELPER").is_some() {
            std::thread::sleep(std::time::Duration::from_secs(30));
        }
    }

    #[cfg(unix)]
    #[test]
    fn startup_stops_previous_instance_of_the_same_binary() {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .env("YM_PREVIOUS_INSTANCE_HELPER", "1")
            .args(["--exact", "single_instance::tests::previous_instance_helper"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("lock.pid");
        write_pid(&p, child.id());
        ensure_single_instance_at(&p);
        let stopped = exited_within(&mut child, std::time::Duration::from_secs(5));
        if !stopped {
            let _ = child.kill();
            let _ = child.wait();
        }
        assert!(stopped, "прежний экземпляр того же бинаря должен быть завершён при запуске");
        assert_eq!(read_pid(&p), Some(std::process::id()));
    }
}
