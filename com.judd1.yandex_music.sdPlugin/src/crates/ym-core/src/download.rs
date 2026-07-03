use std::path::PathBuf;
use std::sync::Arc;

use ym_model::StateEvent;

use crate::action::Shared;

pub async fn run_download(shared: &Arc<Shared>, track_id: String) -> Result<PathBuf, String> {
    if shared.download_begin() {
        shared.publish(StateEvent::Download { active: true });
    }
    let result = download_inner(shared, &track_id).await;
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
        .or_else(|| shared.token())
        .filter(|t| !t.is_empty())
        .ok_or_else(|| "нет токена".to_owned())?;
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
    use ym_model::{Downloader, StubController};

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

    fn shared_with(result: fn() -> anyhow::Result<PathBuf>) -> (Arc<Shared>, Calls) {
        let calls: Calls = Default::default();
        let dl = RecordingDownloader { calls: calls.clone(), result };
        let (bus, _) = broadcast::channel(16);
        let shared =
            Shared::wired(bus, Arc::new(StubController), ym_render::Renderers::new(), Arc::new(dl));
        (shared, calls)
    }

    #[tokio::test]
    async fn run_download_passes_args_to_port() {
        let (shared, calls) = shared_with(|| Ok(PathBuf::from("/tmp/t.mp3")));
        shared.set_token(Some("T".into()));
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
            shared_with(|| Err(anyhow::anyhow!("нет сети").context("скачивание трека")));
        shared.set_token(Some("T".into()));
        let mut bus = shared.subscribe();

        let res = run_download(&shared, "42".into()).await;
        assert_eq!(res, Err("скачивание трека: нет сети".to_owned()));
        assert_eq!(bus.try_recv().unwrap(), StateEvent::Download { active: true });
        assert_eq!(bus.try_recv().unwrap(), StateEvent::Download { active: false });
    }

    #[tokio::test]
    async fn run_download_with_default_stub_errors() {
        let shared = Shared::with(Arc::new(StubController), ym_render::Renderers::new());
        shared.set_token(Some("T".into()));
        let res = run_download(&shared, "42".into()).await;
        let err = res.unwrap_err();
        assert!(err.contains("загрузчик не подключён"), "получено: {err}");
    }

    #[tokio::test]
    async fn run_download_empty_id_skips_port() {
        let (shared, calls) = shared_with(|| Ok(PathBuf::new()));
        shared.set_token(Some("T".into()));
        let res = run_download(&shared, " ".into()).await;
        assert_eq!(res, Err("пустой track_id".to_owned()));
        assert!(calls.lock().unwrap().is_empty());
    }
}
