use std::collections::HashMap;
use std::sync::Arc;

use sd_protocol::{ApplySettingsPayload, Inbound, Outbound};
use tokio::sync::mpsc;
use ym_model::{ControlMode, GlobalSettings, PluginSettings};

use crate::action::{Action, Shared};
use crate::actor::{spawn_actor, ActorHandle, ActorMsg};

pub type ActionFactory = Arc<dyn Fn(&str) -> Option<Box<dyn Action>> + Send + Sync>;

pub struct Orchestrator {
    plugin_uuid: String,
    host: mpsc::Sender<Outbound>,
    shared: Arc<Shared>,
    factory: ActionFactory,
    actors: HashMap<String, ActorHandle>,
    global: GlobalSettings,
}

impl Orchestrator {
    pub fn new(
        plugin_uuid: String,
        host: mpsc::Sender<Outbound>,
        shared: Arc<Shared>,
        factory: ActionFactory,
    ) -> Self {
        Self {
            plugin_uuid,
            host,
            shared,
            factory,
            actors: HashMap::new(),
            global: GlobalSettings::default(),
        }
    }

    pub fn global_settings(&self) -> &GlobalSettings {
        &self.global
    }

    pub async fn run(mut self, mut inbound: mpsc::Receiver<Inbound>) {
        while let Some(ev) = inbound.recv().await {
            self.handle(ev).await;
        }
        for h in self.actors.values() {
            h.cancel.cancel();
        }
        tracing::info!("orchestrator: входящий канал закрыт — завершение, плагин {}", self.plugin_uuid);
    }

    async fn handle(&mut self, ev: Inbound) {
        match ev {
            Inbound::WillAppear(a) => {
                let settings_val = a.settings();
                let (Some(context), Some(uuid)) = (a.context, a.action) else { return };
                if !self.actors.contains_key(&context) {
                    match (self.factory)(&uuid) {
                        Some(action) => {
                            let settings = PluginSettings::from_value(&settings_val);
                            let handle = spawn_actor(
                                action,
                                context.clone(),
                                uuid,
                                settings,
                                self.host.clone(),
                                self.shared.clone(),
                            );
                            self.actors.insert(context.clone(), handle);
                        }
                        None => {
                            tracing::error!("неизвестный UUID действия: {uuid}");
                            return;
                        }
                    }
                }
                self.forward(&context, ActorMsg::Appear).await;
                self.recalc_local();
            }

            Inbound::WillDisappear(a) => {
                if let Some(context) = a.context {
                    if let Some(h) = self.actors.remove(&context) {
                        h.cancel.cancel();
                    }
                    self.recalc_ynison();
                    self.recalc_local();
                }
            }

            Inbound::KeyDown(a) => {
                self.record_action(a.context.as_deref());
                self.kick_if_local_disconnected(a.context.as_deref());
                self.forward_opt(a.context, ActorMsg::KeyDown).await
            }
            Inbound::KeyUp(a) => self.forward_opt(a.context, ActorMsg::KeyUp).await,
            Inbound::DialRotate(a) => {
                let ticks = a.ticks();
                self.forward_opt(a.context, ActorMsg::DialRotate(ticks)).await
            }
            Inbound::DialDown(a) => {
                self.record_action(a.context.as_deref());
                self.kick_if_local_disconnected(a.context.as_deref());
                self.forward_opt(a.context, ActorMsg::DialDown).await
            }
            Inbound::DialUp(a) => self.forward_opt(a.context, ActorMsg::DialUp).await,
            Inbound::PropertyInspectorDidAppear(a) => self.forward_opt(a.context, ActorMsg::PiAppear).await,
            Inbound::TitleParametersDidChange(_) => {}

            Inbound::DidReceiveSettings(a) => {
                let s = PluginSettings::from_value(&a.settings());
                if let Some(context) = a.context {
                    if let Some(h) = self.actors.get_mut(&context) {
                        h.control_mode = s.control_mode;
                    }
                    self.forward(&context, ActorMsg::Settings(s)).await;
                    self.recalc_ynison();
                    self.recalc_local();
                }
            }

            Inbound::SendToPlugin(a) => {
                if let Some(ap) = ApplySettingsPayload::matches(&a.payload) {
                    let mode = PluginSettings::from_value(&ap.settings).control_mode;
                    let contexts: Vec<String> = self.actors.keys().cloned().collect();
                    for ctx in &contexts {
                        if let Some(h) = self.actors.get_mut(ctx) {
                            h.control_mode = mode;
                        }
                        self.forward(ctx, ActorMsg::ApplySettings(ap.settings.clone())).await;
                    }
                    self.recalc_ynison();
                    self.recalc_local();
                } else if a.payload.get("event").and_then(serde_json::Value::as_str)
                    == Some("check_client_path")
                {
                    self.answer_client_path_check(&a).await;
                } else if a.payload.get("event").and_then(serde_json::Value::as_str) == Some("getUpdateInfo") {
                    let payload = self.shared.update_info_payload();
                    self.reply_to_pi(&a, payload).await;
                } else if a.payload.get("event").and_then(serde_json::Value::as_str) == Some("checkUpdates") {
                    self.spawn_update_check(&a);
                } else if a.payload.get("event").and_then(serde_json::Value::as_str) == Some("getLogs") {
                    let payload = self.shared.log_buffer_payload();
                    self.reply_to_pi(&a, payload).await;
                } else if a.payload.get("event").and_then(serde_json::Value::as_str) == Some("clearLogs") {
                    self.shared.log_clear();
                } else if a.payload.get("event").and_then(serde_json::Value::as_str) == Some("getStats") {
                    let period = a.payload.get("period").and_then(serde_json::Value::as_str).unwrap_or("week");
                    let payload = self.shared.stats_payload(period);
                    self.reply_to_pi(&a, payload).await;
                } else if a.payload.get("event").and_then(serde_json::Value::as_str) == Some("getEdition") {
                    let payload = self.shared.edition_payload();
                    self.reply_to_pi(&a, payload).await;
                } else if a.payload.get("event").and_then(serde_json::Value::as_str) == Some("launchClient") {
                    self.shared.launch_kick();
                    let payload = sd_protocol::local_status_payload(sd_protocol::LocalStatus::Loading, None);
                    self.reply_to_pi(&a, payload).await;
                } else if a.payload.get("event").and_then(serde_json::Value::as_str) == Some("checkConnection") {
                    self.reply_connection_status(&a).await;
                } else if a.payload.get("event").and_then(serde_json::Value::as_str) == Some("getHealth") {
                    let payload = self.shared.health_payload();
                    self.reply_to_pi(&a, payload).await;
                } else if a.payload.get("event").and_then(serde_json::Value::as_str) == Some("openLogsFolder")
                    && let Some(dir) = self.shared.log_dir()
                {
                    open_folder(&dir);
                } else if a.payload.get("event").and_then(serde_json::Value::as_str) == Some("pickFolder") {
                    self.spawn_pick_folder(&a);
                }
            }

            Inbound::DidReceiveGlobalSettings(g) => {
                self.global = GlobalSettings::from_value(&g.settings());
                self.shared.set_token(self.global.token.clone());
                if let Some(port) = self.global.local_port {
                    self.shared.cdp.set_local_port(port);
                }
                self.shared.set_discord_config(self.global.discord());
                self.shared.set_launch_config(self.global.launch());
                self.shared.set_download_config(self.global.download_path.clone(), self.global.download_format.clone());
                self.shared.set_logs_enabled(self.global.debug_mode);
                self.recalc_ynison();
            }

            Inbound::SystemDidWakeUp => {
                let contexts: Vec<String> = self.actors.keys().cloned().collect();
                for ctx in contexts {
                    self.forward(&ctx, ActorMsg::Health).await;
                }
            }

            Inbound::ApplicationDidLaunch | Inbound::ApplicationDidTerminate | Inbound::Unknown => {}
        }
    }

    async fn forward(&self, context: &str, msg: ActorMsg) {
        if let Some(h) = self.actors.get(context) {
            let _ = h.cmd_tx.send(msg).await;
        }
    }

    async fn forward_opt(&self, context: Option<String>, msg: ActorMsg) {
        if let Some(c) = context {
            self.forward(&c, msg).await;
        }
    }

    fn recalc_ynison(&self) {
        let needs = self.actors.values().any(|h| h.control_mode == ControlMode::Ynison);
        tracing::debug!("recalc_ynison: needs={needs} (v1 — клиент не запускается)");
    }

    fn recalc_local(&self) {
        let needs = self.actors.values().any(|h| h.control_mode == ControlMode::Local);
        self.shared.set_any_local(needs);
    }

    fn record_action(&self, context: Option<&str>) {
        if let Some(ctx) = context
            && let Some(h) = self.actors.get(ctx)
            && let Some(kind) = crate::registry::action_kind_for(&h.uuid)
        {
            self.shared.stats_record_action(kind);
        }
    }

    fn kick_if_local_disconnected(&self, context: Option<&str>) {
        if let Some(ctx) = context
            && let Some(h) = self.actors.get(ctx)
            && crate::registry::canonical_uuid(&h.uuid) != crate::registry::uuids::LAUNCH
            && self.global.launch_from_any_button
            && h.control_mode == ControlMode::Local
            && !self.shared.cdp.is_connected()
        {
            self.shared.launch_kick();
        }
    }

    async fn answer_client_path_check(&self, a: &sd_protocol::ActionEvt) {
        let raw = a.payload.get("path").and_then(serde_json::Value::as_str).unwrap_or("");
        let Some(report) = self.shared.check_client_path(raw) else { return };
        self.reply_to_pi(a, report.payload()).await;
    }

    async fn reply_connection_status(&self, a: &sd_protocol::ActionEvt) {
        let status = if self.shared.cdp.is_connected() {
            sd_protocol::LocalStatus::Connected
        } else {
            sd_protocol::LocalStatus::Disconnected
        };
        let reason = self.shared.launch_reason();
        let mut payload = sd_protocol::local_status_payload(status, reason.as_deref());
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("port".to_owned(), serde_json::json!(self.shared.cdp.active_port()));
        }
        self.reply_to_pi(a, payload).await;
    }

    fn spawn_update_check(&self, a: &sd_protocol::ActionEvt) {
        let shared = self.shared.clone();
        let host = self.host.clone();
        let action = a
            .payload
            .get("reply_action")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .or_else(|| a.action.clone());
        let context = a
            .payload
            .get("reply_context")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .or_else(|| a.context.clone());
        tokio::spawn(async move {
            shared.run_update_check().await;
            if let (Some(action), Some(context)) = (action, context) {
                let _ = host
                    .send(Outbound::SendToPropertyInspector {
                        action,
                        context,
                        payload: shared.update_info_payload(),
                    })
                    .await;
            }
        });
    }

    fn spawn_pick_folder(&self, a: &sd_protocol::ActionEvt) {
        let target = a.payload.get("target").and_then(serde_json::Value::as_str).unwrap_or("download");
        let kind = if target == "client" {
            crate::action::PickKind::ClientApp
        } else {
            crate::action::PickKind::Folder
        };
        let target = target.to_owned();
        let action = a
            .payload
            .get("reply_action")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .or_else(|| a.action.clone());
        let context = a
            .payload
            .get("reply_context")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .or_else(|| a.context.clone());
        let (Some(action), Some(context), Some(picker)) = (action, context, self.shared.folder_picker()) else {
            return;
        };
        let host = self.host.clone();
        tokio::spawn(async move {
            let path = tokio::task::spawn_blocking(move || picker(kind)).await.ok().flatten();
            let payload = serde_json::json!({ "event": "FolderPicked", "target": target, "path": path });
            let _ = host.send(Outbound::SendToPropertyInspector { action, context, payload }).await;
        });
    }

    async fn reply_to_pi(&self, a: &sd_protocol::ActionEvt, payload: serde_json::Value) {
        let action = a
            .payload
            .get("reply_action")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .or_else(|| a.action.clone());
        let context = a
            .payload
            .get("reply_context")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .or_else(|| a.context.clone());
        let (Some(action), Some(context)) = (action, context) else { return };
        let _ = self.host.send(Outbound::SendToPropertyInspector { action, context, payload }).await;
    }
}

fn open_folder(dir: &std::path::Path) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(dir).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(dir).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{ActionCtx, Interests};
    use async_trait::async_trait;
    use sd_protocol::ActionEvt;
    use serde_json::json;
    use std::time::Duration;
    use ym_model::StateEvent;

    struct TestAction {
        interests: Interests,
    }

    #[async_trait]
    impl Action for TestAction {
        fn interests(&self) -> Interests {
            self.interests
        }
        async fn on_key_down(&mut self, cx: &ActionCtx) {
            cx.set_state(9).await;
        }
        async fn on_dial_rotate(&mut self, cx: &ActionCtx, ticks: i32) {
            cx.set_title(format!("dial:{ticks}")).await;
        }
        async fn on_dial_down(&mut self, cx: &ActionCtx) {
            cx.set_state(8).await;
        }
        async fn on_settings(&mut self, cx: &ActionCtx) {
            cx.set_state(2).await;
        }
        async fn on_state(&mut self, cx: &ActionCtx, ev: &StateEvent) {
            cx.set_title(format!("{:?}", ev.kind())).await;
        }
        async fn render(&mut self, cx: &ActionCtx) {
            cx.set_state(1).await;
        }
    }

    fn run_orch(interests: Interests) -> (mpsc::Sender<Inbound>, mpsc::Receiver<Outbound>, Arc<Shared>) {
        let (in_tx, in_rx) = mpsc::channel(64);
        let (out_tx, out_rx) = mpsc::channel(256);
        let shared = Shared::new();
        let factory: ActionFactory =
            Arc::new(move |_uuid: &str| Some(Box::new(TestAction { interests }) as Box<dyn Action>));
        let orch = Orchestrator::new("PLUGIN".into(), out_tx, shared.clone(), factory);
        tokio::spawn(orch.run(in_rx));
        (in_tx, out_rx, shared)
    }

    fn appear(context: &str) -> Inbound {
        Inbound::WillAppear(ActionEvt {
            context: Some(context.into()),
            action: Some("com.judd1.yandex_music.action.next".into()),
            device: None,
            payload: json!({ "settings": {} }),
        })
    }
    fn appear_with(context: &str, settings: serde_json::Value) -> Inbound {
        Inbound::WillAppear(ActionEvt {
            context: Some(context.into()),
            action: Some("com.judd1.yandex_music.action.next".into()),
            device: None,
            payload: json!({ "settings": settings }),
        })
    }
    fn plain(context: &str) -> ActionEvt {
        ActionEvt {
            context: Some(context.into()),
            action: None,
            device: None,
            payload: json!({}),
        }
    }

    async fn next_out(rx: &mut mpsc::Receiver<Outbound>) -> Outbound {
        tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("таймаут ожидания Outbound")
            .expect("канал закрыт")
    }
    async fn expect_silence(rx: &mut mpsc::Receiver<Outbound>) {
        assert!(
            tokio::time::timeout(Duration::from_millis(150), rx.recv()).await.is_err(),
            "ожидалась тишина"
        );
    }
    fn state_of(o: &Outbound) -> u8 {
        match o {
            Outbound::SetState { payload, .. } => payload.state,
            other => panic!("ожидался SetState, {other:?}"),
        }
    }

    #[tokio::test]
    async fn will_appear_spawns_actor_and_renders() {
        let (tx, mut rx, _s) = run_orch(Interests::all());
        tx.send(appear("c1")).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 1);
    }

    #[tokio::test]
    async fn update_hint_pushes_notice_to_pi() {
        let (tx, mut rx, shared) = run_orch(Interests::CONNECTION);
        tx.send(appear("c1")).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 1);

        shared.apply_update_notice("9.9.9".into(), None, None);
        match next_out(&mut rx).await {
            Outbound::SendToPropertyInspector { payload, .. } => {
                assert_eq!(payload["event"], "LocalStatus");
            }
            other => panic!("ожидался LocalStatus, {other:?}"),
        }
        match next_out(&mut rx).await {
            Outbound::SendToPropertyInspector { payload, .. } => {
                assert_eq!(payload["event"], "UpdateNotice");
                assert_eq!(payload["version"], "9.9.9");
            }
            other => panic!("ожидался UpdateNotice, {other:?}"),
        }
    }

    #[tokio::test]
    async fn check_client_path_request_is_answered_to_reply_address() {
        let (tx, mut rx, shared) = run_orch(Interests::all());
        shared.set_client_path_checker(Arc::new(|raw: &str| crate::action::ClientPathReport {
            verdict: if raw == "C:\\ok.exe" { "ok" } else { "missing" },
            resolved: None,
            expected: "Яндекс Музыка.exe",
        }));
        tx.send(Inbound::SendToPlugin(ActionEvt {
            context: None,
            action: None,
            device: None,
            payload: json!({
                "event": "check_client_path",
                "path": "C:\\ok.exe",
                "reply_action": "com.judd1.yandex_music.action.next",
                "reply_context": "BTN-1",
            }),
        }))
        .await
        .unwrap();
        match next_out(&mut rx).await {
            Outbound::SendToPropertyInspector { action, context, payload } => {
                assert_eq!(action, "com.judd1.yandex_music.action.next");
                assert_eq!(context, "BTN-1");
                assert_eq!(payload["event"], "ClientPathCheck");
                assert_eq!(payload["verdict"], "ok");
                assert_eq!(payload["expected"], "Яндекс Музыка.exe");
            }
            other => panic!("ожидался SendToPropertyInspector, {other:?}"),
        }
    }

    #[tokio::test]
    async fn pick_folder_replies_with_selected_path() {
        let (tx, mut rx, shared) = run_orch(Interests::all());
        shared.set_folder_picker(Arc::new(|kind: crate::action::PickKind| match kind {
            crate::action::PickKind::Folder => Some("/Users/me/Music".to_owned()),
            crate::action::PickKind::ClientApp => None,
        }));
        tx.send(Inbound::SendToPlugin(ActionEvt {
            context: None,
            action: None,
            device: None,
            payload: json!({
                "event": "pickFolder",
                "target": "download",
                "reply_action": "com.judd1.yandex_music.action.next",
                "reply_context": "BTN-1",
            }),
        }))
        .await
        .unwrap();
        match next_out(&mut rx).await {
            Outbound::SendToPropertyInspector { action, context, payload } => {
                assert_eq!(action, "com.judd1.yandex_music.action.next");
                assert_eq!(context, "BTN-1");
                assert_eq!(payload["event"], "FolderPicked");
                assert_eq!(payload["target"], "download");
                assert_eq!(payload["path"], "/Users/me/Music");
            }
            other => panic!("ожидался SendToPropertyInspector, {other:?}"),
        }
    }

    #[tokio::test]
    async fn pick_folder_cancel_replies_with_null_path() {
        let (tx, mut rx, shared) = run_orch(Interests::all());
        shared.set_folder_picker(Arc::new(|_kind: crate::action::PickKind| None));
        tx.send(Inbound::SendToPlugin(ActionEvt {
            context: None,
            action: None,
            device: None,
            payload: json!({
                "event": "pickFolder",
                "target": "client",
                "reply_action": "com.judd1.yandex_music.action.next",
                "reply_context": "BTN-1",
            }),
        }))
        .await
        .unwrap();
        match next_out(&mut rx).await {
            Outbound::SendToPropertyInspector { payload, .. } => {
                assert_eq!(payload["event"], "FolderPicked");
                assert_eq!(payload["target"], "client");
                assert!(payload["path"].is_null());
            }
            other => panic!("ожидался SendToPropertyInspector, {other:?}"),
        }
    }

    #[tokio::test]
    async fn check_client_path_is_silent_without_checker_or_reply_address() {
        let (tx, mut rx, shared) = run_orch(Interests::all());
        let req = |path: &str| {
            Inbound::SendToPlugin(ActionEvt {
                context: None,
                action: None,
                device: None,
                payload: json!({ "event": "check_client_path", "path": path }),
            })
        };
        tx.send(req("x")).await.unwrap();
        expect_silence(&mut rx).await;

        shared.set_client_path_checker(Arc::new(|_raw: &str| crate::action::ClientPathReport {
            verdict: "ok",
            resolved: None,
            expected: "Яндекс Музыка.exe",
        }));
        tx.send(req("x")).await.unwrap();
        expect_silence(&mut rx).await;
    }

    #[tokio::test]
    async fn get_update_info_replies_with_current_version() {
        let (tx, mut rx, shared) = run_orch(Interests::all());
        shared.set_current_version("2.4.0".into());
        tx.send(Inbound::SendToPlugin(ActionEvt {
            context: None,
            action: None,
            device: None,
            payload: json!({
                "event": "getUpdateInfo",
                "reply_action": "com.judd1.yandex_music.action.next",
                "reply_context": "BTN-1",
            }),
        }))
        .await
        .unwrap();
        match next_out(&mut rx).await {
            Outbound::SendToPropertyInspector { payload, .. } => {
                assert_eq!(payload["event"], "UpdateInfo");
                assert_eq!(payload["currentVersion"], "2.4.0");
                assert_eq!(payload["hasUpdate"], false);
            }
            other => panic!("ожидался UpdateInfo, {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_logs_replies_with_buffer() {
        let (tx, mut rx, _s) = run_orch(Interests::all());
        tx.send(Inbound::SendToPlugin(ActionEvt {
            context: None,
            action: None,
            device: None,
            payload: json!({
                "event": "getLogs",
                "reply_action": "com.judd1.yandex_music.action.next",
                "reply_context": "BTN-1",
            }),
        }))
        .await
        .unwrap();
        match next_out(&mut rx).await {
            Outbound::SendToPropertyInspector { payload, .. } => {
                assert_eq!(payload["event"], "LogBuffer");
                assert!(payload["lines"].is_array());
            }
            other => panic!("ожидался LogBuffer, {other:?}"),
        }
    }

    #[test]
    fn log_buffer_payload_splits_lines_into_at_level_text() {
        let shared = Shared::new();
        let ring = std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::from([
            "2026-07-24T12:34:56.789012Z  INFO Plugin Start".to_owned(),
            "plain fallback line".to_owned(),
        ])));
        shared.set_log_ring(ring);
        shared.set_time_offset(10800);
        let payload = shared.log_buffer_payload();
        let lines = payload["lines"].as_array().unwrap();
        assert_eq!(lines[0]["at"], "15:34:56");
        assert_eq!(lines[0]["level"], "info");
        assert_eq!(lines[0]["text"], "Plugin Start");
        assert!(lines[1].get("at").is_none());
        assert!(lines[1].get("level").is_none());
        assert_eq!(lines[1]["text"], "plain fallback line");
    }

    #[tokio::test]
    async fn check_updates_runs_checker_and_replies_with_notes() {
        let (tx, mut rx, shared) = run_orch(Interests::all());
        shared.set_current_version("2.4.0".into());
        shared.set_update_checker(Arc::new(|| {
            Box::pin(async {
                Some(ym_model::UpdateCheck {
                    latest: "2.5.0".into(),
                    notes: Some("• fix".into()),
                    url: Some("https://x/rel".into()),
                    has_update: true,
                })
            })
        }));
        tx.send(Inbound::SendToPlugin(ActionEvt {
            context: None,
            action: None,
            device: None,
            payload: json!({
                "event": "checkUpdates",
                "reply_action": "com.judd1.yandex_music.action.next",
                "reply_context": "BTN-1",
            }),
        }))
        .await
        .unwrap();
        match next_out(&mut rx).await {
            Outbound::SendToPropertyInspector { payload, .. } => {
                assert_eq!(payload["event"], "UpdateInfo");
                assert_eq!(payload["latestVersion"], "2.5.0");
                assert_eq!(payload["releaseNotes"], "• fix");
                assert_eq!(payload["hasUpdate"], true);
            }
            other => panic!("ожидался UpdateInfo, {other:?}"),
        }
    }

    #[tokio::test]
    async fn key_down_is_dispatched_to_actor() {
        let (tx, mut rx, _s) = run_orch(Interests::all());
        tx.send(appear("c1")).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 1);
        tx.send(Inbound::KeyDown(plain("c1"))).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 9);
    }

    #[tokio::test]
    async fn will_disappear_stops_actor() {
        let (tx, mut rx, _s) = run_orch(Interests::all());
        tx.send(appear("c1")).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 1);
        tx.send(Inbound::WillDisappear(plain("c1"))).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        tx.send(Inbound::KeyDown(plain("c1"))).await.unwrap();
        expect_silence(&mut rx).await;
    }

    #[tokio::test]
    async fn did_receive_settings_updates_and_rerenders() {
        let (tx, mut rx, _s) = run_orch(Interests::all());
        tx.send(appear("c1")).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 1);
        tx.send(Inbound::DidReceiveSettings(plain("c1"))).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 2);
    }

    #[tokio::test]
    async fn apply_settings_to_all_fans_out_and_saves_back() {
        let (tx, mut rx, _s) = run_orch(Interests::all());
        tx.send(appear("c1")).await.unwrap();
        tx.send(appear("c2")).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 1);
        assert_eq!(state_of(&next_out(&mut rx).await), 1);

        tx.send(Inbound::SendToPlugin(ActionEvt {
            context: None,
            action: None,
            device: None,
            payload: json!({ "event": "applySettingsToAll", "settings": { "play_style": "v4" } }),
        }))
        .await
        .unwrap();

        let mut set_states = 0;
        let mut saved_contexts = std::collections::HashSet::new();
        for _ in 0..4 {
            match next_out(&mut rx).await {
                Outbound::SetState { payload, .. } => {
                    assert_eq!(payload.state, 2);
                    set_states += 1;
                }
                Outbound::SetSettings { context, payload } => {
                    assert_eq!(payload["play_style"], "v4");
                    saved_contexts.insert(context);
                }
                other => panic!("неожиданно: {other:?}"),
            }
        }
        assert_eq!(set_states, 2);
        assert_eq!(saved_contexts, ["c1".to_string(), "c2".to_string()].into_iter().collect());
    }

    #[tokio::test]
    async fn apply_settings_to_all_merges_partial_patch_preserving_other_fields() {
        let (tx, mut rx, _s) = run_orch(Interests::all());
        tx.send(appear_with("c1", json!({ "next_style": "v3" }))).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 1);

        tx.send(Inbound::SendToPlugin(ActionEvt {
            context: None,
            action: None,
            device: None,
            payload: json!({ "event": "applySettingsToAll", "settings": { "control_mode": "ynison" } }),
        }))
        .await
        .unwrap();

        let mut saved = None;
        for _ in 0..2 {
            match next_out(&mut rx).await {
                Outbound::SetState { payload, .. } => assert_eq!(payload.state, 2),
                Outbound::SetSettings { payload, .. } => saved = Some(payload),
                other => panic!("неожиданно: {other:?}"),
            }
        }
        let saved = saved.expect("ожидался SetSettings");
        assert_eq!(saved["control_mode"], "ynison");
        assert_eq!(saved["next_style"], "v3");
    }

    #[tokio::test]
    async fn debounced_save_flushed_on_disappear() {
        let (tx, mut rx, _s) = run_orch(Interests::all());
        tx.send(appear("c1")).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 1);

        tx.send(Inbound::SendToPlugin(ActionEvt {
            context: None,
            action: None,
            device: None,
            payload: json!({ "event": "applySettingsToAll", "settings": { "play_style": "v4" } }),
        }))
        .await
        .unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 2);

        tx.send(Inbound::WillDisappear(plain("c1"))).await.unwrap();
        match next_out(&mut rx).await {
            Outbound::SetSettings { payload, .. } => assert_eq!(payload["play_style"], "v4"),
            other => panic!("ожидался flush SetSettings при исчезновении, {other:?}"),
        }
    }

    #[tokio::test]
    async fn state_events_filtered_by_interests() {
        let (tx, mut rx, shared) = run_orch(Interests::CONNECTION | Interests::LIKE);
        tx.send(appear("c1")).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 1);

        shared.publish(StateEvent::Volume(Default::default()));
        expect_silence(&mut rx).await;

        shared.publish(StateEvent::Like(Default::default()));
        match next_out(&mut rx).await {
            Outbound::SetTitle { payload, .. } => assert_eq!(payload.title, "Like"),
            other => panic!("ожидался SetTitle, {other:?}"),
        }
    }

    #[tokio::test]
    async fn unknown_uuid_does_not_spawn() {
        let (in_tx, in_rx) = mpsc::channel(64);
        let (out_tx, mut out_rx) = mpsc::channel(64);
        let shared = Shared::new();
        let factory: ActionFactory = Arc::new(|_uuid: &str| None);
        tokio::spawn(Orchestrator::new("P".into(), out_tx, shared, factory).run(in_rx));
        in_tx.send(appear("c1")).await.unwrap();
        expect_silence(&mut out_rx).await;
    }

    #[tokio::test]
    async fn local_mode_real_like_action_renders_on_image() {
        use ym_model::ports::MediaController;
        use ym_model::{ActionResult, LikeData, MediaState};

        struct Connected;
        #[async_trait]
        impl MediaController for Connected {
            fn is_connected(&self) -> bool {
                true
            }
            fn snapshot(&self) -> MediaState {
                MediaState { like: LikeData { is_liked: true }, ..Default::default() }
            }
            async fn play_pause(&self) -> ActionResult {
                ActionResult::default()
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
            async fn toggle_shuffle(&self) -> ActionResult {
                ActionResult::default()
            }
            async fn toggle_repeat(&self) -> ActionResult {
                ActionResult::default()
            }
            async fn change_volume(&self, _a: ym_model::ports::VolumeAction) -> ActionResult {
                ActionResult::default()
            }
        }

        let (in_tx, in_rx) = mpsc::channel(64);
        let (out_tx, mut out_rx) = mpsc::channel(64);
        let render = ym_render::Renderers::new();
        let (bus, _) = tokio::sync::broadcast::channel(64);
        let shared =
            Shared::wired(bus, Arc::new(Connected), render.clone(), Arc::new(ym_model::StubDownloader));
        let factory: ActionFactory = Arc::new(|u: &str| crate::registry::build_action(u));
        tokio::spawn(Orchestrator::new("P".into(), out_tx, shared, factory).run(in_rx));

        in_tx
            .send(Inbound::WillAppear(ActionEvt {
                context: Some("c1".into()),
                action: Some(crate::registry::uuids::LIKE.into()),
                device: None,
                payload: json!({ "settings": {} }),
            }))
            .await
            .unwrap();

        let expected = render.icon_b64("btn_yandex_music_like_v1_on.png").unwrap().as_ref().to_owned();
        match next_out(&mut out_rx).await {
            Outbound::SetState { payload, .. } => assert_eq!(payload.state, 1),
            o => panic!("ждал SetState(1), {o:?}"),
        }
        match next_out(&mut out_rx).await {
            Outbound::SetImage { payload, .. } => assert_eq!(payload.image, expected),
            o => panic!("ждал SetImage(like_on), {o:?}"),
        }
    }

    async fn wait_flag(flag: &Arc<std::sync::atomic::AtomicBool>, want: bool) {
        for _ in 0..200 {
            if flag.load(std::sync::atomic::Ordering::Acquire) == want {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("флаг any_local не стал {want}");
    }

    fn settings_evt(context: &str, settings: serde_json::Value) -> Inbound {
        Inbound::DidReceiveSettings(ActionEvt {
            context: Some(context.into()),
            action: None,
            device: None,
            payload: json!({ "settings": settings }),
        })
    }

    #[tokio::test]
    async fn recalc_local_tracks_local_actors() {
        let (tx, mut rx, shared) = run_orch(Interests::all());
        let flag = shared.any_local_flag();
        assert!(!flag.load(std::sync::atomic::Ordering::Acquire));

        tx.send(appear("c1")).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 1);
        wait_flag(&flag, true).await;

        tx.send(settings_evt("c1", json!({ "control_mode": "ynison" }))).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 2);
        wait_flag(&flag, false).await;

        tx.send(settings_evt("c1", json!({ "control_mode": "local" }))).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 2);
        wait_flag(&flag, true).await;

        tx.send(Inbound::WillDisappear(plain("c1"))).await.unwrap();
        wait_flag(&flag, false).await;
    }

    #[tokio::test]
    async fn key_down_kicks_launcher_when_local_and_disconnected() {
        let (tx, mut rx, shared) = run_orch(Interests::all());
        let (ktx, mut krx) = mpsc::channel(4);
        shared.set_launch_kick(ktx);

        tx.send(appear("c1")).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 1);
        tx.send(Inbound::KeyDown(plain("c1"))).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 9);
        assert!(krx.try_recv().is_ok(), "ожидался кик от KeyDown при дисконнекте");
    }

    #[tokio::test]
    async fn key_down_no_kick_for_ynison_action() {
        let (tx, mut rx, shared) = run_orch(Interests::all());
        let (ktx, mut krx) = mpsc::channel(4);
        shared.set_launch_kick(ktx);

        tx.send(appear_with("c1", json!({ "control_mode": "ynison" }))).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 1);
        tx.send(Inbound::KeyDown(plain("c1"))).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 9);
        assert!(krx.try_recv().is_err(), "кик не должен уходить в ynison-режиме");
    }

    fn dial_rotate_evt(context: &str, ticks: i64) -> Inbound {
        Inbound::DialRotate(ActionEvt {
            context: Some(context.into()),
            action: None,
            device: None,
            payload: json!({ "ticks": ticks, "pressed": false }),
        })
    }

    #[tokio::test]
    async fn dial_rotate_routes_ticks_to_actor() {
        let (tx, mut rx, _s) = run_orch(Interests::all());
        tx.send(appear("c1")).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 1);
        tx.send(dial_rotate_evt("c1", 2)).await.unwrap();
        match next_out(&mut rx).await {
            Outbound::SetTitle { payload, .. } => assert_eq!(payload.title, "dial:2"),
            other => panic!("ожидался SetTitle dial:2, {other:?}"),
        }
        tx.send(dial_rotate_evt("c1", -3)).await.unwrap();
        match next_out(&mut rx).await {
            Outbound::SetTitle { payload, .. } => assert_eq!(payload.title, "dial:-3"),
            other => panic!("ожидался SetTitle dial:-3, {other:?}"),
        }
    }

    #[tokio::test]
    async fn dial_down_kicks_launcher_when_local_and_disconnected() {
        let (tx, mut rx, shared) = run_orch(Interests::all());
        let (ktx, mut krx) = mpsc::channel(4);
        shared.set_launch_kick(ktx);

        tx.send(appear("c1")).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 1);
        tx.send(Inbound::DialDown(plain("c1"))).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 8);
        assert!(krx.try_recv().is_ok(), "ожидался кик от DialDown при дисконнекте");
    }

    #[tokio::test]
    async fn dial_down_no_kick_for_ynison_action() {
        let (tx, mut rx, shared) = run_orch(Interests::all());
        let (ktx, mut krx) = mpsc::channel(4);
        shared.set_launch_kick(ktx);

        tx.send(appear_with("c1", json!({ "control_mode": "ynison" }))).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 1);
        tx.send(Inbound::DialDown(plain("c1"))).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 8);
        assert!(krx.try_recv().is_err(), "кик не должен уходить в ynison-режиме");
    }

    fn global_settings_evt(settings: serde_json::Value) -> Inbound {
        Inbound::DidReceiveGlobalSettings(sd_protocol::GlobalEvt {
            context: None,
            payload: json!({ "settings": settings }),
        })
    }

    fn appear_launch(context: &str) -> Inbound {
        Inbound::WillAppear(ActionEvt {
            context: Some(context.into()),
            action: Some("com.judd1.yandex_music.action.launch".into()),
            device: None,
            payload: json!({ "settings": {} }),
        })
    }

    fn run_real_orch() -> (mpsc::Sender<Inbound>, mpsc::Receiver<Outbound>, mpsc::Receiver<()>) {
        let (in_tx, in_rx) = mpsc::channel(64);
        let (out_tx, out_rx) = mpsc::channel(256);
        let shared = Shared::new();
        let (ktx, krx) = mpsc::channel(4);
        shared.set_launch_kick(ktx);
        let factory: ActionFactory = Arc::new(|uuid: &str| crate::registry::build_action(uuid));
        tokio::spawn(Orchestrator::new("P".into(), out_tx, shared, factory).run(in_rx));
        (in_tx, out_rx, krx)
    }

    #[tokio::test]
    async fn key_down_no_kick_when_any_button_scope_disabled() {
        let (tx, mut rx, shared) = run_orch(Interests::all());
        let (ktx, mut krx) = mpsc::channel(4);
        shared.set_launch_kick(ktx);

        tx.send(global_settings_evt(json!({ "launch_from_any_button": false }))).await.unwrap();
        tx.send(appear("c1")).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 1);
        tx.send(Inbound::KeyDown(plain("c1"))).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 9);
        assert!(krx.try_recv().is_err(), "при выключенном scope обычная кнопка не пинает запуск");
    }

    #[tokio::test]
    async fn dial_down_no_kick_when_any_button_scope_disabled() {
        let (tx, mut rx, shared) = run_orch(Interests::all());
        let (ktx, mut krx) = mpsc::channel(4);
        shared.set_launch_kick(ktx);

        tx.send(global_settings_evt(json!({ "launch_from_any_button": false }))).await.unwrap();
        tx.send(appear("c1")).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 1);
        tx.send(Inbound::DialDown(plain("c1"))).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 8);
        assert!(krx.try_recv().is_err(), "при выключенном scope энкодер не пинает запуск");
    }

    #[tokio::test]
    async fn launcher_key_down_kicks_even_when_scope_disabled() {
        let (tx, mut rx, mut krx) = run_real_orch();
        tx.send(global_settings_evt(json!({ "launch_from_any_button": false }))).await.unwrap();
        tx.send(appear_launch("c1")).await.unwrap();
        assert!(matches!(next_out(&mut rx).await, Outbound::SetImage { .. }));
        tx.send(Inbound::KeyDown(plain("c1"))).await.unwrap();
        assert!(matches!(next_out(&mut rx).await, Outbound::ShowOk { .. }));
        assert!(krx.try_recv().is_ok(), "лаунчер обязан пинать запуск при любом scope");
        assert!(krx.try_recv().is_err());
    }

    #[tokio::test]
    async fn launcher_key_down_single_kick_when_scope_enabled() {
        let (tx, mut rx, mut krx) = run_real_orch();
        tx.send(appear_launch("c1")).await.unwrap();
        assert!(matches!(next_out(&mut rx).await, Outbound::SetImage { .. }));
        tx.send(Inbound::KeyDown(plain("c1"))).await.unwrap();
        assert!(matches!(next_out(&mut rx).await, Outbound::ShowOk { .. }));
        assert!(krx.try_recv().is_ok(), "ожидался ровно один кик от лаунчера");
        assert!(krx.try_recv().is_err(), "оркестратор не должен дублировать кик лаунчера");
    }

    #[tokio::test]
    async fn dial_up_default_is_noop() {
        let (tx, mut rx, _s) = run_orch(Interests::all());
        tx.send(appear("c1")).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 1);
        tx.send(Inbound::DialUp(plain("c1"))).await.unwrap();
        expect_silence(&mut rx).await;
    }

    #[tokio::test]
    async fn key_down_no_kick_when_connected() {
        use ym_model::ports::{MediaController, VolumeAction};
        use ym_model::{ActionResult, MediaState};

        struct ConnectedStub;
        #[async_trait]
        impl MediaController for ConnectedStub {
            fn is_connected(&self) -> bool {
                true
            }
            fn snapshot(&self) -> MediaState {
                MediaState::default()
            }
            async fn play_pause(&self) -> ActionResult {
                ActionResult::default()
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
            async fn toggle_shuffle(&self) -> ActionResult {
                ActionResult::default()
            }
            async fn toggle_repeat(&self) -> ActionResult {
                ActionResult::default()
            }
            async fn change_volume(&self, _a: VolumeAction) -> ActionResult {
                ActionResult::default()
            }
        }

        let (in_tx, in_rx) = mpsc::channel(64);
        let (out_tx, mut out_rx) = mpsc::channel(256);
        let shared = Shared::with(Arc::new(ConnectedStub), ym_render::Renderers::new());
        let (ktx, mut krx) = mpsc::channel(4);
        shared.set_launch_kick(ktx);
        let factory: ActionFactory =
            Arc::new(|_uuid: &str| Some(Box::new(TestAction { interests: Interests::all() }) as Box<dyn Action>));
        tokio::spawn(Orchestrator::new("P".into(), out_tx, shared, factory).run(in_rx));

        in_tx.send(appear("c1")).await.unwrap();
        assert_eq!(state_of(&next_out(&mut out_rx).await), 1);
        in_tx.send(Inbound::KeyDown(plain("c1"))).await.unwrap();
        assert_eq!(state_of(&next_out(&mut out_rx).await), 9);
        assert!(krx.try_recv().is_err(), "кик не должен уходить при живом подключении");
    }

    fn command(event: &str, extra: serde_json::Value) -> Inbound {
        let mut payload = json!({
            "event": event,
            "reply_action": "com.judd1.yandex_music.action.next",
            "reply_context": "BTN-1",
        });
        if let (Some(obj), Some(ex)) = (payload.as_object_mut(), extra.as_object()) {
            for (k, v) in ex {
                obj.insert(k.clone(), v.clone());
            }
        }
        Inbound::SendToPlugin(ActionEvt { context: None, action: None, device: None, payload })
    }

    #[tokio::test]
    async fn get_stats_replies_with_stats_data() {
        let (tx, mut rx, _s) = run_orch(Interests::all());
        tx.send(command("getStats", json!({ "period": "week" }))).await.unwrap();
        match next_out(&mut rx).await {
            Outbound::SendToPropertyInspector { payload, .. } => {
                assert_eq!(payload["event"], "StatsData");
                assert_eq!(payload["period"], "week");
                assert!(payload["byHour"].is_array());
                assert!(payload["actions"].is_object());
            }
            other => panic!("ожидался StatsData, {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_edition_replies_with_edition_info() {
        let (tx, mut rx, shared) = run_orch(Interests::all());
        shared.set_edition("elgato");
        tx.send(command("getEdition", json!({}))).await.unwrap();
        match next_out(&mut rx).await {
            Outbound::SendToPropertyInspector { payload, .. } => {
                assert_eq!(payload["event"], "EditionInfo");
                assert_eq!(payload["edition"], "elgato");
            }
            other => panic!("ожидался EditionInfo, {other:?}"),
        }
    }

    #[tokio::test]
    async fn check_connection_replies_local_status_with_port() {
        let (tx, mut rx, _s) = run_orch(Interests::all());
        tx.send(command("checkConnection", json!({}))).await.unwrap();
        match next_out(&mut rx).await {
            Outbound::SendToPropertyInspector { payload, .. } => {
                assert_eq!(payload["event"], "LocalStatus");
                assert_eq!(payload["status"], "disconnected");
                assert!(payload.as_object().unwrap().contains_key("port"));
            }
            other => panic!("ожидался LocalStatus, {other:?}"),
        }
    }

    #[tokio::test]
    async fn launch_client_kicks_and_replies_loading() {
        let (tx, mut rx, shared) = run_orch(Interests::all());
        let (ktx, mut krx) = mpsc::channel(4);
        shared.set_launch_kick(ktx);
        tx.send(command("launchClient", json!({}))).await.unwrap();
        match next_out(&mut rx).await {
            Outbound::SendToPropertyInspector { payload, .. } => {
                assert_eq!(payload["event"], "LocalStatus");
                assert_eq!(payload["status"], "loading");
            }
            other => panic!("ожидался LocalStatus loading, {other:?}"),
        }
        assert!(krx.try_recv().is_ok(), "launchClient должен пнуть лаунчер");
    }

    #[tokio::test]
    async fn key_down_records_action_counter() {
        let (tx, mut rx, shared) = run_orch(Interests::all());
        tx.send(appear("c1")).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 1);
        tx.send(Inbound::KeyDown(plain("c1"))).await.unwrap();
        assert_eq!(state_of(&next_out(&mut rx).await), 9);
        let stats = shared.stats_payload("week");
        assert_eq!(stats["actions"]["next"], 1);
    }

    #[tokio::test]
    async fn get_health_replies_with_health_data() {
        let (tx, mut rx, shared) = run_orch(Interests::all());
        shared.set_current_version("2.5.0".into());
        shared.set_edition("full");
        tx.send(command("getHealth", json!({}))).await.unwrap();
        match next_out(&mut rx).await {
            Outbound::SendToPropertyInspector { payload, .. } => {
                assert_eq!(payload["event"], "HealthData");
                assert_eq!(payload["version"], "2.5.0");
                assert_eq!(payload["edition"], "full");
                assert_eq!(payload["connected"], false);
                assert_eq!(payload["hasToken"], false);
                let checks = payload["checks"].as_array().expect("checks array");
                assert_eq!(checks.len(), 10);
                let by = |id: &str| checks.iter().find(|c| c["id"] == id).unwrap_or_else(|| panic!("нет проверки {id}"));
                assert_eq!(by("host")["ok"], true);
                assert_eq!(by("version")["ok"], true);
                assert_eq!(by("version")["detail"], "2.5.0");
                assert_eq!(by("client")["ok"], false);
                assert_eq!(by("port")["ok"], false);
            }
            other => panic!("ожидался HealthData, {other:?}"),
        }
    }
}
