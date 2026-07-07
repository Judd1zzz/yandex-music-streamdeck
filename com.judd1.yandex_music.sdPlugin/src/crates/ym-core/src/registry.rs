use crate::action::Action;
use crate::actions::{
    Dislike, Download, Info, Like, Mute, NextTrack, PlayPause, PrevTrack, Progress, VolumeDisplay, VolumeKnob,
    VolumeStep,
};

pub mod uuids {
    pub const PLAYPAUSE: &str = "com.judd1.yandex-music.action.playpause";
    pub const NEXT: &str = "com.judd1.yandex-music.action.next";
    pub const PREV: &str = "com.judd1.yandex-music.action.prev";
    pub const LIKE: &str = "com.judd1.yandex-music.action.like";
    pub const DISLIKE: &str = "com.judd1.yandex-music.action.dislike";
    pub const MUTE: &str = "com.judd1.yandex-music.action.mute";
    pub const VOLUME_UP: &str = "com.judd1.yandex-music.action.volumeup";
    pub const VOLUME_DOWN: &str = "com.judd1.yandex-music.action.volumedown";
    pub const VOLUME_DISPLAY: &str = "com.judd1.yandex-music.action.volume-display";
    pub const VOLUME_KNOB: &str = "com.judd1.yandex-music.action.volume-knob";
    pub const INFO: &str = "com.judd1.yandex-music.action.info";
    pub const PROGRESS: &str = "com.judd1.yandex-music.action.progress";
    pub const DOWNLOAD: &str = "com.judd1.yandex-music.action.download";
}

pub fn canonical_uuid(uuid: &str) -> String {
    uuid.replace('_', "-")
}

pub fn build_action(uuid: &str) -> Option<Box<dyn Action>> {
    use uuids::*;
    Some(match canonical_uuid(uuid).as_str() {
        PLAYPAUSE => Box::new(PlayPause::default()),
        NEXT => Box::new(NextTrack),
        PREV => Box::new(PrevTrack),
        LIKE => Box::new(Like),
        DISLIKE => Box::new(Dislike),
        MUTE => Box::new(Mute::default()),
        VOLUME_UP => Box::new(VolumeStep::up()),
        VOLUME_DOWN => Box::new(VolumeStep::down()),
        VOLUME_DISPLAY => Box::new(VolumeDisplay::default()),
        VOLUME_KNOB => Box::new(VolumeKnob::default()),
        INFO => Box::new(Info::default()),
        PROGRESS => Box::new(Progress::default()),
        DOWNLOAD => Box::new(Download::default()),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEGACY_UNDERSCORE_UUIDS: [&str; 13] = [
        "com.judd1.yandex_music.action.playpause",
        "com.judd1.yandex_music.action.next",
        "com.judd1.yandex_music.action.prev",
        "com.judd1.yandex_music.action.like",
        "com.judd1.yandex_music.action.dislike",
        "com.judd1.yandex_music.action.mute",
        "com.judd1.yandex_music.action.volumeup",
        "com.judd1.yandex_music.action.volumedown",
        "com.judd1.yandex_music.action.volume_display",
        "com.judd1.yandex_music.action.volume_knob",
        "com.judd1.yandex_music.action.info",
        "com.judd1.yandex_music.action.progress",
        "com.judd1.yandex_music.action.download",
    ];

    #[test]
    fn all_thirteen_dash_uuids_build_and_unknown_is_none() {
        use uuids::*;
        for u in [
            PLAYPAUSE, NEXT, PREV, LIKE, DISLIKE, MUTE, VOLUME_UP, VOLUME_DOWN, VOLUME_DISPLAY, VOLUME_KNOB, INFO,
            PROGRESS, DOWNLOAD,
        ] {
            assert!(build_action(u).is_some(), "{u} должен строиться");
        }
        assert!(build_action("com.judd1.yandex-music.action.unknown").is_none());
    }

    #[test]
    fn all_thirteen_legacy_underscore_uuids_still_build() {
        for u in LEGACY_UNDERSCORE_UUIDS {
            assert!(build_action(u).is_some(), "{u} должен строиться");
        }
        assert!(build_action("com.judd1.yandex_music.action.unknown").is_none());
    }

    #[test]
    fn canonical_uuid_maps_legacy_onto_constants_without_collisions() {
        use uuids::*;
        let dash = [
            PLAYPAUSE, NEXT, PREV, LIKE, DISLIKE, MUTE, VOLUME_UP, VOLUME_DOWN, VOLUME_DISPLAY, VOLUME_KNOB, INFO,
            PROGRESS, DOWNLOAD,
        ];
        for (legacy, expected) in LEGACY_UNDERSCORE_UUIDS.iter().zip(dash.iter()) {
            assert_eq!(canonical_uuid(legacy), *expected);
        }
        let mut canon: Vec<String> = dash.iter().map(|u| canonical_uuid(u)).collect();
        canon.sort();
        canon.dedup();
        assert_eq!(canon.len(), dash.len());
    }
}
