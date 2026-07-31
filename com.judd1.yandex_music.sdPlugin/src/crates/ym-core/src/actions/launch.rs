use async_trait::async_trait;

use crate::action::{Action, ActionCtx};

use super::helpers::push;

pub struct LaunchClient;

impl LaunchClient {
    async fn press(&self, cx: &ActionCtx) {
        if !cx.shared.cdp.is_connected() {
            cx.shared.launch_kick();
        }
        cx.show_ok().await;
    }
}

#[async_trait]
impl Action for LaunchClient {
    async fn render(&mut self, cx: &ActionCtx) {
        push(cx, "ym.png", None).await;
    }
    async fn on_key_down(&mut self, cx: &ActionCtx) {
        self.press(cx).await;
    }
    async fn on_dial_down(&mut self, cx: &ActionCtx) {
        self.press(cx).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::test_support::*;
    use sd_protocol::Outbound;
    use tokio::sync::mpsc;
    use ym_model::MediaState;

    fn shared_with_kick(
        connected: bool,
    ) -> (std::sync::Arc<crate::action::Shared>, mpsc::Receiver<()>) {
        let (shared, _c) = MockController { connected, state: MediaState::default(), calls: Default::default() }.shared();
        let (ktx, krx) = mpsc::channel(4);
        shared.set_launch_kick(ktx);
        (shared, krx)
    }

    #[tokio::test]
    async fn launch_render_pushes_ym_logo() {
        let (shared, _krx) = shared_with_kick(false);
        let expected = icon(&shared, "ym.png");
        let (ctx, mut rx) = make_ctx(shared);
        LaunchClient.render(&ctx).await;
        match rx.recv().await.unwrap() {
            Outbound::SetImage { payload, .. } => assert_eq!(payload.image, expected),
            o => panic!("ждал SetImage, {o:?}"),
        }
    }

    #[tokio::test]
    async fn launch_key_down_disconnected_kicks_and_oks() {
        let (shared, mut krx) = shared_with_kick(false);
        let (ctx, mut rx) = make_ctx(shared);
        LaunchClient.on_key_down(&ctx).await;
        assert!(krx.try_recv().is_ok(), "при отключённом клиенте должен уйти kick");
        assert!(matches!(rx.recv().await.unwrap(), Outbound::ShowOk { .. }));
    }

    #[tokio::test]
    async fn launch_key_down_connected_no_kick() {
        let (shared, mut krx) = shared_with_kick(true);
        let (ctx, mut rx) = make_ctx(shared);
        LaunchClient.on_key_down(&ctx).await;
        assert!(krx.try_recv().is_err(), "при подключённом клиенте kick не нужен");
        assert!(matches!(rx.recv().await.unwrap(), Outbound::ShowOk { .. }));
    }

    #[tokio::test]
    async fn launch_kicks_even_in_ynison_mode() {
        let (shared, mut krx) = shared_with_kick(false);
        let (ctx, _rx) = make_ctx_with(shared, ynison_settings());
        LaunchClient.on_key_down(&ctx).await;
        assert!(krx.try_recv().is_ok(), "режим управления не влияет на лаунчер");
    }

    #[tokio::test]
    async fn launch_dial_down_mirrors_key_down() {
        let (shared, mut krx) = shared_with_kick(false);
        let (ctx, mut rx) = make_ctx(shared);
        LaunchClient.on_dial_down(&ctx).await;
        assert!(krx.try_recv().is_ok());
        assert!(matches!(rx.recv().await.unwrap(), Outbound::ShowOk { .. }));
    }
}
