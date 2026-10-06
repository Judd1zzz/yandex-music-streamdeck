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
    async fn launch(&self, target: &LaunchTarget) -> Result<(), LaunchError>;
}

#[derive(Default)]
pub struct RealOps {
    pub launched: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

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
        sys.process(spid)
            .map(sysinfo::Process::kill)
            .unwrap_or(true)
    }

    fn is_alive_ym(&self, pid: u32) -> bool {
        let spid = sysinfo::Pid::from_u32(pid);
        let mut sys = sysinfo::System::new();
        let kind = sysinfo::ProcessRefreshKind::nothing().with_exe(sysinfo::UpdateKind::Always);
        sys.refresh_processes_specifics(sysinfo::ProcessesToUpdate::Some(&[spid]), true, kind);
        sys.process(spid).is_some_and(|p| {
            let name = p.name().to_string_lossy();
            p.exe()
                .is_some_and(|exe| crate::scan::is_ym_process(exe, &name))
        })
    }

    async fn launch(&self, target: &LaunchTarget) -> Result<(), LaunchError> {
        let res = launch_target(target).await;
        if res.is_ok()
            && let Some(flag) = &self.launched
        {
            flag.store(true, std::sync::atomic::Ordering::Release);
        }
        res
    }
}

async fn launch_target(target: &LaunchTarget) -> Result<(), LaunchError> {
    match target {
        LaunchTarget::MacBundle { fallback_app } => {
            #[cfg(target_os = "macos")]
            if let Some(app) = fallback_app.as_ref()
                && try_mac_spawn_with_bridge(app)
            {
                return Ok(());
            }
            if run_open(&["-b", YM_BUNDLE_ID]).await {
                return Ok(());
            }
            match fallback_app {
                Some(app) if open_app(app).await => Ok(()),
                _ => Err(LaunchError::Failed(
                    "open не смог запустить клиент по bundle id".to_owned(),
                )),
            }
        }
        LaunchTarget::MacApp(app) => {
            #[cfg(target_os = "macos")]
            if try_mac_spawn_with_bridge(app) {
                return Ok(());
            }
            if open_app(app).await {
                Ok(())
            } else {
                Err(LaunchError::Failed(format!(
                    "open не смог запустить {}",
                    app.display()
                )))
            }
        }
        LaunchTarget::Exe(exe) => launch_exe(exe).await,
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
    cmd.output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

async fn run_open(args: &[&str]) -> bool {
    tokio::process::Command::new("open")
        .args(args)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

async fn open_app(app: &Path) -> bool {
    let app = app.to_string_lossy();
    run_open(&[&app]).await
}

pub(crate) fn needs_elevation(raw_os_error: Option<i32>) -> bool {
    raw_os_error == Some(ERROR_ELEVATION_REQUIRED)
}

async fn launch_exe(exe: &Path) -> Result<(), LaunchError> {
    match spawn_exe(exe) {
        Ok(()) => Ok(()),
        Err(e) if needs_elevation(e.raw_os_error()) => {
            tracing::info!(
                "launch: клиенту требуются права администратора (os error 740) — показываю запрос UAC"
            );
            elevate_exe(exe).await
        }
        Err(e) => Err(LaunchError::Failed(format!(
            "не удалось запустить {}: {e}",
            exe.display()
        ))),
    }
}

fn spawn_exe(exe: &Path) -> std::io::Result<()> {
    let mut cmd = tokio::process::Command::new(exe);
    cmd.stdin(std::process::Stdio::null())
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
async fn elevate_exe(exe: &Path) -> Result<(), LaunchError> {
    let exe = exe.to_path_buf();
    match tokio::task::spawn_blocking(move || windows_elevated::shell_open(&exe)).await {
        Ok(res) => res,
        Err(e) => Err(LaunchError::Failed(format!(
            "elevate: сбой blocking-задачи: {e}"
        ))),
    }
}

#[cfg(not(windows))]
async fn elevate_exe(exe: &Path) -> Result<(), LaunchError> {
    Err(LaunchError::Failed(format!(
        "не удалось запустить {}: требуются права администратора",
        exe.display()
    )))
}

#[cfg(target_os = "macos")]
fn try_mac_spawn_with_bridge(app: &Path) -> bool {
    let Some(exe) = mac_exe_in_bundle(app) else {
        return false;
    };
    let Some(dylib) = bridge_dylib_path() else {
        return false;
    };
    if !dylib.is_file() {
        tracing::info!(
            "mac launch: ym-bridge.dylib не найден рядом с плагином ({}) — клиент запустится без моста",
            dylib.display()
        );
        return false;
    }
    if !ym_model::attest::bridge_trusted(&dylib) {
        tracing::error!(
            "mac launch: ym-bridge.dylib не прошёл проверку подписи ({}) — отказ от инъекции (fail-closed)",
            dylib.display()
        );
        return false;
    }
    if !mac_bridge_arch_ok(&exe) {
        tracing::warn!(
            "mac launch: в ym-bridge.dylib нет среза под архитектуру клиента ({}) — запуск без моста, иначе dyld завершит клиент",
            exe.display()
        );
        return false;
    }
    let mut cmd = std::process::Command::new(&exe);
    cmd.env("DYLD_INSERT_LIBRARIES", &dylib)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    match spawn_reaped(&mut cmd) {
        Ok(_) => {
            tracing::info!(
                "mac launch: клиент запущен с DYLD_INSERT_LIBRARIES={} (bridge активен)",
                dylib.display()
            );
            true
        }
        Err(e) => {
            tracing::warn!(
                "mac launch: не удалось spawn с DYLD ({}): {e}",
                exe.display()
            );
            false
        }
    }
}

#[cfg(target_os = "macos")]
fn spawn_reaped(cmd: &mut std::process::Command) -> std::io::Result<u32> {
    let mut child = cmd.spawn()?;
    let pid = child.id();
    let _ = std::thread::Builder::new()
        .name("ym-client-reaper".into())
        .spawn(move || {
            let _ = child.wait();
        });
    Ok(pid)
}

#[cfg(target_os = "macos")]
fn mac_exe_in_bundle(app: &Path) -> Option<PathBuf> {
    let dir = app.join("Contents/MacOS");
    for name in ["Яндекс Музыка", "Yandex Music", "YandexMusic"] {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    std::fs::read_dir(&dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.is_file())
}

#[cfg(target_os = "macos")]
fn bridge_dylib_path() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(|p| p.join("ym-bridge.dylib"))
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Arch {
    Arm64,
    X86_64,
}

#[cfg(target_os = "macos")]
fn host_arch() -> Arch {
    if cfg!(target_arch = "aarch64") {
        Arch::Arm64
    } else {
        Arch::X86_64
    }
}

#[cfg(target_os = "macos")]
fn arches_from_bytes(d: &[u8]) -> Vec<Arch> {
    if d.len() < 8 {
        return Vec::new();
    }
    let be = |o: usize| u32::from_be_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
    let le = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
    let arch = |ct: u32| match ct {
        0x0100_000c => Some(Arch::Arm64),
        0x0100_0007 => Some(Arch::X86_64),
        _ => None,
    };
    let mut out = Vec::new();
    let m = be(0);
    if m == 0xcafe_babe || m == 0xcafe_babf {
        let n = be(4) as usize;
        let step = if m == 0xcafe_babe { 20 } else { 32 };
        for i in 0..n {
            let base = 8 + i * step;
            if base + 4 > d.len() {
                break;
            }
            if let Some(a) = arch(be(base))
                && !out.contains(&a)
            {
                out.push(a);
            }
        }
    } else if matches!(le(0), 0xfeed_face | 0xfeed_facf)
        && let Some(a) = arch(le(4))
    {
        out.push(a);
    }
    out
}

#[cfg(target_os = "macos")]
fn macho_arches(path: &Path) -> Vec<Arch> {
    use std::io::Read;
    let mut buf = [0u8; 4096];
    std::fs::File::open(path)
        .and_then(|mut f| f.read(&mut buf))
        .map(|n| arches_from_bytes(&buf[..n]))
        .unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn client_run_arch(client: &[Arch], host: Arch) -> Option<Arch> {
    if client.contains(&host) {
        Some(host)
    } else {
        client.first().copied()
    }
}

#[cfg(target_os = "macos")]
fn bridge_covers(client: &[Arch], dylib: &[Arch], host: Arch) -> bool {
    client_run_arch(client, host).is_some_and(|a| dylib.contains(&a))
}

#[cfg(target_os = "macos")]
pub(crate) fn mac_bridge_arch_ok(exe: &Path) -> bool {
    match bridge_dylib_path() {
        Some(dylib) => bridge_covers(&macho_arches(exe), &macho_arches(&dylib), host_arch()),
        None => false,
    }
}

#[cfg(any(windows, target_os = "macos"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BridgeFile {
    Missing,
    Untrusted,
    Ready,
}

#[cfg(any(windows, target_os = "macos"))]
fn classify_bridge(path: Option<PathBuf>) -> BridgeFile {
    match path {
        Some(p) if p.is_file() => {
            if ym_model::attest::bridge_trusted(&p) {
                BridgeFile::Ready
            } else {
                BridgeFile::Untrusted
            }
        }
        _ => BridgeFile::Missing,
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn mac_bridge_file() -> BridgeFile {
    classify_bridge(bridge_dylib_path())
}

#[cfg(windows)]
pub(crate) fn win_bridge_dll_path() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.join("ym-bridge.dll")))
}

#[cfg(windows)]
pub(crate) fn win_bridge_file() -> BridgeFile {
    classify_bridge(win_bridge_dll_path())
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

    pub fn shell_open(exe: &Path) -> Result<(), LaunchError> {
        let file = wide(exe.as_os_str());
        let dir = exe.parent().map(|d| wide(d.as_os_str()));
        let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI;
        info.lpFile = file.as_ptr();
        info.lpDirectory = dir.as_ref().map_or(std::ptr::null(), |d| d.as_ptr());
        info.nShow = SW_SHOWNORMAL;
        let ok = unsafe { ShellExecuteExW(&mut info) };
        if ok != 0 {
            return Ok(());
        }
        match unsafe { GetLastError() } {
            ERROR_CANCELLED => Err(LaunchError::UserDeclined),
            code => Err(LaunchError::Failed(format!(
                "ShellExecuteExW не удался (код {code})"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[tokio::test]
    async fn successful_launch_raises_the_launched_flag_and_failed_one_does_not() {
        let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let ops = RealOps {
            launched: Some(flag.clone()),
        };
        assert!(
            ops.launch(&LaunchTarget::Exe(PathBuf::from("/nonexistent/ym-client")))
                .await
                .is_err()
        );
        assert!(
            !flag.load(std::sync::atomic::Ordering::Acquire),
            "неудачный запуск не будит сторожа внедрения"
        );
        assert!(
            ops.launch(&LaunchTarget::Exe(PathBuf::from("/usr/bin/true")))
                .await
                .is_ok()
        );
        assert!(
            flag.load(std::sync::atomic::Ordering::Acquire),
            "после запуска клиента сторож внедрения будится сразу, а не на тике"
        );
    }

    #[test]
    fn elevation_classifier() {
        assert!(needs_elevation(Some(740)));
        assert!(!needs_elevation(Some(5)));
        assert!(!needs_elevation(Some(2)));
        assert!(!needs_elevation(None));
    }

    #[test]
    fn launch_error_display() {
        assert_eq!(
            LaunchError::UserDeclined.to_string(),
            "пользователь отклонил запрос UAC"
        );
        assert_eq!(LaunchError::Failed("x".into()).to_string(), "x");
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn elevate_on_non_windows_fails_gracefully() {
        let err = elevate_exe(Path::new("/x/ym")).await.unwrap_err();
        assert!(matches!(err, LaunchError::Failed(_)));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn exited_client_is_reaped_and_never_lingers_as_a_zombie() {
        let pid = spawn_reaped(std::process::Command::new("/bin/sleep").arg("0.3")).unwrap();
        let spid = sysinfo::Pid::from_u32(pid);
        let mut sys = sysinfo::System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        assert!(
            sys.process(spid).is_some(),
            "долгоживущий System, как в watcher, видит клиента живым"
        );
        let gone = (0..150).any(|_| {
            std::thread::sleep(std::time::Duration::from_millis(20));
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            sys.process(spid).is_none()
        });
        assert!(
            gone,
            "закрытый клиент остался в таблице: незабранный зомби держит прежние exe и cmd, scan видит второй корень"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn bridge_file_classifies_missing_vs_present() {
        let dir = tempfile::tempdir().unwrap();
        let dylib = dir.path().join("ym-bridge.dylib");
        assert_eq!(classify_bridge(Some(dylib.clone())), BridgeFile::Missing);
        std::fs::write(&dylib, b"payload").unwrap();
        let expect = if ym_model::attest::bridge_trusted(&dylib) {
            BridgeFile::Ready
        } else {
            BridgeFile::Untrusted
        };
        assert_eq!(classify_bridge(Some(dylib)), expect);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macho_arches_parses_thin_and_fat_slices() {
        assert_eq!(
            arches_from_bytes(&[0xcf, 0xfa, 0xed, 0xfe, 0x0c, 0x00, 0x00, 0x01]),
            vec![Arch::Arm64]
        );
        assert_eq!(
            arches_from_bytes(&[0xcf, 0xfa, 0xed, 0xfe, 0x07, 0x00, 0x00, 0x01]),
            vec![Arch::X86_64]
        );
        let mut fat = vec![0xca, 0xfe, 0xba, 0xbe, 0x00, 0x00, 0x00, 0x02];
        fat.extend_from_slice(&[0x01, 0x00, 0x00, 0x07]);
        fat.extend_from_slice(&[0u8; 16]);
        fat.extend_from_slice(&[0x01, 0x00, 0x00, 0x0c]);
        fat.extend_from_slice(&[0u8; 16]);
        assert_eq!(arches_from_bytes(&fat), vec![Arch::X86_64, Arch::Arm64]);
        assert!(arches_from_bytes(&[0, 0]).is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn bridge_covers_gates_arch_mismatch() {
        assert!(bridge_covers(
            &[Arch::X86_64, Arch::Arm64],
            &[Arch::Arm64],
            Arch::Arm64
        ));
        assert!(!bridge_covers(&[Arch::X86_64], &[Arch::Arm64], Arch::Arm64));
        assert!(!bridge_covers(
            &[Arch::X86_64, Arch::Arm64],
            &[Arch::Arm64],
            Arch::X86_64
        ));
        assert!(bridge_covers(
            &[Arch::X86_64, Arch::Arm64],
            &[Arch::X86_64, Arch::Arm64],
            Arch::X86_64
        ));
        assert!(!bridge_covers(&[], &[Arch::Arm64], Arch::Arm64));
    }

    #[test]
    fn launch_path_never_opens_debug_port() {
        let prod = include_str!("ops.rs").split("#[cfg(test)]").next().unwrap();
        assert!(
            !prod.contains("remote-debugging"),
            "клиент запускается только под мостом — отладочный порт открывать нельзя"
        );
    }
}
