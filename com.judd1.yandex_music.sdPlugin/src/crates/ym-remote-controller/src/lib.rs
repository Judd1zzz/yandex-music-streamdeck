use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use ym_bridge_injection::{
    ajazz_guard_module, InjectedHandle, InjectionConfig, InjectionEndpoints, InjectionSubsystem,
};
use ym_bridge_session::{DefaultFactory, Session, SessionConfig, SessionEvent, SessionHandle};
use ym_model::state_event::StateEvent;
use ym_model::time::epoch_secs;
use ym_model::{
    ActionResult, BetterInfoConfig, DislikeData, LikeData, MediaController, MediaState, RepeatData, ShuffleData,
    TrackData, VideoCoverConfig, VolumeData, VolumeAction,
};
pub use ym_remote::remote_module;
use ym_remote::{SET_BETTER_INFO, SET_VIDEO_COVER, SET_YNISON_DEVICE};

const OPT_WINDOW: Duration = Duration::from_secs(2);
const PULT_SYNC_EVERY: Duration = Duration::from_secs(2);

const MODULE: &str = "ym_remote";
const GUARD: &str = "ajazz_guard";

#[derive(Default)]
struct Opt {
    playback_until: Option<Instant>,
    volume_until: Option<Instant>,
}

type Dispatch = (SessionHandle, InjectionEndpoints);

pub struct RemoteController {
    state: RwLock<MediaState>,
    connected: AtomicBool,
    bus: broadcast::Sender<StateEvent>,
    opt: Mutex<Opt>,
    download_tx: RwLock<Option<mpsc::Sender<String>>>,
    dispatch: RwLock<Option<Dispatch>>,
    pult_settings: Mutex<BTreeMap<&'static str, Value>>,
    pult_delivered: Mutex<BTreeMap<&'static str, (String, Value)>>,
}

impl RemoteController {
    pub fn new(bus: broadcast::Sender<StateEvent>) -> Arc<Self> {
        Arc::new(Self {
            state: RwLock::new(MediaState::default()),
            connected: AtomicBool::new(false),
            bus,
            opt: Mutex::new(Opt::default()),
            download_tx: RwLock::new(None),
            dispatch: RwLock::new(None),
            pult_settings: Mutex::new(BTreeMap::new()),
            pult_delivered: Mutex::new(BTreeMap::new()),
        })
    }

    fn module_instance(&self) -> Option<String> {
        let (_, endpoints) = self.dispatch_snapshot()?;
        endpoints.get(MODULE).map(|ep| ep.object_id.clone())
    }

    fn pult_due(&self, instance: &str) -> Vec<(&'static str, Value)> {
        let delivered = self.pult_delivered.lock().unwrap();
        self.pult_settings
            .lock()
            .unwrap()
            .iter()
            .filter(|(method, value)| delivered.get(*method).is_none_or(|(at, sent)| at != instance || sent != *value))
            .map(|(method, value)| (*method, value.clone()))
            .collect()
    }

    async fn sync_pult_settings(&self) {
        let Some(instance) = self.module_instance() else { return };
        for (method, value) in self.pult_due(&instance) {
            if let Some(r) = self.call_on(MODULE, method, std::slice::from_ref(&value)).await
                && r.get("success").and_then(Value::as_bool) == Some(true)
            {
                self.pult_delivered.lock().unwrap().insert(method, (instance.clone(), value));
            }
        }
    }

    fn put_pult_setting(&self, method: &'static str, value: Value) {
        self.pult_settings.lock().unwrap().insert(method, value);
    }

    pub fn start(self: &Arc<Self>, shutdown: CancellationToken) -> JoinHandle<()> {
        tokio::spawn(connection_loop(self.clone(), shutdown))
    }

    pub fn set_download_tx(&self, tx: mpsc::Sender<String>) {
        *self.download_tx.write().unwrap() = Some(tx);
    }

    pub async fn probe_rival(&self) -> bool {
        matches!(self.call_on(GUARD, "probe", &[]).await, Some(Value::Bool(true)))
    }

    fn dispatch_snapshot(&self) -> Option<Dispatch> {
        self.dispatch.read().unwrap().clone()
    }

    async fn call_on(&self, module: &str, method: &str, args: &[Value]) -> Option<Value> {
        let (handle, endpoints) = self.dispatch_snapshot()?;
        let ep = endpoints.get(module)?;
        let h = InjectedHandle::new(&handle, ep.session_id.as_deref(), &ep.object_id);
        h.call(method, args).await.ok()
    }

    async fn run_cmd(&self, method: &str, args: &[Value], kind: &str) -> ActionResult {
        match self.call_on(MODULE, method, args).await {
            Some(v) if v.is_object() => {
                let r = ActionResult::from_value(&v);
                self.apply_optimistic(kind, &r);
                r
            }
            Some(_) => ActionResult { success: false, error: Some("Invalid response".into()), ..Default::default() },
            None => ActionResult::default(),
        }
    }

    fn set_connected(&self, c: bool) {
        if self.connected.swap(c, Ordering::Release) != c {
            if c {
                self.pult_delivered.lock().unwrap().clear();
                tracing::info!("пульт подключён");
            } else {
                tracing::warn!("пульт отключён");
            }
            let _ = self.bus.send(StateEvent::Connection(c));
        }
    }

    fn in_window(&self, pick: impl Fn(&Opt) -> Option<Instant>) -> bool {
        pick(&self.opt.lock().unwrap()).is_some_and(|t| Instant::now() <= t)
    }

    fn apply_optimistic(&self, kind: &str, r: &ActionResult) {
        if !r.success {
            return;
        }
        let mut evs = Vec::new();
        {
            let mut s = self.state.write().unwrap();
            match kind {
                "playPause" => {
                    if let Some(p) = r.is_playing {
                        s.playback.is_playing = p;
                        evs.push(StateEvent::Playback(s.playback.clone()));
                        self.opt.lock().unwrap().playback_until = Some(Instant::now() + OPT_WINDOW);
                    }
                }
                "changeVolume" => {
                    let mut changed = false;
                    if let Some(v) = r.volume {
                        s.volume.current = v;
                        changed = true;
                    }
                    if let Some(m) = r.is_muted {
                        s.volume.is_muted = m;
                        changed = true;
                    }
                    if changed {
                        evs.push(StateEvent::Volume(s.volume.clone()));
                        self.opt.lock().unwrap().volume_until = Some(Instant::now() + OPT_WINDOW);
                    }
                }
                "toggleLike" => {
                    if let Some(v) = r.new_state {
                        s.like.is_liked = v;
                        evs.push(StateEvent::Like(s.like.clone()));
                    }
                }
                "toggleDislike" => {
                    if let Some(v) = r.new_state {
                        s.dislike.is_disliked = v;
                        evs.push(StateEvent::Dislike(s.dislike.clone()));
                    }
                }
                _ => {}
            }
        }
        for ev in evs {
            let _ = self.bus.send(ev);
        }
    }

    fn apply_binding(&self, kind: &str, payload: &Value) {
        if kind == "download" {
            if let Some(id) = payload.get("track_id").and_then(Value::as_str)
                && !id.is_empty()
                && let Some(tx) = self.download_tx.read().unwrap().clone()
            {
                let _ = tx.try_send(id.to_owned());
            }
            return;
        }
        let suppress_pb = self.in_window(|o| o.playback_until);
        let suppress_vol = self.in_window(|o| o.volume_until);
        let mut evs = Vec::new();
        {
            let mut s = self.state.write().unwrap();
            match kind {
                "trackChanged" => {
                    if let Ok(t) = serde_json::from_value::<TrackData>(payload.clone()) {
                        s.track = t;
                        evs.push(StateEvent::Track(s.track.clone()));
                    }
                }
                "playbackChanged" => {
                    if let Some(p) = payload.get("is_playing").and_then(Value::as_bool) {
                        s.playback.is_playing = p;
                        evs.push(StateEvent::Playback(s.playback.clone()));
                    }
                }
                "progressChanged" => {
                    let mut changed = false;
                    if let Some(n) = payload.get("current_sec").and_then(Value::as_f64) {
                        s.playback.current_sec = n;
                        changed = true;
                    }
                    if let Some(t) = payload.get("total_sec").and_then(Value::as_f64) {
                        s.playback.total_sec = t;
                        changed = true;
                    }
                    if let Some(r) = payload.get("progress").and_then(Value::as_f64) {
                        s.playback.progress = r;
                        changed = true;
                    }
                    if changed {
                        s.playback.timestamp = epoch_secs();
                        if !suppress_pb {
                            evs.push(StateEvent::Playback(s.playback.clone()));
                        }
                    }
                }
                "volumeChanged" => {
                    if let Ok(v) = serde_json::from_value::<VolumeData>(payload.clone()) {
                        let mute_changed = s.volume.is_muted != v.is_muted;
                        s.volume = v;
                        if !suppress_vol || mute_changed {
                            evs.push(StateEvent::Volume(s.volume.clone()));
                        }
                    }
                }
                "likeChanged" => {
                    if let Ok(l) = serde_json::from_value::<LikeData>(payload.clone()) {
                        s.like = l;
                        evs.push(StateEvent::Like(s.like.clone()));
                    }
                }
                "dislikeChanged" => {
                    if let Ok(d) = serde_json::from_value::<DislikeData>(payload.clone()) {
                        s.dislike = d;
                        evs.push(StateEvent::Dislike(s.dislike.clone()));
                    }
                }
                "shuffleChanged" => {
                    if let Ok(sh) = serde_json::from_value::<ShuffleData>(payload.clone()) {
                        s.shuffle = sh;
                        evs.push(StateEvent::Shuffle(s.shuffle.clone()));
                    }
                }
                "repeatChanged" => {
                    if let Ok(mut rp) = serde_json::from_value::<RepeatData>(payload.clone()) {
                        rp.mode = rp.mode.min(2);
                        s.repeat = rp;
                        evs.push(StateEvent::Repeat(s.repeat.clone()));
                    }
                }
                _ => {}
            }
        }
        for ev in evs {
            let _ = self.bus.send(ev);
        }
    }
}

async fn connection_loop(ctrl: Arc<RemoteController>, shutdown: CancellationToken) {
    let session = Session::spawn(DefaultFactory::default(), SessionConfig::default());
    let handle = session.handle();
    let injection = InjectionSubsystem::spawn(
        handle.clone(),
        vec![remote_module(), ajazz_guard_module()],
        InjectionConfig::default(),
    );
    *ctrl.dispatch.write().unwrap() = Some((handle.clone(), injection.endpoints()));

    let mut events = handle.subscribe_events();
    let mut bindings = injection.subscribe();
    let mut pult_tick = tokio::time::interval(PULT_SYNC_EVERY);
    loop {
        tokio::select! {
            biased;
            _ = shutdown.cancelled() => break,
            _ = pult_tick.tick() => ctrl.sync_pult_settings().await,
            ev = events.recv() => match ev {
                Ok(SessionEvent::Ready { page_session_id, .. }) => {
                    if page_session_id.is_some() {
                        ctrl.set_connected(true);
                    }
                }
                Ok(SessionEvent::Shutdown) => break,
                Ok(SessionEvent::Disconnected { .. } | SessionEvent::Unresponsive) => ctrl.set_connected(false),
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            },
            b = bindings.recv() => match b {
                Ok(be) if be.module == MODULE => ctrl.apply_binding(&be.kind, &be.payload),
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            },
        }
    }
    *ctrl.dispatch.write().unwrap() = None;
    ctrl.set_connected(false);
    injection.shutdown().await;
    session.shutdown().await;
}

#[async_trait]
impl MediaController for RemoteController {
    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }
    fn snapshot(&self) -> MediaState {
        self.state.read().unwrap().clone()
    }
    async fn oauth_token(&self) -> Option<String> {
        match self.call_on(MODULE, "getOauthToken", &[]).await {
            Some(Value::String(s)) if !s.is_empty() => Some(s),
            _ => None,
        }
    }
    async fn play_pause(&self) -> ActionResult {
        self.run_cmd("playPause", &[], "playPause").await
    }
    async fn next(&self) -> ActionResult {
        self.run_cmd("next", &[], "next").await
    }
    async fn prev(&self) -> ActionResult {
        self.run_cmd("prev", &[], "prev").await
    }
    async fn toggle_like(&self) -> ActionResult {
        self.run_cmd("toggleLike", &[], "toggleLike").await
    }
    async fn toggle_dislike(&self) -> ActionResult {
        self.run_cmd("toggleDislike", &[], "toggleDislike").await
    }
    async fn toggle_shuffle(&self) -> ActionResult {
        self.run_cmd("toggleShuffle", &[], "toggleShuffle").await
    }
    async fn toggle_repeat(&self) -> ActionResult {
        self.run_cmd("toggleRepeat", &[], "toggleRepeat").await
    }
    async fn change_volume(&self, action: VolumeAction) -> ActionResult {
        let args: Vec<Value> = match action {
            VolumeAction::Up => vec![json!("UP")],
            VolumeAction::Down => vec![json!("DOWN")],
            VolumeAction::Set(p) => vec![json!("SET"), json!(p)],
            VolumeAction::Mute => vec![json!("MUTE")],
        };
        self.run_cmd("changeVolume", &args, "changeVolume").await
    }

    fn set_video_cover(&self, cfg: VideoCoverConfig) {
        self.put_pult_setting(SET_VIDEO_COVER, serde_json::to_value(&cfg).unwrap_or(Value::Null));
    }

    fn set_better_info(&self, cfg: BetterInfoConfig) {
        self.put_pult_setting(SET_BETTER_INFO, serde_json::to_value(&cfg).unwrap_or(Value::Null));
    }

    fn set_ynison_device(&self, on: bool) {
        self.put_pult_setting(SET_YNISON_DEVICE, Value::Bool(on));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctrl() -> (Arc<RemoteController>, broadcast::Receiver<StateEvent>) {
        let (tx, rx) = broadcast::channel(32);
        (RemoteController::new(tx), rx)
    }

    #[test]
    fn track_binding_replaces_and_emits() {
        let (c, mut rx) = ctrl();
        c.apply_binding("trackChanged", &json!({"title":"T","artist":"A","cover_url":"C","track_id":"7"}));
        assert_eq!(
            c.snapshot().track,
            TrackData { title: "T".into(), artist: "A".into(), cover_url: "C".into(), track_id: "7".into() }
        );
        assert!(matches!(rx.try_recv().unwrap(), StateEvent::Track(_)));
    }

    #[test]
    fn volume_and_shuffle_repeat_bindings_map() {
        let (c, mut rx) = ctrl();
        c.apply_binding("volumeChanged", &json!({"current": 42.0, "is_muted": true}));
        assert_eq!(c.snapshot().volume, VolumeData { current: 42.0, is_muted: true });
        assert!(matches!(rx.try_recv().unwrap(), StateEvent::Volume(_)));
        c.apply_binding("shuffleChanged", &json!({"on": true}));
        assert!(c.snapshot().shuffle.on);
        c.apply_binding("repeatChanged", &json!({"mode": 9}));
        assert_eq!(c.snapshot().repeat.mode, 2, "mode clamped");
    }

    #[test]
    fn playback_and_progress_merge_into_same_struct() {
        let (c, _rx) = ctrl();
        c.apply_binding("playbackChanged", &json!({"is_playing": true}));
        c.apply_binding("progressChanged", &json!({"current_sec": 30.0, "total_sec": 180.0, "progress": 0.17}));
        let p = c.snapshot().playback;
        assert!(p.is_playing);
        assert_eq!(p.current_sec, 30.0);
        assert_eq!(p.total_sec, 180.0);
    }

    #[test]
    fn optimistic_window_suppresses_volume_delta_but_writes_state() {
        let (c, mut rx) = ctrl();
        c.apply_optimistic(
            "changeVolume",
            &ActionResult { success: true, volume: Some(50.0), is_muted: Some(false), ..Default::default() },
        );
        assert!(matches!(rx.try_recv().unwrap(), StateEvent::Volume(_)), "optimistic Volume emit");
        c.apply_binding("volumeChanged", &json!({"current": 80.0, "is_muted": false}));
        assert_eq!(c.snapshot().volume.current, 80.0, "state пишется даже в окне");
        assert!(rx.try_recv().is_err(), "дельта в окне подавлена — события нет");
    }

    #[test]
    fn optimistic_never_invents_shuffle_repeat() {
        let (c, mut rx) = ctrl();
        c.apply_optimistic("toggleShuffle", &ActionResult { success: true, ..Default::default() });
        c.apply_optimistic("toggleRepeat", &ActionResult { success: true, ..Default::default() });
        assert!(!c.snapshot().shuffle.on, "shuffle ждёт правду из store");
        assert_eq!(c.snapshot().repeat.mode, 0, "repeat ждёт правду из store");
        assert!(rx.try_recv().is_err(), "слепых событий нет");
    }

    #[test]
    fn optimistic_ignores_failed_result() {
        let (c, mut rx) = ctrl();
        c.apply_optimistic(
            "playPause",
            &ActionResult { success: false, is_playing: Some(true), ..Default::default() },
        );
        assert!(!c.snapshot().playback.is_playing);
        c.apply_optimistic(
            "toggleLike",
            &ActionResult { success: false, new_state: Some(true), ..Default::default() },
        );
        assert!(!c.snapshot().like.is_liked);
        assert!(rx.try_recv().is_err(), "неуспешная команда состояние не двигает");
    }

    #[test]
    fn volume_window_does_not_suppress_mute_change() {
        let (c, mut rx) = ctrl();
        c.apply_optimistic(
            "changeVolume",
            &ActionResult { success: true, volume: Some(50.0), ..Default::default() },
        );
        assert!(matches!(rx.try_recv().unwrap(), StateEvent::Volume(_)));
        c.apply_binding("volumeChanged", &json!({"current": 50.0, "is_muted": false}));
        assert!(rx.try_recv().is_err(), "одна лишь громкость в окне подавлена");
        c.apply_binding("volumeChanged", &json!({"current": 0.0, "is_muted": true}));
        assert!(matches!(rx.try_recv().unwrap(), StateEvent::Volume(_)), "смена mute проходит окно");
    }

    #[test]
    fn playback_change_is_never_suppressed_by_window() {
        let (c, mut rx) = ctrl();
        c.apply_optimistic(
            "playPause",
            &ActionResult { success: true, is_playing: Some(true), ..Default::default() },
        );
        assert!(matches!(rx.try_recv().unwrap(), StateEvent::Playback(_)));
        c.apply_binding("playbackChanged", &json!({"is_playing": false}));
        assert!(matches!(rx.try_recv().unwrap(), StateEvent::Playback(_)));
        assert!(!c.snapshot().playback.is_playing);
        c.apply_binding("progressChanged", &json!({"current_sec": 5.0}));
        assert!(rx.try_recv().is_err(), "progress в окне подавлен");
        assert!(c.snapshot().playback.timestamp > 0.0, "timestamp пишется");
    }

    #[test]
    fn download_binding_forwards_nonempty_track_id() {
        let (c, mut rx) = ctrl();
        let (tx, mut dl) = mpsc::channel(4);
        c.set_download_tx(tx);
        c.apply_binding("download", &json!({"track_id": "12345"}));
        assert_eq!(dl.try_recv().unwrap(), "12345");
        c.apply_binding("download", &json!({"track_id": ""}));
        c.apply_binding("download", &json!({}));
        assert!(dl.try_recv().is_err(), "пустой track_id в канал не уходит");
        assert!(rx.try_recv().is_err(), "download не порождает StateEvent");
    }

    #[test]
    fn connection_edge_emits_once() {
        let (c, mut rx) = ctrl();
        c.set_connected(true);
        c.set_connected(true);
        assert!(matches!(rx.try_recv().unwrap(), StateEvent::Connection(true)));
        assert!(rx.try_recv().is_err(), "повтор того же состояния — без события");
    }

    #[test]
    fn pult_setting_is_due_until_this_exact_value_reached_this_pult() {
        let (c, _rx) = ctrl();
        assert!(c.pult_due("pult-1").is_empty(), "до глобальных настроек слать нечего");
        let a = serde_json::to_value(VideoCoverConfig::default()).unwrap();
        let b = serde_json::to_value(VideoCoverConfig { brightness: 70, ..Default::default() }).unwrap();
        c.set_video_cover(VideoCoverConfig::default());
        assert_eq!(c.pult_due("pult-1"), vec![(SET_VIDEO_COVER, a.clone())]);
        c.pult_delivered.lock().unwrap().insert(SET_VIDEO_COVER, ("pult-1".into(), a.clone()));
        assert!(c.pult_due("pult-1").is_empty(), "доставленная настройка не шлётся каждые 2 с");
        c.set_video_cover(VideoCoverConfig::default());
        assert!(c.pult_due("pult-1").is_empty(), "повтор той же настройки от хоста не будит пульт");
        assert_eq!(c.pult_due("pult-2"), vec![(SET_VIDEO_COVER, a.clone())], "новый экземпляр после переинъекции получает настройку");
        c.set_video_cover(VideoCoverConfig { brightness: 70, ..Default::default() });
        assert_eq!(
            c.pult_due("pult-1"),
            vec![(SET_VIDEO_COVER, b.clone())],
            "успех по A, пришедший во время смены на B, не закрывает B — иначе B теряется до следующей правки"
        );
        c.set_better_info(BetterInfoConfig::default());
        let due: Vec<&str> = c.pult_due("pult-1").into_iter().map(|(m, _)| m).collect();
        assert_eq!(due, vec![SET_BETTER_INFO, SET_VIDEO_COVER], "каждая настройка пульта доставляется своим вызовом");
        c.set_connected(true);
        for (m, v) in c.pult_due("pult-1") {
            c.pult_delivered.lock().unwrap().insert(m, ("pult-1".into(), v));
        }
        assert!(c.pult_due("pult-1").is_empty());
        c.set_connected(false);
        c.set_connected(true);
        assert_eq!(c.pult_due("pult-1").len(), 2, "после переподключения настройки шлются заново, даже если id объекта повторился");
    }


    #[test]
    fn ynison_device_setting_is_delivered_to_each_pult_as_a_bool() {
        let (c, _rx) = ctrl();
        c.set_ynison_device(true);
        assert_eq!(c.pult_due("pult-1"), vec![(SET_YNISON_DEVICE, Value::Bool(true))]);
        c.set_ynison_device(false);
        assert_eq!(c.pult_due("pult-1"), vec![(SET_YNISON_DEVICE, Value::Bool(false))], "выключение тоже доставляется: пульт откатывает подмену");
    }

    #[cfg(not(feature = "download-button"))]
    #[test]
    fn marketplace_build_carries_the_marker_xtask_looks_for() {
        assert!(
            remote_module().source.starts_with(ym_model::dist::MARKETPLACE_MARKER),
            "префикс маркетплейс-сборки разошёлся с ym_model::dist::MARKETPLACE_MARKER — \
             xtask ищет эту строку внутри .exe и иначе завалит сборку Elgato"
        );
    }

    #[cfg(feature = "download-button")]
    #[test]
    fn full_build_does_not_carry_marketplace_marker() {
        assert!(
            !remote_module().source.contains(ym_model::dist::MARKETPLACE_MARKER),
            "полная сборка не должна нести маркер маркетплейса — иначе проверка xtask потеряет смысл"
        );
    }
}
