use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct ActionResult {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub is_playing: Option<bool>,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default)]
    pub is_muted: Option<bool>,
    #[serde(default)]
    pub new_state: Option<bool>,
    #[serde(default)]
    pub is_disliked: Option<bool>,
}

impl ActionResult {
    pub fn from_value(v: &Value) -> Self {
        let mut r: ActionResult = serde_json::from_value(v.clone()).unwrap_or_default();
        if r.new_state.is_none() {
            r.new_state = r.is_disliked;
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn play_pause_result() {
        let r = ActionResult::from_value(&json!({"success": true, "is_playing": false}));
        assert!(r.success);
        assert_eq!(r.is_playing, Some(false));
    }

    #[test]
    fn toggle_like_uses_new_state() {
        let r = ActionResult::from_value(&json!({"success": true, "new_state": true}));
        assert_eq!(r.new_state, Some(true));
    }

    #[test]
    fn toggle_dislike_folds_is_disliked_into_new_state() {
        let r = ActionResult::from_value(&json!({"success": true, "is_disliked": true}));
        assert_eq!(r.new_state, Some(true));
        assert_eq!(r.is_disliked, Some(true));
    }

    #[test]
    fn mute_result_has_no_volume_or_muted() {
        let r = ActionResult::from_value(&json!({"success": true}));
        assert!(r.success);
        assert_eq!(r.volume, None);
        assert_eq!(r.is_muted, None);
        assert_eq!(r.new_state, None);
    }

    #[test]
    fn change_volume_result() {
        let r = ActionResult::from_value(&json!({"success": true, "volume": 55.0}));
        assert_eq!(r.volume, Some(55.0));
    }

    #[test]
    fn failure_result() {
        let r = ActionResult::from_value(&json!({"success": false, "error": "Injection failed"}));
        assert!(!r.success);
        assert_eq!(r.error.as_deref(), Some("Injection failed"));
    }
}
