use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use ym_model::time::epoch_secs;
use ym_model::StateEvent;
use ym_render::{InfoInput, ProgressInput};

use crate::action::{Action, ActionCtx, Interests};

use super::helpers::*;

fn playback_ms(cx: &ActionCtx) -> (f64, f64) {
    if !is_connected(cx) {
        return (0.0, 0.0);
    }
    let pb = snap(cx).playback;
    let mut p = pb.current_sec * 1000.0;
    let d = pb.total_sec * 1000.0;
    if pb.is_playing && pb.timestamp > 0.0 {
        p += (epoch_secs() - pb.timestamp) * 1000.0;
        p = if d > 0.0 { p.min(d) } else { 0.0 };
    }
    (p, d)
}

pub struct Info {
    marquee_offset: u32,
    last_sig: String,
    needs_scroll: Arc<AtomicBool>,
    loop_token: Option<CancellationToken>,
}

impl Default for Info {
    fn default() -> Self {
        Self {
            marquee_offset: 0,
            last_sig: String::new(),
            needs_scroll: Arc::new(AtomicBool::new(false)),
            loop_token: None,
        }
    }
}

#[async_trait]
impl Action for Info {
    fn interests(&self) -> Interests {
        Interests::CONNECTION | Interests::TRACK | Interests::PLAYBACK
    }
    async fn on_state(&mut self, cx: &ActionCtx, ev: &StateEvent) {
        if matches!(ev, StateEvent::Playback(_)) && !cx.settings().show_timecodes {
            return;
        }
        self.render(cx).await;
    }
    async fn on_appear(&mut self, cx: &ActionCtx) {
        if let Some(t) = self.loop_token.take() {
            t.cancel();
        }
        let token = cx.child_token();
        self.loop_token = Some(token.clone());
        tokio::spawn(info_loop(cx.clone(), token, self.needs_scroll.clone()));
        self.render(cx).await;
    }
    async fn on_disappear(&mut self, _cx: &ActionCtx) {
        if let Some(t) = self.loop_token.take() {
            t.cancel();
        }
    }
    async fn on_tick(&mut self, cx: &ActionCtx) {
        self.marquee_offset = self.marquee_offset.wrapping_add(5);
        self.render(cx).await;
    }
    async fn render(&mut self, cx: &ActionCtx) {
        let s = cx.settings();
        let (title, artist, cover_url) = if is_connected(cx) {
            let t = snap(cx).track;
            (t.title, t.artist, t.cover_url)
        } else {
            ("Waiting...".to_owned(), String::new(), String::new())
        };
        let sig = format!("{title}|{artist}");
        if sig != self.last_sig {
            self.marquee_offset = 0;
            self.last_sig = sig;
        }
        let cover = if s.show_cover && !cover_url.is_empty() {
            cx.shared.render.cover(&cover_url).await
        } else {
            None
        };
        let (progress_ms, duration_ms) = playback_ms(cx);
        let input = InfoInput {
            cover,
            title,
            artist,
            marquee_offset: self.marquee_offset,
            show_cover: s.show_cover,
            show_title: s.show_title,
            show_artist: s.show_artist,
            show_timecodes: s.show_timecodes,
            progress_ms,
            duration_ms,
        };
        let renderers = cx.shared.render.clone();
        let (b64, needs) = tokio::task::spawn_blocking(move || renderers.render_info(input)).await.unwrap_or_default();
        self.needs_scroll.store(needs, Ordering::Relaxed);
        if !b64.is_empty() {
            cx.set_image(b64, None).await;
        }
    }
    async fn on_key_down(&mut self, cx: &ActionCtx) {
        match cx.settings().info_press.as_str() {
            "copy" => {
                let t = snap(cx).track;
                let Some(text) = clipboard_text(&t.title, &t.artist) else {
                    return;
                };
                let ok = tokio::task::spawn_blocking(move || {
                    arboard::Clipboard::new().and_then(|mut c| c.set_text(text)).is_ok()
                })
                .await
                .unwrap_or(false);
                if ok {
                    cx.show_ok().await;
                } else {
                    cx.show_alert().await;
                }
            }
            "none" => {}
            _ => {
                if let Some(c) = require_local(cx).await {
                    let _ = c.play_pause().await;
                }
            }
        }
    }
}

async fn info_loop(cx: ActionCtx, token: CancellationToken, needs_scroll: Arc<AtomicBool>) {
    loop {
        let dur = if !is_playing_local(&cx) {
            Duration::from_millis(500)
        } else if !needs_scroll.load(Ordering::Relaxed) {
            Duration::from_millis(1000)
        } else {
            Duration::from_millis(200)
        };
        tokio::select! {
            _ = token.cancelled() => break,
            _ = tokio::time::sleep(dur) => {}
        }
        if is_playing_local(&cx)
            && (needs_scroll.load(Ordering::Relaxed) || cx.settings().show_timecodes)
            && !cx.tick().await
        {
            break;
        }
    }
}


#[derive(Default)]
pub struct Progress {
    loop_token: Option<CancellationToken>,
}

#[async_trait]
impl Action for Progress {
    fn interests(&self) -> Interests {
        Interests::CONNECTION
    }
    async fn on_appear(&mut self, cx: &ActionCtx) {
        if let Some(t) = self.loop_token.take() {
            t.cancel();
        }
        let token = cx.child_token();
        self.loop_token = Some(token.clone());
        tokio::spawn(progress_loop(cx.clone(), token));
        self.render(cx).await;
    }
    async fn on_disappear(&mut self, _cx: &ActionCtx) {
        if let Some(t) = self.loop_token.take() {
            t.cancel();
        }
    }
    async fn render(&mut self, cx: &ActionCtx) {
        let s = cx.settings();
        let (progress_ms, duration_ms) = playback_ms(cx);
        let input = ProgressInput { progress_ms, duration_ms, mode: s.progress_mode };
        let renderers = cx.shared.render.clone();
        let b64 = tokio::task::spawn_blocking(move || renderers.render_progress(input)).await.unwrap_or_default();
        if !b64.is_empty() {
            cx.set_image(b64, None).await;
        }
    }
}

async fn progress_loop(cx: ActionCtx, token: CancellationToken) {
    loop {
        let dur = if is_playing_local(&cx) {
            Duration::from_millis(500)
        } else {
            Duration::from_millis(1000)
        };
        tokio::select! {
            _ = token.cancelled() => break,
            _ = tokio::time::sleep(dur) => {}
        }
        if is_playing_local(&cx) && !cx.tick().await {
            break;
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::test_support::*;
    
    use sd_protocol::Outbound;
    
    
    use ym_model::{MediaState, PlaybackData, TrackData};

    fn track_state(title: &str) -> MediaState {
        MediaState {
            track: TrackData { title: title.to_owned(), artist: "Artist".to_owned(), ..Default::default() },
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn info_render_emits_png_image() {
        let (shared, _c) = MockController { connected: true, state: track_state("Song"), calls: Default::default() }.shared();
        let (ctx, mut rx) = make_ctx(shared);
        Info::default().render(&ctx).await;
        match rx.recv().await.unwrap() {
            Outbound::SetImage { payload, .. } => assert!(payload.image.starts_with("data:image/png;base64,")),
            o => panic!("ждал SetImage, {o:?}"),
        }
    }

    #[tokio::test]
    async fn info_on_tick_advances_marquee() {
        let (shared, _c) = MockController { connected: true, state: track_state("Song"), calls: Default::default() }.shared();
        let (ctx, mut rx) = make_ctx(shared);
        let mut info = Info::default();
        info.render(&ctx).await;
        let _ = rx.recv().await;
        assert_eq!(info.marquee_offset, 0);
        info.on_tick(&ctx).await;
        assert_eq!(info.marquee_offset, 5);
        assert!(matches!(rx.recv().await.unwrap(), Outbound::SetImage { .. }));
    }

    #[tokio::test]
    async fn progress_render_emits_png_image() {
        let state = MediaState {
            playback: PlaybackData { current_sec: 30.0, total_sec: 180.0, is_playing: false, ..Default::default() },
            ..Default::default()
        };
        let (shared, _c) = MockController { connected: true, state, calls: Default::default() }.shared();
        let (ctx, mut rx) = make_ctx(shared);
        Progress::default().render(&ctx).await;
        match rx.recv().await.unwrap() {
            Outbound::SetImage { payload, .. } => assert!(payload.image.starts_with("data:image/png;base64,")),
            o => panic!("ждал SetImage, {o:?}"),
        }
    }

    #[tokio::test]
    async fn info_disconnected_still_renders_waiting() {
        let (shared, _c) = MockController { connected: false, state: MediaState::default(), calls: Default::default() }.shared();
        let (ctx, mut rx) = make_ctx(shared);
        Info::default().render(&ctx).await;
        assert!(matches!(rx.recv().await.unwrap(), Outbound::SetImage { .. }));
    }

    #[tokio::test]
    async fn info_press_default_playpause() {
        let (shared, calls) = MockController { connected: true, state: track_state("Song"), calls: Default::default() }.shared();
        let (ctx, _rx) = make_ctx(shared);
        Info::default().on_key_down(&ctx).await;
        assert_eq!(*calls.lock().unwrap(), vec!["play_pause"]);
    }

    #[tokio::test]
    async fn info_press_copy_runs_clipboard_not_controller() {
        let (shared, calls) = MockController { connected: true, state: track_state("Song"), calls: Default::default() }.shared();
        let (ctx, mut rx) = make_ctx_with(shared, info_settings("copy", false));
        Info::default().on_key_down(&ctx).await;
        assert!(calls.lock().unwrap().is_empty(), "copy не должен дергать контроллер");
        assert!(matches!(rx.recv().await.unwrap(), Outbound::ShowOk { .. } | Outbound::ShowAlert { .. }));
    }

    #[tokio::test]
    async fn info_press_none_is_silent() {
        let (shared, calls) = MockController { connected: true, state: track_state("Song"), calls: Default::default() }.shared();
        let (ctx, mut rx) = make_ctx_with(shared, info_settings("none", false));
        Info::default().on_key_down(&ctx).await;
        assert!(calls.lock().unwrap().is_empty());
        assert!(try_next(&mut rx).await.is_none());
    }

    #[tokio::test]
    async fn info_press_playpause_ynison_alerts() {
        let (shared, calls) = MockController { connected: false, state: MediaState::default(), calls: Default::default() }.shared();
        let (ctx, mut rx) = make_ctx_with(shared, ynison_settings());
        Info::default().on_key_down(&ctx).await;
        assert!(matches!(rx.recv().await.unwrap(), Outbound::ShowAlert { .. }));
        assert!(calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn info_on_state_playback_skipped_without_timecodes() {
        let (shared, _c) = MockController { connected: true, state: track_state("Song"), calls: Default::default() }.shared();
        let (ctx, mut rx) = make_ctx(shared.clone());
        Info::default().on_state(&ctx, &StateEvent::Playback(PlaybackData::default())).await;
        assert!(try_next(&mut rx).await.is_none(), "без тайм-кодов Playback игнорируется");
        let (ctx2, mut rx2) = make_ctx_with(shared, info_settings("playpause", true));
        Info::default().on_state(&ctx2, &StateEvent::Playback(PlaybackData::default())).await;
        assert!(matches!(rx2.recv().await.unwrap(), Outbound::SetImage { .. }));
    }

    #[tokio::test]
    async fn info_render_with_timecodes_emits_png() {
        let state = MediaState {
            track: TrackData { title: "Song".to_owned(), artist: "Artist".to_owned(), ..Default::default() },
            playback: PlaybackData { current_sec: 30.0, total_sec: 180.0, ..Default::default() },
            ..Default::default()
        };
        let (shared, _c) = MockController { connected: true, state, calls: Default::default() }.shared();
        let (ctx, mut rx) = make_ctx_with(shared, info_settings("playpause", true));
        Info::default().render(&ctx).await;
        match rx.recv().await.unwrap() {
            Outbound::SetImage { payload, .. } => assert!(payload.image.starts_with("data:image/png;base64,")),
            o => panic!("ждал SetImage, {o:?}"),
        }
    }

    #[tokio::test]
    async fn playback_ms_extrapolates_when_playing() {
        let playing = MediaState {
            playback: PlaybackData {
                current_sec: 30.0,
                total_sec: 180.0,
                is_playing: true,
                timestamp: epoch_secs() - 2.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let (shared, _c) = MockController { connected: true, state: playing, calls: Default::default() }.shared();
        let (ctx, _rx) = make_ctx(shared);
        let (p, d) = playback_ms(&ctx);
        assert_eq!(d, 180_000.0);
        assert!((31_900.0..=34_000.0).contains(&p), "ожидал экстраполяцию ~32с, получил {p}");

        let paused = MediaState {
            playback: PlaybackData { current_sec: 30.0, total_sec: 180.0, is_playing: false, timestamp: epoch_secs(), ..Default::default() },
            ..Default::default()
        };
        let (shared2, _c2) = MockController { connected: true, state: paused, calls: Default::default() }.shared();
        let (ctx2, _rx2) = make_ctx(shared2);
        assert_eq!(playback_ms(&ctx2), (30_000.0, 180_000.0));
    }
}
