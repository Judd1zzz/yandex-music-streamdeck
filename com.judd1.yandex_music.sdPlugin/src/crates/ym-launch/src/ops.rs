use std::path::{Path, PathBuf};

use async_trait::async_trait;

pub const YM_BUNDLE_ID: &str = "ru.yandex.desktop.music";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchTarget {
    MacBundle { fallback_app: Option<PathBuf> },
    MacApp(PathBuf),
    Exe(PathBuf),
}

#[async_trait]
pub trait PlatformOps: Send + Sync {
    async fn quit_graceful(&self, pid: u32) -> bool;
    async fn force_kill(&self, pid: u32) -> bool;
    fn is_alive_ym(&self, pid: u32) -> bool;
    async fn launch(&self, target: &LaunchTarget, port: u16) -> Result<(), String>;
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

    async fn launch(&self, target: &LaunchTarget, port: u16) -> Result<(), String> {
        let flag = format!("--remote-debugging-port={port}");
        match target {
            LaunchTarget::MacBundle { fallback_app } => {
                if run_open(&["-b", YM_BUNDLE_ID, "--args", &flag]).await {
                    return Ok(());
                }
                match fallback_app {
                    Some(app) if open_app(app, &flag).await => Ok(()),
                    _ => Err("open не смог запустить клиент по bundle id".to_owned()),
                }
            }
            LaunchTarget::MacApp(app) => {
                if open_app(app, &flag).await {
                    Ok(())
                } else {
                    Err(format!("open не смог запустить {}", app.display()))
                }
            }
            LaunchTarget::Exe(exe) => spawn_exe(exe, &flag),
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

fn spawn_exe(exe: &Path, flag: &str) -> Result<(), String> {
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
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("не удалось запустить {}: {e}", exe.display()))
}
