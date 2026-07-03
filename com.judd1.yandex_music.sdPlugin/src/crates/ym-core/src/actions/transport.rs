
use async_trait::async_trait;
use ym_model::StateEvent;

use crate::action::{Action, ActionCtx, Interests};
use crate::names;

use super::helpers::*;

pub struct NextTrack;
pub struct PrevTrack;

#[async_trait]
impl Action for NextTrack {
    async fn render(&mut self, cx: &ActionCtx) {
        let style = cx.settings().next_style;
        push(cx, &names::skip_icon("next", &style, is_connected(cx)), None).await;
    }
    async fn on_key_down(&mut self, cx: &ActionCtx) {
        if let Some(c) = require_local(cx).await {
            let _ = c.next().await;
        }
    }
}

#[async_trait]
impl Action for PrevTrack {
    async fn render(&mut self, cx: &ActionCtx) {
        let style = cx.settings().prev_style;
        push(cx, &names::skip_icon("prev", &style, is_connected(cx)), None).await;
    }
    async fn on_key_down(&mut self, cx: &ActionCtx) {
        if let Some(c) = require_local(cx).await {
            let _ = c.prev().await;
        }
    }
}

#[derive(Default)]
pub struct PlayPause {
    last_is_playing: Option<bool>,
}

#[async_trait]
impl Action for PlayPause {
    fn interests(&self) -> Interests {
        Interests::CONNECTION | Interests::PLAYBACK
    }
    async fn render(&mut self, cx: &ActionCtx) {
        let style = cx.settings().play_style;
        let (state, icon) = names::playpause(&style, is_connected(cx), snap(cx).playback.is_playing);
        push(cx, &icon, state).await;
    }
    async fn on_state(&mut self, cx: &ActionCtx, ev: &StateEvent) {
        if let StateEvent::Playback(p) = ev {
            if self.last_is_playing == Some(p.is_playing) {
                return;
            }
            self.last_is_playing = Some(p.is_playing);
        }
        self.render(cx).await;
    }
    async fn on_key_down(&mut self, cx: &ActionCtx) {
        if let Some(c) = require_local(cx).await {
            let _ = c.play_pause().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::test_support::*;
    
    use sd_protocol::Outbound;
    
    
    use ym_model::{MediaState, PlaybackData};

    #[tokio::test]
    async fn playpause_playing_uses_pause_image_state_zero() {
        let state = MediaState {
            playback: PlaybackData { is_playing: true, ..Default::default() },
            ..Default::default()
        };
        let (shared, _c) = MockController { connected: true, state, calls: Default::default() }.shared();
        let expected = icon(&shared, "btn_yandex_music_pause_v1.png");
        let (ctx, mut rx) = make_ctx(shared);
        PlayPause::default().render(&ctx).await;
        match rx.recv().await.unwrap() {
            Outbound::SetState { payload, .. } => assert_eq!(payload.state, 0),
            o => panic!("{o:?}"),
        }
        match rx.recv().await.unwrap() {
            Outbound::SetImage { payload, .. } => assert_eq!(payload.image, expected),
            o => panic!("{o:?}"),
        }
    }

    #[tokio::test]
    async fn ynison_key_down_alerts_and_runs_no_command() {
        let (shared, calls) = MockController { connected: false, state: MediaState::default(), calls: Default::default() }.shared();
        let (ctx, mut rx) = make_ctx_with(shared, ynison_settings());
        NextTrack.on_key_down(&ctx).await;
        assert!(matches!(rx.recv().await.unwrap(), Outbound::ShowAlert { .. }));
        assert!(calls.lock().unwrap().is_empty(), "команда не должна выполняться в ynison v1");
    }
}
