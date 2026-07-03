use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use ym_model::{LaunchConfig, StateEvent};

use crate::decide::{Backoff, DecideInput, Decision, MainProc, decide};
use crate::ops::{LaunchTarget, PlatformOps};
use crate::probe::probe;
use crate::resolve::{Os, current_os, load_cached_exe, resolve_launch_target, store_cached_exe};
use crate::scan::{MainPick, app_bundle_from_exe, main_client, scan};

pub trait CdpLink: Send + Sync {
    fn port(&self) -> u16;
    fn set_port(&self, port: u16);
    fn is_connected(&self) -> bool;
}

pub struct WatcherDeps {
    pub cdp: Arc<dyn CdpLink>,
    pub events: broadcast::Receiver<StateEvent>,
    pub any_local: Arc<AtomicBool>,
    pub config: watch::Receiver<LaunchConfig>,
    pub kick: mpsc::Receiver<()>,
    pub ops: Arc<dyn PlatformOps>,
    pub cache_path: PathBuf,
    pub shutdown: CancellationToken,
}

const TICK: Duration = Duration::from_secs(15);
const HINT_SPACING: Duration = Duration::from_secs(60);
const CONNECT_WAIT: Duration = Duration::from_secs(20);
const QUIT_WAIT: Duration = Duration::from_secs(10);
const KILL_WAIT: Duration = Duration::from_secs(5);

pub fn spawn(deps: WatcherDeps) -> JoinHandle<()> {
    tokio::spawn(run(deps))
}

async fn run(mut deps: WatcherDeps) {
    let mut sys = sysinfo::System::new();
    let mut backoff = Backoff::default();
    let mut last_hint: Option<Instant> = None;
    loop {
        let kick = tokio::select! {
            _ = deps.shutdown.cancelled() => return,
            _ = tokio::time::sleep(TICK) => false,
            k = deps.kick.recv() => match k {
                Some(()) => {
                    backoff.note_kick();
                    true
                }
                None => return,
            },
            ev = deps.events.recv() => match ev {
                Ok(StateEvent::Connection(false)) => false,
                Ok(_) => continue,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return,
            },
            c = deps.config.changed() => match c {
                Ok(()) => false,
                Err(_) => return,
            },
        };
        cycle(&mut deps, &mut sys, &mut backoff, &mut last_hint, kick).await;
    }
}

async fn cycle(
    deps: &mut WatcherDeps,
    sys: &mut sysinfo::System,
    backoff: &mut Backoff,
    last_hint: &mut Option<Instant>,
    kick: bool,
) {
    if deps.cdp.is_connected() {
        return;
    }
    let cfg = deps.config.borrow().clone();
    if !cfg.enabled {
        return;
    }
    let any_local = deps.any_local.load(Ordering::Acquire);
    if !any_local && !kick {
        return;
    }
    let port = deps.cdp.port();
    let port_status = probe(port).await;
    let procs = scan(sys);
    let now_epoch = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (main_proc, ambiguous) = match main_client(&procs) {
        MainPick::One(p) => {
            if load_cached_exe(&deps.cache_path).as_deref() != Some(p.exe.as_path()) {
                store_cached_exe(&deps.cache_path, &p.exe);
            }
            let age_secs = now_epoch.saturating_sub(p.start_time);
            (
                Some(MainProc { pid: p.pid, exe: p.exe.clone(), debug_port: p.debug_port, age_secs }),
                false,
            )
        }
        MainPick::NotFound => (None, false),
        MainPick::Ambiguous => (None, true),
    };
    let other_port_status = match main_proc.as_ref().and_then(|m| m.debug_port) {
        Some(p) if p != port => Some(probe(p).await),
        _ => None,
    };
    let input = DecideInput {
        enabled: true,
        connected: false,
        any_local,
        kick,
        port,
        port_status,
        other_port_status,
        main_proc,
        ambiguous,
        cooldown_ok: backoff.cooldown_ok(kick, Instant::now()),
    };
    match decide(&input) {
        Decision::Nothing => {
            if ambiguous && hint_due(last_hint) {
                tracing::warn!("launch: найдено несколько экземпляров клиента — авто-перезапуск пропущен");
            }
        }
        Decision::AdoptPort(p) => {
            tracing::info!("launch: клиент уже запущен с --remote-debugging-port={p} — переключаю CDP на этот порт");
            deps.cdp.set_port(p);
            wait_connected(deps.cdp.as_ref(), CONNECT_WAIT).await;
        }
        Decision::Restart { pid, exe } => {
            tracing::info!("launch: клиент запущен без порта отладки — перезапускаю с --remote-debugging-port={port}");
            backoff.note_attempt(Instant::now());
            let target = restart_target(&exe);
            let ok = restart_flow(deps.ops.as_ref(), pid, &target, port, deps.cdp.as_ref()).await;
            backoff.note_result(ok);
            if ok {
                tracing::info!("launch: клиент перезапущен, подключение установлено");
            } else {
                tracing::warn!("launch: перезапуск клиента не удался");
            }
        }
        Decision::Launch => {
            backoff.note_attempt(Instant::now());
            let cached = load_cached_exe(&deps.cache_path);
            let target = resolve_launch_target(
                current_os(),
                cfg.client_exe_path.as_deref(),
                cached.as_deref(),
                local_app_data().as_deref(),
                &|p| p.exists(),
            );
            let ok = match &target {
                Some(t) => {
                    tracing::info!("launch: запускаю клиент с --remote-debugging-port={port}");
                    match deps.ops.launch(t, port).await {
                        Ok(()) => wait_connected(deps.cdp.as_ref(), CONNECT_WAIT).await,
                        Err(e) => {
                            tracing::warn!("launch: {e}");
                            false
                        }
                    }
                }
                None => {
                    tracing::warn!(
                        "launch: клиент не найден — установите десктоп-версию с music.yandex.ru/download (версия из Microsoft Store не поддерживается)"
                    );
                    false
                }
            };
            backoff.note_result(ok);
        }
        Decision::HintForeignPort => {
            if hint_due(last_hint) {
                tracing::warn!("launch: порт {port} занят посторонним приложением — укажите другой порт в настройках плагина");
            }
        }
    }
}

fn hint_due(last_hint: &mut Option<Instant>) -> bool {
    let now = Instant::now();
    if last_hint.is_none_or(|t| now.duration_since(t) >= HINT_SPACING) {
        *last_hint = Some(now);
        true
    } else {
        false
    }
}

fn local_app_data() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

fn restart_target(exe: &Path) -> LaunchTarget {
    match current_os() {
        Os::Mac => app_bundle_from_exe(exe)
            .map_or_else(|| LaunchTarget::Exe(exe.to_path_buf()), LaunchTarget::MacApp),
        Os::Windows => LaunchTarget::Exe(exe.to_path_buf()),
    }
}

pub async fn restart_flow(
    ops: &dyn PlatformOps,
    pid: u32,
    target: &LaunchTarget,
    port: u16,
    cdp: &dyn CdpLink,
) -> bool {
    ops.quit_graceful(pid).await;
    if !wait_gone(ops, pid, QUIT_WAIT).await {
        ops.force_kill(pid).await;
        if !wait_gone(ops, pid, KILL_WAIT).await {
            tracing::warn!("launch: не удалось завершить процесс клиента (pid {pid})");
            return false;
        }
    }
    match ops.launch(target, port).await {
        Ok(()) => wait_connected(cdp, CONNECT_WAIT).await,
        Err(e) => {
            tracing::warn!("launch: {e}");
            false
        }
    }
}

pub async fn wait_gone(ops: &dyn PlatformOps, pid: u32, limit: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + limit;
    loop {
        if !ops.is_alive_ym(pid) {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
}

pub async fn wait_connected(cdp: &dyn CdpLink, limit: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + limit;
    loop {
        if cdp.is_connected() {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU16, AtomicU32};

    struct MockOps {
        journal: Mutex<Vec<String>>,
        alive_until_force: AtomicBool,
        alive_polls_left: AtomicI32,
        launch_ok: bool,
    }

    impl MockOps {
        fn new(alive_polls_left: i32, alive_until_force: bool, launch_ok: bool) -> Self {
            Self {
                journal: Mutex::new(Vec::new()),
                alive_until_force: AtomicBool::new(alive_until_force),
                alive_polls_left: AtomicI32::new(alive_polls_left),
                launch_ok,
            }
        }
        fn log(&self, s: impl Into<String>) {
            self.journal.lock().unwrap().push(s.into());
        }
        fn journal(&self) -> Vec<String> {
            self.journal.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl PlatformOps for MockOps {
        async fn quit_graceful(&self, pid: u32) -> bool {
            self.log(format!("quit {pid}"));
            true
        }
        async fn force_kill(&self, pid: u32) -> bool {
            self.log(format!("force {pid}"));
            if self.alive_until_force.load(Ordering::SeqCst) {
                self.alive_polls_left.store(0, Ordering::SeqCst);
            }
            true
        }
        fn is_alive_ym(&self, _pid: u32) -> bool {
            self.alive_polls_left.fetch_sub(1, Ordering::SeqCst) > 0
        }
        async fn launch(&self, target: &LaunchTarget, port: u16) -> Result<(), String> {
            self.log(format!("launch {target:?} {port}"));
            if self.launch_ok { Ok(()) } else { Err("мок-отказ".to_owned()) }
        }
    }

    struct MockLink {
        port: AtomicU16,
        connect_after_polls: AtomicU32,
    }

    impl MockLink {
        fn new(port: u16, connect_after_polls: u32) -> Self {
            Self { port: AtomicU16::new(port), connect_after_polls: AtomicU32::new(connect_after_polls) }
        }
    }

    impl CdpLink for MockLink {
        fn port(&self) -> u16 {
            self.port.load(Ordering::SeqCst)
        }
        fn set_port(&self, port: u16) {
            self.port.store(port, Ordering::SeqCst);
        }
        fn is_connected(&self) -> bool {
            let left = self.connect_after_polls.load(Ordering::SeqCst);
            if left == 0 {
                return true;
            }
            self.connect_after_polls.store(left - 1, Ordering::SeqCst);
            false
        }
    }

    fn exe_target() -> LaunchTarget {
        LaunchTarget::Exe(PathBuf::from("/x/ym"))
    }

    #[tokio::test(start_paused = true)]
    async fn restart_happy_path_graceful() {
        let ops = MockOps::new(1, false, true);
        let link = MockLink::new(9222, 2);
        assert!(restart_flow(&ops, 42, &exe_target(), 9222, &link).await);
        let j = ops.journal();
        assert_eq!(j, vec!["quit 42".to_owned(), format!("launch {:?} 9222", exe_target())]);
    }

    #[tokio::test(start_paused = true)]
    async fn restart_escalates_to_force_kill() {
        let ops = MockOps::new(i32::MAX, true, true);
        let link = MockLink::new(9222, 0);
        assert!(restart_flow(&ops, 42, &exe_target(), 9222, &link).await);
        let j = ops.journal();
        assert_eq!(j.first().map(String::as_str), Some("quit 42"));
        assert_eq!(j.get(1).map(String::as_str), Some("force 42"));
        assert!(j.last().unwrap().starts_with("launch"));
    }

    #[tokio::test(start_paused = true)]
    async fn restart_aborts_when_process_immortal() {
        let ops = MockOps::new(i32::MAX, false, true);
        let link = MockLink::new(9222, 0);
        assert!(!restart_flow(&ops, 42, &exe_target(), 9222, &link).await);
        let j = ops.journal();
        assert!(j.iter().all(|e| !e.starts_with("launch")));
        assert_eq!(j, vec!["quit 42", "force 42"]);
    }

    #[tokio::test(start_paused = true)]
    async fn restart_fails_when_launch_errors() {
        let ops = MockOps::new(1, false, false);
        let link = MockLink::new(9222, 0);
        assert!(!restart_flow(&ops, 42, &exe_target(), 9222, &link).await);
    }

    #[tokio::test(start_paused = true)]
    async fn restart_fails_when_never_connects() {
        let ops = MockOps::new(1, false, true);
        let link = MockLink::new(9222, u32::MAX);
        assert!(!restart_flow(&ops, 42, &exe_target(), 9222, &link).await);
    }

    #[tokio::test(start_paused = true)]
    async fn wait_connected_polls_until_flip() {
        let link = MockLink::new(9222, 3);
        assert!(wait_connected(&link, Duration::from_secs(20)).await);
        let never = MockLink::new(9222, u32::MAX);
        assert!(!wait_connected(&never, Duration::from_secs(20)).await);
    }

    #[tokio::test(start_paused = true)]
    async fn watcher_exits_on_shutdown() {
        let dir = tempfile::tempdir().unwrap();
        let (bus, _) = tokio::sync::broadcast::channel(16);
        let (_cfg_tx, cfg_rx) = tokio::sync::watch::channel(ym_model::LaunchConfig::default());
        let (_kick_tx, kick_rx) = tokio::sync::mpsc::channel(4);
        let shutdown = CancellationToken::new();
        let deps = WatcherDeps {
            cdp: Arc::new(MockLink::new(9222, u32::MAX)),
            events: bus.subscribe(),
            any_local: Arc::new(AtomicBool::new(false)),
            config: cfg_rx,
            kick: kick_rx,
            ops: Arc::new(MockOps::new(0, false, true)),
            cache_path: dir.path().join(".ym_client_path"),
            shutdown: shutdown.clone(),
        };
        let task = spawn(deps);
        tokio::time::sleep(Duration::from_millis(50)).await;
        shutdown.cancel();
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("watcher должен завершиться по shutdown")
            .unwrap();
    }

    #[test]
    fn restart_target_derives_bundle_on_mac() {
        let t = restart_target(Path::new("/Applications/Яндекс Музыка.app/Contents/MacOS/Яндекс Музыка"));
        if current_os() == Os::Mac {
            assert_eq!(t, LaunchTarget::MacApp(PathBuf::from("/Applications/Яндекс Музыка.app")));
        } else {
            assert!(matches!(t, LaunchTarget::Exe(_)));
        }
        let t = restart_target(Path::new("/usr/local/bin/ym"));
        assert!(matches!(t, LaunchTarget::Exe(_)));
    }
}
