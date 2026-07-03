use std::path::{Path, PathBuf};

use crate::ops::LaunchTarget;
use crate::scan::app_bundle_from_exe;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Mac,
    Windows,
}

pub fn current_os() -> Os {
    if cfg!(target_os = "windows") { Os::Windows } else { Os::Mac }
}

pub fn resolve_launch_target(
    os: Os,
    override_path: Option<&str>,
    cached_exe: Option<&Path>,
    local_app_data: Option<&Path>,
    exists: &dyn Fn(&Path) -> bool,
) -> Option<LaunchTarget> {
    if let Some(raw) = override_path {
        let p = PathBuf::from(raw);
        if exists(&p) {
            return Some(match os {
                Os::Mac if p.extension().is_some_and(|e| e == "app") => LaunchTarget::MacApp(p),
                Os::Mac => app_bundle_from_exe(&p).map_or(LaunchTarget::Exe(p), LaunchTarget::MacApp),
                Os::Windows => LaunchTarget::Exe(p),
            });
        }
        tracing::warn!("launch: указанный в настройках путь к клиенту не существует: {raw}");
    }
    if let Some(exe) = cached_exe
        && exists(exe)
    {
        return Some(match os {
            Os::Mac => LaunchTarget::MacBundle { fallback_app: app_bundle_from_exe(exe) },
            Os::Windows => LaunchTarget::Exe(exe.to_path_buf()),
        });
    }
    match os {
        Os::Mac => Some(LaunchTarget::MacBundle { fallback_app: None }),
        Os::Windows => {
            let def = local_app_data?.join("Programs").join("YandexMusic").join("Яндекс Музыка.exe");
            exists(&def).then_some(LaunchTarget::Exe(def))
        }
    }
}

pub fn load_cached_exe(path: &Path) -> Option<PathBuf> {
    let s = std::fs::read_to_string(path).ok()?;
    let t = s.trim();
    if t.is_empty() { None } else { Some(PathBuf::from(t)) }
}

pub fn store_cached_exe(path: &Path, exe: &Path) {
    if let Err(e) = std::fs::write(path, exe.to_string_lossy().as_bytes()) {
        tracing::warn!("launch: не удалось сохранить путь клиента в {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAC_EXE: &str = "/Applications/Яндекс Музыка.app/Contents/MacOS/Яндекс Музыка";
    const WIN_EXE: &str = "C:\\Users\\u\\AppData\\Local\\Programs\\YandexMusic\\Яндекс Музыка.exe";

    fn all_exist(_: &Path) -> bool {
        true
    }
    fn none_exist(_: &Path) -> bool {
        false
    }

    #[test]
    fn override_wins() {
        let t = resolve_launch_target(
            Os::Windows,
            Some("D:\\Portable\\Яндекс Музыка.exe"),
            Some(Path::new(WIN_EXE)),
            Some(Path::new("C:\\Users\\u\\AppData\\Local")),
            &all_exist,
        );
        assert_eq!(t, Some(LaunchTarget::Exe(PathBuf::from("D:\\Portable\\Яндекс Музыка.exe"))));
    }

    #[test]
    fn missing_override_falls_back_to_cache() {
        let exists = |p: &Path| p == Path::new(WIN_EXE);
        let t = resolve_launch_target(
            Os::Windows,
            Some("D:\\gone.exe"),
            Some(Path::new(WIN_EXE)),
            None,
            &exists,
        );
        assert_eq!(t, Some(LaunchTarget::Exe(PathBuf::from(WIN_EXE))));
    }

    #[test]
    fn cache_falls_back_to_default_then_none() {
        let def = PathBuf::from("C:\\Users\\u\\AppData\\Local")
            .join("Programs")
            .join("YandexMusic")
            .join("Яндекс Музыка.exe");
        let exists = move |p: &Path| p == def;
        let t = resolve_launch_target(
            Os::Windows,
            None,
            Some(Path::new("C:\\gone.exe")),
            Some(Path::new("C:\\Users\\u\\AppData\\Local")),
            &exists,
        );
        assert!(matches!(t, Some(LaunchTarget::Exe(p)) if p.ends_with("Яндекс Музыка.exe")));

        let t = resolve_launch_target(Os::Windows, None, None, Some(Path::new("C:\\x")), &none_exist);
        assert_eq!(t, None);
        let t = resolve_launch_target(Os::Windows, None, None, None, &all_exist);
        assert_eq!(t, None);
    }

    #[test]
    fn mac_always_has_bundle_target() {
        let t = resolve_launch_target(Os::Mac, None, None, None, &none_exist);
        assert_eq!(t, Some(LaunchTarget::MacBundle { fallback_app: None }));

        let t = resolve_launch_target(Os::Mac, None, Some(Path::new(MAC_EXE)), None, &all_exist);
        assert_eq!(
            t,
            Some(LaunchTarget::MacBundle {
                fallback_app: Some(PathBuf::from("/Applications/Яндекс Музыка.app"))
            })
        );
    }

    #[test]
    fn mac_override_variants() {
        let t = resolve_launch_target(Os::Mac, Some("/Applications/Яндекс Музыка.app"), None, None, &all_exist);
        assert_eq!(t, Some(LaunchTarget::MacApp(PathBuf::from("/Applications/Яндекс Музыка.app"))));

        let t = resolve_launch_target(Os::Mac, Some(MAC_EXE), None, None, &all_exist);
        assert_eq!(t, Some(LaunchTarget::MacApp(PathBuf::from("/Applications/Яндекс Музыка.app"))));

        let t = resolve_launch_target(Os::Mac, Some("/usr/local/bin/ym"), None, None, &all_exist);
        assert_eq!(t, Some(LaunchTarget::Exe(PathBuf::from("/usr/local/bin/ym"))));
    }

    #[test]
    fn cache_roundtrip_and_edge_cases() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join(".ym_client_path");
        assert_eq!(load_cached_exe(&cache), None);
        store_cached_exe(&cache, Path::new(MAC_EXE));
        assert_eq!(load_cached_exe(&cache), Some(PathBuf::from(MAC_EXE)));
        std::fs::write(&cache, "   \n").unwrap();
        assert_eq!(load_cached_exe(&cache), None);
    }
}
