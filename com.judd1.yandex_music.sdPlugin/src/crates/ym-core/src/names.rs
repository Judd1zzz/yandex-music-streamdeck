pub fn pause_style(play_style: &str) -> &'static str {
    if play_style == "v1" || play_style == "v2" {
        "v1"
    } else {
        "v2"
    }
}

pub fn playpause(style: &str, connected: bool, is_playing: bool) -> (Option<u8>, String) {
    if !connected {
        return (None, format!("btn_yandex_music_play_{style}_loading.png"));
    }
    if !is_playing {
        (Some(1), format!("btn_yandex_music_play_{style}.png"))
    } else {
        (Some(0), format!("btn_yandex_music_pause_{}.png", pause_style(style)))
    }
}

pub fn skip_icon(kind: &str, style: &str, ready: bool) -> String {
    if ready {
        format!("btn_yandex_music_{kind}_{style}.png")
    } else {
        format!("btn_yandex_music_{kind}_{style}_loading.png")
    }
}

pub fn like_dislike(kind: &str, style: &str, connected: bool, is_active: bool) -> (Option<u8>, String) {
    if !connected {
        return (None, format!("btn_yandex_music_{kind}_{style}_off_loading.png"));
    }
    let onoff = if is_active { "on" } else { "off" };
    let state = if is_active { 1 } else { 0 };
    (Some(state), format!("btn_yandex_music_{kind}_{style}_{onoff}.png"))
}

pub fn mute_icon(style: &str, connected: bool, is_muted: bool) -> String {
    let onoff = if is_muted { "on" } else { "off" };
    if connected {
        format!("btn_yandex_music_mute_{style}_{onoff}.png")
    } else {
        format!("btn_yandex_music_mute_{style}_{onoff}_loading.png")
    }
}

pub fn vol_step_icon(kind: &str, style: &str) -> String {
    format!("btn_yandex_music_{kind}_{style}.png")
}

pub fn vol_step_loading(kind: &str, style: &str) -> String {
    format!("btn_yandex_music_{kind}_{style}_loading.png")
}

pub fn download_icon(style: &str, connected: bool, downloading: bool) -> String {
    if downloading || !connected {
        format!("btn_yandex_music_download_{style}_loading.png")
    } else {
        format!("btn_yandex_music_download_{style}.png")
    }
}

pub fn vol_level_variant(pct: u32) -> u8 {
    if pct == 0 {
        0
    } else if pct <= 29 {
        1
    } else {
        2
    }
}

pub fn vol_level_icon(style: &str, variant: u8, connected: bool) -> String {
    if connected {
        format!("btn_yandex_music_vol_level_{style}_{variant}.png")
    } else {
        format!("btn_yandex_music_vol_level_{style}_0_loading.png")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_style_remap() {
        assert_eq!(pause_style("v1"), "v1");
        assert_eq!(pause_style("v2"), "v1");
        assert_eq!(pause_style("v3"), "v2");
        assert_eq!(pause_style("v4"), "v2");
    }

    #[test]
    fn playpause_states_and_names() {
        assert_eq!(playpause("v3", false, true), (None, "btn_yandex_music_play_v3_loading.png".into()));
        assert_eq!(playpause("v3", true, false), (Some(1), "btn_yandex_music_play_v3.png".into()));
        assert_eq!(playpause("v3", true, true), (Some(0), "btn_yandex_music_pause_v2.png".into()));
        assert_eq!(playpause("v1", true, true), (Some(0), "btn_yandex_music_pause_v1.png".into()));
    }

    #[test]
    fn skip_names() {
        assert_eq!(skip_icon("next", "v2", true), "btn_yandex_music_next_v2.png");
        assert_eq!(skip_icon("prev", "v4", false), "btn_yandex_music_prev_v4_loading.png");
    }

    #[test]
    fn like_dislike_names() {
        assert_eq!(like_dislike("like", "v1", false, false), (None, "btn_yandex_music_like_v1_off_loading.png".into()));
        assert_eq!(like_dislike("like", "v2", true, true), (Some(1), "btn_yandex_music_like_v2_on.png".into()));
        assert_eq!(like_dislike("dislike", "v3", true, false), (Some(0), "btn_yandex_music_dislike_v3_off.png".into()));
    }

    #[test]
    fn mute_names() {
        assert_eq!(mute_icon("v1", true, true), "btn_yandex_music_mute_v1_on.png");
        assert_eq!(mute_icon("v1", true, false), "btn_yandex_music_mute_v1_off.png");
        assert_eq!(mute_icon("v4", false, false), "btn_yandex_music_mute_v4_off_loading.png");
    }

    #[test]
    fn volume_buckets_and_level_icons() {
        assert_eq!(vol_level_variant(0), 0);
        assert_eq!(vol_level_variant(1), 1);
        assert_eq!(vol_level_variant(29), 1);
        assert_eq!(vol_level_variant(30), 2);
        assert_eq!(vol_level_variant(100), 2);
        assert_eq!(vol_level_icon("v2", 2, true), "btn_yandex_music_vol_level_v2_2.png");
        assert_eq!(vol_level_icon("v2", 1, false), "btn_yandex_music_vol_level_v2_0_loading.png");
    }

    #[test]
    fn vol_step_names() {
        assert_eq!(vol_step_icon("vol_up", "v3"), "btn_yandex_music_vol_up_v3.png");
        assert_eq!(vol_step_loading("vol_down", "v1"), "btn_yandex_music_vol_down_v1_loading.png");
    }

    #[test]
    fn download_names() {
        assert_eq!(download_icon("v1", true, false), "btn_yandex_music_download_v1.png");
        assert_eq!(download_icon("v2", true, true), "btn_yandex_music_download_v2_loading.png");
        assert_eq!(download_icon("v1", false, false), "btn_yandex_music_download_v1_loading.png");
    }
}
