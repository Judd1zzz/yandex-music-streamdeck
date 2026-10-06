use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LocalStatus {
    Connected,
    Disconnected,
    Loading,
}

pub fn local_status_payload(status: LocalStatus, reason: Option<&str>) -> Value {
    match reason {
        Some(r) => serde_json::json!({ "event": "LocalStatus", "status": status, "reason": r }),
        None => serde_json::json!({ "event": "LocalStatus", "status": status }),
    }
}

pub fn update_notice_payload(version: &str) -> Value {
    serde_json::json!({ "event": "UpdateNotice", "version": version })
}

pub fn discord_status_payload(status: &str) -> Value {
    serde_json::json!({ "event": "DiscordStatus", "status": status })
}

pub fn rival_notice_payload(level: &str) -> Value {
    serde_json::json!({ "event": "RivalNotice", "level": level })
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApplySettingsPayload {
    pub event: String,
    #[serde(default)]
    pub settings: Value,
}

impl ApplySettingsPayload {
    pub const EVENT: &'static str = "applySettingsToAll";
    pub fn matches(payload: &Value) -> Option<Self> {
        let parsed: ApplySettingsPayload = serde_json::from_value(payload.clone()).ok()?;
        (parsed.event == Self::EVENT).then_some(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn status_payloads_use_lowercase_strings() {
        assert_eq!(
            local_status_payload(LocalStatus::Connected, None),
            json!({"event":"LocalStatus","status":"connected"})
        );
        assert_eq!(
            local_status_payload(LocalStatus::Loading, None),
            json!({"event":"LocalStatus","status":"loading"})
        );
    }

    #[test]
    fn local_status_payload_carries_reason() {
        assert_eq!(
            local_status_payload(LocalStatus::Disconnected, Some("клиент запущен от администратора")),
            json!({"event":"LocalStatus","status":"disconnected","reason":"клиент запущен от администратора"})
        );
    }

    #[test]
    fn update_notice_payload_shape() {
        assert_eq!(update_notice_payload("2.1.3"), json!({"event":"UpdateNotice","version":"2.1.3"}));
    }

    #[test]
    fn discord_status_payload_shape() {
        assert_eq!(
            discord_status_payload("connected"),
            json!({"event":"DiscordStatus","status":"connected"})
        );
    }

    #[test]
    fn rival_notice_payload_shape() {
        assert_eq!(rival_notice_payload("installed"), json!({"event":"RivalNotice","level":"installed"}));
        assert_eq!(rival_notice_payload("active"), json!({"event":"RivalNotice","level":"active"}));
    }

    #[test]
    fn apply_settings_matches_only_correct_event() {
        let ok = json!({"event":"applySettingsToAll","settings":{"control_mode":"local"}});
        let parsed = ApplySettingsPayload::matches(&ok).unwrap();
        assert_eq!(parsed.settings["control_mode"], "local");
        assert!(ApplySettingsPayload::matches(&json!({"event":"somethingElse"})).is_none());
    }
}
