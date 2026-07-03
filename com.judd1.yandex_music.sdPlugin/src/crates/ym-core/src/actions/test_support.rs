use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use sd_protocol::Outbound;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ym_model::{ActionResult, ControlMode, MediaState, PluginSettings, VolumeData};

use crate::action::{ActionCtx, Shared};
use ym_model::ports::{MediaController, VolumeAction};

pub(super) struct MockController {
    pub(super) connected: bool,
    pub(super) state: MediaState,
    pub(super) calls: Arc<Mutex<Vec<&'static str>>>,
}
impl MockController {
    pub(super) fn shared(self) -> (Arc<Shared>, Arc<Mutex<Vec<&'static str>>>) {
        let calls = self.calls.clone();
        (Shared::with(Arc::new(self), ym_render::Renderers::new()), calls)
    }
}
#[async_trait]
impl MediaController for MockController {
    fn is_connected(&self) -> bool {
        self.connected
    }
    fn snapshot(&self) -> MediaState {
        self.state.clone()
    }
    async fn play_pause(&self) -> ActionResult {
        self.calls.lock().unwrap().push("play_pause");
        ActionResult { success: true, ..Default::default() }
    }
    async fn next(&self) -> ActionResult {
        self.calls.lock().unwrap().push("next");
        ActionResult { success: true, ..Default::default() }
    }
    async fn prev(&self) -> ActionResult {
        self.calls.lock().unwrap().push("prev");
        ActionResult { success: true, ..Default::default() }
    }
    async fn toggle_like(&self) -> ActionResult {
        self.calls.lock().unwrap().push("toggle_like");
        ActionResult { success: true, ..Default::default() }
    }
    async fn toggle_dislike(&self) -> ActionResult {
        self.calls.lock().unwrap().push("toggle_dislike");
        ActionResult { success: true, ..Default::default() }
    }
    async fn change_volume(&self, _a: VolumeAction) -> ActionResult {
        self.calls.lock().unwrap().push("change_volume");
        ActionResult { success: true, ..Default::default() }
    }
}

pub(super) fn make_ctx(shared: Arc<Shared>) -> (ActionCtx, mpsc::Receiver<Outbound>) {
    make_ctx_with(shared, PluginSettings::default())
}

pub(super) fn make_ctx_with(shared: Arc<Shared>, settings: PluginSettings) -> (ActionCtx, mpsc::Receiver<Outbound>) {
    let (tx, rx) = mpsc::channel(64);
    let (self_tx, _self_rx) = mpsc::channel(8);
    let ctx = ActionCtx::new(
        "ctx".into(),
        "uuid".into(),
        tx,
        Arc::new(RwLock::new(settings)),
        CancellationToken::new(),
        shared,
        self_tx,
    );
    (ctx, rx)
}

pub(super) fn icon(shared: &Arc<Shared>, name: &str) -> String {
    shared.render.icon_b64(name).unwrap().as_ref().to_owned()
}

pub(super) async fn try_next(rx: &mut mpsc::Receiver<Outbound>) -> Option<Outbound> {
    tokio::time::timeout(Duration::from_millis(100), rx.recv()).await.ok().flatten()
}

pub(super) struct KnobMock {
    connected: bool,
    volume: f64,
    vol: Arc<Mutex<Vec<VolumeAction>>>,
    plays: Arc<Mutex<u32>>,
}
impl KnobMock {
    pub(super) fn new(connected: bool, volume: f64) -> Self {
        Self { connected, volume, vol: Default::default(), plays: Default::default() }
    }
    #[allow(clippy::type_complexity)]
    pub(super) fn shared(self) -> (Arc<Shared>, Arc<Mutex<Vec<VolumeAction>>>, Arc<Mutex<u32>>) {
        let vol = self.vol.clone();
        let plays = self.plays.clone();
        (Shared::with(Arc::new(self), ym_render::Renderers::new()), vol, plays)
    }
}
#[async_trait]
impl MediaController for KnobMock {
    fn is_connected(&self) -> bool {
        self.connected
    }
    fn snapshot(&self) -> MediaState {
        MediaState { volume: VolumeData { current: self.volume, is_muted: false }, ..Default::default() }
    }
    async fn play_pause(&self) -> ActionResult {
        *self.plays.lock().unwrap() += 1;
        ActionResult { success: true, ..Default::default() }
    }
    async fn next(&self) -> ActionResult {
        ActionResult::default()
    }
    async fn prev(&self) -> ActionResult {
        ActionResult::default()
    }
    async fn toggle_like(&self) -> ActionResult {
        ActionResult::default()
    }
    async fn toggle_dislike(&self) -> ActionResult {
        ActionResult::default()
    }
    async fn change_volume(&self, a: VolumeAction) -> ActionResult {
        self.vol.lock().unwrap().push(a);
        ActionResult { success: true, ..Default::default() }
    }
}

pub(super) fn knob_settings(step: u8, press: &str) -> PluginSettings {
    PluginSettings { knob_step: step, knob_press: press.to_owned(), ..Default::default() }
}

pub(super) fn backdated(secs: u64) -> std::time::Instant {
    std::time::Instant::now()
        .checked_sub(Duration::from_secs(secs))
        .expect("монотонные часы моложе бекдейта")
}

pub(super) fn ynison_settings() -> PluginSettings {
    PluginSettings { control_mode: ControlMode::Ynison, ..Default::default() }
}

pub(super) fn assert_pi_status(o: &Outbound, event: &str, status: &str) {
    match o {
        Outbound::SendToPropertyInspector { payload, .. } => {
            assert_eq!(payload["event"], event);
            assert_eq!(payload["status"], status);
        }
        other => panic!("ожидался SendToPropertyInspector, {other:?}"),
    }
}
