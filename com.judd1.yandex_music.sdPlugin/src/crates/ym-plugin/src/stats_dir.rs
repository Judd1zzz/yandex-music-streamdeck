use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

const APP_DIR: &str = "com.judd1.yandex_music";
const STATS_FILE: &str = "stats.json";
const MIGRATED_FILE: &str = "stats.json.migrated";

pub fn default_stats_dir() -> Option<PathBuf> {
    stats_dir_from(
        std::env::var_os("YM_STATS_DIR"),
        std::env::var_os("HOME"),
        std::env::var_os("APPDATA"),
        std::env::consts::OS,
    )
}

fn stats_dir_from(
    over: Option<OsString>,
    home: Option<OsString>,
    appdata: Option<OsString>,
    os: &str,
) -> Option<PathBuf> {
    let present = |v: Option<OsString>| v.filter(|s| !s.is_empty()).map(PathBuf::from);
    if let Some(dir) = present(over) {
        return Some(dir);
    }
    let base = match os {
        "windows" => present(appdata)?,
        "macos" => present(home)?.join("Library").join("Application Support"),
        _ => present(home)?.join(".local").join("share"),
    };
    Some(base.join(APP_DIR))
}

pub fn prepare(legacy: &Path, target: Option<PathBuf>) -> PathBuf {
    let Some(target) = target else {
        tracing::warn!("статистика: каталог данных ОС не определён — остаётся в {}", legacy.display());
        return legacy.to_path_buf();
    };
    if let Err(e) = fs::create_dir_all(&target) {
        tracing::warn!("статистика: не создать {} ({e}) — остаётся в {}", target.display(), legacy.display());
        return legacy.to_path_buf();
    }
    let new_file = target.join(STATS_FILE);
    let old_file = legacy.join(STATS_FILE);
    if new_file.exists() || !old_file.exists() {
        return target;
    }
    let tmp = target.join("stats.json.tmp");
    match fs::copy(&old_file, &tmp).and_then(|_| fs::rename(&tmp, &new_file)) {
        Ok(()) => {
            if let Err(e) = fs::rename(&old_file, legacy.join(MIGRATED_FILE)) {
                tracing::warn!("статистика: старый {} не переименован ({e}), главный теперь новый", old_file.display());
            }
            tracing::info!("статистика перенесена: {} → {}", old_file.display(), new_file.display());
            target
        }
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            tracing::warn!("статистика: перенос {} не удался ({e}) — остаётся в {}", old_file.display(), legacy.display());
            legacy.to_path_buf()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ym_core::StatsStore;

    const T0: f64 = 1_609_459_200.0;

    fn os(s: &str) -> Option<OsString> {
        Some(OsString::from(s))
    }

    #[test]
    fn stats_dir_is_os_data_dir_unless_overridden() {
        assert_eq!(
            stats_dir_from(None, os("/Users/u"), None, "macos"),
            Some(PathBuf::from("/Users/u/Library/Application Support/com.judd1.yandex_music"))
        );
        assert_eq!(
            stats_dir_from(None, None, os("C:/Users/u/AppData/Roaming"), "windows"),
            Some(PathBuf::from("C:/Users/u/AppData/Roaming").join("com.judd1.yandex_music"))
        );
        assert_eq!(stats_dir_from(os("/tmp/x"), os("/Users/u"), None, "macos"), Some(PathBuf::from("/tmp/x")));
        assert!(stats_dir_from(os(""), os("/Users/u"), None, "macos").unwrap().ends_with(APP_DIR), "пустое переопределение не считается");
        assert_eq!(stats_dir_from(None, None, None, "windows"), None);
        assert_eq!(stats_dir_from(None, None, os("C:/x"), "macos"), None);
    }

    #[test]
    fn old_stats_move_to_os_dir_and_old_file_becomes_backup() {
        let root = tempfile::tempdir().unwrap();
        let legacy = root.path().join("plugin/logs");
        fs::create_dir_all(&legacy).unwrap();
        {
            let mut s = StatsStore::new(0);
            s.attach(legacy.clone(), 0, T0);
            s.on_track("1", "A", "X", T0);
            s.on_track("stop", "", "", T0 + 30.0);
            s.flush();
        }
        let old_bytes = fs::read(legacy.join(STATS_FILE)).unwrap();
        let target = root.path().join("data/com.judd1.yandex_music");

        let dir = prepare(&legacy, Some(target.clone()));

        assert_eq!(dir, target);
        assert_eq!(fs::read(target.join(STATS_FILE)).unwrap(), old_bytes, "данные перенесены байт в байт");
        assert!(!legacy.join(STATS_FILE).exists(), "старый файл больше не главный");
        assert_eq!(fs::read(legacy.join(MIGRATED_FILE)).unwrap(), old_bytes, "старый файл остался резервом");
        let mut s = StatsStore::new(0);
        s.attach(dir, 0, T0 + 60.0);
        assert_eq!(s.payload("week", T0 + 60.0)["tracksPlayed"], 1, "счётчики видны из нового места");
    }

    #[test]
    fn when_both_exist_new_wins_and_old_is_left_alone() {
        let root = tempfile::tempdir().unwrap();
        let legacy = root.path().join("logs");
        let target = root.path().join("data");
        fs::create_dir_all(&legacy).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(legacy.join(STATS_FILE), b"{\"old\":true}").unwrap();
        fs::write(target.join(STATS_FILE), b"{\"new\":true}").unwrap();

        assert_eq!(prepare(&legacy, Some(target.clone())), target);
        assert_eq!(fs::read(target.join(STATS_FILE)).unwrap(), b"{\"new\":true}");
        assert_eq!(fs::read(legacy.join(STATS_FILE)).unwrap(), b"{\"old\":true}");
        assert!(!legacy.join(MIGRATED_FILE).exists());
    }

    #[test]
    fn without_any_stats_the_os_dir_is_created_empty() {
        let root = tempfile::tempdir().unwrap();
        let legacy = root.path().join("logs");
        let target = root.path().join("data/com.judd1.yandex_music");

        assert_eq!(prepare(&legacy, Some(target.clone())), target);
        assert!(target.is_dir());
        assert!(!target.join(STATS_FILE).exists(), "файл появится при первом flush");
    }

    #[test]
    fn unusable_os_dir_keeps_stats_where_they_were() {
        let root = tempfile::tempdir().unwrap();
        let legacy = root.path().join("logs");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join(STATS_FILE), b"{}").unwrap();
        let blocker = root.path().join("file");
        fs::write(&blocker, b"").unwrap();

        assert_eq!(prepare(&legacy, Some(blocker.join("sub"))), legacy);
        assert_eq!(prepare(&legacy, None), legacy);
        assert!(legacy.join(STATS_FILE).exists(), "при сбое старый файл не трогается");
    }
}
