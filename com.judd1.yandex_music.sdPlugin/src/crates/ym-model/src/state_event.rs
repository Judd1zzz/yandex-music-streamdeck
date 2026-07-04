use crate::media::{DislikeData, LikeData, PlaybackData, TrackData, VolumeData};

#[derive(Debug, Clone, PartialEq)]
pub enum StateEvent {
    Connection(bool),
    Track(TrackData),
    Playback(PlaybackData),
    Like(LikeData),
    Dislike(DislikeData),
    Volume(VolumeData),
    Download { active: bool },
    LaunchHint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateKind {
    Connection,
    Track,
    Playback,
    Like,
    Dislike,
    Volume,
    Download,
    LaunchHint,
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
            StateEvent::Download { .. } => StateKind::Download,
            StateEvent::LaunchHint => StateKind::LaunchHint,
        }
    }
}
