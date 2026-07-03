use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ActionEvt {
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub device: Option<String>,
    #[serde(default)]
    pub payload: Value,
}

impl ActionEvt {
    pub fn settings(&self) -> Value {
        self.payload.get("settings").cloned().unwrap_or(Value::Null)
    }
    pub fn ticks(&self) -> i32 {
        self.payload
            .get("ticks")
            .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f.round() as i64)))
            .map(|t| t.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
            .unwrap_or(0)
    }
    pub fn dial_pressed(&self) -> bool {
        self.payload.get("pressed").and_then(Value::as_bool).unwrap_or(false)
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GlobalEvt {
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub payload: Value,
}

impl GlobalEvt {
    pub fn settings(&self) -> Value {
        self.payload.get("settings").cloned().unwrap_or(Value::Null)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "event")]
pub enum Inbound {
    #[serde(rename = "keyDown")]
    KeyDown(ActionEvt),
    #[serde(rename = "keyUp")]
    KeyUp(ActionEvt),
    #[serde(rename = "dialRotate")]
    DialRotate(ActionEvt),
    #[serde(rename = "dialDown")]
    DialDown(ActionEvt),
    #[serde(rename = "dialUp")]
    DialUp(ActionEvt),
    #[serde(rename = "willAppear")]
    WillAppear(ActionEvt),
    #[serde(rename = "willDisappear")]
    WillDisappear(ActionEvt),
    #[serde(rename = "didReceiveSettings")]
    DidReceiveSettings(ActionEvt),
    #[serde(rename = "titleParametersDidChange")]
    TitleParametersDidChange(ActionEvt),
    #[serde(rename = "propertyInspectorDidAppear")]
    PropertyInspectorDidAppear(ActionEvt),
    #[serde(rename = "sendToPlugin")]
    SendToPlugin(ActionEvt),
    #[serde(rename = "didReceiveGlobalSettings")]
    DidReceiveGlobalSettings(GlobalEvt),
    #[serde(rename = "systemDidWakeUp")]
    SystemDidWakeUp,
    #[serde(rename = "applicationDidLaunch")]
    ApplicationDidLaunch,
    #[serde(rename = "applicationDidTerminate")]
    ApplicationDidTerminate,
    #[serde(other)]
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(v: serde_json::Value) -> Inbound {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn key_down_parsed_with_context_and_action() {
        let ev = parse(json!({
            "event": "keyDown",
            "action": "com.judd1.yandex_music.action.next",
            "context": "ctx-1",
            "device": "dev-1",
            "payload": {"settings": {"next_style": "v2"}}
        }));
        match ev {
            Inbound::KeyDown(a) => {
                assert_eq!(a.context.as_deref(), Some("ctx-1"));
                assert_eq!(a.action.as_deref(), Some("com.judd1.yandex_music.action.next"));
                assert_eq!(a.settings()["next_style"], "v2");
            }
            other => panic!("ожидался KeyDown, получено {other:?}"),
        }
    }

    #[test]
    fn will_appear_and_will_disappear() {
        assert!(matches!(parse(json!({"event":"willAppear","context":"c"})), Inbound::WillAppear(_)));
        assert!(matches!(parse(json!({"event":"willDisappear","context":"c"})), Inbound::WillDisappear(_)));
    }

    #[test]
    fn global_settings_carries_token() {
        let ev = parse(json!({"event":"didReceiveGlobalSettings","payload":{"settings":{"token":"T","local_port":9222}}}));
        match ev {
            Inbound::DidReceiveGlobalSettings(g) => {
                assert_eq!(g.settings()["token"], "T");
                assert_eq!(g.settings()["local_port"], 9222);
            }
            other => panic!("ожидался DidReceiveGlobalSettings, {other:?}"),
        }
    }

    #[test]
    fn unit_events_parse_and_ignore_extra_fields() {
        assert!(matches!(parse(json!({"event":"systemDidWakeUp","device":"d"})), Inbound::SystemDidWakeUp));
        assert!(matches!(parse(json!({"event":"applicationDidLaunch","payload":{"application":{"font":"x"}}})), Inbound::ApplicationDidLaunch));
        assert!(matches!(parse(json!({"event":"applicationDidTerminate"})), Inbound::ApplicationDidTerminate));
    }

    #[test]
    fn unknown_event_maps_to_unknown() {
        assert!(matches!(parse(json!({"event":"deviceDidConnect","device":"d"})), Inbound::Unknown));
        assert!(matches!(parse(json!({"event":"touchTap","context":"c"})), Inbound::Unknown));
    }

    #[test]
    fn dial_rotate_parsed_with_ticks_context_settings() {
        let ev = parse(json!({
            "event": "dialRotate",
            "action": "com.judd1.yandex_music.action.volume_knob",
            "context": "ctx-d",
            "device": "dev-1",
            "payload": {
                "controller": "Knob",
                "settings": {"knob_step": 10},
                "coordinates": {"column": 3, "row": 0},
                "ticks": -3,
                "pressed": false
            }
        }));
        match ev {
            Inbound::DialRotate(a) => {
                assert_eq!(a.ticks(), -3);
                assert!(!a.dial_pressed());
                assert_eq!(a.context.as_deref(), Some("ctx-d"));
                assert_eq!(a.settings()["knob_step"], 10);
            }
            other => panic!("ожидался DialRotate, {other:?}"),
        }
    }

    #[test]
    fn dial_rotate_ticks_edge_cases() {
        let evt = |payload: serde_json::Value| ActionEvt { payload, ..Default::default() };
        assert_eq!(evt(json!({"ticks": 5})).ticks(), 5);
        assert_eq!(evt(json!({})).ticks(), 0);
        assert_eq!(evt(json!({"ticks": 2.0})).ticks(), 2);
        assert_eq!(evt(json!({"ticks": "x"})).ticks(), 0);
        assert!(evt(json!({"pressed": true})).dial_pressed());
        assert!(!evt(json!({})).dial_pressed());
    }

    #[test]
    fn dial_down_and_dial_up_parse() {
        let down = parse(json!({
            "event": "dialDown",
            "action": "a",
            "context": "c",
            "payload": {"controller": "Knob", "settings": {}, "coordinates": {"column": 0, "row": 0}}
        }));
        assert!(matches!(down, Inbound::DialDown(a) if a.context.as_deref() == Some("c")));
        let up = parse(json!({"event": "dialUp", "context": "c", "payload": {}}));
        assert!(matches!(up, Inbound::DialUp(_)));
    }

    #[test]
    fn send_to_plugin_is_an_action_evt_with_nested_event() {
        let ev = parse(json!({"event":"sendToPlugin","context":"c","action":"a","payload":{"event":"applySettingsToAll","settings":{"control_mode":"local"}}}));
        match ev {
            Inbound::SendToPlugin(a) => assert_eq!(a.payload["event"], "applySettingsToAll"),
            other => panic!("ожидался SendToPlugin, {other:?}"),
        }
    }
}
