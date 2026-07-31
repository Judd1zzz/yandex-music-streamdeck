use async_trait::async_trait;

use crate::action::{Action, ActionCtx, Interests};
use crate::names;

use super::helpers::*;

pub struct Shuffle;
pub struct Repeat;

#[async_trait]
impl Action for Shuffle {
    fn interests(&self) -> Interests {
        Interests::CONNECTION | Interests::SHUFFLE
    }
    async fn render(&mut self, cx: &ActionCtx) {
        let style = cx.settings().shuffle_style;
        let (state, icon) = names::shuffle_icon(&style, is_connected(cx), snap(cx).shuffle.on);
        push(cx, &icon, state).await;
    }
    async fn on_key_down(&mut self, cx: &ActionCtx) {
        if let Some(c) = require_local(cx).await {
            let _ = c.toggle_shuffle().await;
        }
    }
}

#[async_trait]
impl Action for Repeat {
    fn interests(&self) -> Interests {
        Interests::CONNECTION | Interests::REPEAT
    }
    async fn render(&mut self, cx: &ActionCtx) {
        let style = cx.settings().repeat_style;
        let (state, icon) = names::repeat_icon(&style, is_connected(cx), snap(cx).repeat.mode);
        push(cx, &icon, state).await;
    }
    async fn on_key_down(&mut self, cx: &ActionCtx) {
        if let Some(c) = require_local(cx).await {
            let _ = c.toggle_repeat().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::test_support::*;

    use sd_protocol::Outbound;

    use ym_model::{MediaState, PluginSettings, RepeatData, ShuffleData};

    fn styled(shuffle: &str, repeat: &str) -> PluginSettings {
        PluginSettings { shuffle_style: shuffle.to_owned(), repeat_style: repeat.to_owned(), ..Default::default() }
    }

    #[tokio::test]
    async fn shuffle_connected_on_sets_state_and_image() {
        let state = MediaState { shuffle: ShuffleData { on: true }, ..Default::default() };
        let (shared, _c) = MockController { connected: true, state, calls: Default::default() }.shared();
        let expected = icon(&shared, "btn_yandex_music_shuffle_v1_on.png");
        let (ctx, mut rx) = make_ctx(shared);

        Shuffle.render(&ctx).await;
        match rx.recv().await.unwrap() {
            Outbound::SetState { payload, .. } => assert_eq!(payload.state, 1),
            o => panic!("ждал SetState, {o:?}"),
        }
        match rx.recv().await.unwrap() {
            Outbound::SetImage { payload, .. } => assert_eq!(payload.image, expected),
            o => panic!("ждал SetImage, {o:?}"),
        }
    }

    #[tokio::test]
    async fn shuffle_disconnected_pushes_loading_without_state() {
        let (shared, _c) =
            MockController { connected: false, state: MediaState::default(), calls: Default::default() }.shared();
        let expected = icon(&shared, "btn_yandex_music_shuffle_v2_off_loading.png");
        let (ctx, mut rx) = make_ctx_with(shared, styled("v2", "v1"));

        Shuffle.render(&ctx).await;
        match rx.recv().await.unwrap() {
            Outbound::SetImage { payload, .. } => assert_eq!(payload.image, expected),
            o => panic!("ждал сразу SetImage (без SetState), {o:?}"),
        }
        assert!(try_next(&mut rx).await.is_none());
    }

    #[tokio::test]
    async fn shuffle_key_down_calls_toggle_shuffle() {
        let (shared, calls) =
            MockController { connected: true, state: MediaState::default(), calls: Default::default() }.shared();
        let (ctx, _rx) = make_ctx(shared);
        Shuffle.on_key_down(&ctx).await;
        assert_eq!(*calls.lock().unwrap(), vec!["toggle_shuffle"]);
    }

    #[tokio::test]
    async fn shuffle_ynison_alerts_without_command() {
        let (shared, calls) =
            MockController { connected: true, state: MediaState::default(), calls: Default::default() }.shared();
        let (ctx, mut rx) = make_ctx_with(shared, ynison_settings());
        Shuffle.on_key_down(&ctx).await;
        assert!(calls.lock().unwrap().is_empty());
        match rx.recv().await.unwrap() {
            Outbound::ShowAlert { .. } => {}
            o => panic!("ждал ShowAlert, {o:?}"),
        }
    }

    #[tokio::test]
    async fn repeat_mode_two_sets_state_and_image() {
        let state = MediaState { repeat: RepeatData { mode: 2 }, ..Default::default() };
        let (shared, _c) = MockController { connected: true, state, calls: Default::default() }.shared();
        let expected = icon(&shared, "btn_yandex_music_repeat_v1_2.png");
        let (ctx, mut rx) = make_ctx(shared);

        Repeat.render(&ctx).await;
        match rx.recv().await.unwrap() {
            Outbound::SetState { payload, .. } => assert_eq!(payload.state, 2),
            o => panic!("ждал SetState, {o:?}"),
        }
        match rx.recv().await.unwrap() {
            Outbound::SetImage { payload, .. } => assert_eq!(payload.image, expected),
            o => panic!("ждал SetImage, {o:?}"),
        }
    }

    #[tokio::test]
    async fn repeat_disconnected_keeps_mode_in_loading_icon() {
        let state = MediaState { repeat: RepeatData { mode: 1 }, ..Default::default() };
        let (shared, _c) = MockController { connected: false, state, calls: Default::default() }.shared();
        let expected = icon(&shared, "btn_yandex_music_repeat_v1_1_loading.png");
        let (ctx, mut rx) = make_ctx(shared);

        Repeat.render(&ctx).await;
        match rx.recv().await.unwrap() {
            Outbound::SetImage { payload, .. } => assert_eq!(payload.image, expected),
            o => panic!("ждал сразу SetImage (без SetState), {o:?}"),
        }
        assert!(try_next(&mut rx).await.is_none());
    }

    #[tokio::test]
    async fn repeat_key_down_calls_toggle_repeat() {
        let (shared, calls) =
            MockController { connected: true, state: MediaState::default(), calls: Default::default() }.shared();
        let (ctx, _rx) = make_ctx_with(shared, styled("v1", "v2"));
        Repeat.on_key_down(&ctx).await;
        assert_eq!(*calls.lock().unwrap(), vec!["toggle_repeat"]);
    }
}
