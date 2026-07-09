use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use ym_model::{LaunchConfig, StateEvent};

use crate::decide::{Backoff, DecideInput, Decision, MainProc, decide};
use crate::ops::{LaunchError, LaunchTarget, PlatformOps};
use crate::probe::probe;
use crate::resolve::{Os, current_os, fs_path_kind, load_cached_exe, resolve_launch_target, store_cached_exe};
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
    pub reason: watch::Sender<Option<String>>,
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
    let mut declined = false;
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
        cycle(&mut deps, &mut sys, &mut backoff, &mut last_hint, &mut declined, kick).await;
    }
}

fn declined_gate(declined: &mut bool, kick: bool) -> bool {
    if kick {
        *declined = false;
    }
    *declined
}

fn set_reason(tx: &watch::Sender<Option<String>>, v: Option<String>) {
    tx.send_if_modified(|cur| {
        if *cur == v {
            false
        } else {
            *cur = v;
            true
        }
    });
}

async fn cycle(
    deps: &mut WatcherDeps,
    sys: &mut sysinfo::System,
    backoff: &mut Backoff,
    last_hint: &mut Option<Instant>,
    declined: &mut bool,
    kick: bool,
) {
    if deps.cdp.is_connected() {
        *declined = false;
        set_reason(&deps.reason, None);
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
    if declined_gate(declined, kick) {
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
                Some(MainProc {
                    pid: p.pid,
                    exe: p.exe.clone(),
                    debug_port: p.debug_port,
                    age_secs,
                    cmd_unreadable: p.cmd_unreadable,
                }),
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
            let res = restart_flow(deps.ops.as_ref(), pid, &target, port, deps.cdp.as_ref()).await;
            backoff.note_result(res == FlowResult::Connected);
            match res {
                FlowResult::Connected => {
                    set_reason(&deps.reason, None);
                    tracing::info!("launch: клиент перезапущен, подключение установлено");
                }
                FlowResult::Declined => {
                    *declined = true;
                    set_reason(&deps.reason, Some(REASON_DECLINED.to_owned()));
                }
                FlowResult::Failed => tracing::warn!("launch: перезапуск клиента не удался"),
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
                &fs_path_kind,
            );
            let ok = match &target {
                Some(t) => {
                    tracing::info!("launch: запускаю клиент с --remote-debugging-port={port}");
                    match deps.ops.launch(t, port).await {
                        Ok(()) => {
                            let connected = wait_connected(deps.cdp.as_ref(), CONNECT_WAIT).await;
                            if connected {
                                set_reason(&deps.reason, None);
                            }
                            connected
                        }
                        Err(LaunchError::UserDeclined) => {
                            tracing::info!(
                                "launch: пользователь отклонил запрос UAC — автозапуск приостановлен до следующего нажатия кнопки плагина"
                            );
                            *declined = true;
                            set_reason(&deps.reason, Some(REASON_DECLINED.to_owned()));
                            false
                        }
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
                    set_reason(&deps.reason, Some(REASON_NOT_FOUND.to_owned()));
                    false
                }
            };
            backoff.note_result(ok);
        }
        Decision::HintForeignPort => {
            set_reason(&deps.reason, Some(REASON_PORT_BUSY.to_owned()));
            if hint_due(last_hint) {
                tracing::warn!("launch: порт {port} занят посторонним приложением — укажите другой порт в настройках плагина");
            }
        }
        Decision::HintElevated => {
            set_reason(&deps.reason, Some(REASON_ELEVATED.to_owned()));
            if hint_due(last_hint) {
                tracing::warn!(
                    "launch: клиент, похоже, запущен от имени администратора — плагин не может им управлять. Снимите галочку «Запускать эту программу от имени администратора» в свойствах Яндекс Музыки или перезапустите клиент вручную"
                );
            }
        }
    }
}

pub const REASON_PORT_BUSY: &str = "port_busy";
pub const REASON_ELEVATED: &str = "client_elevated";
pub const REASON_DECLINED: &str = "elevation_declined";
pub const REASON_NOT_FOUND: &str = "client_not_found";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowResult {
    Connected,
    Failed,
    Declined,
}

pub async fn restart_flow(
    ops: &dyn PlatformOps,
    pid: u32,
    target: &LaunchTarget,
    port: u16,
    cdp: &dyn CdpLink,
) -> FlowResult {
    ops.quit_graceful(pid).await;
    if !wait_gone(ops, pid, QUIT_WAIT).await {
        ops.force_kill(pid).await;
        if !wait_gone(ops, pid, KILL_WAIT).await {
            tracing::warn!("launch: не удалось завершить процесс клиента (pid {pid})");
            return FlowResult::Failed;
        }
    }
    match ops.launch(target, port).await {
        Ok(()) => {
            if wait_connected(cdp, CONNECT_WAIT).await {
                FlowResult::Connected
            } else {
                FlowResult::Failed
            }
        }
        Err(LaunchError::UserDeclined) => {
            tracing::info!("launch: пользователь отклонил запрос UAC — перезапуск отменён");
            FlowResult::Declined
        }
        Err(e) => {
            tracing::warn!("launch: {e}");
            FlowResult::Failed
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
        launch_result: Result<(), LaunchError>,
    }

    impl MockOps {
        fn new(alive_polls_left: i32, alive_until_force: bool, launch_result: Result<(), LaunchError>) -> Self {
            Self {
                journal: Mutex::new(Vec::new()),
                alive_until_force: AtomicBool::new(alive_until_force),
                alive_polls_left: AtomicI32::new(alive_polls_left),
                launch_result,
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
        async fn launch(&self, target: &LaunchTarget, port: u16) -> Result<(), LaunchError> {
            self.log(format!("launch {target:?} {port}"));
            self.launch_result.clone()
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
        let ops = MockOps::new(1, false, Ok(()));
        let link = MockLink::new(9222, 2);
        assert_eq!(restart_flow(&ops, 42, &exe_target(), 9222, &link).await, FlowResult::Connected);
        let j = ops.journal();
        assert_eq!(j, vec!["quit 42".to_owned(), format!("launch {:?} 9222", exe_target())]);
    }

    #[tokio::test(start_paused = true)]
    async fn restart_escalates_to_force_kill() {
        let ops = MockOps::new(i32::MAX, true, Ok(()));
        let link = MockLink::new(9222, 0);
        assert_eq!(restart_flow(&ops, 42, &exe_target(), 9222, &link).await, FlowResult::Connected);
        let j = ops.journal();
        assert_eq!(j.first().map(String::as_str), Some("quit 42"));
        assert_eq!(j.get(1).map(String::as_str), Some("force 42"));
        assert!(j.last().unwrap().starts_with("launch"));
    }

    #[tokio::test(start_paused = true)]
    async fn restart_aborts_when_process_immortal() {
        let ops = MockOps::new(i32::MAX, false, Ok(()));
        let link = MockLink::new(9222, 0);
        assert_eq!(restart_flow(&ops, 42, &exe_target(), 9222, &link).await, FlowResult::Failed);
        let j = ops.journal();
        assert!(j.iter().all(|e| !e.starts_with("launch")));
        assert_eq!(j, vec!["quit 42", "force 42"]);
    }

    #[tokio::test(start_paused = true)]
    async fn restart_fails_when_launch_errors() {
        let ops = MockOps::new(1, false, Err(LaunchError::Failed("мок-отказ".into())));
        let link = MockLink::new(9222, 0);
        assert_eq!(restart_flow(&ops, 42, &exe_target(), 9222, &link).await, FlowResult::Failed);
    }

    #[tokio::test(start_paused = true)]
    async fn restart_declined_when_uac_rejected() {
        let ops = MockOps::new(1, false, Err(LaunchError::UserDeclined));
        let link = MockLink::new(9222, u32::MAX);
        assert_eq!(restart_flow(&ops, 42, &exe_target(), 9222, &link).await, FlowResult::Declined);
    }

    #[tokio::test(start_paused = true)]
    async fn restart_fails_when_never_connects() {
        let ops = MockOps::new(1, false, Ok(()));
        let link = MockLink::new(9222, u32::MAX);
        assert_eq!(restart_flow(&ops, 42, &exe_target(), 9222, &link).await, FlowResult::Failed);
    }

    #[test]
    fn set_reason_dedups_and_notifies_on_change() {
        let (tx, mut rx) = tokio::sync::watch::channel::<Option<String>>(None);
        set_reason(&tx, None);
        assert!(!rx.has_changed().unwrap(), "повтор того же значения не будит подписчиков");
        set_reason(&tx, Some("причина".into()));
        assert!(rx.has_changed().unwrap());
        assert_eq!(rx.borrow_and_update().clone(), Some("причина".to_owned()));
        set_reason(&tx, Some("причина".into()));
        assert!(!rx.has_changed().unwrap());
        set_reason(&tx, None);
        assert!(rx.has_changed().unwrap());
        assert_eq!(rx.borrow_and_update().clone(), None);
    }

    #[test]
    fn declined_gate_suppresses_until_kick() {
        let mut declined = true;
        assert!(declined_gate(&mut declined, false));
        assert!(declined, "без кика состояние сохраняется");
        assert!(!declined_gate(&mut declined, true));
        assert!(!declined, "кик сбрасывает отказ");
        let mut fresh = false;
        assert!(!declined_gate(&mut fresh, false));
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
            ops: Arc::new(MockOps::new(0, false, Ok(()))),
            cache_path: dir.path().join(".ym_client_path"),
            reason: tokio::sync::watch::channel(None).0,
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
