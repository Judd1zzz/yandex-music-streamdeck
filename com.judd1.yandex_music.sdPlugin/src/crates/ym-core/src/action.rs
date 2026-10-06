use std::collections::VecDeque;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use async_trait::async_trait;
use bitflags::bitflags;
use sd_protocol::{LocalStatus, LogPayload, Outbound, UrlPayload};
use tokio::sync::{broadcast, mpsc, watch};
use tokio_util::sync::CancellationToken;
use ym_model::time::epoch_secs;
use ym_model::{DiscordConfig, LaunchConfig, PluginSettings, StateEvent, StateKind, TrackData, UpdateCheck};
use ym_render::Renderers;

use ym_model::ports::{MediaController, StubController};
use ym_model::{Downloader, StubDownloader};

use crate::stats::{ActionKind, StatsStore};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Interests: u16 {
        const CONNECTION = 1;
        const TRACK = 2;
        const PLAYBACK = 4;
        const LIKE = 8;
        const DISLIKE = 16;
        const VOLUME = 32;
        const DOWNLOAD = 64;
        const SHUFFLE = 128;
        const REPEAT = 256;
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
        StateKind::Shuffle => Interests::SHUFFLE,
        StateKind::Repeat => Interests::REPEAT,
        StateKind::Download => Interests::DOWNLOAD,
        StateKind::LaunchHint => Interests::CONNECTION,
        StateKind::UpdateHint => Interests::CONNECTION,
        StateKind::DiscordHint => Interests::CONNECTION,
        StateKind::RivalHint => Interests::CONNECTION,
    }
}

const STATE_BUS_CAP: usize = 256;

pub struct Shared {
    pub state_bus: broadcast::Sender<StateEvent>,
    pub cdp: Arc<dyn MediaController>,
    pub render: Arc<Renderers>,
    pub downloader: Arc<dyn Downloader>,
    discord: watch::Sender<DiscordConfig>,
    launch: watch::Sender<LaunchConfig>,
    any_local: Arc<AtomicBool>,
    launch_kick: RwLock<Option<mpsc::Sender<()>>>,
    launch_reason: RwLock<Option<String>>,
    update: RwLock<UpdateState>,
    discord_status: RwLock<Option<String>>,
    path_checker: RwLock<Option<ClientPathChecker>>,
    update_checker: RwLock<Option<UpdateChecker>>,
    download: RwLock<(String, String)>,
    active_downloads: AtomicUsize,
    log_ring: RwLock<Option<Arc<Mutex<VecDeque<String>>>>>,
    log_dir: RwLock<Option<PathBuf>>,
    time_offset: AtomicI32,
    stats: Mutex<StatsStore>,
    edition: RwLock<&'static str>,
    logs_enabled: RwLock<Option<Arc<AtomicBool>>>,
    rival: RwLock<crate::rival::RivalState>,
    folder_picker: RwLock<Option<FolderPicker>>,
}

pub type ClientPathChecker = Arc<dyn Fn(&str) -> ClientPathReport + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickKind {
    Folder,
    ClientApp,
}

pub type FolderPicker = Arc<dyn Fn(PickKind) -> Option<String> + Send + Sync>;

#[derive(Debug, Clone, Default)]
struct UpdateState {
    current: String,
    applied: Option<String>,
    latest: Option<String>,
    notes: Option<String>,
    url: Option<String>,
    has_update: bool,
    last_check_ok: Option<bool>,
}

pub type UpdateChecker = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Option<UpdateCheck>> + Send>> + Send + Sync>;

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
            discord,
            launch,
            any_local: Arc::new(AtomicBool::new(false)),
            launch_kick: RwLock::new(None),
            launch_reason: RwLock::new(None),
            update: RwLock::new(UpdateState::default()),
            discord_status: RwLock::new(None),
            path_checker: RwLock::new(None),
            update_checker: RwLock::new(None),
            download: RwLock::new((String::new(), "lossless".to_owned())),
            active_downloads: AtomicUsize::new(0),
            log_ring: RwLock::new(None),
            log_dir: RwLock::new(None),
            time_offset: AtomicI32::new(0),
            stats: Mutex::new(StatsStore::new(0)),
            edition: RwLock::new("full"),
            logs_enabled: RwLock::new(None),
            rival: RwLock::new(crate::rival::RivalState::default()),
            folder_picker: RwLock::new(None),
        })
    }
    pub fn subscribe(&self) -> broadcast::Receiver<StateEvent> {
        self.state_bus.subscribe()
    }
    pub fn publish(&self, ev: StateEvent) {
        let _ = self.state_bus.send(ev);
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
    pub fn set_current_version(&self, v: String) {
        self.update.write().expect("update lock").current = v;
    }
    pub fn update_notice(&self) -> Option<String> {
        self.update.read().expect("update lock").applied.clone()
    }
    pub fn apply_update_notice(&self, version: String, notes: Option<String>, url: Option<String>) {
        let mut g = self.update.write().expect("update lock");
        let changed = g.applied.as_deref() != Some(version.as_str());
        g.applied = Some(version.clone());
        g.latest = Some(version);
        g.notes = notes;
        g.url = url;
        drop(g);
        if changed {
            self.publish(StateEvent::UpdateHint);
        }
    }
    pub fn set_update_checker(&self, f: UpdateChecker) {
        *self.update_checker.write().expect("update checker lock") = Some(f);
    }
    pub async fn run_update_check(&self) {
        let checker = self.update_checker.read().expect("update checker lock").clone();
        let Some(checker) = checker else { return };
        let res = checker().await;
        let mut g = self.update.write().expect("update lock");
        g.last_check_ok = Some(res.is_some());
        if let Some(res) = res {
            g.latest = Some(res.latest);
            g.notes = res.notes;
            g.url = res.url;
            g.has_update = res.has_update;
        }
    }
    pub fn update_info_payload(&self) -> serde_json::Value {
        let has_checker = self.update_checker.read().expect("update checker lock").is_some();
        let g = self.update.read().expect("update lock");
        let latest = g.latest.clone().or_else(|| g.applied.clone());
        let status = match (has_checker, g.last_check_ok) {
            (false, _) => "off",
            (true, None) => "unchecked",
            (true, Some(true)) => "ok",
            (true, Some(false)) => "failed",
        };
        serde_json::json!({
            "event": "UpdateInfo",
            "currentVersion": g.current,
            "latestVersion": latest,
            "releaseNotes": g.notes,
            "pageUrl": g.url,
            "hasUpdate": g.has_update || g.applied.is_some(),
            "status": status,
        })
    }
    pub fn discord_status(&self) -> Option<String> {
        self.discord_status.read().expect("discord status lock").clone()
    }
    pub fn set_discord_status(&self, v: Option<String>) -> bool {
        let mut g = self.discord_status.write().expect("discord status lock");
        if *g == v {
            return false;
        }
        *g = v;
        true
    }
    pub fn apply_discord_status(&self, status: String) {
        if self.set_discord_status(Some(status)) {
            self.publish(StateEvent::DiscordHint);
        }
    }
    pub fn rival(&self) -> crate::rival::RivalState {
        *self.rival.read().expect("rival lock")
    }
    pub fn set_rival(&self, state: crate::rival::RivalState) {
        let mut g = self.rival.write().expect("rival lock");
        if *g == state {
            return;
        }
        *g = state;
        drop(g);
        self.publish(StateEvent::RivalHint);
    }
    pub fn rival_payload(&self) -> Option<serde_json::Value> {
        let state = self.rival();
        state.present().then(|| sd_protocol::rival_notice_payload(state.level()))
    }
    pub fn set_client_path_checker(&self, f: ClientPathChecker) {
        *self.path_checker.write().expect("path checker lock") = Some(f);
    }
    pub fn check_client_path(&self, raw: &str) -> Option<ClientPathReport> {
        let g = self.path_checker.read().expect("path checker lock");
        g.as_ref().map(|f| f(raw))
    }
    pub fn set_folder_picker(&self, f: FolderPicker) {
        *self.folder_picker.write().expect("folder picker lock") = Some(f);
    }
    pub fn folder_picker(&self) -> Option<FolderPicker> {
        self.folder_picker.read().expect("folder picker lock").clone()
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
    pub fn set_log_ring(&self, ring: Arc<Mutex<VecDeque<String>>>) {
        *self.log_ring.write().expect("log ring lock") = Some(ring);
    }
    pub fn set_log_dir(&self, dir: PathBuf) {
        *self.log_dir.write().expect("log dir lock") = Some(dir);
    }
    pub fn log_dir(&self) -> Option<PathBuf> {
        self.log_dir.read().expect("log dir lock").clone()
    }
    pub fn log_snapshot(&self) -> Vec<String> {
        match self.log_ring.read().expect("log ring lock").as_ref() {
            Some(ring) => ring.lock().expect("ring mutex").iter().cloned().collect(),
            None => Vec::new(),
        }
    }
    pub fn log_clear(&self) {
        if let Some(ring) = self.log_ring.read().expect("log ring lock").as_ref() {
            ring.lock().expect("ring mutex").clear();
        }
    }
    pub fn set_time_offset(&self, offset_secs: i32) {
        self.time_offset.store(offset_secs, Ordering::Release);
    }
    pub fn log_buffer_payload(&self) -> serde_json::Value {
        let offset = self.time_offset.load(Ordering::Acquire);
        let lines: Vec<serde_json::Value> = self
            .log_snapshot()
            .into_iter()
            .map(|t| {
                let p = crate::log_lines::parse_ring_line(&t, offset);
                let mut obj = serde_json::Map::new();
                if let Some(at) = p.at {
                    obj.insert("at".to_owned(), at.into());
                }
                if let Some(level) = p.level {
                    obj.insert("level".to_owned(), level.into());
                }
                obj.insert("text".to_owned(), p.text.into());
                serde_json::Value::Object(obj)
            })
            .collect();
        serde_json::json!({ "event": "LogBuffer", "lines": lines })
    }
    pub fn set_stats_dir(&self, dir: PathBuf, offset_secs: i32) {
        self.stats.lock().expect("stats lock").attach(dir, offset_secs, epoch_secs());
    }
    pub fn stats_on_track(&self, t: &TrackData) {
        self.stats.lock().expect("stats lock").on_track(&t.track_id, &t.title, &t.artist, epoch_secs());
    }
    pub fn stats_on_connection(&self, connected: bool) {
        self.stats.lock().expect("stats lock").on_connection(connected, epoch_secs());
    }
    pub fn stats_record_action(&self, kind: ActionKind) {
        self.stats.lock().expect("stats lock").record_action(epoch_secs(), kind);
    }
    pub fn stats_record_download(&self, bytes: u64) {
        self.stats.lock().expect("stats lock").record_download(epoch_secs(), bytes);
    }
    pub fn stats_tick_and_flush(&self) {
        let mut g = self.stats.lock().expect("stats lock");
        g.tick(epoch_secs());
        g.flush();
    }
    pub fn stats_finalize_and_flush(&self) {
        self.stats.lock().expect("stats lock").finalize(epoch_secs());
    }
    pub fn stats_payload(&self, period: &str) -> serde_json::Value {
        self.stats.lock().expect("stats lock").payload(period, epoch_secs())
    }
    pub fn set_edition(&self, edition: &'static str) {
        *self.edition.write().expect("edition lock") = edition;
    }
    pub fn edition(&self) -> &'static str {
        *self.edition.read().expect("edition lock")
    }
    pub fn edition_payload(&self) -> serde_json::Value {
        serde_json::json!({ "event": "EditionInfo", "edition": self.edition() })
    }
    pub fn set_logs_enabled_flag(&self, flag: Arc<AtomicBool>) {
        *self.logs_enabled.write().expect("logs enabled lock") = Some(flag);
    }
    pub fn set_logs_enabled(&self, enabled: bool) {
        if let Some(flag) = self.logs_enabled.read().expect("logs enabled lock").as_ref() {
            flag.store(enabled, Ordering::Release);
        }
    }
    pub fn current_version(&self) -> String {
        self.update.read().expect("update lock").current.clone()
    }
    pub fn health_payload(&self) -> serde_json::Value {
        let connected = self.cdp.is_connected();
        let version = self.current_version();
        let edition = self.edition();
        let launch_ok = self.launch_reason().is_none();
        let stats_ok = self.stats.lock().expect("stats lock").attached();
        let discord_ok = !matches!(self.discord_status().as_deref(), Some("no_client") | Some("error"));
        let check = |id: &str, ok: bool, detail: String| serde_json::json!({ "id": id, "ok": ok, "detail": detail });
        let checks = serde_json::json!([
            check("host", true, String::new()),
            check("settings", true, String::new()),
            check("renderer", true, String::new()),
            check("stats", stats_ok, String::new()),
            check("client", connected, String::new()),
            check("launch", launch_ok, String::new()),
            check("discord", discord_ok, String::new()),
            check("version", !version.is_empty(), version.clone()),
            check("edition", true, edition.to_string()),
        ]);
        serde_json::json!({
            "event": "HealthData",
            "connected": connected,
            "version": version,
            "edition": edition,
            "checks": checks,
        })
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
    pub async fn send_local_status(&self, s: LocalStatus) {
        let reason = self.shared.launch_reason();
        self.send_to_pi(sd_protocol::local_status_payload(s, reason.as_deref())).await;
    }

    pub async fn report_status(&self) {
        let s = if self.shared.cdp.is_connected() {
            LocalStatus::Connected
        } else {
            LocalStatus::Disconnected
        };
        self.send_local_status(s).await;
        if let Some(v) = self.shared.update_notice() {
            self.send_to_pi(sd_protocol::update_notice_payload(&v)).await;
        }
        if let Some(s) = self.shared.discord_status() {
            self.send_to_pi(sd_protocol::discord_status_payload(&s)).await;
        }
        if let Some(p) = self.shared.rival_payload() {
            self.send_to_pi(p).await;
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
