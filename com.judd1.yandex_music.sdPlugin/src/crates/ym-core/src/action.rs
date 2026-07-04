use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use bitflags::bitflags;
use sd_protocol::{LocalStatus, LogPayload, Outbound, TokenStatus, UrlPayload};
use tokio::sync::{broadcast, mpsc, watch};
use tokio_util::sync::CancellationToken;
use ym_model::{ControlMode, DiscordConfig, LaunchConfig, PluginSettings, StateEvent, StateKind};
use ym_render::Renderers;

use ym_model::ports::{MediaController, StubController};
use ym_model::{Downloader, StubDownloader};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Interests: u8 {
        const CONNECTION = 1;
        const TRACK = 2;
        const PLAYBACK = 4;
        const LIKE = 8;
        const DISLIKE = 16;
        const VOLUME = 32;
        const DOWNLOAD = 64;
    }
}

pub fn interest_of(kind: StateKind) -> Interests {
    match kind {
        StateKind::Connection => Interests::CONNECTION,
        StateKind::Track => Interests::TRACK,
        StateKind::Playback => Interests::PLAYBACK,
        StateKind::Like => Interests::LIKE,
        StateKind::Dislike => Interests::DISLIKE,
        StateKind::Volume => Interests::VOLUME,
        StateKind::Download => Interests::DOWNLOAD,
        StateKind::LaunchHint => Interests::CONNECTION,
    }
}

const STATE_BUS_CAP: usize = 256;

pub struct Shared {
    pub state_bus: broadcast::Sender<StateEvent>,
    pub cdp: Arc<dyn MediaController>,
    pub render: Arc<Renderers>,
    pub downloader: Arc<dyn Downloader>,
    token: RwLock<Option<String>>,
    discord: watch::Sender<DiscordConfig>,
    launch: watch::Sender<LaunchConfig>,
    any_local: Arc<AtomicBool>,
    launch_kick: RwLock<Option<mpsc::Sender<()>>>,
    launch_reason: RwLock<Option<String>>,
    path_checker: RwLock<Option<ClientPathChecker>>,
    download: RwLock<(String, String)>,
    active_downloads: AtomicUsize,
}

pub type ClientPathChecker = Arc<dyn Fn(&str) -> ClientPathReport + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientPathReport {
    pub verdict: &'static str,
    pub resolved: Option<String>,
    pub expected: &'static str,
}

impl ClientPathReport {
    pub fn payload(&self) -> serde_json::Value {
        serde_json::json!({
            "event": "ClientPathCheck",
            "verdict": self.verdict,
            "resolved": self.resolved,
            "expected": self.expected,
        })
    }
}

impl Shared {
    pub fn new() -> Arc<Self> {
        Self::with(Arc::new(StubController), Renderers::new())
    }
    pub fn with(cdp: Arc<dyn MediaController>, render: Arc<Renderers>) -> Arc<Self> {
        let (tx, _) = broadcast::channel(STATE_BUS_CAP);
        Self::wired(tx, cdp, render, Arc::new(StubDownloader))
    }
    pub fn wired(
        state_bus: broadcast::Sender<StateEvent>,
        cdp: Arc<dyn MediaController>,
        render: Arc<Renderers>,
        downloader: Arc<dyn Downloader>,
    ) -> Arc<Self> {
        let (discord, _) = watch::channel(DiscordConfig::default());
        let (launch, _) = watch::channel(LaunchConfig::default());
        Arc::new(Self {
            state_bus,
            cdp,
            render,
            downloader,
            token: RwLock::new(None),
            discord,
            launch,
            any_local: Arc::new(AtomicBool::new(false)),
            launch_kick: RwLock::new(None),
            launch_reason: RwLock::new(None),
            path_checker: RwLock::new(None),
            download: RwLock::new((String::new(), "lossless".to_owned())),
            active_downloads: AtomicUsize::new(0),
        })
    }
    pub fn subscribe(&self) -> broadcast::Receiver<StateEvent> {
        self.state_bus.subscribe()
    }
    pub fn publish(&self, ev: StateEvent) {
        let _ = self.state_bus.send(ev);
    }
    pub fn set_token(&self, token: Option<String>) {
        *self.token.write().expect("token lock") = token;
    }
    pub fn has_token(&self) -> bool {
        self.token.read().expect("token lock").as_deref().is_some_and(|t| !t.is_empty())
    }
    pub fn token(&self) -> Option<String> {
        self.token.read().expect("token lock").clone()
    }
    pub fn set_discord_config(&self, cfg: DiscordConfig) {
        let _ = self.discord.send(cfg);
    }
    pub fn subscribe_discord(&self) -> watch::Receiver<DiscordConfig> {
        self.discord.subscribe()
    }
    pub fn set_launch_config(&self, cfg: LaunchConfig) {
        let _ = self.launch.send(cfg);
    }
    pub fn subscribe_launch(&self) -> watch::Receiver<LaunchConfig> {
        self.launch.subscribe()
    }
    pub fn set_any_local(&self, v: bool) {
        self.any_local.store(v, Ordering::Release);
    }
    pub fn any_local_flag(&self) -> Arc<AtomicBool> {
        self.any_local.clone()
    }
    pub fn set_launch_kick(&self, tx: mpsc::Sender<()>) {
        *self.launch_kick.write().expect("launch kick lock") = Some(tx);
    }
    pub fn launch_kick(&self) {
        if let Some(tx) = self.launch_kick.read().expect("launch kick lock").as_ref() {
            let _ = tx.try_send(());
        }
    }
    pub fn launch_reason(&self) -> Option<String> {
        self.launch_reason.read().expect("launch reason lock").clone()
    }
    pub fn set_launch_reason(&self, v: Option<String>) -> bool {
        let mut g = self.launch_reason.write().expect("launch reason lock");
        if *g == v {
            return false;
        }
        *g = v;
        true
    }
    pub fn apply_launch_reason(&self, v: Option<String>) {
        if self.set_launch_reason(v) {
            self.publish(StateEvent::LaunchHint);
        }
    }
    pub fn set_client_path_checker(&self, f: ClientPathChecker) {
        *self.path_checker.write().expect("path checker lock") = Some(f);
    }
    pub fn check_client_path(&self, raw: &str) -> Option<ClientPathReport> {
        let g = self.path_checker.read().expect("path checker lock");
        g.as_ref().map(|f| f(raw))
    }
    pub fn set_download_config(&self, path: String, format: String) {
        *self.download.write().expect("download lock") = (path, format);
    }
    pub fn download_path(&self) -> String {
        self.download.read().expect("download lock").0.clone()
    }
    pub fn download_format(&self) -> String {
        self.download.read().expect("download lock").1.clone()
    }
    pub fn download_begin(&self) -> bool {
        self.active_downloads.fetch_add(1, Ordering::SeqCst) == 0
    }
    pub fn download_end(&self) -> bool {
        self.active_downloads.fetch_sub(1, Ordering::SeqCst) == 1
    }
}

#[derive(Clone)]
pub struct ActionCtx {
    pub context: String,
    pub uuid: String,
    host: mpsc::Sender<Outbound>,
    settings: Arc<RwLock<PluginSettings>>,
    pub cancel: CancellationToken,
    pub shared: Arc<Shared>,
    self_tx: mpsc::Sender<crate::actor::ActorMsg>,
}

impl ActionCtx {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        context: String,
        uuid: String,
        host: mpsc::Sender<Outbound>,
        settings: Arc<RwLock<PluginSettings>>,
        cancel: CancellationToken,
        shared: Arc<Shared>,
        self_tx: mpsc::Sender<crate::actor::ActorMsg>,
    ) -> Self {
        Self { context, uuid, host, settings, cancel, shared, self_tx }
    }

    pub fn settings(&self) -> PluginSettings {
        self.settings.read().expect("settings lock").clone()
    }

    pub fn child_token(&self) -> CancellationToken {
        self.cancel.child_token()
    }

    pub async fn tick(&self) -> bool {
        self.self_tx.send(crate::actor::ActorMsg::Tick).await.is_ok()
    }

    pub(crate) fn set_settings_local(&self, s: PluginSettings) {
        *self.settings.write().expect("settings lock") = s;
    }

    pub async fn send(&self, o: Outbound) {
        let _ = self.host.send(o).await;
    }
    pub async fn set_image(&self, image: impl Into<String>, state: Option<u8>) {
        self.send(Outbound::set_image(self.context.clone(), image, state)).await;
    }
    pub async fn set_state(&self, state: u8) {
        self.send(Outbound::set_state(self.context.clone(), state)).await;
    }
    pub async fn set_title(&self, title: impl Into<String>) {
        self.send(Outbound::set_title(self.context.clone(), title)).await;
    }
    pub async fn show_ok(&self) {
        self.send(Outbound::ShowOk { context: self.context.clone() }).await;
    }
    pub async fn show_alert(&self) {
        self.send(Outbound::ShowAlert { context: self.context.clone() }).await;
    }
    pub async fn open_url(&self, url: impl Into<String>) {
        self.send(Outbound::OpenUrl { payload: UrlPayload { url: url.into() } }).await;
    }
    pub async fn send_to_pi(&self, payload: serde_json::Value) {
        self.send(Outbound::SendToPropertyInspector {
            action: self.uuid.clone(),
            context: self.context.clone(),
            payload,
        })
        .await;
    }
    pub async fn send_token_status(&self, s: TokenStatus) {
        self.send_to_pi(sd_protocol::token_status_payload(s)).await;
    }
    pub async fn send_local_status(&self, s: LocalStatus) {
        let reason = self.shared.launch_reason();
        self.send_to_pi(sd_protocol::local_status_payload(s, reason.as_deref())).await;
    }

    pub async fn report_status(&self) {
        match self.settings().control_mode {
            ControlMode::Local => {
                let s = if self.shared.cdp.is_connected() {
                    LocalStatus::Connected
                } else {
                    LocalStatus::Disconnected
                };
                self.send_local_status(s).await;
            }
            ControlMode::Ynison => {
                let s = if self.shared.has_token() {
                    TokenStatus::Offline
                } else {
                    TokenStatus::Missing
                };
                self.send_token_status(s).await;
            }
        }
    }
    pub async fn log(&self, message: impl Into<String>) {
        self.send(Outbound::LogMessage { payload: LogPayload { message: message.into() } }).await;
    }
}

#[async_trait]
pub trait Action: Send {
    fn interests(&self) -> Interests {
        Interests::CONNECTION
    }
    async fn on_appear(&mut self, cx: &ActionCtx) {
        self.render(cx).await;
    }
    async fn on_disappear(&mut self, _cx: &ActionCtx) {}
    async fn on_key_down(&mut self, _cx: &ActionCtx) {}
    async fn on_key_up(&mut self, _cx: &ActionCtx) {}
    async fn on_dial_rotate(&mut self, _cx: &ActionCtx, _ticks: i32) {}
    async fn on_dial_down(&mut self, _cx: &ActionCtx) {}
    async fn on_dial_up(&mut self, _cx: &ActionCtx) {}
    async fn on_settings(&mut self, cx: &ActionCtx) {
        self.render(cx).await;
    }
    async fn on_pi_appear(&mut self, cx: &ActionCtx) {
        cx.report_status().await;
    }
    async fn on_state(&mut self, cx: &ActionCtx, _ev: &StateEvent) {
        self.render(cx).await;
    }
    async fn on_health(&mut self, _cx: &ActionCtx) {}
    async fn on_tick(&mut self, cx: &ActionCtx) {
        self.render(cx).await;
    }
    async fn render(&mut self, cx: &ActionCtx);
}
