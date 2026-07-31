use std::path::{Path, PathBuf};

pub const RIVAL_ID: &str = "com.whxtelxs.streamdock.yandexmusicajazz";
pub const RIVAL_DIR: &str = "com.whxtelxs.streamdock.yandexmusicajazz.sdPlugin";
pub const RIVAL_AUTHOR: &str = "whxtelxs";
pub const OUR_ID: &str = "com.judd1.yandex_music";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RivalState {
    pub installed: bool,
    pub active: bool,
}

impl RivalState {
    pub fn present(&self) -> bool {
        self.installed || self.active
    }
    pub fn level(&self) -> &'static str {
        if self.active { "active" } else { "installed" }
    }
}

pub fn plugins_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.parent()?.parent()?.parent().map(Path::to_path_buf)
}

pub fn installed_in(plugins_dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(plugins_dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.contains(OUR_ID) {
            continue;
        }
        if name == RIVAL_DIR {
            return true;
        }
        if !entry.path().is_dir() {
            continue;
        }
        if manifest_is_rival(&entry.path().join("manifest.json")) {
            return true;
        }
    }
    false
}

fn manifest_is_rival(manifest: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(manifest) else {
        return false;
    };
    if text.contains(RIVAL_ID) {
        return true;
    }
    text.contains(&format!("\"{RIVAL_AUTHOR}\"")) && (text.contains("Yandex Music") || text.contains("yandexmusic"))
}

pub fn proc_is_rival(exe: Option<&Path>, cmd: &[String], cwd: Option<&Path>) -> bool {
    if let Some(e) = exe
        && e.to_string_lossy().contains(RIVAL_ID)
    {
        return true;
    }
    if let Some(c) = cwd
        && c.to_string_lossy().contains(RIVAL_ID)
    {
        return true;
    }
    cmd.iter().any(|a| a.contains(RIVAL_ID))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_manifest(dir: &Path, author: &str, uuid_prefix: &str) {
        fs::create_dir_all(dir).unwrap();
        let body = format!(
            r#"{{"Author":"{author}","Name":"Yandex Music Ajazz Plugin","Actions":[{{"UUID":"{uuid_prefix}.ym-play-pause"}}]}}"#
        );
        fs::write(dir.join("manifest.json"), body).unwrap();
    }

    #[test]
    fn detects_rival_by_dir_name() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(RIVAL_DIR)).unwrap();
        assert!(installed_in(tmp.path()));
    }

    #[test]
    fn detects_rival_by_manifest_author_and_uuid() {
        let tmp = tempfile::tempdir().unwrap();
        write_manifest(&tmp.path().join("some.random.sdPlugin"), RIVAL_AUTHOR, RIVAL_ID);
        assert!(installed_in(tmp.path()));
    }

    #[test]
    fn ignores_our_own_plugin() {
        let tmp = tempfile::tempdir().unwrap();
        write_manifest(&tmp.path().join("com.judd1.yandex_music.sdPlugin"), "Judd1", "com.judd1.yandex-music");
        assert!(!installed_in(tmp.path()));
    }

    #[test]
    fn absent_when_no_rival() {
        let tmp = tempfile::tempdir().unwrap();
        write_manifest(&tmp.path().join("com.other.thing.sdPlugin"), "someone", "com.other.thing");
        assert!(!installed_in(tmp.path()));
        assert!(!installed_in(&tmp.path().join("does-not-exist")));
    }

    #[test]
    fn proc_matched_by_cmd_and_cwd() {
        let cmd = vec!["node".to_owned(), format!("/x/{RIVAL_DIR}/plugin/index.js")];
        assert!(proc_is_rival(None, &cmd, None));
        assert!(proc_is_rival(None, &[], Some(Path::new(&format!("/plugins/{RIVAL_DIR}")))));
        assert!(!proc_is_rival(None, &["node".to_owned(), "/other/app.js".to_owned()], None));
    }

    #[test]
    fn rival_state_level_and_present() {
        assert!(!RivalState::default().present());
        assert_eq!(RivalState { installed: true, active: false }.level(), "installed");
        assert_eq!(RivalState { installed: true, active: true }.level(), "active");
        assert!(RivalState { installed: false, active: true }.present());
    }
}
