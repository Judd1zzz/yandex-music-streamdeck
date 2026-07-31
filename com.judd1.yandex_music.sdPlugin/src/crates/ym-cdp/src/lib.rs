pub mod commands;
pub mod controller;
pub mod discovery;

pub use commands::LocalCommand;
pub use controller::{CdpController, CdpError};

#[cfg(feature = "download-button")]
pub const INJECTED_API_JS: &str = include_str!("../assets/injected_api.js");

#[cfg(not(feature = "download-button"))]
pub const INJECTED_API_JS: &str = concat!(
    "window.__YM_NODL_FLAG__=true;",
    include_str!("../assets/injected_api.js")
);

pub const AJAZZ_GUARD_JS: &str = include_str!("../assets/ajazz_guard.js");

pub const PLACEHOLDERS: [&str; 7] = [
    "__YM_CONTROLLER_HANDLE__",
    "__YM_GUARD_HANDLE__",
    "__YM_NOTIFY_HANDLE__",
    "__YM_REAL_KEY__",
    "__YM_GUARD_KEY__",
    "__YM_NODL_FLAG__",
    "__YM_NONCE__",
];

fn random_bits() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    let probe = 0u8;
    h.write_usize(&probe as *const u8 as usize);
    h.write_u64(SEQ.fetch_add(1, Ordering::Relaxed));
    if let Ok(d) = SystemTime::now().duration_since(UNIX_EPOCH) {
        h.write_u128(d.as_nanos());
    }
    h.finish()
}

fn random_token(words: usize) -> String {
    let mut s = String::new();
    for _ in 0..words {
        s.push_str(&format!("{:016x}", random_bits()));
    }
    s
}

pub fn random_handle() -> String {
    format!("_{}", random_token(2))
}

#[derive(Clone, Debug)]
pub struct JsHandles {
    pub ctrl: String,
    pub guard: String,
    pub notify: String,
    pub real_key: String,
    pub guard_key: String,
    pub nodl_flag: String,
    pub nonce: String,
}

impl JsHandles {
    pub fn random() -> Self {
        Self {
            ctrl: random_handle(),
            guard: random_handle(),
            notify: random_handle(),
            real_key: random_handle(),
            guard_key: random_handle(),
            nodl_flag: random_handle(),
            nonce: random_token(2),
        }
    }

    fn substitute(&self, src: &str) -> String {
        src.replace(PLACEHOLDERS[0], &self.ctrl)
            .replace(PLACEHOLDERS[1], &self.guard)
            .replace(PLACEHOLDERS[2], &self.notify)
            .replace(PLACEHOLDERS[3], &self.real_key)
            .replace(PLACEHOLDERS[4], &self.guard_key)
            .replace(PLACEHOLDERS[5], &self.nodl_flag)
            .replace(PLACEHOLDERS[6], &self.nonce)
    }

    pub fn injected_api(&self) -> String {
        self.substitute(INJECTED_API_JS)
    }

    pub fn guard_js(&self) -> String {
        self.substitute(AJAZZ_GUARD_JS)
    }

    pub fn ctrl_ref(&self) -> String {
        format!("window[{}]", js_str(&self.ctrl))
    }

    pub fn health_expr(&self) -> String {
        format!(
            "(function(){{try{{var c=window[{ctrl}];return{{alive:!!(c&&c[{key}]===true),\
             observing:!!(c&&c.observing===true),bind:typeof window[{notify}]==='function',\
             guard:!!(window[{guard}]&&window[{guard}][{gkey}]===true)}}}}\
             catch(e){{return{{alive:false,observing:false,bind:false,guard:false}}}}}})()",
            ctrl = js_str(&self.ctrl),
            key = js_str(&self.real_key),
            notify = js_str(&self.notify),
            guard = js_str(&self.guard),
            gkey = js_str(&self.guard_key),
        )
    }

    pub fn rival_expr(&self) -> String {
        format!(
            "!!(window[{g}]&&window[{g}].rivalSeen===true)",
            g = js_str(&self.guard)
        )
    }
}

fn js_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard_is_plain() -> bool {
        AJAZZ_GUARD_JS.contains("function enforce(")
    }

    #[test]
    fn injected_js_is_embedded_and_sane() {
        assert!(!INJECTED_API_JS.is_empty(), "injected_api.js не вшит");
        assert!(
            !INJECTED_API_JS.contains("_PyYMController"),
            "вшитый JS не должен содержать статичное имя контроллера"
        );
        assert!(
            INJECTED_API_JS.contains("getFullState"),
            "вшитый JS не содержит getFullState"
        );
        for p in [
            "__YM_CONTROLLER_HANDLE__",
            "__YM_GUARD_HANDLE__",
            "__YM_NOTIFY_HANDLE__",
            "__YM_REAL_KEY__",
            "__YM_NONCE__",
        ] {
            assert!(
                INJECTED_API_JS.contains(p),
                "в injected_api.js нет плейсхолдера {p}"
            );
        }
    }

    #[cfg(not(feature = "download-button"))]
    #[test]
    fn marketplace_build_carries_the_marker_xtask_looks_for() {
        assert!(
            INJECTED_API_JS.starts_with(ym_model::dist::MARKETPLACE_MARKER),
            "префикс маркетплейс-сборки разошёлся с ym_model::dist::MARKETPLACE_MARKER — \
             xtask ищет эту строку внутри .exe и иначе завалит сборку Elgato"
        );
    }

    #[cfg(feature = "download-button")]
    #[test]
    fn full_build_does_not_carry_marketplace_marker() {
        assert!(
            !INJECTED_API_JS.contains(ym_model::dist::MARKETPLACE_MARKER),
            "полная сборка не должна нести маркер маркетплейса — иначе проверка xtask потеряет смысл"
        );
    }

    #[test]
    fn injected_js_has_no_stable_public_markers() {
        for banned in [
            "sdNotify",
            "__ymReal",
            "__ymAjazzGuard__",
            "__ymNoDownloadUi",
        ] {
            assert!(
                !INJECTED_API_JS.contains(banned),
                "стабильное публичное имя {banned} должно быть заменено плейсхолдером"
            );
        }
    }

    #[test]
    fn guard_handle_is_randomized_but_stub_marker_stays_stable() {
        assert!(
            !AJAZZ_GUARD_JS.contains("__ymAjazzGuard__"),
            "хендл guard должен быть рандомным, иначе форк отключит его болванкой"
        );
        for p in ["__YM_GUARD_HANDLE__", "__YM_GUARD_KEY__"] {
            assert!(
                AJAZZ_GUARD_JS.contains(p),
                "в ajazz_guard.js нет плейсхолдера {p}"
            );
        }
        if guard_is_plain() {
            assert!(
                AJAZZ_GUARD_JS.contains("var STUB = '__ymStub'"),
                "маркер стабов обязан быть стабильным: иначе новая версия примет стабы старой за конкурента"
            );
        }
    }

    #[test]
    fn handles_are_random_and_valid_identifiers() {
        let a = JsHandles::random();
        let b = JsHandles::random();
        assert_ne!(a.ctrl, b.ctrl, "имена должны различаться между процессами");
        assert_ne!(
            a.nonce, b.nonce,
            "nonce должен различаться между процессами"
        );

        let all = [
            &a.ctrl,
            &a.guard,
            &a.notify,
            &a.real_key,
            &a.guard_key,
            &a.nodl_flag,
        ];
        for h in all {
            assert!(
                h.starts_with('_'),
                "имя должно начинаться с валидного символа: {h}"
            );
            assert!(
                h.chars().all(|c| c == '_' || c.is_ascii_alphanumeric()),
                "имя должно быть валидным JS-идентификатором: {h}"
            );
            assert!(h.len() >= 16, "имя слишком короткое: {h}");
        }
        let uniq: std::collections::HashSet<_> = all.iter().collect();
        assert_eq!(
            uniq.len(),
            all.len(),
            "хендлы внутри набора должны быть различны"
        );
    }

    #[test]
    fn substitution_leaves_no_placeholders() {
        let h = JsHandles::random();
        for js in [h.injected_api(), h.guard_js()] {
            for p in PLACEHOLDERS {
                assert!(!js.contains(p), "плейсхолдер {p} не подставлен");
            }
        }
        assert!(
            h.injected_api().contains(&h.ctrl),
            "имя контроллера не попало в скрипт"
        );
        assert!(
            h.injected_api().contains(&h.nonce),
            "nonce не попал в скрипт"
        );
        assert!(
            h.guard_js().contains(&h.guard),
            "имя guard не попало в скрипт"
        );
    }

    #[test]
    fn exprs_reference_current_handles() {
        let h = JsHandles::random();
        assert_eq!(h.ctrl_ref(), format!("window[\"{}\"]", h.ctrl));
        let health = h.health_expr();
        for needle in [&h.ctrl, &h.real_key, &h.notify, &h.guard, &h.guard_key] {
            assert!(
                health.contains(needle.as_str()),
                "health_expr не ссылается на {needle}"
            );
        }
        assert!(
            h.rival_expr().contains(&h.guard),
            "rival_expr не ссылается на guard"
        );
    }

    #[test]
    fn ajazz_guard_is_embedded_and_sane() {
        assert!(!AJAZZ_GUARD_JS.is_empty(), "ajazz_guard.js не вшит");
        if !guard_is_plain() {
            return;
        }
        for marker in [
            "__YM_AJAZZ_OBSERVER__",
            "__YM_AJAZZ_STATE",
            "__YM_AJAZZ_ROOT_TIMER__",
            "__YM_AJAZZ_HELPERS_READY__",
            "ymInstallAjazzObserver",
            "ymCollectAjazzState",
            "ymStateFingerprint",
            "ymAjazzNotify",
            "rivalSeen",
            "ymTogglePlayback",
            "ymToggleShuffle",
            "ymToggleRepeat",
            "ymClickLike",
            "ymSetVolume",
            "ymFindVibePlayerBar",
            "ymFindSonataPlayerBar",
        ] {
            assert!(
                AJAZZ_GUARD_JS.contains(marker),
                "guard не покрывает {marker}"
            );
        }
    }

    #[test]
    fn rival_detect_requires_running_machinery_not_just_names() {
        if !guard_is_plain() {
            return;
        }
        assert!(
            AJAZZ_GUARD_JS.contains("function machineryRunning()"),
            "детект конкурента обязан опираться на признаки работающей машинерии"
        );
        assert!(
            AJAZZ_GUARD_JS.contains("if (machineryRunning()) {")
                && AJAZZ_GUARD_JS.contains("} else if (foreignFunctions()) {"),
            "голое наличие ym*-функций должно давать лишь слабый намёк (hints), а не rivalSeen: \
             иначе форк подсадкой пустышек заставит нас показать ложную плашку «конкурент активен»"
        );
        for probe in [
            "ymAjazzNotify",
            "__YM_AJAZZ_STATE",
            "__YM_AJAZZ_ROOT_TIMER__",
        ] {
            assert!(
                AJAZZ_GUARD_JS.contains(probe),
                "признак работающей машинерии {probe} не проверяется"
            );
        }
    }

    #[test]
    fn ajazz_guard_covers_full_command_surface() {
        if !guard_is_plain() {
            return;
        }
        let names = [
            "ymTogglePlayback",
            "ymClickTrackControl",
            "ymClickLike",
            "ymClickDislike",
            "ymToggleMute",
            "ymGetVolume",
            "ymSetVolume",
            "ymChangeVolume",
            "ymGetTrackInfo",
            "ymGetTrackTime",
            "ymSeekTo",
            "ymSeekRelative",
            "ymToggleShuffle",
            "ymToggleRepeat",
            "ymDetectShufflePressed",
            "ymDetectRepeatMode",
            "ymDetectPlaybackIsPlaying",
            "ymDetectLikeIsLiked",
            "ymDetectMuteIsMuted",
        ];
        let count = names
            .iter()
            .filter(|n| AJAZZ_GUARD_JS.contains(**n))
            .count();
        assert_eq!(
            count,
            names.len(),
            "guard должен глушить все командные функции чужого плагина"
        );
    }
}
