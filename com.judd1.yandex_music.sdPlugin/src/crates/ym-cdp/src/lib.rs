pub mod commands;
pub mod controller;
pub mod discovery;

pub use commands::LocalCommand;
pub use controller::{CdpController, CdpError};

pub const INJECTED_API_JS: &str = include_str!("../assets/injected_api.js");

pub const JS_CONTROLLER_NAME: &str = "window._PyYMController";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injected_js_is_embedded_and_sane() {
        assert!(!INJECTED_API_JS.is_empty(), "injected_api.js не вшит");
        assert!(
            INJECTED_API_JS.contains("window._PyYMController"),
            "вшитый JS не содержит имя контроллера"
        );
        assert!(
            INJECTED_API_JS.contains("getFullState"),
            "вшитый JS не содержит getFullState"
        );
    }
}
