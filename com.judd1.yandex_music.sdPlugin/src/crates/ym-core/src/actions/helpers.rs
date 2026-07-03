use std::sync::Arc;

use ym_model::{ControlMode, MediaState};

use crate::action::ActionCtx;
use ym_model::ports::{MediaController, VolumeAction};

pub(super) fn is_playing_local(cx: &ActionCtx) -> bool {
    local_ctl(cx).is_some_and(|c| c.is_connected() && c.snapshot().playback.is_playing)
}

pub fn clipboard_text(title: &str, artist: &str) -> Option<String> {
    if title.is_empty() || title == "Unknown" {
        return None;
    }
    Some(if artist.is_empty() {
        title.to_owned()
    } else {
        format!("{artist} - {title}")
    })
}

pub(super) fn local_ctl(cx: &ActionCtx) -> Option<&Arc<dyn MediaController>> {
    matches!(cx.settings().control_mode, ControlMode::Local).then(|| &cx.shared.cdp)
}

pub(super) fn is_connected(cx: &ActionCtx) -> bool {
    local_ctl(cx).is_some_and(|c| c.is_connected())
}

pub(super) fn snap(cx: &ActionCtx) -> MediaState {
    local_ctl(cx).map(|c| c.snapshot()).unwrap_or_default()
}

pub(super) async fn require_local(cx: &ActionCtx) -> Option<&Arc<dyn MediaController>> {
    match local_ctl(cx) {
        Some(c) => Some(c),
        None => {
            cx.show_alert().await;
            None
        }
    }
}

pub(super) async fn push(cx: &ActionCtx, filename: &str, state: Option<u8>) {
    if let Some(uri) = cx.shared.render.icon_b64(filename) {
        if let Some(st) = state {
            cx.set_state(st).await;
        }
        cx.set_image(uri.as_ref(), None).await;
    }
}

pub(super) async fn change_vol(cx: &ActionCtx, action: VolumeAction) {
    if let Some(c) = local_ctl(cx) {
        let _ = c.change_volume(action).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_text_matches_oracle() {
        assert_eq!(clipboard_text("Song", "Artist").as_deref(), Some("Artist - Song"));
        assert_eq!(clipboard_text("Song", "").as_deref(), Some("Song"));
        assert!(clipboard_text("", "Artist").is_none());
        assert!(clipboard_text("Unknown", "Artist").is_none());
    }
}
