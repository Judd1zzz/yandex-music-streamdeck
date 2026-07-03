use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ControlMode {
    #[default]
    Local,
    Ynison,
}

fn d_display_mode() -> String {
    "cover_title_artists".to_owned()
}
fn d_style() -> String {
    "v1".to_owned()
}
fn d_progress_mode() -> String {
    "stacked".to_owned()
}
fn d_true() -> bool {
    true
}
fn d_download_format() -> String {
    "lossless".to_owned()
}

pub const KNOB_STEP_DEFAULT: u8 = 5;
pub const KNOB_STEP_MIN: u8 = 1;
pub const KNOB_STEP_MAX: u8 = 20;

fn d_knob_step() -> u8 {
    KNOB_STEP_DEFAULT
}
fn d_knob_press() -> String {
    "mute".to_owned()
}

fn de_knob_step<'de, D>(d: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let n = match serde_json::Value::deserialize(d)? {
        serde_json::Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f.round() as i64)),
        serde_json::Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    };
    Ok(n.map_or(KNOB_STEP_DEFAULT, |x| {
        x.clamp(i64::from(KNOB_STEP_MIN), i64::from(KNOB_STEP_MAX)) as u8
    }))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginSettings {
    #[serde(default)]
    pub control_mode: ControlMode,
    #[serde(default = "d_display_mode")]
    pub display_mode: String,
    #[serde(default = "d_style")]
    pub play_style: String,
    #[serde(default = "d_style")]
    pub prev_style: String,
    #[serde(default = "d_style")]
    pub next_style: String,
    #[serde(default = "d_style")]
    pub like_style: String,
    #[serde(default = "d_style")]
    pub dislike_style: String,
    #[serde(default = "d_style")]
    pub volume_style: String,
    #[serde(default = "d_style")]
    pub mute_style: String,
    #[serde(default = "d_style")]
    pub download_style: String,
    #[serde(default = "d_knob_step", deserialize_with = "de_knob_step")]
    pub knob_step: u8,
    #[serde(default = "d_knob_press")]
    pub knob_press: String,
    #[serde(default = "d_progress_mode")]
    pub progress_mode: String,
    #[serde(default = "d_true")]
    pub show_cover: bool,
    #[serde(default = "d_true")]
    pub show_title: bool,
    #[serde(default = "d_true")]
    pub show_artist: bool,
}

impl Default for PluginSettings {
    fn default() -> Self {
        serde_json::from_value(serde_json::json!({})).expect("дефолты PluginSettings")
    }
}

impl PluginSettings {
    pub fn from_value(v: &serde_json::Value) -> Self {
        serde_json::from_value(v.clone()).unwrap_or_default()
    }
    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("сериализация PluginSettings")
    }
}

fn de_opt_u16<'de, D>(d: D) -> Result<Option<u16>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::Number(n) => n.as_u64().and_then(|x| u16::try_from(x).ok()),
        serde_json::Value::String(s) => s.trim().parse::<u16>().ok(),
        _ => None,
    })
}

pub const DEFAULT_DISCORD_APP_ID: &str = "1521796597434941470";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscordConfig {
    pub enabled: bool,
    pub app_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchConfig {
    pub enabled: bool,
    pub client_exe_path: Option<String>,
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self { enabled: true, client_exe_path: None }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlobalSettings {
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default, deserialize_with = "de_opt_u16")]
    pub local_port: Option<u16>,
    #[serde(default = "d_true")]
    pub client_autofix_enabled: bool,
    #[serde(default)]
    pub client_exe_path: Option<String>,
    #[serde(default)]
    pub discord_rpc_enabled: bool,
    #[serde(default)]
    pub discord_app_id: Option<String>,
    #[serde(default)]
    pub download_path: String,
    #[serde(default = "d_download_format")]
    pub download_format: String,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        serde_json::from_value(serde_json::json!({})).expect("дефолты GlobalSettings")
    }
}

impl GlobalSettings {
    pub fn from_value(v: &serde_json::Value) -> Self {
        serde_json::from_value(v.clone()).unwrap_or_default()
    }
    pub fn discord(&self) -> DiscordConfig {
        let app_id = self
            .discord_app_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| DEFAULT_DISCORD_APP_ID.to_owned());
        DiscordConfig { enabled: self.discord_rpc_enabled, app_id: Some(app_id) }
    }
    pub fn launch(&self) -> LaunchConfig {
        let client_exe_path = self
            .client_exe_path
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
        LaunchConfig { enabled: self.client_autofix_enabled, client_exe_path }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_match_python_from_dict() {
        let s = PluginSettings::default();
        assert_eq!(s.control_mode, ControlMode::Local);
        assert_eq!(s.display_mode, "cover_title_artists");
        assert_eq!(s.play_style, "v1");
        assert_eq!(s.progress_mode, "stacked");
        assert!(s.show_cover && s.show_title && s.show_artist);
    }

    #[test]
    fn partial_settings_fill_defaults() {
        let s = PluginSettings::from_value(&json!({"control_mode": "ynison", "next_style": "v3"}));
        assert_eq!(s.control_mode, ControlMode::Ynison);
        assert_eq!(s.next_style, "v3");
        assert_eq!(s.play_style, "v1");
        assert!(s.show_cover);
    }

    #[test]
    fn control_mode_serializes_lowercase() {
        assert_eq!(json!(ControlMode::Local), json!("local"));
        assert_eq!(json!(ControlMode::Ynison), json!("ynison"));
    }

    #[test]
    fn unknown_keys_ignored_and_roundtrip() {
        let s = PluginSettings::from_value(&json!({"token": "should-be-ignored", "play_style": "v4"}));
        assert_eq!(s.play_style, "v4");
        let back = PluginSettings::from_value(&s.to_value());
        assert_eq!(s, back);
    }

    #[test]
    fn global_settings_parse() {
        let g = GlobalSettings::from_value(&json!({"token": "T", "local_port": 9333}));
        assert_eq!(g.token.as_deref(), Some("T"));
        assert_eq!(g.local_port, Some(9333));
        assert_eq!(GlobalSettings::from_value(&json!({})), GlobalSettings::default());
    }

    #[test]
    fn global_settings_string_local_port_keeps_token() {
        let g = GlobalSettings::from_value(&json!({"token": "T", "local_port": "9222"}));
        assert_eq!(g.token.as_deref(), Some("T"));
        assert_eq!(g.local_port, Some(9222));
        let g2 = GlobalSettings::from_value(&json!({"token": "T2", "local_port": "abc"}));
        assert_eq!(g2.token.as_deref(), Some("T2"));
        assert_eq!(g2.local_port, None);
    }

    #[test]
    fn global_download_settings_defaults_and_parse() {
        let def = GlobalSettings::default();
        assert_eq!(def.download_path, "");
        assert_eq!(def.download_format, "lossless");
        let g = GlobalSettings::from_value(&json!({"download_path": "/Music", "download_format": "mp3"}));
        assert_eq!(g.download_path, "/Music");
        assert_eq!(g.download_format, "mp3");
    }

    #[test]
    fn knob_defaults() {
        let s = PluginSettings::default();
        assert_eq!(s.knob_step, KNOB_STEP_DEFAULT);
        assert_eq!(s.knob_press, "mute");
    }

    #[test]
    fn knob_step_lenient_parse_preserves_other_fields() {
        let s = PluginSettings::from_value(&json!({"knob_step": "7", "control_mode": "ynison"}));
        assert_eq!(s.knob_step, 7);
        assert_eq!(s.control_mode, ControlMode::Ynison);

        let s = PluginSettings::from_value(&json!({"knob_step": "abc", "next_style": "v3"}));
        assert_eq!(s.knob_step, KNOB_STEP_DEFAULT);
        assert_eq!(s.next_style, "v3");

        let s = PluginSettings::from_value(&json!({"knob_step": 12}));
        assert_eq!(s.knob_step, 12);
    }

    #[test]
    fn knob_step_clamped_on_parse() {
        assert_eq!(PluginSettings::from_value(&json!({"knob_step": 100})).knob_step, KNOB_STEP_MAX);
        assert_eq!(PluginSettings::from_value(&json!({"knob_step": 0})).knob_step, KNOB_STEP_MIN);
        assert_eq!(PluginSettings::from_value(&json!({"knob_step": -3})).knob_step, KNOB_STEP_MIN);
    }

    #[test]
    fn knob_settings_roundtrip() {
        let s = PluginSettings::from_value(&json!({"knob_step": 10, "knob_press": "playpause"}));
        assert_eq!(s.knob_press, "playpause");
        let back = PluginSettings::from_value(&s.to_value());
        assert_eq!(s, back);
    }

    #[test]
    fn launch_config_defaults_and_parse() {
        let def = GlobalSettings::default();
        assert!(def.client_autofix_enabled);
        assert_eq!(def.client_exe_path, None);
        assert_eq!(def.launch(), LaunchConfig::default());
        let off = GlobalSettings::from_value(&json!({"client_autofix_enabled": false}));
        assert!(!off.launch().enabled);
        let with_path = GlobalSettings::from_value(
            &json!({"client_exe_path": "  C:\\Apps\\Яндекс Музыка.exe  "}),
        );
        assert_eq!(with_path.launch().client_exe_path.as_deref(), Some("C:\\Apps\\Яндекс Музыка.exe"));
        let blank = GlobalSettings::from_value(&json!({"client_exe_path": "   "}));
        assert_eq!(blank.launch().client_exe_path, None);
    }

    #[test]
    fn launch_config_roundtrip_via_global() {
        let g = GlobalSettings::from_value(&json!({"client_autofix_enabled": true, "token": "T"}));
        let v = serde_json::to_value(&g).unwrap();
        let back = GlobalSettings::from_value(&v);
        assert_eq!(g, back);
        assert!(back.client_autofix_enabled);
    }

    #[test]
    fn discord_config_from_global() {
        let g = GlobalSettings::from_value(&json!({"discord_rpc_enabled": true, "discord_app_id": "1124"}));
        assert_eq!(g.discord(), DiscordConfig { enabled: true, app_id: Some("1124".to_owned()) });
        let def = GlobalSettings::default().discord();
        assert!(!def.enabled);
        assert_eq!(def.app_id.as_deref(), Some(DEFAULT_DISCORD_APP_ID));
        let blank = GlobalSettings::from_value(&json!({"discord_rpc_enabled": true, "discord_app_id": "  "}));
        assert_eq!(
            blank.discord(),
            DiscordConfig { enabled: true, app_id: Some(DEFAULT_DISCORD_APP_ID.to_owned()) }
        );
    }
}
