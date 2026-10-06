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
use crate::resolve::{
    Os, current_os, fs_path_kind, load_cached_exe, resolve_launch_target, store_cached_exe,
};
use crate::scan::{MainPick, app_bundle_from_exe, main_client, scan};

pub trait CdpLink: Send + Sync {
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
const RECONNECT_GRACE: Duration = Duration::from_secs(30);
const HINT_SPACING: Duration = Duration::from_secs(60);
const CONNECT_WAIT: Duration = Duration::from_secs(20);
const RESTART_CONFIRM: Duration = Duration::from_secs(5);
const QUIT_WAIT: Duration = Duration::from_secs(10);
const QUIT_WAIT_HUNG: Duration = Duration::from_secs(2);
const KILL_WAIT: Duration = Duration::from_secs(5);

pub fn spawn(deps: WatcherDeps) -> JoinHandle<()> {
    tokio::spawn(run(deps))
}

async fn run(mut deps: WatcherDeps) {
    let mut sys = sysinfo::System::new();
    let mut backoff = Backoff::default();
    let mut last_hint: Option<Instant> = None;
    let mut last_connected: Option<Instant> = None;
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
        cycle(
            &mut deps,
            &mut sys,
            &mut backoff,
            &mut last_hint,
            &mut last_connected,
            &mut declined,
            kick,
        )
        .await;
    }
}

fn note_restart(backoff: &mut Backoff, started: Instant, res: FlowResult) {
    if res != FlowResult::NotNeeded {
        backoff.note_attempt(started);
        backoff.note_result(res == FlowResult::Connected);
    }
}

fn restart_confirm(kick: bool) -> Duration {
    if kick {
        Duration::ZERO
    } else {
        RESTART_CONFIRM
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartPlan {
    confirm: Duration,
    quit_wait: Duration,
    why: &'static str,
}

fn restart_plan(kick: bool, bridge: Option<WinBridge>) -> RestartPlan {
    let confirm = restart_confirm(kick);
    match bridge {
        Some(WinBridge::PipeAlive) => RestartPlan {
            confirm,
            quit_wait: QUIT_WAIT_HUNG,
            why: "мост в клиенте работает, а сессии нет дольше 30 с — клиент завис, перезапускаю по кнопке",
        },
        Some(WinBridge::LoadedDead) => RestartPlan {
            confirm,
            quit_wait: QUIT_WAIT,
            why: "мост в клиенте загружен, но канал не отвечает — перезапускаю с инъекцией",
        },
        _ => RestartPlan {
            confirm,
            quit_wait: QUIT_WAIT,
            why: "клиент запущен без моста — перезапускаю с инъекцией",
        },
    }
}

fn reconnect_grace_active(last_connected: Option<Instant>, kick: bool, now: Instant) -> bool {
    !kick && last_connected.is_some_and(|t| now.duration_since(t) < RECONNECT_GRACE)
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
    last_connected: &mut Option<Instant>,
    declined: &mut bool,
    kick: bool,
) {
    if deps.cdp.is_connected() {
        *declined = false;
        *last_connected = Some(Instant::now());
        set_reason(&deps.reason, None);
        return;
    }
    if reconnect_grace_active(*last_connected, kick, Instant::now()) {
        return;
    }
    let cfg = deps.config.borrow().clone();
    if !cfg.enabled && !kick {
        return;
    }
    let any_local = deps.any_local.load(Ordering::Acquire);
    if !any_local && !kick {
        return;
    }
    if declined_gate(declined, kick) {
        return;
    }
    let procs = scan(sys);
    let now_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
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
                    age_secs,
                    cmd_unreadable: p.cmd_unreadable,
                }),
                false,
            )
        }
        MainPick::NotFound => (None, false),
        MainPick::Ambiguous => (None, true),
    };
    let input = DecideInput {
        enabled: true,
        connected: false,
        any_local,
        kick,
        main_proc,
        ambiguous,
        cooldown_ok: backoff.cooldown_ok(kick, Instant::now()),
    };
    let decision = decide(&input);
    #[cfg(not(windows))]
    let win_bridge: Option<WinBridge> = None;
    #[cfg(windows)]
    let (decision, win_bridge) = match decision {
        Decision::Restart { .. } => {
            let pid = input.main_proc.as_ref().map(|m| m.pid);
            let bridge = win_bridge_state(pid);
            if matches!(bridge, WinBridge::FileMissing | WinBridge::Untrusted)
                && hint_due(last_hint)
            {
                tracing::warn!(
                    "launch: ym-bridge.dll рядом с плагином отсутствует или не подписан — перезапуск клиента ничего не даст, переустановите плагин из архива релиза"
                );
            }
            let stale = session_stale(deps.cdp.is_connected(), *last_connected, Instant::now());
            let (d, reason) = win_bridge_gate(decision, kick, bridge, stale);
            if bridge == WinBridge::PipeAlive {
                let grace = RECONNECT_GRACE.as_secs();
                match (&d, kick) {
                    (Decision::Restart { .. }, _) => {}
                    (_, true) => tracing::info!(
                        "launch: сессия разорвалась меньше {grace} с назад, канал моста жив — жду переподключения"
                    ),
                    (_, false) if hint_due(last_hint) => tracing::info!(
                        "launch: канал моста жив, сессии с клиентом нет — автоперезапуск не делаю, перезапуск по кнопке запуска"
                    ),
                    _ => {}
                }
            }
            set_reason(&deps.reason, reason.map(|r| r.to_owned()));
            (d, Some(bridge))
        }
        d => (d, None),
    };
    #[cfg(target_os = "macos")]
    let decision = match decision {
        Decision::Restart { .. } => {
            let reason = match crate::ops::mac_bridge_file() {
                crate::ops::BridgeFile::Missing => Some(REASON_BRIDGE_MISSING),
                crate::ops::BridgeFile::Untrusted => Some(REASON_BRIDGE_UNTRUSTED),
                crate::ops::BridgeFile::Ready => match input.main_proc.as_ref() {
                    Some(m) if !crate::ops::mac_bridge_arch_ok(&m.exe) => Some(REASON_BRIDGE_ARCH),
                    _ => None,
                },
            };
            if let Some(r) = reason
                && hint_due(last_hint)
            {
                tracing::warn!(
                    "launch: мост недоступен для клиента (reason={r}) — перезапуск не поможет"
                );
            }
            set_reason(&deps.reason, reason.map(|r| r.to_owned()));
            if reason.is_some() {
                Decision::Nothing
            } else {
                decision
            }
        }
        d => d,
    };
    match decision {
        Decision::Nothing => {
            if ambiguous && hint_due(last_hint) {
                tracing::warn!(
                    "launch: найдено несколько экземпляров клиента — авто-перезапуск пропущен"
                );
            }
        }
        Decision::Restart { pid, exe } => {
            let started = Instant::now();
            let target = restart_target(&exe);
            let res = restart_flow(
                deps.ops.as_ref(),
                pid,
                &target,
                deps.cdp.as_ref(),
                restart_plan(kick, win_bridge),
            )
            .await;
            note_restart(backoff, started, res);
            match res {
                FlowResult::NotNeeded => {
                    *last_connected = Some(Instant::now());
                    set_reason(&deps.reason, None);
                }
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
                home_dir().as_deref(),
                &fs_path_kind,
            );
            let ok = match &target {
                Some(t) => {
                    tracing::info!("launch: запускаю клиент");
                    match deps.ops.launch(t).await {
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

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WinBridge {
    FileMissing,
    Untrusted,
    PipeAlive,
    LoadedDead,
    NotLoaded,
}

#[cfg_attr(not(windows), allow(dead_code))]
fn session_stale(connected: bool, last_connected: Option<Instant>, now: Instant) -> bool {
    !connected && last_connected.is_none_or(|t| now.duration_since(t) >= RECONNECT_GRACE)
}

#[cfg_attr(not(windows), allow(dead_code))]
fn win_bridge_gate(
    restart: Decision,
    kick: bool,
    bridge: WinBridge,
    stale: bool,
) -> (Decision, Option<&'static str>) {
    match bridge {
        WinBridge::FileMissing => (Decision::Nothing, Some(REASON_BRIDGE_MISSING)),
        WinBridge::Untrusted => (Decision::Nothing, Some(REASON_BRIDGE_UNTRUSTED)),
        WinBridge::PipeAlive => (
            if kick && stale {
                restart
            } else {
                Decision::Nothing
            },
            None,
        ),
        WinBridge::LoadedDead => (restart, None),
        WinBridge::NotLoaded => (if kick { restart } else { Decision::Nothing }, None),
    }
}

#[cfg(windows)]
fn win_bridge_state(pid: Option<u32>) -> WinBridge {
    use crate::ops::{BridgeFile, win_bridge_dll_path, win_bridge_file};
    match win_bridge_file() {
        BridgeFile::Missing => WinBridge::FileMissing,
        BridgeFile::Untrusted => WinBridge::Untrusted,
        BridgeFile::Ready => {
            if ym_launcher::inject::bridge_pipe_exists() {
                WinBridge::PipeAlive
            } else {
                let loaded = pid
                    .zip(win_bridge_dll_path())
                    .and_then(|(p, dll)| ym_launcher::inject::module_loaded(p, &dll));
                if loaded == Some(true) {
                    WinBridge::LoadedDead
                } else {
                    WinBridge::NotLoaded
                }
            }
        }
    }
}

pub const REASON_BRIDGE_MISSING: &str = "bridge_missing";
pub const REASON_BRIDGE_UNTRUSTED: &str = "bridge_untrusted";
pub const REASON_BRIDGE_ARCH: &str = "bridge_arch_mismatch";
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

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn restart_target(exe: &Path) -> LaunchTarget {
    match current_os() {
        Os::Mac => app_bundle_from_exe(exe).map_or_else(
            || LaunchTarget::Exe(exe.to_path_buf()),
            LaunchTarget::MacApp,
        ),
        Os::Windows => LaunchTarget::Exe(exe.to_path_buf()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowResult {
    NotNeeded,
    Connected,
    Failed,
    Declined,
}

pub async fn restart_flow(
    ops: &dyn PlatformOps,
    pid: u32,
    target: &LaunchTarget,
    cdp: &dyn CdpLink,
    plan: RestartPlan,
) -> FlowResult {
    if wait_connected(cdp, plan.confirm).await {
        tracing::info!("launch: сессия с клиентом поднялась до перезапуска — клиент не трогаю");
        return FlowResult::NotNeeded;
    }
    tracing::info!("launch: {}", plan.why);
    ops.quit_graceful(pid).await;
    if !wait_gone(ops, pid, plan.quit_wait).await {
        ops.force_kill(pid).await;
        if !wait_gone(ops, pid, KILL_WAIT).await {
            tracing::warn!("launch: не удалось завершить процесс клиента (pid {pid})");
            return FlowResult::Failed;
        }
    }
    match ops.launch(target).await {
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
    use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32};

    struct MockOps {
        journal: Mutex<Vec<String>>,
        launched: Arc<AtomicBool>,
        t0: tokio::time::Instant,
        force_at: Mutex<Option<Duration>>,
        alive_until_force: AtomicBool,
        alive_polls_left: AtomicI32,
        launch_result: Result<(), LaunchError>,
    }

    impl MockOps {
        fn new(
            alive_polls_left: i32,
            alive_until_force: bool,
            launch_result: Result<(), LaunchError>,
        ) -> Self {
            Self {
                journal: Mutex::new(Vec::new()),
                launched: Arc::new(AtomicBool::new(false)),
                t0: tokio::time::Instant::now(),
                force_at: Mutex::new(None),
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
            *self.force_at.lock().unwrap() = Some(self.t0.elapsed());
            if self.alive_until_force.load(Ordering::SeqCst) {
                self.alive_polls_left.store(0, Ordering::SeqCst);
            }
            true
        }
        fn is_alive_ym(&self, _pid: u32) -> bool {
            self.alive_polls_left.fetch_sub(1, Ordering::SeqCst) > 0
        }
        async fn launch(&self, target: &LaunchTarget) -> Result<(), LaunchError> {
            self.log(format!("launch {target:?}"));
            self.launched.store(true, Ordering::SeqCst);
            self.launch_result.clone()
        }
    }

    struct MockLink {
        connect_after_polls: AtomicU32,
        gate: Option<Arc<AtomicBool>>,
    }

    impl MockLink {
        fn new(connect_after_polls: u32) -> Self {
            Self {
                connect_after_polls: AtomicU32::new(connect_after_polls),
                gate: None,
            }
        }
        fn after_launch(ops: &MockOps, connect_after_polls: u32) -> Self {
            Self {
                connect_after_polls: AtomicU32::new(connect_after_polls),
                gate: Some(ops.launched.clone()),
            }
        }
    }

    impl CdpLink for MockLink {
        fn is_connected(&self) -> bool {
            if self
                .gate
                .as_ref()
                .is_some_and(|g| !g.load(Ordering::SeqCst))
            {
                return false;
            }
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
        let link = MockLink::after_launch(&ops, 2);
        assert_eq!(
            restart_flow(&ops, 42, &exe_target(), &link, restart_plan(false, None)).await,
            FlowResult::Connected
        );
        let j = ops.journal();
        assert_eq!(
            j,
            vec!["quit 42".to_owned(), format!("launch {:?}", exe_target())]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn restart_escalates_to_force_kill() {
        let ops = MockOps::new(i32::MAX, true, Ok(()));
        let link = MockLink::after_launch(&ops, 0);
        assert_eq!(
            restart_flow(&ops, 42, &exe_target(), &link, restart_plan(false, None)).await,
            FlowResult::Connected
        );
        let j = ops.journal();
        assert_eq!(j.first().map(String::as_str), Some("quit 42"));
        assert_eq!(j.get(1).map(String::as_str), Some("force 42"));
        assert!(j.last().unwrap().starts_with("launch"));
    }

    #[tokio::test(start_paused = true)]
    async fn restart_aborts_when_process_immortal() {
        let ops = MockOps::new(i32::MAX, false, Ok(()));
        let link = MockLink::after_launch(&ops, 0);
        assert_eq!(
            restart_flow(&ops, 42, &exe_target(), &link, restart_plan(false, None)).await,
            FlowResult::Failed
        );
        let j = ops.journal();
        assert!(j.iter().all(|e| !e.starts_with("launch")));
        assert_eq!(j, vec!["quit 42", "force 42"]);
    }

    #[tokio::test(start_paused = true)]
    async fn restart_fails_when_launch_errors() {
        let ops = MockOps::new(1, false, Err(LaunchError::Failed("мок-отказ".into())));
        let link = MockLink::after_launch(&ops, 0);
        assert_eq!(
            restart_flow(&ops, 42, &exe_target(), &link, restart_plan(false, None)).await,
            FlowResult::Failed
        );
    }

    #[tokio::test(start_paused = true)]
    async fn restart_declined_when_uac_rejected() {
        let ops = MockOps::new(1, false, Err(LaunchError::UserDeclined));
        let link = MockLink::new(u32::MAX);
        assert_eq!(
            restart_flow(&ops, 42, &exe_target(), &link, restart_plan(false, None)).await,
            FlowResult::Declined
        );
    }

    #[tokio::test(start_paused = true)]
    async fn restart_fails_when_never_connects() {
        let ops = MockOps::new(1, false, Ok(()));
        let link = MockLink::new(u32::MAX);
        assert_eq!(
            restart_flow(&ops, 42, &exe_target(), &link, restart_plan(false, None)).await,
            FlowResult::Failed
        );
    }

    #[tokio::test(start_paused = true)]
    async fn bridged_client_is_left_alone_when_session_lands_during_confirm() {
        let ops = MockOps::new(1, false, Ok(()));
        let link = MockLink::new(3);
        assert_eq!(
            restart_flow(&ops, 42, &exe_target(), &link, restart_plan(false, None)).await,
            FlowResult::NotNeeded
        );
        assert!(
            ops.journal().is_empty(),
            "сессия с клиентом под мостом пришла сразу после решения сторожа — клиент убит без нужды"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn kick_restart_does_not_wait_for_a_session() {
        let ops = MockOps::new(1, false, Ok(()));
        let link = MockLink::new(1);
        assert_eq!(
            restart_flow(&ops, 42, &exe_target(), &link, restart_plan(true, None)).await,
            FlowResult::Connected
        );
        assert_eq!(
            ops.journal().first().map(String::as_str),
            Some("quit 42"),
            "по кнопке перезапуск идёт сразу"
        );
    }

    #[test]
    fn called_off_restart_spends_no_cooldown_and_no_failure() {
        let t0 = Instant::now();
        let mut b = Backoff::default();
        for _ in 0..3 {
            note_restart(&mut b, t0, FlowResult::NotNeeded);
        }
        assert!(
            b.auto_ok(t0),
            "отменённый перезапуск не тратит паузу и не копит неудачи — иначе настоящий перезапуск заблокирован"
        );
        note_restart(&mut b, t0, FlowResult::Failed);
        assert!(!b.auto_ok(t0), "настоящая попытка по-прежнему ставит паузу");
    }

    #[tokio::test(start_paused = true)]
    async fn hung_client_with_live_bridge_is_killed_after_a_short_graceful_wait() {
        let ops = MockOps::new(i32::MAX, true, Ok(()));
        let link = MockLink::after_launch(&ops, 0);
        let plan = restart_plan(true, Some(WinBridge::PipeAlive));
        assert_eq!(
            restart_flow(&ops, 42, &exe_target(), &link, plan).await,
            FlowResult::Connected
        );
        let at = ops
            .force_at
            .lock()
            .unwrap()
            .expect("зависший клиент не убит");
        assert!(
            at <= Duration::from_secs(3),
            "зависший клиент с живым мостом убит через {at:?}, цель — не позже 3 с"
        );

        let ops = MockOps::new(i32::MAX, true, Ok(()));
        let link = MockLink::after_launch(&ops, 0);
        restart_flow(&ops, 42, &exe_target(), &link, restart_plan(true, None)).await;
        let at = ops.force_at.lock().unwrap().expect("клиент не убит");
        assert!(
            at >= QUIT_WAIT,
            "без признаков зависания штатному закрытию дают полный срок, ждали {at:?}"
        );
    }

    #[test]
    fn restart_line_and_graceful_wait_follow_the_bridge_branch() {
        let hung = restart_plan(true, Some(WinBridge::PipeAlive));
        assert!(
            !hung.why.contains("без моста"),
            "в ветке D-7 мост в клиенте загружен: {}",
            hung.why
        );
        assert!(hung.quit_wait < QUIT_WAIT);
        let dead = restart_plan(false, Some(WinBridge::LoadedDead));
        assert!(
            !dead.why.contains("без моста"),
            "мост загружен, хоть канал и не отвечает: {}",
            dead.why
        );
        assert_eq!(dead.quit_wait, QUIT_WAIT);
        for bare in [
            restart_plan(false, None),
            restart_plan(true, Some(WinBridge::NotLoaded)),
        ] {
            assert!(bare.why.contains("без моста"));
            assert_eq!(bare.quit_wait, QUIT_WAIT);
        }
        assert_eq!(
            restart_plan(true, Some(WinBridge::PipeAlive)).confirm,
            Duration::ZERO
        );
        assert_eq!(restart_plan(false, None).confirm, RESTART_CONFIRM);
    }

    #[test]
    fn automatic_restart_waits_for_a_session_and_kick_does_not() {
        assert_eq!(restart_confirm(true), Duration::ZERO);
        assert!(
            restart_confirm(false) >= Duration::from_secs(2),
            "окно подтверждения перекрывает обычный подъём сессии"
        );
    }

    #[test]
    fn set_reason_dedups_and_notifies_on_change() {
        let (tx, mut rx) = tokio::sync::watch::channel::<Option<String>>(None);
        set_reason(&tx, None);
        assert!(
            !rx.has_changed().unwrap(),
            "повтор того же значения не будит подписчиков"
        );
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
    fn reconnect_grace_holds_off_restarts_right_after_a_drop() {
        let t0 = Instant::now();
        assert!(reconnect_grace_active(
            Some(t0),
            false,
            t0 + Duration::from_secs(5)
        ));
        assert!(
            !reconnect_grace_active(Some(t0), true, t0 + Duration::from_secs(5)),
            "кнопка пробивает паузу"
        );
        assert!(!reconnect_grace_active(
            Some(t0),
            false,
            t0 + RECONNECT_GRACE
        ));
        assert!(
            !reconnect_grace_active(None, false, t0),
            "без прошлого коннекта пауза не нужна"
        );
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

    fn restart() -> Decision {
        Decision::Restart {
            pid: 1,
            exe: PathBuf::from("/x/ym"),
        }
    }

    #[test]
    fn win_gate_dead_bridge_restarts_auto_and_on_kick() {
        assert!(matches!(
            win_bridge_gate(restart(), false, WinBridge::LoadedDead, true).0,
            Decision::Restart { .. }
        ));
        assert!(matches!(
            win_bridge_gate(restart(), true, WinBridge::LoadedDead, true).0,
            Decision::Restart { .. }
        ));
    }

    #[test]
    fn session_is_stale_only_after_grace_or_without_any_connection() {
        let t0 = Instant::now();
        assert!(
            !session_stale(true, Some(t0), t0 + RECONNECT_GRACE * 2),
            "подключённая сессия не устаревает"
        );
        assert!(
            !session_stale(false, Some(t0), t0 + Duration::from_secs(5)),
            "свежий разрыв — ждём переподключения"
        );
        assert!(session_stale(false, Some(t0), t0 + RECONNECT_GRACE));
        assert!(
            session_stale(false, None, t0),
            "сессии не было с запуска плагина"
        );
    }

    #[test]
    fn win_gate_live_pipe_restarts_on_kick_once_session_is_stale() {
        let t0 = Instant::now();
        let stale = session_stale(false, Some(t0), t0 + RECONNECT_GRACE);
        let (d, reason) = win_bridge_gate(restart(), true, WinBridge::PipeAlive, stale);
        assert!(
            matches!(d, Decision::Restart { .. }),
            "D-7: замороженный клиент держит канал моста, а сессии нет — кнопка обязана перезапустить"
        );
        assert_eq!(reason, None);
    }

    #[test]
    fn win_gate_live_pipe_holds_when_connected_fresh_or_not_kicked() {
        let t0 = Instant::now();
        let connected = session_stale(true, Some(t0), t0 + RECONNECT_GRACE);
        assert_eq!(
            win_bridge_gate(restart(), true, WinBridge::PipeAlive, connected).0,
            Decision::Nothing,
            "подключённый клиент кнопка не трогает"
        );
        let fresh = session_stale(false, Some(t0), t0 + Duration::from_secs(5));
        assert_eq!(
            win_bridge_gate(restart(), true, WinBridge::PipeAlive, fresh).0,
            Decision::Nothing,
            "свежий разрыв — даём переподключиться"
        );
        assert_eq!(
            win_bridge_gate(restart(), false, WinBridge::PipeAlive, true).0,
            Decision::Nothing,
            "без кнопки живой канал не перезапускаем"
        );
    }

    #[test]
    fn win_gate_no_usable_bridge_surfaces_distinct_reason_and_holds() {
        let (dm, rm) = win_bridge_gate(restart(), true, WinBridge::FileMissing, true);
        assert_eq!(
            dm,
            Decision::Nothing,
            "без файла моста перезапуск бесполезен даже по кнопке"
        );
        assert_eq!(rm, Some(REASON_BRIDGE_MISSING));
        let (du, ru) = win_bridge_gate(restart(), true, WinBridge::Untrusted, true);
        assert_eq!(
            du,
            Decision::Nothing,
            "с неподписанным мостом перезапуск бесполезен даже по кнопке"
        );
        assert_eq!(ru, Some(REASON_BRIDGE_UNTRUSTED));
    }

    #[test]
    fn win_gate_not_loaded_waits_for_watchdog_unless_kicked() {
        assert_eq!(
            win_bridge_gate(restart(), false, WinBridge::NotLoaded, true).0,
            Decision::Nothing
        );
        assert!(matches!(
            win_bridge_gate(restart(), true, WinBridge::NotLoaded, true).0,
            Decision::Restart { .. }
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn wait_connected_polls_until_flip() {
        let link = MockLink::new(3);
        assert!(wait_connected(&link, Duration::from_secs(20)).await);
        let never = MockLink::new(u32::MAX);
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
            cdp: Arc::new(MockLink::new(u32::MAX)),
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
        let t = restart_target(Path::new(
            "/Applications/Яндекс Музыка.app/Contents/MacOS/Яндекс Музыка",
        ));
        if current_os() == Os::Mac {
            assert_eq!(
                t,
                LaunchTarget::MacApp(PathBuf::from("/Applications/Яндекс Музыка.app"))
            );
        } else {
            assert!(matches!(t, LaunchTarget::Exe(_)));
        }
        let t = restart_target(Path::new("/usr/local/bin/ym"));
        assert!(matches!(t, LaunchTarget::Exe(_)));
    }
}
