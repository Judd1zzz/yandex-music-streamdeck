use crate::media::{DislikeData, LikeData, PlaybackData, RepeatData, ShuffleData, TrackData, VolumeData};

#[derive(Debug, Clone, PartialEq)]
pub enum StateEvent {
    Connection(bool),
    Track(TrackData),
    Playback(PlaybackData),
    Like(LikeData),
    Dislike(DislikeData),
    Volume(VolumeData),
    Shuffle(ShuffleData),
    Repeat(RepeatData),
    Download { active: bool },
    LaunchHint,
    UpdateHint,
    DiscordHint,
    RivalHint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateKind {
    Connection,
    Track,
    Playback,
    Like,
    Dislike,
    Volume,
    Shuffle,
    Repeat,
    Download,
    LaunchHint,
    UpdateHint,
    DiscordHint,
    RivalHint,
}

impl StateEvent {
    pub fn kind(&self) -> StateKind {
        match self {
            StateEvent::Connection(_) => StateKind::Connection,
            StateEvent::Track(_) => StateKind::Track,
            StateEvent::Playback(_) => StateKind::Playback,
            StateEvent::Like(_) => StateKind::Like,
            StateEvent::Dislike(_) => StateKind::Dislike,
            StateEvent::Volume(_) => StateKind::Volume,
            StateEvent::Shuffle(_) => StateKind::Shuffle,
            StateEvent::Repeat(_) => StateKind::Repeat,
            StateEvent::Download { .. } => StateKind::Download,
            StateEvent::LaunchHint => StateKind::LaunchHint,
            StateEvent::UpdateHint => StateKind::UpdateHint,
            StateEvent::DiscordHint => StateKind::DiscordHint,
            StateEvent::RivalHint => StateKind::RivalHint,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_hint_kind() {
        assert_eq!(StateEvent::UpdateHint.kind(), StateKind::UpdateHint);
        assert_eq!(StateEvent::LaunchHint.kind(), StateKind::LaunchHint);
    }

    #[test]
    fn shuffle_and_repeat_kinds() {
        assert_eq!(StateEvent::Shuffle(ShuffleData { on: true }).kind(), StateKind::Shuffle);
        assert_eq!(StateEvent::Repeat(RepeatData { mode: 2 }).kind(), StateKind::Repeat);
    }

    #[test]
    fn rival_hint_kind() {
        assert_eq!(StateEvent::RivalHint.kind(), StateKind::RivalHint);
    }
}
