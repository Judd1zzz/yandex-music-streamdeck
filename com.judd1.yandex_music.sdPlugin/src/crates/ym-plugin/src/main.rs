mod cli;
mod log;
mod single_instance;

use std::sync::Arc;
use std::time::Duration;

use cli::LaunchArgs;
use sd_host::{HostConfig, HostHandle};
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use ym_cdp::CdpController;
use ym_core::{build_action, ActionFactory, MediaController, Orchestrator, Shared};
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
        if let Some(v) = ym_update::run(&owner, &repo, env!("CARGO_PKG_VERSION")).await {
            shared.apply_update_notice(v);
        }
    })
}

#[cfg(not(feature = "self-update"))]
fn spawn_update(_shared: Arc<Shared>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async {})
}

struct CdpPortLink(Arc<CdpController>);

impl ym_launch::CdpLink for CdpPortLink {
    fn port(&self) -> u16 {
        self.0.local_port()
    }
    fn set_port(&self, port: u16) {
        self.0.set_local_port(port);
    }
    fn is_connected(&self) -> bool {
        MediaController::is_connected(self.0.as_ref())
    }
}

fn launch_cache_path() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().and_then(|b| b.parent()).map(|d| d.join(".ym_client_path")))
        .unwrap_or_else(|| std::env::temp_dir().join(".ym_client_path"))
}

fn client_path_report(raw: &str) -> ym_core::ClientPathReport {
    use ym_launch::resolve::{PathVerdict, check_client_path, client_file_name, current_os, fs_path_kind};
    let os = current_os();
    let expected = client_file_name(os);
    match check_client_path(os, raw, &fs_path_kind) {
        PathVerdict::Ok => ym_core::ClientPathReport { verdict: "ok", resolved: None, expected },
        PathVerdict::OkDirCompleted(p) => ym_core::ClientPathReport {
            verdict: "ok_dir",
            resolved: Some(p.to_string_lossy().into_owned()),
            expected,
        },
        PathVerdict::Missing => {
            ym_core::ClientPathReport { verdict: "missing", resolved: None, expected }
        }
        PathVerdict::DirWithoutClient => {
            ym_core::ClientPathReport { verdict: "dir_without_client", resolved: None, expected }
        }
    }
}

fn env_u16(key: &str, default: u16) -> u16 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
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

#[tokio::main]
async fn main() {
    log::init();
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
    let cdp_port = env_u16("YM_CDP_PORT", 9222);
    let cdp = CdpController::new(cdp_port, bus.clone());
    let (dl_tx, dl_rx) = mpsc::channel::<String>(16);
    cdp.set_download_tx(dl_tx);
    let cdp_task = cdp.start(shutdown.clone());
    let launch_link = cdp.clone();
    let shared = Shared::wired(bus, cdp, render, make_downloader());
    let discord_task = ym_discord::spawn(shared.subscribe(), shared.subscribe_discord(), shutdown.clone());
    let (kick_tx, kick_rx) = mpsc::channel::<()>(4);
    shared.set_launch_kick(kick_tx);
    shared.set_client_path_checker(Arc::new(client_path_report));
    let update_task = spawn_update(shared.clone());
    let (reason_tx, mut reason_rx) = tokio::sync::watch::channel::<Option<String>>(None);
    let launch_task = ym_launch::spawn(ym_launch::WatcherDeps {
        cdp: Arc::new(CdpPortLink(launch_link)),
        events: shared.subscribe(),
        any_local: shared.any_local_flag(),
        config: shared.subscribe_launch(),
        kick: kick_rx,
        ops: Arc::new(ym_launch::RealOps),
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
    let dl_task = tokio::spawn(download_consumer(dl_rx, shared.clone(), shutdown.clone()));
    let factory: ActionFactory = Arc::new(build_action);

    let mut host_cfg = HostConfig::new(args.port, args.plugin_uuid.clone(), args.register_event.clone());
    if let Some(ms) = std::env::var("YM_HOST_BACKOFF_MS").ok().and_then(|s| s.parse::<u64>().ok()) {
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
        dl_task.abort_handle(),
        launch_task.abort_handle(),
        reason_task.abort_handle(),
    ];
    let all = async {
        let _ = cdp_task.await;
        let _ = discord_task.await;
        let _ = dl_task.await;
        let _ = launch_task.await;
        let _ = reason_task.await;
    };
    if tokio::time::timeout(Duration::from_secs(3), all).await.is_err() {
        tracing::warn!("часть фоновых задач не завершилась за 3с — принудительная отмена");
        for a in aborts {
            a.abort();
        }
    }
    tracing::info!("Plugin exit");
}

async fn download_consumer(mut rx: mpsc::Receiver<String>, shared: Arc<Shared>, shutdown: CancellationToken) {
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
        assert_eq!(parse_update_repo(Some("me/scratch")), ("me".to_owned(), "scratch".to_owned()));
        assert_eq!(parse_update_repo(Some(" me / scratch ")), ("me".to_owned(), "scratch".to_owned()));
        let def = ("Judd1zzz".to_owned(), "yandex-music-streamdeck".to_owned());
        assert_eq!(parse_update_repo(None), def);
        assert_eq!(parse_update_repo(Some("nope")), def);
        assert_eq!(parse_update_repo(Some("/repo")), def);
        assert_eq!(parse_update_repo(Some("owner/")), def);
        assert_eq!(parse_update_repo(Some("")), def);
    }
}
