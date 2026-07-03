use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct ImagePayload {
    pub target: u8,
    pub image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatePayload {
    pub state: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct TitlePayload {
    pub title: String,
    pub target: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct UrlPayload {
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogPayload {
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum Outbound {
    SetImage { context: String, payload: ImagePayload },
    SetState { context: String, payload: StatePayload },
    SetTitle { context: String, payload: TitlePayload },
    SetSettings { context: String, payload: Value },
    SetGlobalSettings { context: String, payload: Value },
    GetGlobalSettings { context: String },
    SendToPropertyInspector { action: String, context: String, payload: Value },
    ShowOk { context: String },
    ShowAlert { context: String },
    OpenUrl { payload: UrlPayload },
    LogMessage { payload: LogPayload },
}

impl Outbound {
    pub fn set_image(context: impl Into<String>, image: impl Into<String>, state: Option<u8>) -> Self {
        Outbound::SetImage {
            context: context.into(),
            payload: ImagePayload { target: 0, image: image.into(), state },
        }
    }
    pub fn set_state(context: impl Into<String>, state: u8) -> Self {
        Outbound::SetState { context: context.into(), payload: StatePayload { state } }
    }
    pub fn set_title(context: impl Into<String>, title: impl Into<String>) -> Self {
        Outbound::SetTitle { context: context.into(), payload: TitlePayload { title: title.into(), target: 0 } }
    }
}

pub fn register_message(register_event: &str, plugin_uuid: &str) -> Value {
    serde_json::json!({ "event": register_event, "uuid": plugin_uuid })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ser(o: &Outbound) -> Value {
        serde_json::to_value(o).unwrap()
    }

    #[test]
    fn set_image_with_and_without_state() {
        assert_eq!(
            ser(&Outbound::set_image("c", "data:image/png;base64,AAA", Some(1))),
            json!({"event":"setImage","context":"c","payload":{"target":0,"image":"data:image/png;base64,AAA","state":1}})
        );
        assert_eq!(
            ser(&Outbound::set_image("c", "X", None)),
            json!({"event":"setImage","context":"c","payload":{"target":0,"image":"X"}})
        );
    }

    #[test]
    fn set_state_set_title() {
        assert_eq!(ser(&Outbound::set_state("c", 1)), json!({"event":"setState","context":"c","payload":{"state":1}}));
        assert_eq!(
            ser(&Outbound::set_title("c", "50%")),
            json!({"event":"setTitle","context":"c","payload":{"title":"50%","target":0}})
        );
    }

    #[test]
    fn settings_and_pi_and_misc() {
        assert_eq!(
            ser(&Outbound::SetSettings { context: "c".into(), payload: json!({"k":1}) }),
            json!({"event":"setSettings","context":"c","payload":{"k":1}})
        );
        assert_eq!(
            ser(&Outbound::SetGlobalSettings { context: "u".into(), payload: json!({"local_port":9333}) }),
            json!({"event":"setGlobalSettings","context":"u","payload":{"local_port":9333}})
        );
        assert_eq!(ser(&Outbound::GetGlobalSettings { context: "u".into() }), json!({"event":"getGlobalSettings","context":"u"}));
        assert_eq!(
            ser(&Outbound::SendToPropertyInspector { action: "a".into(), context: "c".into(), payload: json!({"event":"TokenStatus","status":"valid"}) }),
            json!({"event":"sendToPropertyInspector","action":"a","context":"c","payload":{"event":"TokenStatus","status":"valid"}})
        );
        assert_eq!(ser(&Outbound::ShowOk { context: "c".into() }), json!({"event":"showOk","context":"c"}));
        assert_eq!(ser(&Outbound::ShowAlert { context: "c".into() }), json!({"event":"showAlert","context":"c"}));
        assert_eq!(ser(&Outbound::OpenUrl { payload: UrlPayload { url: "https://x".into() } }), json!({"event":"openUrl","payload":{"url":"https://x"}}));
        assert_eq!(ser(&Outbound::LogMessage { payload: LogPayload { message: "m".into() } }), json!({"event":"logMessage","payload":{"message":"m"}}));
    }

    #[test]
    fn register_message_uses_dynamic_event() {
        assert_eq!(register_message("registerPlugin", "UUID-1"), json!({"event":"registerPlugin","uuid":"UUID-1"}));
    }
}
