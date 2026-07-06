mod display;
mod download;
mod helpers;
mod likes;
mod transport;
mod volume;

#[cfg(test)]
mod test_support;

pub use display::{Info, Progress};
pub use download::Download;
pub use helpers::clipboard_text;
pub use likes::{Dislike, Like};
pub use transport::{NextTrack, PlayPause, PrevTrack};
pub use volume::{Mute, VolumeDisplay, VolumeKnob, VolumeStep};

#[cfg(test)]
mod tests {
    use super::test_support::*;
    
    
    use ym_model::MediaState;

    #[tokio::test]
    async fn ynison_report_status_missing_without_token() {
        let (shared, _c) = MockController { connected: false, state: MediaState::default(), calls: Default::default() }.shared();
        let (ctx, mut rx) = make_ctx_with(shared, ynison_settings());
        ctx.report_status().await;
        assert_pi_status(&rx.recv().await.unwrap(), "TokenStatus", "missing");
    }

    #[tokio::test]
    async fn ynison_report_status_offline_with_token() {
        let (shared, _c) = MockController { connected: false, state: MediaState::default(), calls: Default::default() }.shared();
        shared.set_token(Some("T".into()));
        let (ctx, mut rx) = make_ctx_with(shared, ynison_settings());
        ctx.report_status().await;
        assert_pi_status(&rx.recv().await.unwrap(), "TokenStatus", "offline");
    }

    #[tokio::test]
    async fn local_report_status_connected_when_cdp_up() {
        let (shared, _c) = MockController { connected: true, state: MediaState::default(), calls: Default::default() }.shared();
        let (ctx, mut rx) = make_ctx(shared);
        ctx.report_status().await;
        assert_pi_status(&rx.recv().await.unwrap(), "LocalStatus", "connected");
    }

    #[tokio::test]
    async fn local_report_status_includes_launch_reason() {
        let (shared, _c) = MockController { connected: false, state: MediaState::default(), calls: Default::default() }.shared();
        shared.set_launch_reason(Some("Клиент запущен от имени администратора".into()));
        let (ctx, mut rx) = make_ctx(shared.clone());
        ctx.report_status().await;
        let out = rx.recv().await.unwrap();
        assert_pi_status(&out, "LocalStatus", "disconnected");
        match &out {
            sd_protocol::Outbound::SendToPropertyInspector { payload, .. } => {
                assert_eq!(payload["reason"], "Клиент запущен от имени администратора");
            }
            other => panic!("ожидался SendToPropertyInspector, {other:?}"),
        }

        shared.set_launch_reason(None);
        ctx.report_status().await;
        match &rx.recv().await.unwrap() {
            sd_protocol::Outbound::SendToPropertyInspector { payload, .. } => {
                assert!(payload.get("reason").is_none(), "без причины поле reason не отправляется");
            }
            other => panic!("ожидался SendToPropertyInspector, {other:?}"),
        }
    }

    #[tokio::test]
    async fn apply_launch_reason_publishes_hint_once() {
        let (shared, _c) = MockController { connected: false, state: MediaState::default(), calls: Default::default() }.shared();
        let mut bus = shared.subscribe();
        shared.apply_launch_reason(Some("причина".into()));
        assert_eq!(bus.recv().await.unwrap(), ym_model::StateEvent::LaunchHint);
        shared.apply_launch_reason(Some("причина".into()));
        assert!(bus.try_recv().is_err(), "повтор той же причины не публикует событие");
        shared.apply_launch_reason(None);
        assert_eq!(bus.recv().await.unwrap(), ym_model::StateEvent::LaunchHint);
    }

    #[tokio::test]
    async fn apply_update_notice_publishes_hint_once() {
        let (shared, _c) = MockController { connected: false, state: MediaState::default(), calls: Default::default() }.shared();
        let mut bus = shared.subscribe();
        shared.apply_update_notice("2.1.3".into());
        assert_eq!(bus.recv().await.unwrap(), ym_model::StateEvent::UpdateHint);
        shared.apply_update_notice("2.1.3".into());
        assert!(bus.try_recv().is_err(), "повтор той же версии не публикует событие");
        shared.apply_update_notice("2.1.4".into());
        assert_eq!(bus.recv().await.unwrap(), ym_model::StateEvent::UpdateHint);
    }

    #[tokio::test]
    async fn report_status_appends_update_notice_local() {
        let (shared, _c) = MockController { connected: true, state: MediaState::default(), calls: Default::default() }.shared();
        let (ctx, mut rx) = make_ctx(shared.clone());
        ctx.report_status().await;
        assert_pi_status(&rx.recv().await.unwrap(), "LocalStatus", "connected");
        assert!(rx.try_recv().is_err(), "без обновления второго сообщения нет");

        shared.apply_update_notice("2.1.3".into());
        ctx.report_status().await;
        assert_pi_status(&rx.recv().await.unwrap(), "LocalStatus", "connected");
        match &rx.recv().await.unwrap() {
            sd_protocol::Outbound::SendToPropertyInspector { payload, .. } => {
                assert_eq!(payload["event"], "UpdateNotice");
                assert_eq!(payload["version"], "2.1.3");
            }
            other => panic!("ожидался SendToPropertyInspector, {other:?}"),
        }
    }

    #[tokio::test]
    async fn report_status_appends_update_notice_ynison() {
        let (shared, _c) = MockController { connected: false, state: MediaState::default(), calls: Default::default() }.shared();
        shared.apply_update_notice("3.0.0".into());
        let (ctx, mut rx) = make_ctx_with(shared, ynison_settings());
        ctx.report_status().await;
        assert_pi_status(&rx.recv().await.unwrap(), "TokenStatus", "missing");
        match &rx.recv().await.unwrap() {
            sd_protocol::Outbound::SendToPropertyInspector { payload, .. } => {
                assert_eq!(payload["event"], "UpdateNotice");
                assert_eq!(payload["version"], "3.0.0");
            }
            other => panic!("ожидался SendToPropertyInspector, {other:?}"),
        }
    }
}
