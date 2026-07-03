use std::time::Duration;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::action::{Action, ActionCtx, Interests};
use ym_model::ports::VolumeAction;
use crate::names;

use super::helpers::*;

#[derive(Default)]
pub struct Mute {
    last_image: Option<String>,
}

#[async_trait]
impl Action for Mute {
    fn interests(&self) -> Interests {
        Interests::CONNECTION | Interests::VOLUME
    }
    async fn render(&mut self, cx: &ActionCtx) {
        let style = cx.settings().mute_style;
        let icon = names::mute_icon(&style, is_connected(cx), snap(cx).volume.is_muted);
        if self.last_image.as_deref() == Some(icon.as_str()) {
            return;
        }
        self.last_image = Some(icon.clone());
        push(cx, &icon, None).await;
    }
    async fn on_key_down(&mut self, cx: &ActionCtx) {
        if let Some(c) = require_local(cx).await {
            let _ = c.change_volume(VolumeAction::Mute).await;
        }
    }
}

async fn render_volume_level(cx: &ActionCtx, last_icon: &mut Option<String>) {
    let style = cx.settings().volume_style;
    if !is_connected(cx) {
        if last_icon.as_deref() != Some("loading") {
            *last_icon = Some("loading".to_owned());
            push(cx, &names::vol_level_icon(&style, 0, false), None).await;
        }
        return;
    }
    let pct = snap(cx).volume.current.round().clamp(0.0, 100.0) as u32;
    let variant = names::vol_level_variant(pct);
    let key = format!("{style}_{variant}");
    if last_icon.as_deref() != Some(key.as_str()) {
        *last_icon = Some(key);
        if let Some(uri) = cx.shared.render.icon_b64(&names::vol_level_icon(&style, variant, true)) {
            cx.set_image(uri.as_ref(), None).await;
        }
    }
    cx.set_title(format!("{pct}%")).await;
}

#[derive(Default)]
pub struct VolumeDisplay {
    last_icon: Option<String>,
}

#[async_trait]
impl Action for VolumeDisplay {
    fn interests(&self) -> Interests {
        Interests::CONNECTION | Interests::VOLUME
    }
    async fn on_settings(&mut self, cx: &ActionCtx) {
        self.last_icon = None;
        self.render(cx).await;
    }
    async fn render(&mut self, cx: &ActionCtx) {
        render_volume_level(cx, &mut self.last_icon).await;
    }
}

const KNOB_TARGET_FRESH: Duration = Duration::from_millis(1500);
const KNOB_PRESS_DEBOUNCE: Duration = Duration::from_millis(300);

#[derive(Default)]
pub struct VolumeKnob {
    target: Option<(u8, std::time::Instant)>,
    last_press: Option<std::time::Instant>,
    last_icon: Option<String>,
}

impl VolumeKnob {
    fn base_volume(&self, cx: &ActionCtx) -> i64 {
        match self.target {
            Some((t, at)) if at.elapsed() < KNOB_TARGET_FRESH => i64::from(t),
            _ => snap(cx).volume.current.round().clamp(0.0, 100.0) as i64,
        }
    }
    async fn press(&mut self, cx: &ActionCtx) {
        let now = std::time::Instant::now();
        if self.last_press.is_some_and(|t| now.duration_since(t) < KNOB_PRESS_DEBOUNCE) {
            return;
        }
        self.last_press = Some(now);
        let Some(c) = require_local(cx).await else { return };
        if cx.settings().knob_press == "playpause" {
            let _ = c.play_pause().await;
        } else {
            let _ = c.change_volume(VolumeAction::Mute).await;
        }
    }
}

#[async_trait]
impl Action for VolumeKnob {
    fn interests(&self) -> Interests {
        Interests::CONNECTION | Interests::VOLUME
    }
    async fn on_dial_rotate(&mut self, cx: &ActionCtx, ticks: i32) {
        let Some(c) = require_local(cx).await else { return };
        let step =
            i64::from(cx.settings().knob_step.clamp(ym_model::KNOB_STEP_MIN, ym_model::KNOB_STEP_MAX));
        let new = (self.base_volume(cx) + i64::from(ticks) * step).clamp(0, 100) as u8;
        self.target = Some((new, std::time::Instant::now()));
        let _ = c.change_volume(VolumeAction::Set(new)).await;
    }
    async fn on_dial_down(&mut self, cx: &ActionCtx) {
        self.press(cx).await;
    }
    async fn on_key_down(&mut self, cx: &ActionCtx) {
        self.press(cx).await;
    }
    async fn on_settings(&mut self, cx: &ActionCtx) {
        self.last_icon = None;
        self.render(cx).await;
    }
    async fn render(&mut self, cx: &ActionCtx) {
        render_volume_level(cx, &mut self.last_icon).await;
    }
}

pub struct VolumeStep {
    up: bool,
    repeat: Option<CancellationToken>,
}

impl VolumeStep {
    pub fn up() -> Self {
        Self { up: true, repeat: None }
    }
    pub fn down() -> Self {
        Self { up: false, repeat: None }
    }
    fn kind(&self) -> &'static str {
        if self.up {
            "vol_up"
        } else {
            "vol_down"
        }
    }
    fn dir(&self) -> VolumeAction {
        if self.up {
            VolumeAction::Up
        } else {
            VolumeAction::Down
        }
    }
    fn stop_repeat(&mut self) {
        if let Some(t) = self.repeat.take() {
            t.cancel();
        }
    }
}

const HOLD_DELAY: Duration = Duration::from_millis(500);
const HOLD_INTERVAL: Duration = Duration::from_millis(100);

#[async_trait]
impl Action for VolumeStep {
    async fn render(&mut self, cx: &ActionCtx) {
        let style = cx.settings().volume_style;
        push(cx, &names::vol_step_icon(self.kind(), &style), None).await;
        if local_ctl(cx).is_some() && !is_connected(cx) {
            push(cx, &names::vol_step_loading(self.kind(), &style), None).await;
        }
    }
    async fn on_key_down(&mut self, cx: &ActionCtx) {
        self.stop_repeat();
        if require_local(cx).await.is_none() {
            return;
        }
        let dir = self.dir();
        change_vol(cx, dir).await;
        let token = cx.cancel.child_token();
        self.repeat = Some(token.clone());
        let cx = cx.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = token.cancelled() => return,
                _ = tokio::time::sleep(HOLD_DELAY) => {}
            }
            loop {
                change_vol(&cx, dir).await;
                tokio::select! {
                    _ = token.cancelled() => break,
                    _ = tokio::time::sleep(HOLD_INTERVAL) => {}
                }
            }
        });
    }
    async fn on_key_up(&mut self, _cx: &ActionCtx) {
        self.stop_repeat();
    }
    async fn on_disappear(&mut self, _cx: &ActionCtx) {
        self.stop_repeat();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::test_support::*;
    
    use sd_protocol::Outbound;
    
    
    use ym_model::{MediaState, VolumeData};

    #[tokio::test]
    async fn mute_dedups_repeated_renders() {
        let (shared, _c) = MockController { connected: true, state: MediaState::default(), calls: Default::default() }.shared();
        let (ctx, mut rx) = make_ctx(shared);
        let mut m = Mute::default();
        m.render(&ctx).await;
        assert!(matches!(rx.recv().await.unwrap(), Outbound::SetImage { .. }));
        m.render(&ctx).await;
        assert!(try_next(&mut rx).await.is_none());
    }

    #[tokio::test]
    async fn volume_display_sets_image_and_title() {
        let state = MediaState {
            volume: VolumeData { current: 50.0, is_muted: false },
            ..Default::default()
        };
        let (shared, _c) = MockController { connected: true, state, calls: Default::default() }.shared();
        let expected = icon(&shared, "btn_yandex_music_vol_level_v1_2.png");
        let (ctx, mut rx) = make_ctx(shared);
        VolumeDisplay::default().render(&ctx).await;
        match rx.recv().await.unwrap() {
            Outbound::SetImage { payload, .. } => assert_eq!(payload.image, expected),
            o => panic!("{o:?}"),
        }
        match rx.recv().await.unwrap() {
            Outbound::SetTitle { payload, .. } => assert_eq!(payload.title, "50%"),
            o => panic!("{o:?}"),
        }
    }

    #[tokio::test]
    async fn volume_step_immediate_then_key_up_stops_repeat() {
        let (shared, calls) = MockController { connected: true, state: MediaState::default(), calls: Default::default() }.shared();
        let (ctx, _rx) = make_ctx(shared);
        let mut v = VolumeStep::up();
        v.on_key_down(&ctx).await;
        v.on_key_up(&ctx).await;
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert_eq!(*calls.lock().unwrap(), vec!["change_volume"]);
    }

    #[tokio::test]
    async fn knob_rotate_sets_volume_from_snapshot() {
        let (shared, vol, _p) = KnobMock::new(true, 50.0).shared();
        let (ctx, _rx) = make_ctx(shared);
        VolumeKnob::default().on_dial_rotate(&ctx, 1).await;
        assert_eq!(*vol.lock().unwrap(), vec![VolumeAction::Set(55)]);
    }

    #[tokio::test]
    async fn knob_rotate_accumulates_fast_rotation() {
        let (shared, vol, _p) = KnobMock::new(true, 50.0).shared();
        let (ctx, _rx) = make_ctx(shared);
        let mut act = VolumeKnob::default();
        act.on_dial_rotate(&ctx, 1).await;
        act.on_dial_rotate(&ctx, 1).await;
        act.on_dial_rotate(&ctx, 2).await;
        assert_eq!(
            *vol.lock().unwrap(),
            vec![VolumeAction::Set(55), VolumeAction::Set(60), VolumeAction::Set(70)]
        );
    }

    #[tokio::test]
    async fn knob_rotate_stale_target_falls_back_to_snapshot() {
        let (shared, vol, _p) = KnobMock::new(true, 50.0).shared();
        let (ctx, _rx) = make_ctx(shared);
        let mut act = VolumeKnob { target: Some((90, backdated(2))), ..Default::default() };
        act.on_dial_rotate(&ctx, 1).await;
        assert_eq!(*vol.lock().unwrap(), vec![VolumeAction::Set(55)]);
    }

    #[tokio::test]
    async fn knob_rotate_clamps_at_bounds() {
        let (shared, vol, _p) = KnobMock::new(true, 98.0).shared();
        let (ctx, _rx) = make_ctx(shared);
        let mut act = VolumeKnob::default();
        act.on_dial_rotate(&ctx, 1).await;
        act.on_dial_rotate(&ctx, 1).await;
        assert_eq!(*vol.lock().unwrap(), vec![VolumeAction::Set(100), VolumeAction::Set(100)]);

        let (shared, vol, _p) = KnobMock::new(true, 3.0).shared();
        let (ctx, _rx) = make_ctx(shared);
        VolumeKnob::default().on_dial_rotate(&ctx, -1).await;
        assert_eq!(*vol.lock().unwrap(), vec![VolumeAction::Set(0)]);
    }

    #[tokio::test]
    async fn knob_rotate_multi_ticks_and_custom_step() {
        let (shared, vol, _p) = KnobMock::new(true, 50.0).shared();
        let (ctx, _rx) = make_ctx_with(shared, knob_settings(10, "mute"));
        let mut act = VolumeKnob::default();
        act.on_dial_rotate(&ctx, -3).await;
        act.on_dial_rotate(&ctx, 2).await;
        assert_eq!(*vol.lock().unwrap(), vec![VolumeAction::Set(20), VolumeAction::Set(40)]);
    }

    #[tokio::test]
    async fn knob_press_default_mute() {
        let (shared, vol, plays) = KnobMock::new(true, 50.0).shared();
        let (ctx, _rx) = make_ctx(shared);
        VolumeKnob::default().on_dial_down(&ctx).await;
        assert_eq!(*vol.lock().unwrap(), vec![VolumeAction::Mute]);
        assert_eq!(*plays.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn knob_press_playpause_setting() {
        let (shared, vol, plays) = KnobMock::new(true, 50.0).shared();
        let (ctx, _rx) = make_ctx_with(shared, knob_settings(5, "playpause"));
        VolumeKnob::default().on_dial_down(&ctx).await;
        assert_eq!(*plays.lock().unwrap(), 1);
        assert!(vol.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn knob_press_debounced_within_300ms() {
        let (shared, vol, _p) = KnobMock::new(true, 50.0).shared();
        let (ctx, _rx) = make_ctx(shared);
        let mut act = VolumeKnob::default();
        act.on_dial_down(&ctx).await;
        act.on_key_down(&ctx).await;
        assert_eq!(vol.lock().unwrap().len(), 1, "дубль keyDown+dialDown должен схлопнуться");
        act.last_press = Some(backdated(1));
        act.on_key_down(&ctx).await;
        assert_eq!(vol.lock().unwrap().len(), 2, "после паузы нажатие снова работает");
    }

    #[tokio::test]
    async fn knob_key_down_alone_triggers_press() {
        let (shared, vol, _p) = KnobMock::new(true, 50.0).shared();
        let (ctx, _rx) = make_ctx(shared);
        VolumeKnob::default().on_key_down(&ctx).await;
        assert_eq!(*vol.lock().unwrap(), vec![VolumeAction::Mute]);
    }

    #[tokio::test]
    async fn knob_render_connected_percent_and_icon() {
        let (shared, _v, _p) = KnobMock::new(true, 50.0).shared();
        let expected = icon(&shared, "btn_yandex_music_vol_level_v1_2.png");
        let (ctx, mut rx) = make_ctx(shared);
        VolumeKnob::default().render(&ctx).await;
        match rx.recv().await.unwrap() {
            Outbound::SetImage { payload, .. } => assert_eq!(payload.image, expected),
            o => panic!("ждал SetImage, {o:?}"),
        }
        match rx.recv().await.unwrap() {
            Outbound::SetTitle { payload, .. } => assert_eq!(payload.title, "50%"),
            o => panic!("ждал SetTitle, {o:?}"),
        }
    }

    #[tokio::test]
    async fn knob_render_disconnected_loading_once() {
        let (shared, _v, _p) = KnobMock::new(false, 0.0).shared();
        let expected = icon(&shared, "btn_yandex_music_vol_level_v1_0_loading.png");
        let (ctx, mut rx) = make_ctx(shared);
        let mut act = VolumeKnob::default();
        act.render(&ctx).await;
        match rx.recv().await.unwrap() {
            Outbound::SetImage { payload, .. } => assert_eq!(payload.image, expected),
            o => panic!("ждал SetImage loading, {o:?}"),
        }
        act.render(&ctx).await;
        assert!(try_next(&mut rx).await.is_none(), "повторный рендер должен дедупиться");
    }

    #[tokio::test]
    async fn knob_rotate_ynison_alerts_and_no_command() {
        let (shared, vol, _p) = KnobMock::new(false, 50.0).shared();
        let (ctx, mut rx) = make_ctx_with(shared, ynison_settings());
        VolumeKnob::default().on_dial_rotate(&ctx, 1).await;
        assert!(matches!(rx.recv().await.unwrap(), Outbound::ShowAlert { .. }));
        assert!(vol.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn knob_interests_are_connection_and_volume() {
        assert_eq!(VolumeKnob::default().interests(), Interests::CONNECTION | Interests::VOLUME);
    }
}
