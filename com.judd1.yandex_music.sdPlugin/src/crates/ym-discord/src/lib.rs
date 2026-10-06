use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{Activity, ActivityType, Assets, Button, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use ym_model::ports::MediaController;
use ym_model::{DiscordConfig, PlaybackData, StateEvent, TrackData};

const THROTTLE: Duration = Duration::from_secs(3);
const HEARTBEAT: Duration = Duration::from_secs(15);
const AFK: Duration = Duration::from_secs(15 * 60);
const TIME_TOLERANCE_MS: i64 = 2000;
const LARGE_TEXT: &str = "git: yandex-music-streamdeck";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PresenceModel {
    pub details: String,
    pub state: String,
    pub large_image: Option<String>,
    pub large_text: String,
    pub small_image: Option<String>,
    pub small_text: String,
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub buttons: Vec<(String, String)>,
}

impl PresenceModel {
    fn same_content(&self, o: &PresenceModel) -> bool {
        self.details == o.details
            && self.state == o.state
            && self.large_image == o.large_image
            && self.large_text == o.large_text
            && self.small_image == o.small_image
            && self.small_text == o.small_text
            && self.buttons == o.buttons
    }
}

fn discord_text(s: &str) -> String {
    let mut out: String = s.trim().chars().take(128).collect();
    if out.chars().count() == 1 {
        out.push(' ');
    }
    out
}

fn normalize_cover(c: &str) -> Option<String> {
    let c = c.trim();
    if c.is_empty() {
        return None;
    }
    let url = if c.contains("%%") { c.replace("%%", "400x400") } else { c.to_owned() };
    Some(if url.starts_with("http") { url } else { format!("https://{url}") })
}

pub fn build_presence(track: &TrackData, playback: Option<&PlaybackData>, now_ms: i64) -> PresenceModel {
    let playing = playback.map(|p| p.is_playing).unwrap_or(false);
    let (start, end) = match (playing, playback) {
        (true, Some(pb)) => {
            let s = now_ms - (pb.current_sec * 1000.0) as i64;
            let e = (pb.total_sec > 0.0).then(|| s + (pb.total_sec * 1000.0) as i64);
            (Some(s), e)
        }
        _ => (None, None),
    };
    PresenceModel {
        details: discord_text(&track.title),
        state: discord_text(&track.artist),
        large_image: normalize_cover(&track.cover_url),
        large_text: LARGE_TEXT.to_owned(),
        small_image: Some(if playing { "playing" } else { "paused" }.to_owned()),
        small_text: if playing { "Playing" } else { "Paused" }.to_owned(),
        start,
        end,
        buttons: Vec::new(),
    }
}

fn ms_diff(a: Option<i64>, b: Option<i64>) -> i64 {
    match (a, b) {
        (Some(x), Some(y)) => (x - y).abs(),
        (None, None) => 0,
        _ => i64::MAX,
    }
}

fn worth_sending(new: &PresenceModel, last: Option<&PresenceModel>) -> bool {
    match last {
        None => true,
        Some(prev) => {
            !new.same_content(prev)
                || ms_diff(new.start, prev.start) > TIME_TOLERANCE_MS
                || ms_diff(new.end, prev.end) > TIME_TOLERANCE_MS
        }
    }
}

pub trait DiscordSink {
    fn set(&mut self, presence: &PresenceModel) -> Result<(), String>;
    fn clear(&mut self) -> Result<(), String>;
}

pub trait SinkFactory {
    type Sink: DiscordSink;
    fn connect(&self, app_id: &str) -> Result<Self::Sink, String>;
}

struct RealFactory;
struct RealSink {
    client: DiscordIpcClient,
}

impl SinkFactory for RealFactory {
    type Sink = RealSink;
    fn connect(&self, app_id: &str) -> Result<RealSink, String> {
        tokio::task::block_in_place(|| {
            let mut client = DiscordIpcClient::new(app_id).map_err(|e| e.to_string())?;
            client.connect().map_err(|e| e.to_string())?;
            Ok(RealSink { client })
        })
    }
}

impl DiscordSink for RealSink {
    fn set(&mut self, p: &PresenceModel) -> Result<(), String> {
        let mut act = Activity::new().activity_type(ActivityType::Listening);
        if !p.details.is_empty() {
            act = act.details(&p.details);
        }
        if !p.state.is_empty() {
            act = act.state(&p.state);
        }
        let mut assets = Assets::new();
        if let Some(ref li) = p.large_image {
            assets = assets.large_image(li);
        }
        if !p.large_text.is_empty() {
            assets = assets.large_text(&p.large_text);
        }
        if let Some(ref si) = p.small_image {
            assets = assets.small_image(si);
        }
        if !p.small_text.is_empty() {
            assets = assets.small_text(&p.small_text);
        }
        act = act.assets(assets);
        if p.start.is_some() || p.end.is_some() {
            let mut ts = Timestamps::new();
            if let Some(s) = p.start {
                ts = ts.start(s);
            }
            if let Some(e) = p.end {
                ts = ts.end(e);
            }
            act = act.timestamps(ts);
        }
        let buttons: Vec<Button> = p.buttons.iter().map(|(l, u)| Button::new(l, u)).collect();
        if !buttons.is_empty() {
            act = act.buttons(buttons);
        }
        self.client.set_activity(act).map_err(|e| e.to_string())
    }
    fn clear(&mut self) -> Result<(), String> {
        self.client.clear_activity().map_err(|e| e.to_string())
    }
}

fn unix_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

struct Service<F: SinkFactory> {
    factory: F,
    cfg: DiscordConfig,
    track: Option<TrackData>,
    playback: Option<PlaybackData>,
    sink: Option<F::Sink>,
    sink_app_id: Option<String>,
    last_sent: Option<PresenceModel>,
    last_send_at: Option<Instant>,
    paused_since: Option<Instant>,
    afk_cleared: bool,
    status_tx: Option<watch::Sender<String>>,
    last_status: Option<String>,
    controller: Option<Arc<dyn MediaController>>,
}

impl<F: SinkFactory> Service<F> {
    fn new(factory: F, cfg: DiscordConfig) -> Self {
        Self {
            factory,
            cfg,
            track: None,
            playback: None,
            sink: None,
            sink_app_id: None,
            last_sent: None,
            last_send_at: None,
            paused_since: None,
            afk_cleared: false,
            status_tx: None,
            last_status: None,
            controller: None,
        }
    }

    fn set_status(&mut self, s: &str) {
        if self.last_status.as_deref() == Some(s) {
            return;
        }
        self.last_status = Some(s.to_owned());
        if let Some(tx) = &self.status_tx {
            let _ = tx.send(s.to_owned());
        }
    }

    fn on_event(&mut self, ev: StateEvent) {
        match ev {
            StateEvent::Track(t) => {
                if self.track.as_ref().is_none_or(|old| old.track_id != t.track_id) {
                    self.paused_since = None;
                    self.afk_cleared = false;
                }
                self.track = Some(t);
            }
            StateEvent::Playback(p) => {
                if p.is_playing {
                    self.paused_since = None;
                    self.afk_cleared = false;
                }
                self.playback = Some(p);
            }
            StateEvent::Connection(false) => self.teardown(),
            _ => {}
        }
    }

    fn on_config(&mut self, cfg: DiscordConfig) {
        self.cfg = cfg;
    }

    fn teardown(&mut self) {
        if let Some(mut s) = self.sink.take() {
            let _ = s.clear();
        }
        self.sink_app_id = None;
        self.last_sent = None;
        self.last_send_at = None;
    }

    fn reconcile(&mut self) {
        let Some(app_id) = self.cfg.enabled.then(|| self.cfg.app_id.clone()).flatten() else {
            self.teardown();
            self.set_status("off");
            return;
        };
        if self.sink.is_some() && self.sink_app_id.as_deref() != Some(app_id.as_str()) {
            self.teardown();
        }
        if self.sink.is_none() {
            match self.factory.connect(&app_id) {
                Ok(s) => {
                    self.sink = Some(s);
                    self.sink_app_id = Some(app_id);
                    self.last_sent = None;
                }
                Err(e) => {
                    tracing::debug!("discord connect failed: {e}");
                    self.set_status("error");
                    return;
                }
            }
        }
        let connected = self.controller.as_ref().map(|c| c.is_connected()).unwrap_or(true);
        if !connected {
            self.set_status("no_client");
            return;
        }
        let track = self.track.clone().or_else(|| {
            self.controller.as_ref().and_then(|c| {
                let t = c.snapshot().track;
                (!t.title.trim().is_empty() || !t.artist.trim().is_empty()).then_some(t)
            })
        });
        let Some(track) = track else {
            self.set_status("no_track");
            return;
        };
        if track.title.trim().is_empty() && track.artist.trim().is_empty() {
            self.set_status("no_track");
            return;
        }
        self.set_status("active");
        let playing = self.playback.as_ref().map(|p| p.is_playing).unwrap_or(false);
        if !playing {
            let since = *self.paused_since.get_or_insert_with(Instant::now);
            if since.elapsed() >= AFK {
                if !self.afk_cleared {
                    if let Some(s) = self.sink.as_mut() {
                        let _ = s.clear();
                    }
                    self.last_sent = None;
                    self.afk_cleared = true;
                }
                return;
            }
        }
        if self.afk_cleared {
            return;
        }
        let presence = build_presence(&track, self.playback.as_ref(), unix_millis());
        let changed = worth_sending(&presence, self.last_sent.as_ref());
        let stale = self.last_send_at.map(|t| t.elapsed() >= HEARTBEAT).unwrap_or(true);
        if !changed && !stale {
            return;
        }
        if self.last_send_at.is_some_and(|t| t.elapsed() < THROTTLE) {
            return;
        }
        let mut failed = false;
        if let Some(s) = self.sink.as_mut() {
            match s.set(&presence) {
                Ok(()) => {
                    self.last_sent = Some(presence);
                    self.last_send_at = Some(Instant::now());
                }
                Err(e) => {
                    tracing::debug!("discord set failed: {e}");
                    self.sink = None;
                    self.sink_app_id = None;
                    failed = true;
                }
            }
        }
        if failed {
            self.set_status("error");
        }
    }
}

async fn run<F: SinkFactory>(
    mut state_rx: broadcast::Receiver<StateEvent>,
    mut cfg_rx: watch::Receiver<DiscordConfig>,
    factory: F,
    shutdown: CancellationToken,
    status_tx: Option<watch::Sender<String>>,
    controller: Option<Arc<dyn MediaController>>,
) {
    let mut svc = Service::new(factory, cfg_rx.borrow().clone());
    svc.status_tx = status_tx;
    svc.controller = controller;
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            ev = state_rx.recv() => match ev {
                Ok(e) => svc.on_event(e),
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            },
            ch = cfg_rx.changed() => {
                if ch.is_err() {
                    break;
                }
                let c = cfg_rx.borrow_and_update().clone();
                svc.on_config(c);
            }
            _ = tick.tick() => {}
        }
        svc.reconcile();
    }
    svc.teardown();
}

pub fn spawn(
    state_rx: broadcast::Receiver<StateEvent>,
    cfg_rx: watch::Receiver<DiscordConfig>,
    shutdown: CancellationToken,
    status_tx: watch::Sender<String>,
    controller: Arc<dyn MediaController>,
) -> JoinHandle<()> {
    tokio::spawn(run(state_rx, cfg_rx, RealFactory, shutdown, Some(status_tx), Some(controller)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn track() -> TrackData {
        TrackData {
            title: "Faded".to_owned(),
            artist: "Alan Walker".to_owned(),
            cover_url: "https://avatars.yandex.net/get-music-content/x/400x400".to_owned(),
            track_id: "12345".to_owned(),
        }
    }

    fn playing(cur: f64, total: f64) -> PlaybackData {
        PlaybackData { is_playing: true, current_sec: cur, total_sec: total, progress: cur / total, timestamp: 0.0 }
    }

    #[test]
    fn presence_maps_track_to_listening_fields() {
        let p = build_presence(&track(), Some(&playing(30.0, 180.0)), 1_000_000);
        assert_eq!(p.details, "Faded");
        assert_eq!(p.state, "Alan Walker");
        assert_eq!(p.large_image.as_deref(), Some("https://avatars.yandex.net/get-music-content/x/400x400"));
        assert_eq!(p.small_image.as_deref(), Some("playing"));
        assert_eq!(p.start, Some(1_000_000 - 30_000));
        assert_eq!(p.end, Some(1_000_000 - 30_000 + 180_000));
        assert!(p.buttons.is_empty());
    }

    #[test]
    fn paused_has_no_timestamps_and_paused_icon() {
        let mut pb = playing(30.0, 180.0);
        pb.is_playing = false;
        let p = build_presence(&track(), Some(&pb), 1_000_000);
        assert_eq!(p.start, None);
        assert_eq!(p.end, None);
        assert_eq!(p.small_image.as_deref(), Some("paused"));
    }

    #[test]
    fn cover_percent_template_normalized() {
        let mut t = track();
        t.cover_url = "avatars.yandex.net/get-music-content/x/%%".to_owned();
        let p = build_presence(&t, None, 0);
        assert_eq!(p.large_image.as_deref(), Some("https://avatars.yandex.net/get-music-content/x/400x400"));
        let mut empty = track();
        empty.cover_url = String::new();
        assert_eq!(build_presence(&empty, None, 0).large_image, None);
    }

    #[test]
    fn presence_never_has_buttons() {
        assert!(build_presence(&track(), Some(&playing(30.0, 180.0)), 1_000_000).buttons.is_empty());
        let mut t = track();
        t.track_id = String::new();
        assert!(build_presence(&t, None, 0).buttons.is_empty());
    }

    #[test]
    fn dedup_ignores_small_time_drift_but_catches_seek() {
        let a = build_presence(&track(), Some(&playing(30.0, 180.0)), 1_000_000);
        let near = build_presence(&track(), Some(&playing(30.5, 180.0)), 1_000_400);
        assert!(!worth_sending(&near, Some(&a)));
        let seek = build_presence(&track(), Some(&playing(90.0, 180.0)), 1_000_000);
        assert!(worth_sending(&seek, Some(&a)));
        assert!(worth_sending(&a, None));
    }

    #[test]
    fn dedup_catches_track_change() {
        let a = build_presence(&track(), Some(&playing(30.0, 180.0)), 1_000_000);
        let mut t2 = track();
        t2.title = "Other".to_owned();
        let b = build_presence(&t2, Some(&playing(30.0, 180.0)), 1_000_000);
        assert!(worth_sending(&b, Some(&a)));
    }

    #[derive(Debug, Clone, PartialEq)]
    enum Op {
        Connect(String),
        Set(String),
        Clear,
    }

    #[derive(Default)]
    struct Log {
        ops: Arc<Mutex<Vec<Op>>>,
        fail_connect: bool,
    }

    struct MockSink {
        ops: Arc<Mutex<Vec<Op>>>,
    }
    impl DiscordSink for MockSink {
        fn set(&mut self, p: &PresenceModel) -> Result<(), String> {
            self.ops.lock().unwrap().push(Op::Set(p.details.clone()));
            Ok(())
        }
        fn clear(&mut self) -> Result<(), String> {
            self.ops.lock().unwrap().push(Op::Clear);
            Ok(())
        }
    }

    struct MockFactory {
        ops: Arc<Mutex<Vec<Op>>>,
        fail: bool,
    }
    impl SinkFactory for MockFactory {
        type Sink = MockSink;
        fn connect(&self, app_id: &str) -> Result<MockSink, String> {
            self.ops.lock().unwrap().push(Op::Connect(app_id.to_owned()));
            if self.fail {
                return Err("nope".to_owned());
            }
            Ok(MockSink { ops: self.ops.clone() })
        }
    }

    fn service(enabled: bool, app: Option<&str>, log: &Log) -> Service<MockFactory> {
        let cfg = DiscordConfig { enabled, app_id: app.map(str::to_owned) };
        Service::new(MockFactory { ops: log.ops.clone(), fail: log.fail_connect }, cfg)
    }

    #[test]
    fn reconcile_connects_and_sets_once_then_dedupes() {
        let log = Log::default();
        let mut svc = service(true, Some("app1"), &log);
        svc.on_event(StateEvent::Track(track()));
        svc.on_event(StateEvent::Playback(playing(10.0, 100.0)));
        svc.reconcile();
        svc.reconcile();
        let ops = log.ops.lock().unwrap().clone();
        assert_eq!(ops[0], Op::Connect("app1".to_owned()));
        assert_eq!(ops.iter().filter(|o| matches!(o, Op::Set(_))).count(), 1);
    }

    #[test]
    fn reconcile_reasserts_after_heartbeat() {
        let log = Log::default();
        let mut svc = service(true, Some("app1"), &log);
        svc.on_event(StateEvent::Track(track()));
        svc.on_event(StateEvent::Playback(playing(10.0, 100.0)));
        svc.reconcile();
        svc.last_send_at = Some(Instant::now() - HEARTBEAT - Duration::from_secs(1));
        svc.reconcile();
        let sets = log.ops.lock().unwrap().iter().filter(|o| matches!(o, Op::Set(_))).count();
        assert_eq!(sets, 2, "после heartbeat presence должен пере-отправиться");
    }

    #[test]
    fn reconcile_disabled_clears_and_drops_sink() {
        let log = Log::default();
        let mut svc = service(true, Some("app1"), &log);
        svc.on_event(StateEvent::Track(track()));
        svc.reconcile();
        svc.on_config(DiscordConfig { enabled: false, app_id: Some("app1".to_owned()) });
        svc.reconcile();
        let ops = log.ops.lock().unwrap().clone();
        assert!(ops.contains(&Op::Clear));
        assert!(svc.sink.is_none());
    }

    #[test]
    fn reconcile_app_id_change_reconnects() {
        let log = Log::default();
        let mut svc = service(true, Some("app1"), &log);
        svc.on_event(StateEvent::Track(track()));
        svc.reconcile();
        svc.on_config(DiscordConfig { enabled: true, app_id: Some("app2".to_owned()) });
        svc.reconcile();
        let ops = log.ops.lock().unwrap().clone();
        let connects: Vec<_> = ops.iter().filter(|o| matches!(o, Op::Connect(_))).cloned().collect();
        assert_eq!(connects, vec![Op::Connect("app1".to_owned()), Op::Connect("app2".to_owned())]);
    }

    #[test]
    fn reconcile_no_app_id_does_nothing() {
        let log = Log::default();
        let mut svc = service(true, None, &log);
        svc.on_event(StateEvent::Track(track()));
        svc.reconcile();
        assert!(log.ops.lock().unwrap().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn run_exits_on_cancel_and_clears_presence() {
        let log = Log::default();
        let ops = log.ops.clone();
        let (state_tx, state_rx) = broadcast::channel(16);
        let (_cfg_tx, cfg_rx) =
            watch::channel(DiscordConfig { enabled: true, app_id: Some("app1".to_owned()) });
        let shutdown = CancellationToken::new();
        let factory = MockFactory { ops: ops.clone(), fail: false };
        let task = tokio::spawn(run(state_rx, cfg_rx, factory, shutdown.clone(), None, None));

        state_tx.send(StateEvent::Track(track())).unwrap();
        state_tx.send(StateEvent::Playback(playing(10.0, 100.0))).unwrap();
        for _ in 0..50 {
            if ops.lock().unwrap().iter().any(|o| matches!(o, Op::Set(_))) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(
            ops.lock().unwrap().iter().any(|o| matches!(o, Op::Set(_))),
            "presence должен успеть выставиться до cancel"
        );

        shutdown.cancel();
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("run должен завершиться по cancel")
            .unwrap();
        let last = ops.lock().unwrap().last().cloned();
        assert_eq!(last, Some(Op::Clear), "teardown обязан снять presence при выходе");
    }

    #[test]
    fn connection_lost_tears_down() {
        let log = Log::default();
        let mut svc = service(true, Some("app1"), &log);
        svc.on_event(StateEvent::Track(track()));
        svc.reconcile();
        svc.on_event(StateEvent::Connection(false));
        assert!(svc.sink.is_none());
        assert!(log.ops.lock().unwrap().contains(&Op::Clear));
    }

    fn paused() -> PlaybackData {
        PlaybackData { is_playing: false, ..playing(30.0, 180.0) }
    }

    fn cleared_by_afk(log: &Log) -> Service<MockFactory> {
        let mut svc = service(true, Some("app1"), log);
        svc.on_event(StateEvent::Track(track()));
        svc.on_event(StateEvent::Playback(paused()));
        svc.reconcile();
        svc.paused_since = Some(Instant::now() - AFK - Duration::from_secs(1));
        svc.reconcile();
        assert!(svc.afk_cleared, "пауза дольше AFK обязана убрать presence");
        svc
    }

    fn sets_since(log: &Log, mark: usize) -> usize {
        log.ops.lock().unwrap()[mark..].iter().filter(|o| matches!(o, Op::Set(_))).count()
    }

    #[test]
    fn reinjection_burst_does_not_bring_back_presence_cleared_by_afk() {
        let log = Log::default();
        let mut svc = cleared_by_afk(&log);
        let mark = log.ops.lock().unwrap().len();
        svc.on_event(StateEvent::Connection(false));
        svc.on_event(StateEvent::Connection(true));
        let mut same = track();
        same.cover_url = "https://avatars.yandex.net/get-music-content/y/400x400".to_owned();
        svc.on_event(StateEvent::Track(same));
        svc.on_event(StateEvent::Playback(paused()));
        svc.reconcile();
        assert_eq!(
            sets_since(&log, mark),
            0,
            "разрыв и полный залп пульта с тем же track_id не действие пользователя — presence после AFK не возвращается"
        );
    }

    #[test]
    fn new_track_after_afk_brings_presence_back() {
        let log = Log::default();
        let mut svc = cleared_by_afk(&log);
        let mark = log.ops.lock().unwrap().len();
        let mut next = track();
        next.track_id = "12346".to_owned();
        next.title = "Alone".to_owned();
        svc.on_event(StateEvent::Track(next));
        svc.last_send_at = None;
        svc.reconcile();
        assert_eq!(sets_since(&log, mark), 1, "смена трека — действие пользователя, presence возвращается");
    }

    #[test]
    fn status_transitions_no_track_active_off() {
        let log = Log::default();
        let (tx, mut rx) = watch::channel(String::new());
        let mut svc = service(true, Some("app1"), &log);
        svc.status_tx = Some(tx);

        svc.reconcile();
        assert_eq!(*rx.borrow_and_update(), "no_track");

        svc.on_event(StateEvent::Track(track()));
        svc.on_event(StateEvent::Playback(playing(10.0, 100.0)));
        svc.reconcile();
        assert_eq!(*rx.borrow_and_update(), "active");

        svc.on_config(DiscordConfig { enabled: false, app_id: Some("app1".to_owned()) });
        svc.reconcile();
        assert_eq!(*rx.borrow_and_update(), "off");
    }

    #[test]
    fn status_error_on_connect_failure() {
        let log = Log { fail_connect: true, ..Default::default() };
        let (tx, mut rx) = watch::channel(String::new());
        let mut svc = service(true, Some("app1"), &log);
        svc.status_tx = Some(tx);
        svc.on_event(StateEvent::Track(track()));
        svc.reconcile();
        assert_eq!(*rx.borrow_and_update(), "error");
    }

    struct MockController {
        connected: bool,
        track: TrackData,
    }
    #[async_trait::async_trait]
    impl MediaController for MockController {
        fn is_connected(&self) -> bool {
            self.connected
        }
        fn snapshot(&self) -> ym_model::MediaState {
            ym_model::MediaState { track: self.track.clone(), ..Default::default() }
        }
        async fn play_pause(&self) -> ym_model::ActionResult {
            ym_model::ActionResult::default()
        }
        async fn next(&self) -> ym_model::ActionResult {
            ym_model::ActionResult::default()
        }
        async fn prev(&self) -> ym_model::ActionResult {
            ym_model::ActionResult::default()
        }
        async fn toggle_like(&self) -> ym_model::ActionResult {
            ym_model::ActionResult::default()
        }
        async fn toggle_dislike(&self) -> ym_model::ActionResult {
            ym_model::ActionResult::default()
        }
        async fn toggle_shuffle(&self) -> ym_model::ActionResult {
            ym_model::ActionResult::default()
        }
        async fn toggle_repeat(&self) -> ym_model::ActionResult {
            ym_model::ActionResult::default()
        }
        async fn change_volume(&self, _a: ym_model::ports::VolumeAction) -> ym_model::ActionResult {
            ym_model::ActionResult::default()
        }
    }

    #[test]
    fn status_no_client_when_cdp_offline() {
        let log = Log::default();
        let (tx, mut rx) = watch::channel(String::new());
        let mut svc = service(true, Some("app1"), &log);
        svc.status_tx = Some(tx);
        svc.controller = Some(Arc::new(MockController { connected: false, track: track() }));
        svc.on_event(StateEvent::Track(track()));
        svc.reconcile();
        assert_eq!(*rx.borrow_and_update(), "no_client");
    }

    #[test]
    fn status_active_from_snapshot_without_track_event() {
        let log = Log::default();
        let (tx, mut rx) = watch::channel(String::new());
        let mut svc = service(true, Some("app1"), &log);
        svc.status_tx = Some(tx);
        svc.controller = Some(Arc::new(MockController { connected: true, track: track() }));
        svc.reconcile();
        assert_eq!(*rx.borrow_and_update(), "active");
    }
}
