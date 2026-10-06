mod cli;
mod log;
mod single_instance;
mod stats_dir;

use std::sync::Arc;
use std::time::Duration;

use cli::LaunchArgs;
use sd_host::{HostConfig, HostHandle};
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use ym_core::{ActionFactory, MediaController, Orchestrator, Shared, build_action};
use ym_remote_controller::RemoteController;
use ym_render::Renderers;

#[cfg(feature = "downloads")]
struct RealDownloader;

#[cfg(feature = "downloads")]
#[async_trait::async_trait]
impl ym_core::Downloader for RealDownloader {
    async fn download(
        &self,
        track_id: &str,
        token: &str,
        dir_setting: &str,
        format: &str,
    ) -> anyhow::Result<std::path::PathBuf> {
        let dir = ym_download::resolve_dir(dir_setting);
        ym_download::download_track(track_id, token, &dir, format).await
    }
}

#[cfg(feature = "downloads")]
fn make_downloader() -> Arc<dyn ym_core::Downloader> {
    Arc::new(RealDownloader)
}

#[cfg(not(feature = "downloads"))]
fn make_downloader() -> Arc<dyn ym_core::Downloader> {
    Arc::new(ym_core::StubDownloader)
}

#[cfg(feature = "self-update")]
fn spawn_update(shared: Arc<Shared>) -> tokio::task::JoinHandle<()> {
    let (owner, repo) = parse_update_repo(std::env::var("YM_UPDATE_REPO").ok().as_deref());
    tokio::spawn(async move {
        if let Some(o) = ym_update::run(&owner, &repo, env!("CARGO_PKG_VERSION")).await {
            shared.apply_update_notice(o.version, o.notes, o.url);
        }
    })
}

#[cfg(not(feature = "self-update"))]
fn spawn_update(_shared: Arc<Shared>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async {})
}

struct RemoteLink(Arc<RemoteController>);

impl ym_launch::CdpLink for RemoteLink {
    fn is_connected(&self) -> bool {
        MediaController::is_connected(self.0.as_ref())
    }
}

fn launch_cache_path() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|e| {
            e.parent()
                .and_then(|b| b.parent())
                .map(|d| d.join(".ym_client_path"))
        })
        .unwrap_or_else(|| std::env::temp_dir().join(".ym_client_path"))
}

#[cfg(windows)]
fn bridge_dll_path() -> Option<std::path::PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(|p| p.join("ym-bridge.dll"))
}

fn rival_process_running(sys: &mut sysinfo::System) -> bool {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, UpdateKind};
    let kind = ProcessRefreshKind::nothing()
        .with_cmd(UpdateKind::Always)
        .with_exe(UpdateKind::Always)
        .with_cwd(UpdateKind::Always);
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
    sys.processes().values().any(|p| {
        let cmd: Vec<String> = p
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        ym_core::rival::proc_is_rival(p.exe(), &cmd, p.cwd())
    })
}

fn client_path_report(raw: &str) -> ym_core::ClientPathReport {
    use ym_launch::resolve::{
        PathVerdict, check_client_path, client_file_name, current_os, fs_path_kind,
    };
    let os = current_os();
    let expected = client_file_name(os);
    match check_client_path(os, raw, &fs_path_kind) {
        PathVerdict::Ok => ym_core::ClientPathReport {
            verdict: "ok",
            resolved: None,
            expected,
        },
        PathVerdict::OkDirCompleted(p) => ym_core::ClientPathReport {
            verdict: "ok_dir",
            resolved: Some(p.to_string_lossy().into_owned()),
            expected,
        },
        PathVerdict::Missing => ym_core::ClientPathReport {
            verdict: "missing",
            resolved: None,
            expected,
        },
        PathVerdict::DirWithoutClient => ym_core::ClientPathReport {
            verdict: "dir_without_client",
            resolved: None,
            expected,
        },
    }
}

#[cfg(feature = "self-update")]
fn parse_update_repo(v: Option<&str>) -> (String, String) {
    if let Some(v) = v
        && let Some((owner, repo)) = v.trim().split_once('/')
        && !owner.trim().is_empty()
        && !repo.trim().is_empty()
    {
        return (owner.trim().to_owned(), repo.trim().to_owned());
    }
    ("Judd1zzz".to_owned(), "yandex-music-streamdeck".to_owned())
}

fn main() {
    let local_offset = ym_core::capture_local_offset();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("не удалось создать tokio runtime");
    rt.block_on(async_main(local_offset));
}

async fn async_main(local_offset: i32) {
    let (log_ring, logs_flag) = log::init();
    single_instance::ensure_single_instance();
    tracing::info!("Plugin Start (ym-plugin {})", env!("CARGO_PKG_VERSION"));

    let args = match LaunchArgs::parse() {
        Ok(a) => a,
        Err(e) => {
            tracing::error!("ошибка разбора argv: {e:#}");
            std::process::exit(0);
        }
    };
    tracing::info!(port = args.port, uuid = %args.plugin_uuid, "registration args parsed");

    let shutdown = CancellationToken::new();
    let render = Renderers::new();
    let (bus, _) = broadcast::channel(256);
    let (dl_tx, dl_rx) = mpsc::channel::<String>(16);
    let remote = RemoteController::new(bus.clone());
    remote.set_download_tx(dl_tx);
    let cdp_task = remote.start(shutdown.clone());
    let launch_link: Arc<dyn ym_launch::CdpLink> = Arc::new(RemoteLink(remote.clone()));
    let rival = remote.clone();
    let cdp: Arc<dyn MediaController> = remote;
    let shared = Shared::wired(bus, cdp, render, make_downloader());
    shared.set_current_version(env!("CARGO_PKG_VERSION").to_owned());
    shared.set_log_ring(log_ring);
    shared.set_logs_enabled_flag(logs_flag);
    shared.set_log_dir(log::default_log_dir());
    shared.set_stats_dir(
        stats_dir::prepare(&log::default_log_dir(), stats_dir::default_stats_dir()),
        local_offset,
    );
    shared.set_time_offset(local_offset);
    shared.set_edition(if cfg!(feature = "downloads") {
        "full"
    } else {
        "elgato"
    });
    let (discord_status_tx, mut discord_status_rx) =
        tokio::sync::watch::channel::<String>("disabled".to_owned());
    let discord_task = ym_discord::spawn(
        shared.subscribe(),
        shared.subscribe_discord(),
        shutdown.clone(),
        discord_status_tx,
        shared.cdp.clone(),
    );
    let discord_status_shared = shared.clone();
    let discord_status_task = tokio::spawn(async move {
        while discord_status_rx.changed().await.is_ok() {
            let s = discord_status_rx.borrow_and_update().clone();
            discord_status_shared.apply_discord_status(s);
        }
    });
    let (kick_tx, kick_rx) = mpsc::channel::<()>(4);
    shared.set_launch_kick(kick_tx);
    shared.set_client_path_checker(Arc::new(client_path_report));
    shared.set_folder_picker(Arc::new(ym_core::folder_dialog::pick));
    #[cfg(feature = "self-update")]
    {
        let (u_owner, u_repo) = parse_update_repo(std::env::var("YM_UPDATE_REPO").ok().as_deref());
        let cur = env!("CARGO_PKG_VERSION").to_owned();
        shared.set_update_checker(Arc::new(move || {
            let (o, r, c) = (u_owner.clone(), u_repo.clone(), cur.clone());
            Box::pin(async move { ym_update::check(&o, &r, &c).await })
        }));
    }
    let update_task = spawn_update(shared.clone());
    let (reason_tx, mut reason_rx) = tokio::sync::watch::channel::<Option<String>>(None);
    let inject_wake = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let launch_task = ym_launch::spawn(ym_launch::WatcherDeps {
        cdp: launch_link,
        events: shared.subscribe(),
        any_local: shared.any_local_flag(),
        config: shared.subscribe_launch(),
        kick: kick_rx,
        ops: Arc::new(ym_launch::RealOps {
            launched: Some(inject_wake.clone()),
        }),
        cache_path: launch_cache_path(),
        reason: reason_tx,
        shutdown: shutdown.clone(),
    });
    let reason_shared = shared.clone();
    let reason_task = tokio::spawn(async move {
        while reason_rx.changed().await.is_ok() {
            let v = reason_rx.borrow_and_update().clone();
            reason_shared.apply_launch_reason(v);
        }
    });
    #[cfg(windows)]
    if let Some(dll) = bridge_dll_path() {
        if !ym_model::attest::bridge_trusted(&dll) {
            tracing::error!(
                "ym-bridge.dll не прошёл проверку подписи ({}) — внедрение заблокировано до совпадения подписи (fail-closed)",
                dll.display()
            );
        }
        let stop = shutdown.clone();
        let wake = inject_wake.clone();
        let _ = std::thread::Builder::new()
            .name("ym-bridge-watchdog".into())
            .spawn(move || {
                let trust_dll = dll.clone();
                ym_launcher::inject::run_watchdog(
                    &dll,
                    Duration::from_secs(3),
                    move || ym_model::attest::bridge_trusted(&trust_dll),
                    move || wake.swap(false, std::sync::atomic::Ordering::AcqRel),
                    || stop.is_cancelled(),
                );
            });
    }
    let dl_task = tokio::spawn(download_consumer(dl_rx, shared.clone(), shutdown.clone()));
    let stats_task = ym_core::stats::spawn(shared.clone(), shared.subscribe(), shutdown.clone());
    let rival_shared = shared.clone();
    let rival_shutdown = shutdown.clone();
    let rival_task = tokio::spawn(async move {
        let installed = ym_core::rival::plugins_dir()
            .map(|d| ym_core::rival::installed_in(&d))
            .unwrap_or(false);
        let mut sys = sysinfo::System::new();
        loop {
            let running = rival_process_running(&mut sys);
            let cdp_seen = rival.probe_rival().await;
            rival_shared.set_rival(ym_core::rival::RivalState {
                installed,
                active: running || cdp_seen,
            });
            tokio::select! {
                _ = rival_shutdown.cancelled() => break,
                _ = tokio::time::sleep(Duration::from_secs(20)) => {}
            }
        }
    });
    let factory: ActionFactory = Arc::new(build_action);

    let mut host_cfg = HostConfig::new(
        args.port,
        args.plugin_uuid.clone(),
        args.register_event.clone(),
    );
    if let Some(ms) = std::env::var("YM_HOST_BACKOFF_MS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
    {
        host_cfg.backoff = Duration::from_millis(ms);
    }
    let HostHandle { tx, inbound, task } = sd_host::spawn(host_cfg);

    let orch = Orchestrator::new(args.plugin_uuid, tx, shared, factory);
    let orch_task = tokio::spawn(orch.run(inbound));

    let _ = task.await;
    let _ = orch_task.await;
    shutdown.cancel();
    update_task.abort();
    let aborts = [
        cdp_task.abort_handle(),
        discord_task.abort_handle(),
        discord_status_task.abort_handle(),
        dl_task.abort_handle(),
        stats_task.abort_handle(),
        launch_task.abort_handle(),
        reason_task.abort_handle(),
        rival_task.abort_handle(),
    ];
    let all = async {
        let _ = cdp_task.await;
        let _ = discord_task.await;
        let _ = discord_status_task.await;
        let _ = dl_task.await;
        let _ = stats_task.await;
        let _ = launch_task.await;
        let _ = reason_task.await;
        let _ = rival_task.await;
    };
    if tokio::time::timeout(Duration::from_secs(3), all)
        .await
        .is_err()
    {
        tracing::warn!("часть фоновых задач не завершилась за 3с — принудительная отмена");
        for a in aborts {
            a.abort();
        }
    }
    tracing::info!("Plugin exit");
}

async fn download_consumer(
    mut rx: mpsc::Receiver<String>,
    shared: Arc<Shared>,
    shutdown: CancellationToken,
) {
    loop {
        let track_id = tokio::select! {
            _ = shutdown.cancelled() => break,
            id = rx.recv() => match id {
                Some(id) => id,
                None => break,
            },
        };
        let shared = shared.clone();
        tokio::spawn(async move {
            match ym_core::run_download(&shared, track_id).await {
                Ok(p) => tracing::info!("трек скачан (кнопка): {}", p.display()),
                Err(e) => tracing::warn!("скачивание (кнопка) не удалось: {e}"),
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{client_path_report, download_consumer};
    use std::time::Duration;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;
    use ym_core::Shared;

    #[test]
    fn client_path_report_maps_verdicts() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("Яндекс Музыка.app")).unwrap();
        std::fs::write(dir.path().join("Яндекс Музыка.exe"), b"x").unwrap();

        let rep = client_path_report(&dir.path().to_string_lossy());
        assert_eq!(rep.verdict, "ok_dir");
        let resolved = rep.resolved.expect("дополненный путь");
        assert!(resolved.starts_with(&*dir.path().to_string_lossy()));
        assert_eq!(
            rep.expected,
            ym_launch::resolve::client_file_name(ym_launch::resolve::current_os())
        );

        let rep = client_path_report(&dir.path().join("nope").to_string_lossy());
        assert_eq!(rep.verdict, "missing");
        assert_eq!(rep.resolved, None);
    }

    #[tokio::test]
    async fn download_consumer_exits_on_cancel() {
        let (_tx, rx) = mpsc::channel::<String>(4);
        let shutdown = CancellationToken::new();
        let task = tokio::spawn(download_consumer(rx, Shared::new(), shutdown.clone()));
        shutdown.cancel();
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("consumer должен завершиться по cancel")
            .unwrap();
    }

    #[tokio::test]
    async fn download_consumer_exits_on_channel_close() {
        let (tx, rx) = mpsc::channel::<String>(4);
        let shutdown = CancellationToken::new();
        let task = tokio::spawn(download_consumer(rx, Shared::new(), shutdown));
        drop(tx);
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("consumer должен завершиться при закрытии канала")
            .unwrap();
    }

    #[cfg(feature = "self-update")]
    #[test]
    fn update_repo_override_and_defaults() {
        use super::parse_update_repo;
        assert_eq!(
            parse_update_repo(Some("me/scratch")),
            ("me".to_owned(), "scratch".to_owned())
        );
        assert_eq!(
            parse_update_repo(Some(" me / scratch ")),
            ("me".to_owned(), "scratch".to_owned())
        );
        let def = ("Judd1zzz".to_owned(), "yandex-music-streamdeck".to_owned());
        assert_eq!(parse_update_repo(None), def);
        assert_eq!(parse_update_repo(Some("nope")), def);
        assert_eq!(parse_update_repo(Some("/repo")), def);
        assert_eq!(parse_update_repo(Some("owner/")), def);
        assert_eq!(parse_update_repo(Some("")), def);
    }

    #[test]
    fn bridge_watchdog_always_runs_and_rechecks_trust_before_inject() {
        let src = include_str!("main.rs");
        let at = src
            .find("    if let Some(dll) = bridge_dll_path() {\n")
            .expect("нет запуска сторожа моста");
        let block = &src[at..];
        let block = &block[..block.find("\n    }\n").expect("конец блока сторожа моста")];
        let spawn = block
            .find("std::thread::Builder::new()")
            .expect("нет запуска потока сторожа");
        let before = &block[..spawn];
        assert_eq!(
            before.matches('{').count(),
            before.matches('}').count() + 1,
            "сторож моста запускается всегда, а не под проверкой подписи на старте: иначе возвращённый .sig не подхватится без перезапуска плагина"
        );
        assert!(
            block.contains("move || ym_model::attest::bridge_trusted(&trust_dll)"),
            "доверие к dll проверяется перед каждым внедрением: иначе подменённый после старта мост внедрится без проверки"
        );
        let call = &block[block
            .find("run_watchdog(")
            .expect("нет вызова run_watchdog")..];
        let trust = call
            .find("bridge_trusted(&trust_dll)")
            .expect("нет замыкания доверия в вызове");
        let wake = call
            .find("wake.swap(false")
            .expect("нет замыкания пробуждения в вызове");
        let stop = call
            .find("stop.is_cancelled()")
            .expect("нет замыкания остановки в вызове");
        assert!(
            trust < wake && wake < stop,
            "run_watchdog(dll, interval, trusted, wake_now, stop): у трёх замыканий один тип, перестановку компилятор не поймает"
        );
        let launch = src
            .find("ym_launch::RealOps { launched: Some(inject_wake.clone()) }")
            .expect("запуск клиента не будит сторожа внедрения");
        assert!(
            launch < at,
            "флаг пробуждения создаётся до сторожа запуска и до потока внедрения"
        );
    }
}
