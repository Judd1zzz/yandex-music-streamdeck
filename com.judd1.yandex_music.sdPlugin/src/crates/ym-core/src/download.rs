use std::path::PathBuf;
use std::sync::Arc;

use ym_model::StateEvent;

use crate::action::Shared;

pub(crate) const NO_CLIENT_TOKEN: &str = "клиент Яндекс Музыки не отдал токен аккаунта — проверьте, что вы вошли в аккаунт в клиенте";

pub async fn run_download(shared: &Arc<Shared>, track_id: String) -> Result<PathBuf, String> {
    if shared.download_begin() {
        shared.publish(StateEvent::Download { active: true });
    }
    let result = download_inner(shared, &track_id).await;
    if let Ok(ref path) = result {
        let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        shared.stats_record_download(bytes);
    }
    if shared.download_end() {
        shared.publish(StateEvent::Download { active: false });
    }
    result
}

async fn download_inner(shared: &Arc<Shared>, track_id: &str) -> Result<PathBuf, String> {
    if track_id.trim().is_empty() {
        return Err("пустой track_id".to_owned());
    }
    let token = shared
        .cdp
        .oauth_token()
        .await
        .filter(|t| !t.is_empty())
        .ok_or_else(|| NO_CLIENT_TOKEN.to_owned())?;
    let dir_setting = shared.download_path();
    let format = shared.download_format();
    shared
        .downloader
        .download(track_id, &token, &dir_setting, &format)
        .await
        .map_err(|e| format!("{e:#}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Mutex;
    use tokio::sync::broadcast;
    use ym_model::{ActionResult, Downloader, MediaController, MediaState, VolumeAction};

    type Calls = Arc<Mutex<Vec<(String, String, String, String)>>>;

    struct RecordingDownloader {
        calls: Calls,
        result: fn() -> anyhow::Result<PathBuf>,
    }

    #[async_trait]
    impl Downloader for RecordingDownloader {
        async fn download(
            &self,
            track_id: &str,
            token: &str,
            dir_setting: &str,
            format: &str,
        ) -> anyhow::Result<PathBuf> {
            self.calls.lock().unwrap().push((
                track_id.to_owned(),
                token.to_owned(),
                dir_setting.to_owned(),
                format.to_owned(),
            ));
            (self.result)()
        }
    }

    struct ClientToken(Option<&'static str>);

    #[async_trait]
    impl MediaController for ClientToken {
        fn is_connected(&self) -> bool {
            true
        }
        fn snapshot(&self) -> MediaState {
            MediaState::default()
        }
        async fn oauth_token(&self) -> Option<String> {
            self.0.map(str::to_owned)
        }
        async fn play_pause(&self) -> ActionResult {
            ActionResult::default()
        }
        async fn next(&self) -> ActionResult {
            ActionResult::default()
        }
        async fn prev(&self) -> ActionResult {
            ActionResult::default()
        }
        async fn toggle_like(&self) -> ActionResult {
            ActionResult::default()
        }
        async fn toggle_dislike(&self) -> ActionResult {
            ActionResult::default()
        }
        async fn toggle_shuffle(&self) -> ActionResult {
            ActionResult::default()
        }
        async fn toggle_repeat(&self) -> ActionResult {
            ActionResult::default()
        }
        async fn change_volume(&self, _action: VolumeAction) -> ActionResult {
            ActionResult::default()
        }
    }

    fn shared_with(token: Option<&'static str>, result: fn() -> anyhow::Result<PathBuf>) -> (Arc<Shared>, Calls) {
        let calls: Calls = Default::default();
        let dl = RecordingDownloader { calls: calls.clone(), result };
        let (bus, _) = broadcast::channel(16);
        let shared = Shared::wired(bus, Arc::new(ClientToken(token)), ym_render::Renderers::new(), Arc::new(dl));
        (shared, calls)
    }

    #[tokio::test]
    async fn run_download_passes_args_to_port() {
        let (shared, calls) = shared_with(Some("T"), || Ok(PathBuf::from("/tmp/t.mp3")));
        shared.set_download_config("/music".into(), "mp3".into());
        let mut bus = shared.subscribe();

        let res = run_download(&shared, "42".into()).await;
        assert_eq!(res, Ok(PathBuf::from("/tmp/t.mp3")));
        assert_eq!(
            *calls.lock().unwrap(),
            vec![("42".to_owned(), "T".to_owned(), "/music".to_owned(), "mp3".to_owned())]
        );
        assert_eq!(bus.try_recv().unwrap(), StateEvent::Download { active: true });
        assert_eq!(bus.try_recv().unwrap(), StateEvent::Download { active: false });
    }

    #[tokio::test]
    async fn run_download_maps_port_error_chain() {
        let (shared, _calls) =
            shared_with(Some("T"), || Err(anyhow::anyhow!("нет сети").context("скачивание трека")));
        let mut bus = shared.subscribe();

        let res = run_download(&shared, "42".into()).await;
        assert_eq!(res, Err("скачивание трека: нет сети".to_owned()));
        assert_eq!(bus.try_recv().unwrap(), StateEvent::Download { active: true });
        assert_eq!(bus.try_recv().unwrap(), StateEvent::Download { active: false });
    }

    #[tokio::test]
    async fn run_download_with_default_stub_errors() {
        let shared = Shared::with(Arc::new(ClientToken(Some("T"))), ym_render::Renderers::new());
        let res = run_download(&shared, "42".into()).await;
        let err = res.unwrap_err();
        assert!(err.contains("загрузчик не подключён"), "получено: {err}");
    }

    #[tokio::test]
    async fn run_download_empty_id_skips_port() {
        let (shared, calls) = shared_with(Some("T"), || Ok(PathBuf::new()));
        let res = run_download(&shared, " ".into()).await;
        assert_eq!(res, Err("пустой track_id".to_owned()));
        assert!(calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn run_download_without_client_token_names_the_reason_and_skips_port() {
        let (shared, calls) = shared_with(None, || Ok(PathBuf::new()));
        let res = run_download(&shared, "42".into()).await;
        assert_eq!(res, Err(NO_CLIENT_TOKEN.to_owned()));
        assert!(calls.lock().unwrap().is_empty());
    }
}
