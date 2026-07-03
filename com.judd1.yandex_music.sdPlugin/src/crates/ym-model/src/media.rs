use serde::Deserialize;
use serde_json::Value;

use crate::state_event::StateEvent;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackData {
    pub title: String,
    pub artist: String,
    pub cover_url: String,
    pub track_id: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlaybackData {
    pub is_playing: bool,
    pub current_sec: f64,
    pub total_sec: f64,
    pub progress: f64,
    pub timestamp: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LikeData {
    pub is_liked: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DislikeData {
    pub is_disliked: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct VolumeData {
    pub current: f64,
    pub is_muted: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MediaState {
    pub track: TrackData,
    pub playback: PlaybackData,
    pub like: LikeData,
    pub dislike: DislikeData,
    pub volume: VolumeData,
}

fn id_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

#[derive(Deserialize, Default)]
struct RawTrack {
    #[serde(default)]
    title: String,
    #[serde(default)]
    artist: String,
    #[serde(default)]
    cover: String,
    #[serde(default)]
    id: Value,
}

#[derive(Deserialize, Default)]
struct RawState {
    #[serde(default)]
    playing: bool,
    #[serde(default)]
    liked: bool,
    #[serde(default)]
    disliked: bool,
}

#[derive(Deserialize, Default)]
struct RawProgress {
    #[serde(default)]
    now_sec: f64,
    #[serde(default)]
    total_sec: f64,
    #[serde(default)]
    ratio: f64,
}

#[derive(Deserialize, Default)]
struct RawVolume {
    #[serde(default)]
    current: f64,
    #[serde(default)]
    is_muted: bool,
}

#[derive(Deserialize, Default)]
struct RawFullState {
    #[serde(default)]
    track: RawTrack,
    #[serde(default)]
    state: RawState,
    #[serde(default)]
    progress: RawProgress,
    #[serde(default)]
    volume: RawVolume,
}

impl MediaState {
    pub fn from_full_value(v: &Value) -> Self {
        let raw: RawFullState = serde_json::from_value(v.clone()).unwrap_or_default();
        MediaState {
            track: TrackData {
                title: raw.track.title,
                artist: raw.track.artist,
                cover_url: raw.track.cover,
                track_id: id_to_string(&raw.track.id),
            },
            playback: PlaybackData {
                is_playing: raw.state.playing,
                current_sec: raw.progress.now_sec,
                total_sec: raw.progress.total_sec,
                progress: raw.progress.ratio,
                timestamp: 0.0,
            },
            like: LikeData { is_liked: raw.state.liked },
            dislike: DislikeData { is_disliked: raw.state.disliked },
            volume: VolumeData { current: raw.volume.current, is_muted: raw.volume.is_muted },
        }
    }

    pub fn apply_delta(&mut self, delta: &Value) -> Vec<StateEvent> {
        let mut events = Vec::new();

        if let Some(track) = delta.get("track") {
            let mut changed = false;
            if let Some(id) = track.get("id") {
                self.track.track_id = id_to_string(id);
                changed = true;
            }
            if let Some(t) = track.get("title").and_then(Value::as_str) {
                self.track.title = t.to_owned();
                changed = true;
            }
            if let Some(a) = track.get("artist").and_then(Value::as_str) {
                self.track.artist = a.to_owned();
                changed = true;
            }
            if let Some(c) = track.get("cover").and_then(Value::as_str) {
                self.track.cover_url = c.to_owned();
                changed = true;
            }
            if changed {
                events.push(StateEvent::Track(self.track.clone()));
            }
        }

        if let Some(state) = delta.get("state") {
            if let Some(p) = state.get("playing").and_then(Value::as_bool) {
                self.playback.is_playing = p;
                events.push(StateEvent::Playback(self.playback.clone()));
            }
            if let Some(l) = state.get("liked").and_then(Value::as_bool) {
                self.like.is_liked = l;
                events.push(StateEvent::Like(self.like.clone()));
            }
            if let Some(d) = state.get("disliked").and_then(Value::as_bool) {
                self.dislike.is_disliked = d;
                events.push(StateEvent::Dislike(self.dislike.clone()));
            }
        }

        if let Some(progress) = delta.get("progress") {
            let mut changed = false;
            if let Some(n) = progress.get("now_sec").and_then(Value::as_f64) {
                self.playback.current_sec = n;
                changed = true;
            }
            if let Some(t) = progress.get("total_sec").and_then(Value::as_f64) {
                self.playback.total_sec = t;
                changed = true;
            }
            if let Some(r) = progress.get("ratio").and_then(Value::as_f64) {
                self.playback.progress = r;
                changed = true;
            }
            if changed {
                events.push(StateEvent::Playback(self.playback.clone()));
            }
        }

        if let Some(volume) = delta.get("volume") {
            let mut changed = false;
            if let Some(c) = volume.get("current").and_then(Value::as_f64) {
                self.volume.current = c;
                changed = true;
            }
            if let Some(m) = volume.get("is_muted").and_then(Value::as_bool) {
                self.volume.is_muted = m;
                changed = true;
            }
            if changed {
                events.push(StateEvent::Volume(self.volume.clone()));
            }
        }

        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn full() -> Value {
        json!({
            "track": {"id": "12345", "title": "Faded", "artist": "Madonna", "cover": "https://x/400x400"},
            "state": {"playing": true, "liked": true, "disliked": false},
            "progress": {"now_sec": 38.0, "total_sec": 179.0, "ratio": 0.21},
            "volume": {"current": 28.0, "is_muted": false}
        })
    }

    #[test]
    fn full_state_remaps_keys() {
        let s = MediaState::from_full_value(&full());
        assert_eq!(s.track.title, "Faded");
        assert_eq!(s.track.cover_url, "https://x/400x400");
        assert_eq!(s.track.track_id, "12345");
        assert!(s.playback.is_playing);
        assert_eq!(s.playback.current_sec, 38.0);
        assert_eq!(s.playback.total_sec, 179.0);
        assert_eq!(s.playback.progress, 0.21);
        assert!(s.like.is_liked);
        assert!(!s.dislike.is_disliked);
        assert_eq!(s.volume.current, 28.0);
    }

    #[test]
    fn numeric_id_and_missing_fields_tolerated() {
        let s = MediaState::from_full_value(&json!({"track":{"id": 777}}));
        assert_eq!(s.track.track_id, "777");
        assert_eq!(s.track.title, "");
        assert!(!s.playback.is_playing);
        let empty = MediaState::from_full_value(&json!({}));
        assert_eq!(empty, MediaState::default());
    }

    #[test]
    fn delta_state_leaf_emits_targeted_events() {
        let mut s = MediaState::from_full_value(&full());
        let evs = s.apply_delta(&json!({"state": {"liked": false}}));
        assert!(!s.like.is_liked);
        assert_eq!(evs, vec![StateEvent::Like(LikeData { is_liked: false })]);
    }

    #[test]
    fn delta_track_merges_only_present_leaves() {
        let mut s = MediaState::from_full_value(&full());
        let evs = s.apply_delta(&json!({"track": {"title": "New Title"}}));
        assert_eq!(s.track.title, "New Title");
        assert_eq!(s.track.artist, "Madonna");
        assert_eq!(s.track.cover_url, "https://x/400x400");
        assert_eq!(evs.len(), 1);
        assert!(matches!(evs[0], StateEvent::Track(_)));
    }

    #[test]
    fn delta_progress_and_volume() {
        let mut s = MediaState::from_full_value(&full());
        let evs = s.apply_delta(&json!({"progress": {"now_sec": 40.0, "ratio": 0.23}, "volume": {"current": 33.0}}));
        assert_eq!(s.playback.current_sec, 40.0);
        assert_eq!(s.playback.progress, 0.23);
        assert_eq!(s.playback.total_sec, 179.0);
        assert_eq!(s.volume.current, 33.0);
        assert_eq!(evs.len(), 2);
    }

    #[test]
    fn empty_delta_emits_nothing() {
        let mut s = MediaState::from_full_value(&full());
        assert!(s.apply_delta(&json!({})).is_empty());
        assert!(s.apply_delta(&json!({"track": {}})).is_empty());
    }
}
