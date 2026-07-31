use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use ym_model::time::{day_index, hour_of_day, weekday_from_day};
use ym_model::StateEvent;

use crate::action::Shared;

const MIN_LISTEN_SECS: f64 = 20.0;
const RETAIN_DAYS: i64 = 31;
const TOP_N: usize = 10;
const MAP_CAP: usize = 200;
const FILE_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    Play,
    Next,
    Prev,
    Like,
    Dislike,
    Shuffle,
    Repeat,
    Download,
    Mute,
    Volume,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ActionCounts {
    #[serde(default)]
    pub play: u32,
    #[serde(default)]
    pub next: u32,
    #[serde(default)]
    pub prev: u32,
    #[serde(default)]
    pub like: u32,
    #[serde(default)]
    pub dislike: u32,
    #[serde(default)]
    pub shuffle: u32,
    #[serde(default)]
    pub repeat: u32,
    #[serde(default)]
    pub download: u32,
    #[serde(default)]
    pub mute: u32,
    #[serde(default)]
    pub volume: u32,
}

impl ActionCounts {
    fn bump(&mut self, k: ActionKind) {
        match k {
            ActionKind::Play => self.play += 1,
            ActionKind::Next => self.next += 1,
            ActionKind::Prev => self.prev += 1,
            ActionKind::Like => self.like += 1,
            ActionKind::Dislike => self.dislike += 1,
            ActionKind::Shuffle => self.shuffle += 1,
            ActionKind::Repeat => self.repeat += 1,
            ActionKind::Download => self.download += 1,
            ActionKind::Mute => self.mute += 1,
            ActionKind::Volume => self.volume += 1,
        }
    }
    fn add(&mut self, o: &ActionCounts) {
        self.play += o.play;
        self.next += o.next;
        self.prev += o.prev;
        self.like += o.like;
        self.dislike += o.dislike;
        self.shuffle += o.shuffle;
        self.repeat += o.repeat;
        self.download += o.download;
        self.mute += o.mute;
        self.volume += o.volume;
    }
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "play": self.play,
            "next": self.next,
            "prev": self.prev,
            "like": self.like,
            "dislike": self.dislike,
            "shuffle": self.shuffle,
            "repeat": self.repeat,
            "download": self.download,
            "mute": self.mute,
            "volume": self.volume,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TrackAgg {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub count: u32,
}

fn zero_hours() -> [u32; 24] {
    [0; 24]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DayBucket {
    pub day: i64,
    #[serde(default)]
    pub tracks_played: u32,
    #[serde(default)]
    pub downloads: u32,
    #[serde(default)]
    pub download_bytes: u64,
    #[serde(default)]
    pub sessions: u32,
    #[serde(default)]
    pub uptime_secs: u64,
    #[serde(default = "zero_hours")]
    pub by_hour: [u32; 24],
    #[serde(default)]
    pub actions: ActionCounts,
    #[serde(default)]
    pub artists: BTreeMap<String, u32>,
    #[serde(default)]
    pub tracks: BTreeMap<String, TrackAgg>,
}

impl DayBucket {
    fn new(day: i64) -> Self {
        Self {
            day,
            tracks_played: 0,
            downloads: 0,
            download_bytes: 0,
            sessions: 0,
            uptime_secs: 0,
            by_hour: [0; 24],
            actions: ActionCounts::default(),
            artists: BTreeMap::new(),
            tracks: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StatsFile {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub uptime_total_secs: u64,
    #[serde(default)]
    pub longest_session_secs: u64,
    #[serde(default)]
    pub days: Vec<DayBucket>,
}

struct LiveTrack {
    key: String,
    title: String,
    artist: String,
    started: f64,
}

pub struct StatsStore {
    dir: Option<PathBuf>,
    offset_secs: i32,
    file: StatsFile,
    dirty: bool,
    session_start: Option<f64>,
    last_accrued: Option<f64>,
    current: Option<LiveTrack>,
}

fn listen_key(track_id: &str, title: &str, artist: &str) -> String {
    let id = track_id.trim();
    if !id.is_empty() {
        return id.to_owned();
    }
    let t = title.trim();
    let a = artist.trim();
    if t.is_empty() && a.is_empty() {
        return String::new();
    }
    format!("{t}\u{1}{a}")
}

fn cap_counts(m: &mut BTreeMap<String, u32>, cap: usize) {
    if m.len() <= cap {
        return;
    }
    let mut v: Vec<(String, u32)> = m.iter().map(|(k, c)| (k.clone(), *c)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v.truncate(cap);
    *m = v.into_iter().collect();
}

fn cap_tracks(m: &mut BTreeMap<String, TrackAgg>, cap: usize) {
    if m.len() <= cap {
        return;
    }
    let mut v: Vec<(String, TrackAgg)> = m.iter().map(|(k, a)| (k.clone(), a.clone())).collect();
    v.sort_by(|a, b| b.1.count.cmp(&a.1.count).then_with(|| a.0.cmp(&b.0)));
    v.truncate(cap);
    *m = v.into_iter().collect();
}

impl StatsStore {
    pub fn new(offset_secs: i32) -> Self {
        Self {
            dir: None,
            offset_secs,
            file: StatsFile::default(),
            dirty: false,
            session_start: None,
            last_accrued: None,
            current: None,
        }
    }

    pub fn attach(&mut self, dir: PathBuf, offset_secs: i32, now: f64) {
        self.offset_secs = offset_secs;
        if let Ok(bytes) = std::fs::read(dir.join("stats.json"))
            && let Ok(f) = serde_json::from_slice::<StatsFile>(&bytes)
        {
            self.file = f;
        }
        self.dir = Some(dir);
        self.prune(now);
    }

    fn bucket(&mut self, day: i64) -> &mut DayBucket {
        if let Some(idx) = self.file.days.iter().position(|b| b.day == day) {
            return &mut self.file.days[idx];
        }
        self.file.days.push(DayBucket::new(day));
        self.file.days.last_mut().expect("just pushed")
    }

    pub fn record_action(&mut self, now: f64, kind: ActionKind) {
        let day = day_index(now, self.offset_secs);
        self.bucket(day).actions.bump(kind);
        self.dirty = true;
    }

    pub fn record_download(&mut self, now: f64, bytes: u64) {
        let day = day_index(now, self.offset_secs);
        {
            let b = self.bucket(day);
            b.downloads += 1;
            b.download_bytes += bytes;
        }
        self.dirty = true;
    }

    pub fn on_track(&mut self, track_id: &str, title: &str, artist: &str, now: f64) {
        let key = listen_key(track_id, title, artist);
        if self.current.as_ref().is_some_and(|c| c.key == key) {
            return;
        }
        self.finalize_current(now);
        self.current = if key.is_empty() {
            None
        } else {
            Some(LiveTrack { key, title: title.trim().to_owned(), artist: artist.trim().to_owned(), started: now })
        };
    }

    fn finalize_current(&mut self, now: f64) {
        let Some(cur) = self.current.take() else { return };
        if now - cur.started < MIN_LISTEN_SECS {
            return;
        }
        if cur.title.is_empty() && cur.artist.is_empty() {
            return;
        }
        let day = day_index(cur.started, self.offset_secs);
        let hour = hour_of_day(cur.started, self.offset_secs) as usize;
        {
            let b = self.bucket(day);
            b.tracks_played += 1;
            if hour < 24 {
                b.by_hour[hour] += 1;
            }
            if !cur.artist.is_empty() {
                *b.artists.entry(cur.artist.clone()).or_insert(0) += 1;
            }
            let tkey = format!("{}\u{1}{}", cur.title, cur.artist);
            let e = b
                .tracks
                .entry(tkey)
                .or_insert_with(|| TrackAgg { title: cur.title.clone(), artist: cur.artist.clone(), count: 0 });
            e.count += 1;
            cap_counts(&mut b.artists, MAP_CAP);
            cap_tracks(&mut b.tracks, MAP_CAP);
        }
        self.dirty = true;
    }

    pub fn on_connection(&mut self, connected: bool, now: f64) {
        if connected {
            if self.session_start.is_none() {
                self.session_start = Some(now);
                self.last_accrued = Some(now);
                let day = day_index(now, self.offset_secs);
                self.bucket(day).sessions += 1;
                self.dirty = true;
            }
        } else {
            self.finalize_current(now);
            self.accrue(now);
            self.session_start = None;
            self.last_accrued = None;
        }
    }

    fn accrue(&mut self, now: f64) {
        let (Some(start), Some(last)) = (self.session_start, self.last_accrued) else { return };
        let delta = now - last;
        if delta <= 0.0 {
            return;
        }
        let day = day_index(now, self.offset_secs);
        self.bucket(day).uptime_secs += delta as u64;
        self.file.uptime_total_secs += delta as u64;
        self.last_accrued = Some(now);
        let session_len = (now - start).max(0.0) as u64;
        if session_len > self.file.longest_session_secs {
            self.file.longest_session_secs = session_len;
        }
        self.dirty = true;
    }

    fn prune(&mut self, now: f64) {
        let today = day_index(now, self.offset_secs);
        let before = self.file.days.len();
        self.file.days.retain(|b| b.day <= today && today - b.day <= RETAIN_DAYS);
        if self.file.days.len() != before {
            self.dirty = true;
        }
    }

    pub fn tick(&mut self, now: f64) {
        self.accrue(now);
        self.prune(now);
    }

    pub fn flush(&mut self) {
        if !self.dirty {
            return;
        }
        let Some(dir) = self.dir.clone() else {
            self.dirty = false;
            return;
        };
        self.file.version = FILE_VERSION;
        let Ok(json) = serde_json::to_vec_pretty(&self.file) else { return };
        let tmp = dir.join("stats.json.tmp");
        if std::fs::write(&tmp, &json).is_ok() && std::fs::rename(&tmp, dir.join("stats.json")).is_ok() {
            self.dirty = false;
        }
    }

    pub fn finalize(&mut self, now: f64) {
        self.finalize_current(now);
        self.accrue(now);
        self.flush();
    }

    pub fn payload(&self, period: &str, now: f64) -> serde_json::Value {
        let today = day_index(now, self.offset_secs);
        let span: i64 = if period == "month" { 30 } else { 7 };
        let start = today - (span - 1);

        let mut tracks_played = 0u32;
        let mut sessions = 0u32;
        let mut downloads = 0u32;
        let mut download_bytes = 0u64;
        let mut uptime = 0u64;
        let mut actions = ActionCounts::default();
        let mut by_hour = [0u32; 24];
        let mut by_day = [0u32; 7];
        let mut artists: BTreeMap<String, u32> = BTreeMap::new();
        let mut tracks: BTreeMap<String, TrackAgg> = BTreeMap::new();

        for b in self.file.days.iter().filter(|b| b.day >= start && b.day <= today) {
            tracks_played += b.tracks_played;
            sessions += b.sessions;
            downloads += b.downloads;
            download_bytes += b.download_bytes;
            uptime += b.uptime_secs;
            actions.add(&b.actions);
            for (h, v) in b.by_hour.iter().enumerate() {
                by_hour[h] += v;
            }
            let wd = weekday_from_day(b.day) as usize;
            if wd < 7 {
                by_day[wd] += b.tracks_played;
            }
            for (k, v) in &b.artists {
                *artists.entry(k.clone()).or_insert(0) += v;
            }
            for (k, agg) in &b.tracks {
                let e = tracks
                    .entry(k.clone())
                    .or_insert_with(|| TrackAgg { title: agg.title.clone(), artist: agg.artist.clone(), count: 0 });
                e.count += agg.count;
            }
        }

        let current_secs = self.session_start.map(|s| (now - s).max(0.0) as u64).unwrap_or(0);
        let longest = self.file.longest_session_secs.max(current_secs);

        let mut art_vec: Vec<(String, u32)> = artists.into_iter().collect();
        art_vec.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        art_vec.truncate(TOP_N);
        let artists_json: Vec<serde_json::Value> =
            art_vec.into_iter().map(|(name, count)| serde_json::json!({"name": name, "count": count})).collect();

        let mut trk_vec: Vec<TrackAgg> = tracks.into_values().collect();
        trk_vec.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.title.cmp(&b.title)));
        trk_vec.truncate(TOP_N);
        let tracks_json: Vec<serde_json::Value> = trk_vec
            .into_iter()
            .map(|t| serde_json::json!({"title": t.title, "artist": t.artist, "count": t.count}))
            .collect();

        serde_json::json!({
            "event": "StatsData",
            "period": period,
            "tracksPlayed": tracks_played,
            "sessions": sessions,
            "downloads": downloads,
            "dlMB": download_bytes / 1_048_576,
            "uptimeMin": uptime / 60,
            "currentMin": current_secs / 60,
            "longestMin": longest / 60,
            "actions": actions.to_json(),
            "artists": artists_json,
            "tracks": tracks_json,
            "byHour": by_hour.to_vec(),
            "byDay": by_day.to_vec(),
        })
    }
}

pub fn spawn(
    shared: Arc<Shared>,
    events: broadcast::Receiver<StateEvent>,
    shutdown: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(run(shared, events, shutdown))
}

async fn run(shared: Arc<Shared>, mut events: broadcast::Receiver<StateEvent>, shutdown: CancellationToken) {
    let mut tick = tokio::time::interval(Duration::from_secs(30));
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            ev = events.recv() => match ev {
                Ok(StateEvent::Track(t)) => shared.stats_on_track(&t),
                Ok(StateEvent::Connection(c)) => shared.stats_on_connection(c),
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            },
            _ = tick.tick() => shared.stats_tick_and_flush(),
        }
    }
    shared.stats_finalize_and_flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: f64 = 86_400.0;
    const T0: f64 = 1_609_459_200.0;

    fn store() -> StatsStore {
        StatsStore::new(0)
    }

    fn play(s: &mut StatsStore, id: &str, title: &str, artist: &str, at: f64, listened: f64) {
        s.on_track(id, title, artist, at);
        s.on_track("stop", "", "", at + listened);
    }

    #[test]
    fn track_counted_only_past_threshold() {
        let mut s = store();
        play(&mut s, "1", "A", "X", T0, 25.0);
        play(&mut s, "2", "B", "Y", T0 + 100.0, 5.0);
        let p = s.payload("week", T0 + 200.0);
        assert_eq!(p["tracksPlayed"], 1);
    }

    #[test]
    fn same_id_reemitted_not_double_counted() {
        let mut s = store();
        s.on_track("1", "A", "X", T0);
        s.on_track("1", "A", "X", T0 + 30.0);
        s.on_track("2", "B", "Y", T0 + 60.0);
        let p = s.payload("week", T0 + 100.0);
        assert_eq!(p["tracksPlayed"], 1);
    }

    #[test]
    fn top_artists_and_tracks_sorted() {
        let mut s = store();
        play(&mut s, "1", "Song1", "Alpha", T0, 30.0);
        play(&mut s, "2", "Song2", "Alpha", T0 + 40.0, 30.0);
        play(&mut s, "3", "Song3", "Beta", T0 + 80.0, 30.0);
        let p = s.payload("week", T0 + 200.0);
        assert_eq!(p["artists"][0]["name"], "Alpha");
        assert_eq!(p["artists"][0]["count"], 2);
        assert_eq!(p["artists"][1]["name"], "Beta");
        assert_eq!(p["tracksPlayed"], 3);
    }

    #[test]
    fn by_hour_and_by_day_bucketing() {
        let mut s = store();
        play(&mut s, "1", "A", "X", T0 + 15.0 * 3600.0, 30.0);
        let p = s.payload("week", T0 + 15.0 * 3600.0 + 100.0);
        assert_eq!(p["byHour"][15], 1);
        assert_eq!(p["byDay"][4], 1);
    }

    #[test]
    fn downloads_and_volume() {
        let mut s = store();
        s.record_download(T0, 3 * 1_048_576);
        s.record_download(T0 + 10.0, 2 * 1_048_576);
        let p = s.payload("week", T0 + 20.0);
        assert_eq!(p["downloads"], 2);
        assert_eq!(p["dlMB"], 5);
    }

    #[test]
    fn actions_counted() {
        let mut s = store();
        s.record_action(T0, ActionKind::Like);
        s.record_action(T0 + 1.0, ActionKind::Like);
        s.record_action(T0 + 2.0, ActionKind::Next);
        s.record_action(T0 + 3.0, ActionKind::Shuffle);
        s.record_action(T0 + 4.0, ActionKind::Repeat);
        s.record_action(T0 + 5.0, ActionKind::Repeat);
        let p = s.payload("week", T0 + 10.0);
        assert_eq!(p["actions"]["like"], 2);
        assert_eq!(p["actions"]["next"], 1);
        assert_eq!(p["actions"]["mute"], 0);
        assert_eq!(p["actions"]["shuffle"], 1);
        assert_eq!(p["actions"]["repeat"], 2);
    }

    #[test]
    fn sessions_uptime_longest() {
        let mut s = store();
        s.on_connection(true, T0);
        s.on_connection(false, T0 + 600.0);
        s.on_connection(true, T0 + 1000.0);
        s.on_connection(false, T0 + 1120.0);
        let p = s.payload("week", T0 + 1200.0);
        assert_eq!(p["sessions"], 2);
        assert_eq!(p["uptimeMin"], (600 + 120) / 60);
        assert_eq!(p["longestMin"], 600 / 60);
    }

    #[test]
    fn current_session_live_in_payload() {
        let mut s = store();
        s.on_connection(true, T0);
        let p = s.payload("week", T0 + 180.0);
        assert_eq!(p["currentMin"], 3);
    }

    #[test]
    fn week_vs_month_window() {
        let mut s = store();
        play(&mut s, "old", "Old", "X", T0, 30.0);
        play(&mut s, "new", "New", "Y", T0 + 20.0 * DAY, 30.0);
        let now = T0 + 20.0 * DAY + 100.0;
        assert_eq!(s.payload("week", now)["tracksPlayed"], 1);
        assert_eq!(s.payload("month", now)["tracksPlayed"], 2);
    }

    #[test]
    fn persist_round_trip_and_prune() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut s = store();
            s.attach(dir.path().to_path_buf(), 0, T0);
            play(&mut s, "1", "A", "X", T0, 30.0);
            s.record_download(T0, 1_048_576);
            s.flush();
        }
        let mut s2 = store();
        s2.attach(dir.path().to_path_buf(), 0, T0 + DAY);
        let p = s2.payload("week", T0 + DAY);
        assert_eq!(p["tracksPlayed"], 1);
        assert_eq!(p["downloads"], 1);

        let mut s3 = store();
        s3.attach(dir.path().to_path_buf(), 0, T0 + 40.0 * DAY);
        assert_eq!(s3.payload("month", T0 + 40.0 * DAY)["tracksPlayed"], 0);
    }
}
