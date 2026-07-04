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
                self.kick_if_local_disconnected(a.context.as_deref());
                self.forward_opt(a.context, ActorMsg::KeyDown).await
            }
            Inbound::KeyUp(a) => self.forward_opt(a.context, ActorMsg::KeyUp).await,
            Inbound::DialRotate(a) => {
                let ticks = a.ticks();
                self.forward_opt(a.context, ActorMsg::DialRotate(ticks)).await
            }
            Inbound::DialDown(a) => {
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

    fn kick_if_local_disconnected(&self, context: Option<&str>) {
        if let Some(ctx) = context
            && self.actors.get(ctx).is_some_and(|h| h.control_mode == ControlMode::Local)
            && !self.shared.cdp.is_connected()
        {
            self.shared.launch_kick();
        }
    }

    async fn answer_client_path_check(&self, a: &sd_protocol::ActionEvt) {
        let raw = a.payload.get("path").and_then(serde_json::Value::as_str).unwrap_or("");
        let Some(report) = self.shared.check_client_path(raw) else { return };
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
        let _ = self
            .host
            .send(Outbound::SendToPropertyInspector { action, context, payload: report.payload() })
            .await;
    }
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
}
