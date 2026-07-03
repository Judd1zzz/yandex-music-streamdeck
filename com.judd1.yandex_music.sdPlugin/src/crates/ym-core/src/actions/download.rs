use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use ym_model::StateEvent;

use crate::action::{Action, ActionCtx, Interests};
use crate::names;

use super::helpers::*;

#[derive(Default)]
pub struct Download {
    busy: Arc<AtomicBool>,
}

#[async_trait]
impl Action for Download {
    fn interests(&self) -> Interests {
        Interests::CONNECTION | Interests::TRACK | Interests::DOWNLOAD
    }
    async fn render(&mut self, cx: &ActionCtx) {
        let style = cx.settings().download_style;
        push(cx, &names::download_icon(&style, is_connected(cx), false), None).await;
    }
    async fn on_state(&mut self, cx: &ActionCtx, ev: &StateEvent) {
        match ev {
            StateEvent::Download { active: true } => {
                let style = cx.settings().download_style;
                push(cx, &names::download_icon(&style, is_connected(cx), true), None).await;
            }
            _ => self.render(cx).await,
        }
    }
    async fn on_key_down(&mut self, cx: &ActionCtx) {
        let Some(ctl) = require_local(cx).await else {
            return;
        };
        if !ctl.is_connected() {
            cx.show_alert().await;
            return;
        }
        if self.busy.swap(true, Ordering::SeqCst) {
            return;
        }
        let track_id = ctl.snapshot().track.track_id;
        if track_id.trim().is_empty() {
            self.busy.store(false, Ordering::SeqCst);
            cx.show_alert().await;
            return;
        }
        if ctl.oauth_token().await.or_else(|| cx.shared.token()).filter(|t| !t.is_empty()).is_none() {
            self.busy.store(false, Ordering::SeqCst);
            cx.show_alert().await;
            return;
        }
        let shared = cx.shared.clone();
        let cx2 = cx.clone();
        let busy = self.busy.clone();
        tokio::spawn(async move {
            match crate::run_download(&shared, track_id).await {
                Ok(path) => {
                    tracing::info!("трек скачан: {}", path.display());
                    cx2.show_ok().await;
                }
                Err(e) => {
                    tracing::error!("скачивание не удалось: {e}");
                    cx2.show_alert().await;
                }
            }
            busy.store(false, Ordering::SeqCst);
        });
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::test_support::*;
    
    use sd_protocol::Outbound;
    
    
    use ym_model::{MediaState, TrackData};

    #[tokio::test]
    async fn download_render_sets_image() {
        let (shared, _c) = MockController { connected: true, state: MediaState::default(), calls: Default::default() }.shared();
        let expected = icon(&shared, "btn_yandex_music_download_v1.png");
        let (ctx, mut rx) = make_ctx(shared);
        Download::default().render(&ctx).await;
        match rx.recv().await.unwrap() {
            Outbound::SetImage { payload, .. } => assert_eq!(payload.image, expected),
            o => panic!("ждал SetImage, {o:?}"),
        }
    }

    #[tokio::test]
    async fn download_disconnected_alerts() {
        let (shared, _c) = MockController { connected: false, state: MediaState::default(), calls: Default::default() }.shared();
        let (ctx, mut rx) = make_ctx(shared);
        Download::default().on_key_down(&ctx).await;
        assert!(matches!(try_next(&mut rx).await, Some(Outbound::ShowAlert { .. })));
    }

    #[tokio::test]
    async fn download_empty_track_alerts() {
        let (shared, _c) = MockController { connected: true, state: MediaState::default(), calls: Default::default() }.shared();
        let (ctx, mut rx) = make_ctx(shared);
        Download::default().on_key_down(&ctx).await;
        assert!(matches!(try_next(&mut rx).await, Some(Outbound::ShowAlert { .. })));
    }

    #[tokio::test]
    async fn download_no_token_alerts_and_busy_recovers() {
        let state = MediaState { track: TrackData { track_id: "123".into(), ..Default::default() }, ..Default::default() };
        let (shared, _c) = MockController { connected: true, state, calls: Default::default() }.shared();
        let (ctx, mut rx) = make_ctx(shared);
        let mut act = Download::default();
        act.on_key_down(&ctx).await;
        assert!(matches!(try_next(&mut rx).await, Some(Outbound::ShowAlert { .. })));
        act.on_key_down(&ctx).await;
        assert!(matches!(try_next(&mut rx).await, Some(Outbound::ShowAlert { .. })));
    }

    #[tokio::test]
    async fn run_download_broadcasts_active_around_attempt() {
        let (shared, _c) =
            MockController { connected: true, state: MediaState::default(), calls: Default::default() }.shared();
        let mut bus = shared.subscribe();
        let res = crate::run_download(&shared, "123".to_owned()).await;
        assert!(res.is_err());
        assert_eq!(bus.try_recv().unwrap(), ym_model::StateEvent::Download { active: true });
        assert_eq!(bus.try_recv().unwrap(), ym_model::StateEvent::Download { active: false });
    }

    #[tokio::test]
    async fn download_on_state_active_then_idle_images() {
        let (shared, _c) =
            MockController { connected: true, state: MediaState::default(), calls: Default::default() }.shared();
        let loading = icon(&shared, "btn_yandex_music_download_v1_loading.png");
        let idle = icon(&shared, "btn_yandex_music_download_v1.png");
        let (ctx, mut rx) = make_ctx(shared);
        let mut act = Download::default();
        act.on_state(&ctx, &ym_model::StateEvent::Download { active: true }).await;
        match try_next(&mut rx).await {
            Some(Outbound::SetImage { payload, .. }) => assert_eq!(payload.image, loading),
            o => panic!("ждал SetImage loading, {o:?}"),
        }
        act.on_state(&ctx, &ym_model::StateEvent::Download { active: false }).await;
        match try_next(&mut rx).await {
            Some(Outbound::SetImage { payload, .. }) => assert_eq!(payload.image, idle),
            o => panic!("ждал SetImage idle, {o:?}"),
        }
    }

    #[test]
    fn download_interest_and_kind_mapping() {
        assert_eq!(
            ym_model::StateEvent::Download { active: true }.kind(),
            ym_model::StateKind::Download
        );
        assert!(crate::interest_of(ym_model::StateKind::Download).contains(crate::Interests::DOWNLOAD));
        assert!(Download::default().interests().contains(crate::Interests::DOWNLOAD));
    }
}
