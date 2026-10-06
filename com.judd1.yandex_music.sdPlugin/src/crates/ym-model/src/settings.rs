use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ControlMode {
    #[default]
    #[serde(alias = "ynison")]
    Local,
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
fn d_info_press() -> String {
    "playpause".to_owned()
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
    #[serde(default = "d_style")]
    pub shuffle_style: String,
    #[serde(default = "d_style")]
    pub repeat_style: String,
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
    #[serde(default = "d_info_press")]
    pub info_press: String,
    #[serde(default)]
    pub show_timecodes: bool,
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
        Self {
            enabled: true,
            client_exe_path: None,
        }
    }
}

pub const CSS_FILTER_MAX: usize = 200;
const CSS_FILTER_FNS: [&str; 9] = [
    "blur",
    "brightness",
    "contrast",
    "grayscale",
    "hue-rotate",
    "invert",
    "opacity",
    "saturate",
    "sepia",
];

pub fn sanitize_css_filter(s: &str) -> String {
    let t = s.trim();
    if t.len() > CSS_FILTER_MAX || !css_filter_valid(t) {
        return String::new();
    }
    t.to_owned()
}

fn css_filter_valid(t: &str) -> bool {
    let b = t.as_bytes();
    let skip_ws = |mut i: usize| {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        i
    };
    let mut i = 0;
    loop {
        i = skip_ws(i);
        if i == b.len() {
            return true;
        }
        let start = i;
        while i < b.len() && (b[i].is_ascii_alphabetic() || b[i] == b'-') {
            i += 1;
        }
        let name = t[start..i].to_ascii_lowercase();
        if !CSS_FILTER_FNS.contains(&name.as_str()) || b.get(i) != Some(&b'(') {
            return false;
        }
        i = skip_ws(i + 1);
        let neg = b.get(i) == Some(&b'-');
        if neg {
            i += 1;
        }
        let digits = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == digits {
            return false;
        }
        if b.get(i) == Some(&b'.') {
            i += 1;
            let frac = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i == frac {
                return false;
            }
        }
        let zero = t[digits..i].bytes().all(|c| c == b'0' || c == b'.');
        let unit = i;
        while i < b.len() && (b[i].is_ascii_alphabetic() || b[i] == b'%') {
            i += 1;
        }
        let unit = t[unit..i].to_ascii_lowercase();
        let fits = match name.as_str() {
            "blur" => !neg && (unit == "px" || (unit.is_empty() && zero)),
            "hue-rotate" => {
                ["deg", "rad", "turn"].contains(&unit.as_str()) || (unit.is_empty() && zero && !neg)
            }
            _ => !neg && (unit.is_empty() || unit == "%"),
        };
        if !fits {
            return false;
        }
        i = skip_ws(i);
        if b.get(i) != Some(&b')') {
            return false;
        }
        i += 1;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"), deny_unknown_fields)]
pub struct VideoCoverConfig {
    #[serde(rename(deserialize = "video_cover_enabled"))]
    pub enabled: bool,
    #[serde(rename(deserialize = "video_cover_hide_poster"))]
    pub hide_poster: bool,
    #[serde(rename(deserialize = "video_cover_brightness"))]
    pub brightness: u8,
    #[serde(rename(deserialize = "video_cover_panel_brightness"))]
    pub panel_brightness: u8,
    #[serde(rename(deserialize = "video_cover_filter"))]
    pub filter: String,
    #[serde(rename(deserialize = "video_cover_plate_enabled"))]
    pub plate_enabled: bool,
    #[serde(rename(deserialize = "video_cover_plate_color"))]
    pub plate_color: String,
    #[serde(rename(deserialize = "video_cover_plate_opacity"))]
    pub plate_opacity: u8,
    #[serde(rename(deserialize = "video_cover_plate_backdrop"))]
    pub plate_backdrop: String,
}

pub const VIDEO_COVER_DEFAULTS_JSON: &str = include_str!("../fixtures/video_cover_defaults.json");

static VIDEO_COVER_DEFAULTS: std::sync::LazyLock<VideoCoverConfig> =
    std::sync::LazyLock::new(|| {
        serde_json::from_str(VIDEO_COVER_DEFAULTS_JSON)
            .expect("fixtures/video_cover_defaults.json не разобран")
    });

impl Default for VideoCoverConfig {
    fn default() -> Self {
        VIDEO_COVER_DEFAULTS.clone()
    }
}

pub const BETTER_INFO_DEFAULTS_JSON: &str = include_str!("../fixtures/better_info_defaults.json");
const BETTER_INFO_PREFIX: &str = "better_info_";

static BETTER_INFO_DEFAULTS: std::sync::LazyLock<std::collections::BTreeMap<String, bool>> =
    std::sync::LazyLock::new(|| {
        serde_json::from_str(BETTER_INFO_DEFAULTS_JSON)
            .expect("fixtures/better_info_defaults.json не разобран")
    });

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct BetterInfoConfig(pub std::collections::BTreeMap<String, bool>);

impl Default for BetterInfoConfig {
    fn default() -> Self {
        better_info(&serde_json::Value::Null)
    }
}

pub fn better_info(global: &serde_json::Value) -> BetterInfoConfig {
    BetterInfoConfig(
        BETTER_INFO_DEFAULTS
            .iter()
            .map(|(key, d)| {
                let on = global
                    .get(key)
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(*d);
                (
                    key.strip_prefix(BETTER_INFO_PREFIX)
                        .unwrap_or(key)
                        .to_owned(),
                    on,
                )
            })
            .collect(),
    )
}

fn lenient_flag(v: &Option<serde_json::Value>, d: bool) -> bool {
    v.as_ref().and_then(serde_json::Value::as_bool).unwrap_or(d)
}

fn lenient_percent(v: &Option<serde_json::Value>, d: u8) -> u8 {
    v.as_ref()
        .and_then(|v| {
            v.as_f64()
                .or_else(|| v.as_str()?.trim().parse::<f64>().ok())
        })
        .filter(|x| x.is_finite())
        .map(|x| x.clamp(0.0, 100.0) as u8)
        .unwrap_or(d)
}

fn lenient_filter(v: &Option<serde_json::Value>, d: &str) -> String {
    v.as_ref()
        .and_then(serde_json::Value::as_str)
        .map_or_else(|| d.to_owned(), sanitize_css_filter)
}

fn hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|c| c.is_ascii_hexdigit())
}

fn lenient_color(v: &Option<serde_json::Value>, d: &str) -> String {
    match v.as_ref().and_then(serde_json::Value::as_str) {
        Some("accent") => "accent".to_owned(),
        Some(s) if hex_color(s) => s.to_ascii_lowercase(),
        _ => d.to_owned(),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlobalSettings {
    #[serde(default = "d_true")]
    pub client_autofix_enabled: bool,
    #[serde(default = "d_true")]
    pub launch_from_any_button: bool,
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
    #[serde(default = "d_true")]
    pub debug_mode: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_cover_enabled: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_cover_hide_poster: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_cover_brightness: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_cover_panel_brightness: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_cover_filter: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_cover_plate_enabled: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_cover_plate_color: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_cover_plate_opacity: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_cover_plate_backdrop: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ynison_device_enabled: Option<serde_json::Value>,
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
        DiscordConfig {
            enabled: self.discord_rpc_enabled,
            app_id: Some(app_id),
        }
    }
    pub fn launch(&self) -> LaunchConfig {
        let client_exe_path = self
            .client_exe_path
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
        LaunchConfig {
            enabled: self.client_autofix_enabled,
            client_exe_path,
        }
    }
    pub fn ynison_device(&self) -> bool {
        lenient_flag(&self.ynison_device_enabled, false)
    }
    pub fn video_cover(&self) -> VideoCoverConfig {
        self.video_cover_over(VideoCoverConfig::default())
    }
    fn video_cover_over(&self, d: VideoCoverConfig) -> VideoCoverConfig {
        VideoCoverConfig {
            enabled: lenient_flag(&self.video_cover_enabled, d.enabled),
            hide_poster: lenient_flag(&self.video_cover_hide_poster, d.hide_poster),
            brightness: lenient_percent(&self.video_cover_brightness, d.brightness),
            panel_brightness: lenient_percent(
                &self.video_cover_panel_brightness,
                d.panel_brightness,
            ),
            filter: lenient_filter(&self.video_cover_filter, &d.filter),
            plate_enabled: lenient_flag(&self.video_cover_plate_enabled, d.plate_enabled),
            plate_color: lenient_color(&self.video_cover_plate_color, &d.plate_color),
            plate_opacity: lenient_percent(&self.video_cover_plate_opacity, d.plate_opacity),
            plate_backdrop: lenient_filter(&self.video_cover_plate_backdrop, &d.plate_backdrop),
        }
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
    fn shuffle_repeat_styles_default_and_roundtrip() {
        let s = PluginSettings::default();
        assert_eq!(s.shuffle_style, "v1");
        assert_eq!(s.repeat_style, "v1");
        let parsed =
            PluginSettings::from_value(&json!({"shuffle_style": "v2", "repeat_style": "v2"}));
        assert_eq!(parsed.shuffle_style, "v2");
        assert_eq!(parsed.repeat_style, "v2");
        let back = PluginSettings::from_value(&parsed.to_value());
        assert_eq!(back, parsed);
    }

    #[test]
    fn partial_settings_fill_defaults() {
        let s = PluginSettings::from_value(&json!({"control_mode": "ynison", "next_style": "v3"}));
        assert_eq!(
            s.control_mode,
            ControlMode::Local,
            "сохранённый ynison читается как local"
        );
        assert_eq!(s.next_style, "v3");
        assert_eq!(s.play_style, "v1");
        assert!(s.show_cover);
    }

    #[test]
    fn control_mode_serializes_lowercase() {
        assert_eq!(json!(ControlMode::Local), json!("local"));
    }

    #[test]
    fn unknown_keys_ignored_and_roundtrip() {
        let s =
            PluginSettings::from_value(&json!({"token": "should-be-ignored", "play_style": "v4"}));
        assert_eq!(s.play_style, "v4");
        let back = PluginSettings::from_value(&s.to_value());
        assert_eq!(s, back);
    }

    #[test]
    fn global_settings_parse() {
        let g = GlobalSettings::from_value(&json!({"token": "T", "download_path": "D"}));
        assert_eq!(
            g.download_path, "D",
            "старый ключ token не сбрасывает остальные глобальные настройки"
        );
        assert_eq!(
            GlobalSettings::from_value(&json!({})),
            GlobalSettings::default()
        );
    }

    #[test]
    fn ynison_device_is_off_by_default_and_a_bad_value_breaks_nothing_else() {
        assert!(!GlobalSettings::default().ynison_device());
        assert!(
            GlobalSettings::from_value(&json!({"ynison_device_enabled": true})).ynison_device()
        );
        let g = GlobalSettings::from_value(
            &json!({"ynison_device_enabled": "yes", "download_path": "D"}),
        );
        assert!(!g.ynison_device(), "не-булево значение — выключено");
        assert_eq!(
            g.download_path, "D",
            "кривое значение не обнуляет остальные глобальные настройки"
        );
    }

    #[test]
    fn stale_port_setting_is_ignored_without_breaking_the_rest() {
        let g = GlobalSettings::from_value(&json!({"download_path": "D", "local_port": 9333}));
        assert_eq!(g.download_path, "D");
    }

    #[test]
    fn global_download_settings_defaults_and_parse() {
        let def = GlobalSettings::default();
        assert_eq!(def.download_path, "");
        assert_eq!(def.download_format, "lossless");
        assert!(def.debug_mode);
        let g = GlobalSettings::from_value(
            &json!({"download_path": "/Music", "download_format": "mp3", "debug_mode": false}),
        );
        assert_eq!(g.download_path, "/Music");
        assert_eq!(g.download_format, "mp3");
        assert!(!g.debug_mode);
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
        assert_eq!(s.control_mode, ControlMode::Local);

        let s = PluginSettings::from_value(&json!({"knob_step": "abc", "next_style": "v3"}));
        assert_eq!(s.knob_step, KNOB_STEP_DEFAULT);
        assert_eq!(s.next_style, "v3");

        let s = PluginSettings::from_value(&json!({"knob_step": 12}));
        assert_eq!(s.knob_step, 12);
    }

    #[test]
    fn knob_step_clamped_on_parse() {
        assert_eq!(
            PluginSettings::from_value(&json!({"knob_step": 100})).knob_step,
            KNOB_STEP_MAX
        );
        assert_eq!(
            PluginSettings::from_value(&json!({"knob_step": 0})).knob_step,
            KNOB_STEP_MIN
        );
        assert_eq!(
            PluginSettings::from_value(&json!({"knob_step": -3})).knob_step,
            KNOB_STEP_MIN
        );
    }

    #[test]
    fn knob_settings_roundtrip() {
        let s = PluginSettings::from_value(&json!({"knob_step": 10, "knob_press": "playpause"}));
        assert_eq!(s.knob_press, "playpause");
        let back = PluginSettings::from_value(&s.to_value());
        assert_eq!(s, back);
    }

    #[test]
    fn info_press_defaults_and_roundtrip() {
        let s = PluginSettings::default();
        assert_eq!(s.info_press, "playpause");
        let copy = PluginSettings::from_value(&json!({"info_press": "copy"}));
        assert_eq!(copy.info_press, "copy");
        let back = PluginSettings::from_value(&copy.to_value());
        assert_eq!(copy, back);
    }

    #[test]
    fn show_timecodes_default_off_and_roundtrip() {
        let s = PluginSettings::default();
        assert!(!s.show_timecodes);
        let on = PluginSettings::from_value(&json!({"show_timecodes": true}));
        assert!(on.show_timecodes);
        let back = PluginSettings::from_value(&on.to_value());
        assert_eq!(on, back);
    }

    #[test]
    fn launch_from_any_button_default_true_and_parse() {
        let def = GlobalSettings::default();
        assert!(def.launch_from_any_button);
        let off = GlobalSettings::from_value(
            &json!({"launch_from_any_button": false, "download_path": "D"}),
        );
        assert!(!off.launch_from_any_button);
        assert_eq!(off.download_path, "D");
        let v = serde_json::to_value(&off).unwrap();
        let back = GlobalSettings::from_value(&v);
        assert_eq!(off, back);
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
        assert_eq!(
            with_path.launch().client_exe_path.as_deref(),
            Some("C:\\Apps\\Яндекс Музыка.exe")
        );
        let blank = GlobalSettings::from_value(&json!({"client_exe_path": "   "}));
        assert_eq!(blank.launch().client_exe_path, None);
    }

    #[test]
    fn launch_config_roundtrip_via_global() {
        let g = GlobalSettings::from_value(
            &json!({"client_autofix_enabled": true, "download_path": "D"}),
        );
        let v = serde_json::to_value(&g).unwrap();
        let back = GlobalSettings::from_value(&v);
        assert_eq!(g, back);
        assert!(back.client_autofix_enabled);
    }

    #[test]
    fn discord_config_from_global() {
        let g = GlobalSettings::from_value(
            &json!({"discord_rpc_enabled": true, "discord_app_id": "1124"}),
        );
        assert_eq!(
            g.discord(),
            DiscordConfig {
                enabled: true,
                app_id: Some("1124".to_owned())
            }
        );
        let def = GlobalSettings::default().discord();
        assert!(!def.enabled);
        assert_eq!(def.app_id.as_deref(), Some(DEFAULT_DISCORD_APP_ID));
        let blank = GlobalSettings::from_value(
            &json!({"discord_rpc_enabled": true, "discord_app_id": "  "}),
        );
        assert_eq!(
            blank.discord(),
            DiscordConfig {
                enabled: true,
                app_id: Some(DEFAULT_DISCORD_APP_ID.to_owned())
            }
        );
    }

    #[test]
    fn css_filter_rule_matches_shared_cases() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/css_filter_cases.json")).unwrap();
        assert_eq!(cases["maxLength"], CSS_FILTER_MAX);
        for pair in cases["accept"].as_array().unwrap() {
            let (input, want) = (pair[0].as_str().unwrap(), pair[1].as_str().unwrap());
            assert_eq!(sanitize_css_filter(input), want, "должно пройти: {input:?}");
        }
        for input in cases["reject"].as_array().unwrap() {
            let input = input.as_str().unwrap();
            assert_eq!(
                sanitize_css_filter(input),
                "",
                "должно отвергаться: {input:?}"
            );
        }
    }

    #[test]
    fn video_cover_defaults_follow_owner_decision() {
        let c = GlobalSettings::default().video_cover();
        assert!(c.enabled, "включено по умолчанию");
        assert!(c.hide_poster, "обложка скрыта по умолчанию");
        assert_eq!(
            (c.brightness, c.panel_brightness, c.plate_opacity),
            (30, 50, 25),
            "яркость 30"
        );
        assert_eq!(c.plate_color, "accent");
        assert!(!c.plate_enabled && c.filter.is_empty() && c.plate_backdrop.is_empty());
    }

    #[test]
    fn video_cover_defaults_file_is_the_single_source_and_passes_validation() {
        let file: serde_json::Value = serde_json::from_str(VIDEO_COVER_DEFAULTS_JSON).unwrap();
        assert_eq!(
            file.as_object().unwrap().len(),
            9,
            "в файле умолчаний ровно 9 ключей video_cover_*"
        );
        assert_eq!(
            GlobalSettings::from_value(&file).video_cover(),
            VideoCoverConfig::default(),
            "умолчания из файла проходят те же проверки, что и настройки пользователя"
        );
        assert_eq!(
            GlobalSettings::default().video_cover(),
            VideoCoverConfig::default()
        );
        let d = VideoCoverConfig::default();
        assert!(
            d.brightness <= 100 && d.panel_brightness <= 100 && d.plate_opacity <= 100,
            "проценты умолчаний в 0..=100"
        );
        assert_eq!(
            sanitize_css_filter(&d.filter),
            d.filter,
            "фильтр умолчания проходит правило"
        );
        assert_eq!(
            sanitize_css_filter(&d.plate_backdrop),
            d.plate_backdrop,
            "backdrop умолчания проходит правило"
        );
        assert!(
            d.plate_color == "accent"
                || (hex_color(&d.plate_color)
                    && d.plate_color == d.plate_color.to_ascii_lowercase()),
            "цвет умолчания — accent или #rrggbb в нижнем регистре"
        );
    }

    #[test]
    fn better_info_flags_come_from_the_defaults_file_and_follow_owner_decision() {
        let file: serde_json::Value = serde_json::from_str(BETTER_INFO_DEFAULTS_JSON).unwrap();
        let file = file.as_object().unwrap();
        assert!(
            file.keys().all(|k| k.starts_with(BETTER_INFO_PREFIX)),
            "все ключи файла — better_info_*"
        );
        assert!(
            file.values().all(serde_json::Value::is_boolean),
            "все умолчания — флаги"
        );
        let d = BetterInfoConfig::default().0;
        assert_eq!(d.len(), file.len());
        for off in ["playlist_date", "artist_ai"] {
            assert_eq!(d.get(off), Some(&false), "{off} выключен по умолчанию");
        }
        assert_eq!(
            d.values().filter(|on| !**on).count(),
            2,
            "остальное включено, как у референса"
        );
        assert_eq!(
            d.get("requests"),
            Some(&true),
            "В1: дополнительные запросы включены"
        );
    }

    #[test]
    fn better_info_reads_flags_leniently_and_ignores_foreign_keys() {
        let c = better_info(&json!({
            "better_info_artist_ai": true,
            "better_info_requests": "no",
            "better_info_playlist_index": false,
            "better_info_unknown": true,
            "token": "x",
        }))
        .0;
        let d = BetterInfoConfig::default().0;
        assert_eq!(c.get("artist_ai"), Some(&true));
        assert_eq!(c.get("playlist_index"), Some(&false));
        assert_eq!(
            c.get("requests"),
            d.get("requests"),
            "не bool — умолчание из файла"
        );
        assert!(
            !c.contains_key("unknown") && !c.contains_key("token"),
            "в пульт уходят только ключи из файла"
        );
        assert_eq!(c.len(), d.len());
        assert_eq!(
            serde_json::to_value(BetterInfoConfig::default())
                .unwrap()
                .as_object()
                .unwrap()
                .len(),
            d.len(),
            "в пульт уходит плоский объект флагов"
        );
    }

    #[test]
    fn color_and_filters_fall_back_to_file_defaults_but_keep_explicit_choice() {
        let black = "#000000";
        assert_eq!(
            lenient_color(&Some(json!("accent")), black),
            "accent",
            "явный выбор акцента не зависит от умолчания"
        );
        assert_eq!(lenient_color(&Some(json!("#ABCDEF")), black), "#abcdef");
        assert_eq!(lenient_color(&Some(json!("red")), black), black);
        assert_eq!(lenient_color(&Some(json!(5)), black), black);
        assert_eq!(lenient_color(&None, black), black);
        let soft = "blur(1px)";
        assert_eq!(
            lenient_filter(&None, soft),
            soft,
            "нет значения — умолчание из файла"
        );
        assert_eq!(
            lenient_filter(&Some(json!(5)), soft),
            soft,
            "не строка — умолчание из файла"
        );
        assert_eq!(
            lenient_filter(&Some(json!("url(x)")), soft),
            "",
            "неверная строка — без фильтра"
        );
        assert_eq!(lenient_filter(&Some(json!("blur(2px)")), soft), "blur(2px)");
        let d = VideoCoverConfig {
            enabled: false,
            hide_poster: false,
            brightness: 7,
            panel_brightness: 8,
            filter: "blur(1px)".to_owned(),
            plate_enabled: true,
            plate_color: black.to_owned(),
            plate_opacity: 9,
            plate_backdrop: "blur(2px)".to_owned(),
        };
        assert_eq!(
            GlobalSettings::default().video_cover_over(d.clone()),
            d,
            "каждый незаданный ключ берёт своё умолчание, а не жёсткий литерал"
        );
    }

    #[test]
    fn bad_video_cover_values_fall_back_without_breaking_other_settings() {
        let g = GlobalSettings::from_value(&json!({
            "download_path": "keep-me",
            "video_cover_enabled": "yes",
            "video_cover_brightness": {"x": 1},
            "video_cover_panel_brightness": "70",
            "video_cover_plate_opacity": 250,
            "video_cover_plate_color": "#ABCDEF",
            "video_cover_filter": "blur(4px) url(x)",
            "video_cover_plate_backdrop": "blur(8px)",
        }));
        assert_eq!(
            g.download_path, "keep-me",
            "кривое значение видеообложки не сбрасывает остальные настройки"
        );
        let c = g.video_cover();
        let d = VideoCoverConfig::default();
        assert_eq!(c.enabled, d.enabled, "строка вместо bool — умолчание");
        assert_eq!(
            c.brightness, d.brightness,
            "объект вместо числа — умолчание"
        );
        assert_eq!(c.panel_brightness, 70, "число строкой принимается");
        assert_eq!(c.plate_opacity, 100, "выход за диапазон обрезается");
        assert_eq!(c.plate_color, "#abcdef");
        assert_eq!(c.filter, "", "фильтр с url() отбрасывается целиком");
        assert_eq!(c.plate_backdrop, "blur(8px)");
        for bad in ["red", "#abcdeg", "#abcdef0", "1abcdef", "#abc", " #abcdef"] {
            let c =
                GlobalSettings::from_value(&json!({"video_cover_plate_color": bad})).video_cover();
            assert_eq!(
                c.plate_color, d.plate_color,
                "цвет {bad:?} не #rrggbb и не accent — умолчание"
            );
        }
        let c = GlobalSettings::from_value(
            &json!({"video_cover_brightness": "NaN", "video_cover_plate_opacity": "inf"}),
        )
        .video_cover();
        assert_eq!(
            (c.brightness, c.plate_opacity),
            (d.brightness, d.plate_opacity),
            "NaN и бесконечность — умолчание, а не 0 или 100"
        );
    }

    #[test]
    fn video_cover_config_is_sent_to_pult_in_camel_case() {
        let v = serde_json::to_value(VideoCoverConfig::default()).unwrap();
        for k in [
            "enabled",
            "hidePoster",
            "brightness",
            "panelBrightness",
            "filter",
            "plateEnabled",
            "plateColor",
            "plateOpacity",
            "plateBackdrop",
        ] {
            assert!(v.get(k).is_some(), "нет ключа {k}");
        }
        let g = serde_json::to_value(GlobalSettings::default()).unwrap();
        assert!(
            g.as_object()
                .unwrap()
                .keys()
                .all(|k| !k.starts_with("video_cover_")),
            "незаданные ключи видеообложки не пишутся обратно как null"
        );
    }
}
