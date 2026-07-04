use std::path::{Path, PathBuf};

use async_trait::async_trait;

pub const YM_BUNDLE_ID: &str = "ru.yandex.desktop.music";

const ERROR_ELEVATION_REQUIRED: i32 = 740;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchTarget {
    MacBundle { fallback_app: Option<PathBuf> },
    MacApp(PathBuf),
    Exe(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchError {
    UserDeclined,
    Failed(String),
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UserDeclined => write!(f, "пользователь отклонил запрос UAC"),
            Self::Failed(e) => write!(f, "{e}"),
        }
    }
}

#[async_trait]
pub trait PlatformOps: Send + Sync {
    async fn quit_graceful(&self, pid: u32) -> bool;
    async fn force_kill(&self, pid: u32) -> bool;
    fn is_alive_ym(&self, pid: u32) -> bool;
    async fn launch(&self, target: &LaunchTarget, port: u16) -> Result<(), LaunchError>;
}

pub struct RealOps;

#[async_trait]
impl PlatformOps for RealOps {
    async fn quit_graceful(&self, pid: u32) -> bool {
        #[cfg(windows)]
        {
            taskkill_graceful(pid).await
        }
        #[cfg(not(windows))]
        {
            signal_term(pid)
        }
    }

    async fn force_kill(&self, pid: u32) -> bool {
        let spid = sysinfo::Pid::from_u32(pid);
        let mut sys = sysinfo::System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[spid]), true);
        sys.process(spid).map(sysinfo::Process::kill).unwrap_or(true)
    }

    fn is_alive_ym(&self, pid: u32) -> bool {
        let spid = sysinfo::Pid::from_u32(pid);
        let mut sys = sysinfo::System::new();
        let kind = sysinfo::ProcessRefreshKind::nothing().with_exe(sysinfo::UpdateKind::Always);
        sys.refresh_processes_specifics(sysinfo::ProcessesToUpdate::Some(&[spid]), true, kind);
        sys.process(spid).is_some_and(|p| {
            let name = p.name().to_string_lossy();
            p.exe().is_some_and(|exe| crate::scan::is_ym_process(exe, &name))
        })
    }

    async fn launch(&self, target: &LaunchTarget, port: u16) -> Result<(), LaunchError> {
        let flag = format!("--remote-debugging-port={port}");
        match target {
            LaunchTarget::MacBundle { fallback_app } => {
                if run_open(&["-b", YM_BUNDLE_ID, "--args", &flag]).await {
                    return Ok(());
                }
                match fallback_app {
                    Some(app) if open_app(app, &flag).await => Ok(()),
                    _ => Err(LaunchError::Failed("open не смог запустить клиент по bundle id".to_owned())),
                }
            }
            LaunchTarget::MacApp(app) => {
                if open_app(app, &flag).await {
                    Ok(())
                } else {
                    Err(LaunchError::Failed(format!("open не смог запустить {}", app.display())))
                }
            }
            LaunchTarget::Exe(exe) => launch_exe(exe, &flag).await,
        }
    }
}

#[cfg(not(windows))]
fn signal_term(pid: u32) -> bool {
    let spid = sysinfo::Pid::from_u32(pid);
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[spid]), true);
    sys.process(spid)
        .and_then(|p| p.kill_with(sysinfo::Signal::Term))
        .unwrap_or(false)
}

#[cfg(windows)]
async fn taskkill_graceful(pid: u32) -> bool {
    let mut cmd = tokio::process::Command::new("taskkill");
    cmd.args(["/PID", &pid.to_string()]);
    cmd.creation_flags(0x0800_0000);
    cmd.output().await.map(|o| o.status.success()).unwrap_or(false)
}

async fn run_open(args: &[&str]) -> bool {
    tokio::process::Command::new("open")
        .args(args)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

async fn open_app(app: &Path, flag: &str) -> bool {
    let app = app.to_string_lossy();
    run_open(&[&app, "--args", flag]).await
}

pub(crate) fn needs_elevation(raw_os_error: Option<i32>) -> bool {
    raw_os_error == Some(ERROR_ELEVATION_REQUIRED)
}

async fn launch_exe(exe: &Path, flag: &str) -> Result<(), LaunchError> {
    match spawn_exe(exe, flag) {
        Ok(()) => Ok(()),
        Err(e) if needs_elevation(e.raw_os_error()) => {
            tracing::info!(
                "launch: клиенту требуются права администратора (os error 740) — показываю запрос UAC"
            );
            elevate_exe(exe, flag).await
        }
        Err(e) => Err(LaunchError::Failed(format!("не удалось запустить {}: {e}", exe.display()))),
    }
}

fn spawn_exe(exe: &Path, flag: &str) -> std::io::Result<()> {
    let mut cmd = tokio::process::Command::new(exe);
    cmd.arg(flag)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Some(dir) = exe.parent() {
        cmd.current_dir(dir);
    }
    #[cfg(windows)]
    cmd.creation_flags(0x0000_0008 | 0x0000_0200);
    cmd.spawn().map(|_| ())
}

#[cfg(windows)]
async fn elevate_exe(exe: &Path, flag: &str) -> Result<(), LaunchError> {
    let exe = exe.to_path_buf();
    let flag = flag.to_owned();
    match tokio::task::spawn_blocking(move || windows_elevated::shell_open(&exe, &flag)).await {
        Ok(res) => res,
        Err(e) => Err(LaunchError::Failed(format!("elevate: сбой blocking-задачи: {e}"))),
    }
}

#[cfg(not(windows))]
async fn elevate_exe(exe: &Path, _flag: &str) -> Result<(), LaunchError> {
    Err(LaunchError::Failed(format!(
        "не удалось запустить {}: требуются права администратора",
        exe.display()
    )))
}

#[cfg(windows)]
mod windows_elevated {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows_sys::Win32::Foundation::{ERROR_CANCELLED, GetLastError};
    use windows_sys::Win32::UI::Shell::{
        SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    use super::LaunchError;

    fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    }

    pub fn shell_open(exe: &Path, flag: &str) -> Result<(), LaunchError> {
        let file = wide(exe.as_os_str());
        let params = wide(std::ffi::OsStr::new(flag));
        let dir = exe.parent().map(|d| wide(d.as_os_str()));
        let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI;
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.lpDirectory = dir.as_ref().map_or(std::ptr::null(), |d| d.as_ptr());
        info.nShow = SW_SHOWNORMAL;
        let ok = unsafe { ShellExecuteExW(&mut info) };
        if ok != 0 {
            return Ok(());
        }
        match unsafe { GetLastError() } {
            ERROR_CANCELLED => Err(LaunchError::UserDeclined),
            code => Err(LaunchError::Failed(format!("ShellExecuteExW не удался (код {code})"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elevation_classifier() {
        assert!(needs_elevation(Some(740)));
        assert!(!needs_elevation(Some(5)));
        assert!(!needs_elevation(Some(2)));
        assert!(!needs_elevation(None));
    }

    #[test]
    fn launch_error_display() {
        assert_eq!(LaunchError::UserDeclined.to_string(), "пользователь отклонил запрос UAC");
        assert_eq!(LaunchError::Failed("x".into()).to_string(), "x");
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn elevate_on_non_windows_fails_gracefully() {
        let err = elevate_exe(Path::new("/x/ym"), "--flag").await.unwrap_err();
        assert!(matches!(err, LaunchError::Failed(_)));
    }
}
