
use async_trait::async_trait;

use crate::action::{Action, ActionCtx, Interests};
use crate::names;

use super::helpers::*;

pub struct Like;
pub struct Dislike;

#[async_trait]
impl Action for Like {
    fn interests(&self) -> Interests {
        Interests::CONNECTION | Interests::LIKE
    }
    async fn render(&mut self, cx: &ActionCtx) {
        let style = cx.settings().like_style;
        let (state, icon) = names::like_dislike("like", &style, is_connected(cx), snap(cx).like.is_liked);
        push(cx, &icon, state).await;
    }
    async fn on_key_down(&mut self, cx: &ActionCtx) {
        if let Some(c) = require_local(cx).await {
            let _ = c.toggle_like().await;
        }
    }
}

#[async_trait]
impl Action for Dislike {
    fn interests(&self) -> Interests {
        Interests::CONNECTION | Interests::DISLIKE
    }
    async fn render(&mut self, cx: &ActionCtx) {
        let style = cx.settings().dislike_style;
        let (state, icon) = names::like_dislike("dislike", &style, is_connected(cx), snap(cx).dislike.is_disliked);
        push(cx, &icon, state).await;
    }
    async fn on_key_down(&mut self, cx: &ActionCtx) {
        if let Some(c) = require_local(cx).await {
            let _ = c.toggle_dislike().await;
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::test_support::*;
    
    use sd_protocol::Outbound;
    
    
    use ym_model::{LikeData, MediaState};

    #[tokio::test]
    async fn like_connected_and_liked_sets_state_and_on_image() {
        let state = MediaState { like: LikeData { is_liked: true }, ..Default::default() };
        let (shared, _calls) = MockController { connected: true, state, calls: Default::default() }.shared();
        let expected = icon(&shared, "btn_yandex_music_like_v1_on.png");
        let (ctx, mut rx) = make_ctx(shared);

        Like.render(&ctx).await;
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
    async fn like_disconnected_pushes_loading_without_state() {
        let (shared, _c) = MockController { connected: false, state: MediaState::default(), calls: Default::default() }.shared();
        let expected = icon(&shared, "btn_yandex_music_like_v1_off_loading.png");
        let (ctx, mut rx) = make_ctx(shared);

        Like.render(&ctx).await;
        match rx.recv().await.unwrap() {
            Outbound::SetImage { payload, .. } => assert_eq!(payload.image, expected),
            o => panic!("ждал сразу SetImage (без SetState), {o:?}"),
        }
        assert!(try_next(&mut rx).await.is_none());
    }

    #[tokio::test]
    async fn like_key_down_calls_toggle_like() {
        let (shared, calls) = MockController { connected: true, state: MediaState::default(), calls: Default::default() }.shared();
        let (ctx, _rx) = make_ctx(shared);
        Like.on_key_down(&ctx).await;
        assert_eq!(*calls.lock().unwrap(), vec!["toggle_like"]);
    }
}
