use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::{Arc, Mutex as StdMutex, RwLock};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::{broadcast, mpsc, Notify};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use ym_bridge_session::{DefaultFactory, Session, SessionConfig, SessionError, SessionEvent, SessionHandle};
use ym_model::time::epoch_secs;
use ym_model::{ActionResult, MediaController, MediaState, StateEvent, VolumeAction};

use crate::{JsHandles, LocalCommand};

const TICK: Duration = Duration::from_secs(2);
const OPT_WINDOW: Duration = Duration::from_secs(2);

#[derive(Debug, thiserror::Error)]
pub enum CdpError {
    #[error("нет соединения")]
    Disconnected,
    #[error("таймаут RPC")]
    Timeout,
    #[error("ошибка CDP: {0}")]
    Cdp(String),
}

#[derive(Clone)]
struct RpcHandle {
    handle: SessionHandle,
    sid: Arc<str>,
}

impl RpcHandle {
    async fn call(&self, method: &str, params: Value) -> Result<Value, CdpError> {
        self.handle
            .execute_raw_on(Some(self.sid.as_ref()), method, params)
            .await
            .map_err(map_session_err)
    }
    async fn eval(&self, expr: &str) -> Result<Value, CdpError> {
        let v = self
            .call(
                "Runtime.evaluate",
                serde_json::json!({"expression": expr, "awaitPromise": true, "returnByValue": true}),
            )
            .await?;
        Ok(v.get("result").and_then(|r| r.get("value")).cloned().unwrap_or(Value::Null))
    }
}

fn map_session_err(e: SessionError) -> CdpError {
    match e {
        SessionError::Timeout => CdpError::Timeout,
        SessionError::Disconnected | SessionError::ActorGone | SessionError::Cancelled => {
            CdpError::Disconnected
        }
        other => CdpError::Cdp(other.to_string()),
    }
}

#[derive(Default)]
struct Opt {
    playback_until: Option<Instant>,
    volume_until: Option<Instant>,
}

pub struct CdpController {
    port: AtomicU16,
    state: RwLock<MediaState>,
    connected: AtomicBool,
    bus: broadcast::Sender<StateEvent>,
    rpc: RwLock<Option<RpcHandle>>,
    opt: StdMutex<Opt>,
    reconnect: Notify,
    download_tx: RwLock<Option<mpsc::Sender<String>>>,
    js: JsHandles,
}

impl CdpController {
    pub fn new(port: u16, bus: broadcast::Sender<StateEvent>) -> Arc<Self> {
        Arc::new(Self {
            port: AtomicU16::new(port),
            state: RwLock::new(MediaState::default()),
            connected: AtomicBool::new(false),
            bus,
            rpc: RwLock::new(None),
            opt: StdMutex::new(Opt::default()),
            reconnect: Notify::new(),
            download_tx: RwLock::new(None),
            js: JsHandles::random(),
        })
    }

    fn ctrl_ref(&self) -> String {
        self.js.ctrl_ref()
    }

    pub fn set_download_tx(&self, tx: mpsc::Sender<String>) {
        *self.download_tx.write().unwrap() = Some(tx);
    }

    pub fn local_port(&self) -> u16 {
        self.port.load(Ordering::Acquire)
    }

    pub fn start(self: &Arc<Self>, shutdown: CancellationToken) -> JoinHandle<()> {
        tokio::spawn(connection_loop(self.clone(), shutdown))
    }

    fn set_connected(&self, b: bool) {
        if self.connected.swap(b, Ordering::AcqRel) != b {
            let _ = self.bus.send(StateEvent::Connection(b));
        }
    }

    async fn bind_notify(&self, rpc: &RpcHandle) {
        let params = serde_json::json!({"name": self.js.notify});
        if let Err(e) = rpc.call("Runtime.addBinding", params).await {
            tracing::warn!("cdp: addBinding не удался: {e}");
        }
    }

    async fn ensure_injection(&self, rpc: &RpcHandle) -> Result<(), CdpError> {
        let h = rpc.eval(&self.js.health_expr()).await?;
        let flag = |k: &str| h.get(k).and_then(Value::as_bool).unwrap_or(false);

        if !flag("bind") {
            tracing::warn!("cdp: канал уведомлений отсутствует, переустанавливаю binding");
            self.bind_notify(rpc).await;
        }
        if !flag("guard") {
            rpc.eval(&self.js.guard_js()).await?;
        }
        if !flag("alive") {
            rpc.eval(&self.js.injected_api()).await?;
            tokio::time::sleep(Duration::from_millis(100)).await;
        } else if !flag("observing") {
            tracing::warn!("cdp: наблюдение снято извне, поднимаю заново");
            rpc.eval(&format!("{}.startObservation()", self.ctrl_ref())).await?;
        }
        Ok(())
    }

    async fn run_cmd(&self, cmd: LocalCommand) -> ActionResult {
        self.run_js(&cmd.expr(&self.ctrl_ref()), cmd.kind()).await
    }

    async fn run_js(&self, expr: &str, kind: &str) -> ActionResult {
        let handle = self.rpc.read().unwrap().clone();
        let Some(rpc) = handle else {
            return ActionResult::default();
        };
        if let Err(e) = self.ensure_injection(&rpc).await {
            tracing::warn!("cdp: инъекция перед '{kind}' не удалась: {e}");
            return ActionResult { success: false, error: Some("Injection failed".into()), ..Default::default() };
        }
        match rpc.eval(expr).await {
            Ok(v) if v.is_object() => {
                let r = ActionResult::from_value(&v);
                self.apply_optimistic(kind, &r);
                r
            }
            Ok(_) => ActionResult { success: false, error: Some("Invalid response".into()), ..Default::default() },
            Err(e) => {
                tracing::warn!("cdp: ошибка выполнения '{kind}': {e}");
                ActionResult { success: false, error: Some("RPC error".into()), ..Default::default() }
            }
        }
    }

    fn apply_optimistic(&self, kind: &str, r: &ActionResult) {
        if !r.success {
            return;
        }
        let mut events = Vec::new();
        let mut set_pb = false;
        let mut set_vol = false;
        {
            let mut st = self.state.write().unwrap();
            match kind {
                "playPause" => {
                    if let Some(p) = r.is_playing {
                        st.playback.is_playing = p;
                        st.playback.timestamp = epoch_secs();
                        set_pb = true;
                        events.push(StateEvent::Playback(st.playback.clone()));
                    }
                }
                "toggleLike" => {
                    if let Some(v) = r.new_state {
                        st.like.is_liked = v;
                        events.push(StateEvent::Like(st.like.clone()));
                    }
                }
                "toggleDislike" => {
                    if let Some(v) = r.new_state {
                        st.dislike.is_disliked = v;
                        events.push(StateEvent::Dislike(st.dislike.clone()));
                    }
                }
                "toggleShuffle" => {
                    st.shuffle.on = !st.shuffle.on;
                    events.push(StateEvent::Shuffle(st.shuffle.clone()));
                }
                "toggleRepeat" => {
                    st.repeat.mode = (st.repeat.mode + 1) % 3;
                    events.push(StateEvent::Repeat(st.repeat.clone()));
                }
                "changeVolume" => {
                    let mut changed = false;
                    if let Some(v) = r.volume {
                        st.volume.current = v;
                        changed = true;
                    }
                    if let Some(m) = r.is_muted {
                        st.volume.is_muted = m;
                        changed = true;
                    }
                    if changed {
                        set_vol = true;
                        events.push(StateEvent::Volume(st.volume.clone()));
                    }
                }
                _ => {}
            }
        }
        if set_pb {
            self.opt.lock().unwrap().playback_until = Some(Instant::now() + OPT_WINDOW);
        }
        if set_vol {
            self.opt.lock().unwrap().volume_until = Some(Instant::now() + OPT_WINDOW);
        }
        for e in events {
            let _ = self.bus.send(e);
        }
    }

    fn set_full(&self, new_state: MediaState) {
        let events = {
            let mut st = self.state.write().unwrap();
            *st = new_state;
            vec![
                StateEvent::Connection(true),
                StateEvent::Track(st.track.clone()),
                StateEvent::Playback(st.playback.clone()),
                StateEvent::Volume(st.volume.clone()),
                StateEvent::Like(st.like.clone()),
                StateEvent::Dislike(st.dislike.clone()),
                StateEvent::Shuffle(st.shuffle.clone()),
                StateEvent::Repeat(st.repeat.clone()),
            ]
        };
        for e in events {
            let _ = self.bus.send(e);
        }
    }

    fn apply_delta(&self, payload: &Value) {
        let suppress_pb = self.in_window(|o| o.playback_until);
        let suppress_vol = self.in_window(|o| o.volume_until);
        let mut events = Vec::new();
        {
            let mut st = self.state.write().unwrap();
            if let Some(track) = payload.get("track") {
                let mut changed = false;
                if let Some(id) = track.get("id") {
                    st.track.track_id = id_to_string(id);
                    changed = true;
                }
                if let Some(t) = track.get("title").and_then(Value::as_str) {
                    st.track.title = t.to_owned();
                    changed = true;
                }
                if let Some(a) = track.get("artist").and_then(Value::as_str) {
                    st.track.artist = a.to_owned();
                    changed = true;
                }
                if let Some(c) = track.get("cover").and_then(Value::as_str) {
                    st.track.cover_url = c.to_owned();
                    changed = true;
                }
                if changed {
                    events.push(StateEvent::Track(st.track.clone()));
                }
            }
            if let Some(s) = payload.get("state") {
                if let Some(p) = s.get("playing").and_then(Value::as_bool) {
                    st.playback.is_playing = p;
                    events.push(StateEvent::Playback(st.playback.clone()));
                }
                if let Some(l) = s.get("liked").and_then(Value::as_bool) {
                    st.like.is_liked = l;
                    events.push(StateEvent::Like(st.like.clone()));
                }
                if let Some(d) = s.get("disliked").and_then(Value::as_bool) {
                    st.dislike.is_disliked = d;
                    events.push(StateEvent::Dislike(st.dislike.clone()));
                }
                if let Some(sh) = s.get("shuffle").and_then(Value::as_bool) {
                    st.shuffle.on = sh;
                    events.push(StateEvent::Shuffle(st.shuffle.clone()));
                }
                if let Some(rm) = s.get("repeat_mode").and_then(Value::as_u64) {
                    st.repeat.mode = (rm as u8).min(2);
                    events.push(StateEvent::Repeat(st.repeat.clone()));
                }
            }
            if let Some(pr) = payload.get("progress") {
                let mut changed = false;
                if let Some(n) = pr.get("now_sec").and_then(Value::as_f64) {
                    st.playback.current_sec = n;
                    changed = true;
                }
                if let Some(t) = pr.get("total_sec").and_then(Value::as_f64) {
                    st.playback.total_sec = t;
                    changed = true;
                }
                if let Some(rt) = pr.get("ratio").and_then(Value::as_f64) {
                    st.playback.progress = rt;
                    changed = true;
                }
                if changed {
                    st.playback.timestamp = epoch_secs();
                    if !suppress_pb {
                        events.push(StateEvent::Playback(st.playback.clone()));
                    }
                }
            }
            if let Some(v) = payload.get("volume") {
                let mut changed = false;
                if let Some(c) = v.get("current").and_then(Value::as_f64) {
                    st.volume.current = c;
                    changed = true;
                }
                if let Some(m) = v.get("is_muted").and_then(Value::as_bool) {
                    st.volume.is_muted = m;
                    changed = true;
                }
                if changed && !suppress_vol {
                    events.push(StateEvent::Volume(st.volume.clone()));
                }
            }
        }
        for e in events {
            let _ = self.bus.send(e);
        }
    }

    fn in_window(&self, pick: impl Fn(&Opt) -> Option<Instant>) -> bool {
        pick(&self.opt.lock().unwrap()).is_some_and(|t| Instant::now() <= t)
    }

    pub async fn probe_rival(&self) -> bool {
        let handle = self.rpc.read().unwrap().clone();
        let Some(rpc) = handle else { return false };
        matches!(rpc.eval(&self.js.rival_expr()).await, Ok(Value::Bool(true)))
    }
}

#[async_trait]
impl MediaController for CdpController {
    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }
    fn snapshot(&self) -> MediaState {
        self.state.read().unwrap().clone()
    }
    fn set_local_port(&self, port: u16) {
        if self.port.swap(port, Ordering::AcqRel) != port {
            self.reconnect.notify_one();
        }
    }
    fn active_port(&self) -> Option<u16> {
        self.connected.load(Ordering::Acquire).then(|| self.port.load(Ordering::Acquire))
    }
    async fn oauth_token(&self) -> Option<String> {
        let handle = self.rpc.read().unwrap().clone();
        let rpc = handle?;
        let expr = "(()=>{try{return JSON.parse(localStorage.getItem('oauth')).value}catch(e){return null}})()";
        match rpc.eval(expr).await {
            Ok(Value::String(s)) if !s.is_empty() => Some(s),
            _ => None,
        }
    }
    async fn play_pause(&self) -> ActionResult {
        self.run_cmd(LocalCommand::PlayPause).await
    }
    async fn next(&self) -> ActionResult {
        self.run_cmd(LocalCommand::Next).await
    }
    async fn prev(&self) -> ActionResult {
        self.run_cmd(LocalCommand::Prev).await
    }
    async fn toggle_like(&self) -> ActionResult {
        self.run_cmd(LocalCommand::ToggleLike).await
    }
    async fn toggle_dislike(&self) -> ActionResult {
        self.run_cmd(LocalCommand::ToggleDislike).await
    }
    async fn toggle_shuffle(&self) -> ActionResult {
        self.run_cmd(LocalCommand::ToggleShuffle).await
    }
    async fn toggle_repeat(&self) -> ActionResult {
        self.run_cmd(LocalCommand::ToggleRepeat).await
    }
    async fn change_volume(&self, action: VolumeAction) -> ActionResult {
        self.run_cmd(LocalCommand::ChangeVolume(action)).await
    }
}

fn id_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn handle_binding(v: &Value, ctrl: &CdpController) {
    let params = v.get("params");
    if params.and_then(|p| p.get("name")).and_then(Value::as_str) != Some(ctrl.js.notify.as_str()) {
        return;
    }
    let payload_str = params.and_then(|p| p.get("payload")).and_then(Value::as_str).unwrap_or("{}");
    let data = match serde_json::from_str::<Value>(payload_str) {
        Ok(d) => d,
        Err(e) => {
            tracing::debug!("cdp: битый payload уведомления: {e}");
            return;
        }
    };
    if data.get("n").and_then(Value::as_str) != Some(ctrl.js.nonce.as_str()) {
        tracing::warn!("cdp: отброшено сообщение без валидного nonce (чужая инъекция в канал)");
        return;
    }
    let typ = data.get("type").and_then(Value::as_str).unwrap_or("");
    let payload = data.get("payload").cloned().unwrap_or(Value::Null);
    match typ {
        "FULL_STATE" => ctrl.set_full(MediaState::from_full_value(&payload)),
        "DELTA" => ctrl.apply_delta(&payload),
        "DOWNLOAD" => {
            if let Some(id) = payload.get("track_id").and_then(Value::as_str)
                && !id.is_empty()
                && let Some(tx) = ctrl.download_tx.read().unwrap().clone()
            {
                let _ = tx.try_send(id.to_owned());
            }
        }
        _ => {}
    }
}

async fn connection_loop(ctrl: Arc<CdpController>, shutdown: CancellationToken) {
    let session = Session::spawn(DefaultFactory::default(), SessionConfig::default());
    let handle = session.handle();

    let mut raw = handle.subscribe_raw();
    let ctrl_raw = ctrl.clone();
    let pump = tokio::spawn(async move {
        loop {
            match raw.recv().await {
                Ok(ev) => {
                    if ev.method == "Runtime.bindingCalled" {
                        let mut msg = serde_json::Map::new();
                        msg.insert("params".to_string(), ev.params.clone());
                        handle_binding(&Value::Object(msg), &ctrl_raw);
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let mut events = handle.subscribe_events();
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            ev = events.recv() => match ev {
                Ok(SessionEvent::Ready { page_session_id: Some(sid), .. }) => {
                    let rpc = RpcHandle { handle: handle.clone(), sid: Arc::from(sid.as_str()) };
                    *ctrl.rpc.write().unwrap() = Some(rpc.clone());
                    ctrl.set_connected(true);
                    reinject(&ctrl, &rpc).await;
                }
                Ok(SessionEvent::Ready { page_session_id: None, .. }) => {}
                Ok(SessionEvent::Shutdown) => break,
                Ok(_) => {
                    *ctrl.rpc.write().unwrap() = None;
                    ctrl.set_connected(false);
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            },
            _ = tokio::time::sleep(TICK) => {
                let rpc = ctrl.rpc.read().unwrap().clone();
                if let Some(rpc) = rpc {
                    let _ = rpc.eval(&ctrl.js.guard_js()).await;
                    let _ = ctrl.ensure_injection(&rpc).await;
                }
            }
        }
    }

    pump.abort();
    *ctrl.rpc.write().unwrap() = None;
    ctrl.set_connected(false);
    session.shutdown().await;
}

async fn reinject(ctrl: &Arc<CdpController>, rpc: &RpcHandle) {
    let _ = rpc.call("Runtime.enable", serde_json::json!({})).await;
    ctrl.bind_notify(rpc).await;
    let guard_js = ctrl.js.guard_js();
    if let Err(e) = rpc
        .call("Page.addScriptToEvaluateOnNewDocument", serde_json::json!({ "source": guard_js }))
        .await
    {
        tracing::warn!("cdp: guard на новые документы не установлен: {e}");
    }
    let _ = rpc.eval(&guard_js).await;
    if let Err(e) = ctrl.ensure_injection(rpc).await {
        tracing::warn!("cdp: первичная инъекция не удалась: {e}");
    }
    let _ = rpc.eval(&format!("{}.stopObservation()", ctrl.ctrl_ref())).await;
    let _ = rpc.eval(&format!("{}.startObservation()", ctrl.ctrl_ref())).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn active_port_none_when_disconnected() {
        let (bus, _) = broadcast::channel(16);
        let c = CdpController::new(9333, bus);
        assert_eq!(c.active_port(), None);
        assert_eq!(c.local_port(), 9333);
    }

    fn binding_frame(js: &JsHandles, typ: &str, payload: Value) -> String {
        let inner = serde_json::json!({"n": js.nonce, "type": typ, "payload": payload}).to_string();
        serde_json::json!({"method": "Runtime.bindingCalled", "params": {"name": js.notify, "payload": inner}})
            .to_string()
    }

    fn frame(js: &JsHandles, typ: &str, payload: Value) -> Value {
        serde_json::from_str(&binding_frame(js, typ, payload)).unwrap()
    }

    #[test]
    fn shuffle_repeat_delta_applies_without_suppression() {
        let (bus, _rx) = broadcast::channel(64);
        let ctrl = CdpController::new(9222, bus);
        ctrl.apply_optimistic("toggleShuffle", &ActionResult { success: true, ..Default::default() });
        ctrl.apply_optimistic("toggleRepeat", &ActionResult { success: true, ..Default::default() });
        assert!(ctrl.snapshot().shuffle.on);
        assert_eq!(ctrl.snapshot().repeat.mode, 1);
        let mut sub = ctrl.bus.subscribe();
        ctrl.apply_delta(&serde_json::json!({"state": {"shuffle": false, "repeat_mode": 2}}));
        assert!(!ctrl.snapshot().shuffle.on);
        assert_eq!(ctrl.snapshot().repeat.mode, 2);
        assert_eq!(sub.try_recv().unwrap(), StateEvent::Shuffle(ym_model::ShuffleData { on: false }));
        assert_eq!(sub.try_recv().unwrap(), StateEvent::Repeat(ym_model::RepeatData { mode: 2 }));
    }

    #[test]
    fn repeat_delta_clamps_unknown_mode() {
        let (bus, _rx) = broadcast::channel(64);
        let ctrl = CdpController::new(9222, bus);
        ctrl.apply_delta(&serde_json::json!({"state": {"repeat_mode": 9}}));
        assert_eq!(ctrl.snapshot().repeat.mode, 2);
    }

    #[tokio::test]
    async fn binding_download_forwards_track_id_to_channel() {
        let (bus, _rx) = broadcast::channel(8);
        let ctrl = CdpController::new(0, bus);
        let (tx, mut dl_rx) = mpsc::channel(8);
        ctrl.set_download_tx(tx);

        let dl = |id: &str| serde_json::json!({"track_id": id});
        handle_binding(&frame(&ctrl.js, "DOWNLOAD", dl("12345")), &ctrl);
        assert_eq!(dl_rx.try_recv().unwrap(), "12345");

        handle_binding(&frame(&ctrl.js, "DOWNLOAD", dl("")), &ctrl);
        assert!(dl_rx.try_recv().is_err(), "пустой track_id не должен уходить в канал");
    }

    #[tokio::test]
    async fn binding_rejects_foreign_injection_without_nonce() {
        let (bus, mut rx) = broadcast::channel(8);
        let ctrl = CdpController::new(0, bus);
        let (tx, mut dl_rx) = mpsc::channel(8);
        ctrl.set_download_tx(tx);

        let forged = |body: Value| {
            serde_json::json!({
                "method": "Runtime.bindingCalled",
                "params": { "name": ctrl.js.notify, "payload": body.to_string() }
            })
        };

        let no_nonce = serde_json::json!({"type": "DOWNLOAD", "payload": {"track_id": "evil"}});
        handle_binding(&forged(no_nonce), &ctrl);
        assert!(dl_rx.try_recv().is_err(), "сообщение без nonce должно отбрасываться");

        let bad_nonce = serde_json::json!({
            "n": "deadbeefdeadbeef", "type": "DOWNLOAD", "payload": {"track_id": "evil"}
        });
        handle_binding(&forged(bad_nonce), &ctrl);
        assert!(dl_rx.try_recv().is_err(), "сообщение с чужим nonce должно отбрасываться");

        let fake_state = serde_json::json!({
            "n": "deadbeefdeadbeef", "type": "FULL_STATE", "payload": {"track": {"title": "Fake"}}
        });
        handle_binding(&forged(fake_state), &ctrl);
        assert_ne!(ctrl.snapshot().track.title, "Fake", "чужой FULL_STATE не должен применяться");
        assert!(rx.try_recv().is_err(), "чужое сообщение не должно порождать события");

        let wrong_channel = serde_json::json!({
            "method": "Runtime.bindingCalled",
            "params": { "name": "sdNotify", "payload": binding_frame(&ctrl.js, "DOWNLOAD", serde_json::json!({"track_id": "evil"})) }
        });
        handle_binding(&wrong_channel, &ctrl);
        assert!(dl_rx.try_recv().is_err(), "сообщение по старому имени канала должно игнорироваться");

        handle_binding(&frame(&ctrl.js, "DOWNLOAD", serde_json::json!({"track_id": "ok"})), &ctrl);
        assert_eq!(dl_rx.try_recv().unwrap(), "ok", "наше подписанное сообщение проходит");
    }

    #[tokio::test]
    async fn disconnected_controller_returns_default_result() {
        let (bus, _rx) = broadcast::channel(64);
        let ctrl = CdpController::new(1, bus);
        let r = ctrl.play_pause().await;
        assert!(!r.success);
        assert!(!ctrl.is_connected());
    }

    #[test]
    fn delta_suppression_respects_optimistic_window() {
        let (bus, _rx) = broadcast::channel(64);
        let ctrl = CdpController::new(9222, bus);
        ctrl.apply_optimistic("changeVolume", &ActionResult { success: true, volume: Some(50.0), ..Default::default() });
        let mut sub = ctrl.bus.subscribe();
        ctrl.apply_delta(&serde_json::json!({"volume": {"current": 99.0}}));
        assert!(sub.try_recv().is_err(), "volume-дельта должна быть подавлена в окне");
        assert_eq!(ctrl.snapshot().volume.current, 99.0);
    }
}
