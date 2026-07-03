use crate::action::Action;
use crate::actions::{
    Dislike, Download, Info, Like, Mute, NextTrack, PlayPause, PrevTrack, Progress, VolumeDisplay, VolumeKnob,
    VolumeStep,
};

pub mod uuids {
    pub const PLAYPAUSE: &str = "com.judd1.yandex_music.action.playpause";
    pub const NEXT: &str = "com.judd1.yandex_music.action.next";
    pub const PREV: &str = "com.judd1.yandex_music.action.prev";
    pub const LIKE: &str = "com.judd1.yandex_music.action.like";
    pub const DISLIKE: &str = "com.judd1.yandex_music.action.dislike";
    pub const MUTE: &str = "com.judd1.yandex_music.action.mute";
    pub const VOLUME_UP: &str = "com.judd1.yandex_music.action.volumeup";
    pub const VOLUME_DOWN: &str = "com.judd1.yandex_music.action.volumedown";
    pub const VOLUME_DISPLAY: &str = "com.judd1.yandex_music.action.volume_display";
    pub const VOLUME_KNOB: &str = "com.judd1.yandex_music.action.volume_knob";
    pub const INFO: &str = "com.judd1.yandex_music.action.info";
    pub const PROGRESS: &str = "com.judd1.yandex_music.action.progress";
    pub const DOWNLOAD: &str = "com.judd1.yandex_music.action.download";
}

pub fn build_action(uuid: &str) -> Option<Box<dyn Action>> {
    use uuids::*;
    Some(match uuid {
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

    #[test]
    fn all_thirteen_uuids_build_and_unknown_is_none() {
        use uuids::*;
        for u in [
            PLAYPAUSE, NEXT, PREV, LIKE, DISLIKE, MUTE, VOLUME_UP, VOLUME_DOWN, VOLUME_DISPLAY, VOLUME_KNOB, INFO,
            PROGRESS, DOWNLOAD,
        ] {
            assert!(build_action(u).is_some(), "{u} должен строиться");
        }
        assert!(build_action("com.judd1.yandex_music.action.unknown").is_none());
    }
}
