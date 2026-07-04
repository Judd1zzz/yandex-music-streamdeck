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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    Missing,
    File,
    Dir,
}

pub fn fs_path_kind(p: &Path) -> PathKind {
    if p.is_file() {
        PathKind::File
    } else if p.is_dir() {
        PathKind::Dir
    } else {
        PathKind::Missing
    }
}

pub fn client_file_name(os: Os) -> &'static str {
    match os {
        Os::Windows => "Яндекс Музыка.exe",
        Os::Mac => "Яндекс Музыка.app",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathVerdict {
    Ok,
    OkDirCompleted(PathBuf),
    Missing,
    DirWithoutClient,
}

pub fn check_client_path(os: Os, raw: &str, probe: &dyn Fn(&Path) -> PathKind) -> PathVerdict {
    let p = PathBuf::from(raw.trim());
    match probe(&p) {
        PathKind::Missing => PathVerdict::Missing,
        PathKind::File => PathVerdict::Ok,
        PathKind::Dir => {
            if os == Os::Mac && p.extension().is_some_and(|e| e == "app") {
                return PathVerdict::Ok;
            }
            let cand = p.join(client_file_name(os));
            let good = match os {
                Os::Windows => probe(&cand) == PathKind::File,
                Os::Mac => probe(&cand) == PathKind::Dir,
            };
            if good { PathVerdict::OkDirCompleted(cand) } else { PathVerdict::DirWithoutClient }
        }
    }
}

fn target_for(os: Os, path: PathBuf) -> LaunchTarget {
    match os {
        Os::Windows => LaunchTarget::Exe(path),
        Os::Mac if path.extension().is_some_and(|e| e == "app") => LaunchTarget::MacApp(path),
        Os::Mac => app_bundle_from_exe(&path).map_or(LaunchTarget::Exe(path), LaunchTarget::MacApp),
    }
}

pub fn resolve_launch_target(
    os: Os,
    override_path: Option<&str>,
    cached_exe: Option<&Path>,
    local_app_data: Option<&Path>,
    probe: &dyn Fn(&Path) -> PathKind,
) -> Option<LaunchTarget> {
    if let Some(raw) = override_path {
        match check_client_path(os, raw, probe) {
            PathVerdict::Ok => return Some(target_for(os, PathBuf::from(raw.trim()))),
            PathVerdict::OkDirCompleted(cand) => return Some(target_for(os, cand)),
            PathVerdict::Missing | PathVerdict::DirWithoutClient => {
                tracing::warn!(
                    "launch: путь к клиенту из настроек не подходит ({raw}) — укажите „{}“; использую авто-детект",
                    client_file_name(os)
                );
            }
        }
    }
    if let Some(exe) = cached_exe
        && probe(exe) == PathKind::File
    {
        return Some(match os {
            Os::Mac => LaunchTarget::MacBundle { fallback_app: app_bundle_from_exe(exe) },
            Os::Windows => LaunchTarget::Exe(exe.to_path_buf()),
        });
    }
    match os {
        Os::Mac => Some(LaunchTarget::MacBundle { fallback_app: None }),
        Os::Windows => {
            let def = local_app_data?.join("Programs").join("YandexMusic").join(client_file_name(os));
            (probe(&def) == PathKind::File).then_some(LaunchTarget::Exe(def))
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
    const MAC_APP: &str = "/Applications/Яндекс Музыка.app";
    const WIN_EXE: &str = "C:\\Users\\u\\AppData\\Local\\Programs\\YandexMusic\\Яндекс Музыка.exe";
    const WIN_DIR: &str = "C:\\Users\\u\\AppData\\Local\\Programs\\YandexMusic";

    fn norm(p: &Path) -> String {
        p.to_string_lossy().replace('\\', "/")
    }

    fn kinds(map: Vec<(&'static str, PathKind)>) -> impl Fn(&Path) -> PathKind {
        move |p: &Path| {
            map.iter()
                .find(|(s, _)| norm(Path::new(s)) == norm(p))
                .map(|(_, k)| *k)
                .unwrap_or(PathKind::Missing)
        }
    }

    fn none_exist(_: &Path) -> PathKind {
        PathKind::Missing
    }

    #[test]
    fn override_wins() {
        let probe = kinds(vec![
            ("D:\\Portable\\Яндекс Музыка.exe", PathKind::File),
            (WIN_EXE, PathKind::File),
        ]);
        let t = resolve_launch_target(
            Os::Windows,
            Some("D:\\Portable\\Яндекс Музыка.exe"),
            Some(Path::new(WIN_EXE)),
            Some(Path::new("C:\\Users\\u\\AppData\\Local")),
            &probe,
        );
        assert_eq!(t, Some(LaunchTarget::Exe(PathBuf::from("D:\\Portable\\Яндекс Музыка.exe"))));
    }

    #[test]
    fn missing_override_falls_back_to_cache() {
        let probe = kinds(vec![(WIN_EXE, PathKind::File)]);
        let t = resolve_launch_target(Os::Windows, Some("D:\\gone.exe"), Some(Path::new(WIN_EXE)), None, &probe);
        assert_eq!(t, Some(LaunchTarget::Exe(PathBuf::from(WIN_EXE))));
    }

    #[test]
    fn override_dir_completed_to_exe_on_windows() {
        let probe = kinds(vec![(WIN_DIR, PathKind::Dir), (WIN_EXE, PathKind::File)]);
        let t = resolve_launch_target(Os::Windows, Some(WIN_DIR), None, None, &probe);
        assert_eq!(t, Some(LaunchTarget::Exe(Path::new(WIN_DIR).join("Яндекс Музыка.exe"))));
    }

    #[test]
    fn override_dir_without_client_falls_back_to_cache() {
        let probe = kinds(vec![("D:\\Empty", PathKind::Dir), (WIN_EXE, PathKind::File)]);
        let t = resolve_launch_target(Os::Windows, Some("D:\\Empty"), Some(Path::new(WIN_EXE)), None, &probe);
        assert_eq!(t, Some(LaunchTarget::Exe(PathBuf::from(WIN_EXE))));
    }

    #[test]
    fn cache_dir_is_rejected() {
        let probe = kinds(vec![(WIN_DIR, PathKind::Dir)]);
        let t = resolve_launch_target(Os::Windows, None, Some(Path::new(WIN_DIR)), None, &probe);
        assert_eq!(t, None);
    }

    #[test]
    fn cache_falls_back_to_default_then_none() {
        let def = "C:\\Users\\u\\AppData\\Local\\Programs\\YandexMusic\\Яндекс Музыка.exe";
        let probe = kinds(vec![(def, PathKind::File)]);
        let t = resolve_launch_target(
            Os::Windows,
            None,
            Some(Path::new("C:\\gone.exe")),
            Some(Path::new("C:\\Users\\u\\AppData\\Local")),
            &probe,
        );
        assert!(matches!(t, Some(LaunchTarget::Exe(p)) if p.ends_with("Яндекс Музыка.exe")));

        let t = resolve_launch_target(Os::Windows, None, None, Some(Path::new("C:\\x")), &none_exist);
        assert_eq!(t, None);
        let t = resolve_launch_target(Os::Windows, None, None, None, &|_: &Path| PathKind::File);
        assert_eq!(t, None);
    }

    #[test]
    fn mac_always_has_bundle_target() {
        let t = resolve_launch_target(Os::Mac, None, None, None, &none_exist);
        assert_eq!(t, Some(LaunchTarget::MacBundle { fallback_app: None }));

        let probe = kinds(vec![(MAC_EXE, PathKind::File)]);
        let t = resolve_launch_target(Os::Mac, None, Some(Path::new(MAC_EXE)), None, &probe);
        assert_eq!(t, Some(LaunchTarget::MacBundle { fallback_app: Some(PathBuf::from(MAC_APP)) }));
    }

    #[test]
    fn mac_override_variants() {
        let probe = kinds(vec![(MAC_APP, PathKind::Dir)]);
        let t = resolve_launch_target(Os::Mac, Some(MAC_APP), None, None, &probe);
        assert_eq!(t, Some(LaunchTarget::MacApp(PathBuf::from(MAC_APP))));

        let probe = kinds(vec![(MAC_EXE, PathKind::File)]);
        let t = resolve_launch_target(Os::Mac, Some(MAC_EXE), None, None, &probe);
        assert_eq!(t, Some(LaunchTarget::MacApp(PathBuf::from(MAC_APP))));

        let probe = kinds(vec![("/usr/local/bin/ym", PathKind::File)]);
        let t = resolve_launch_target(Os::Mac, Some("/usr/local/bin/ym"), None, None, &probe);
        assert_eq!(t, Some(LaunchTarget::Exe(PathBuf::from("/usr/local/bin/ym"))));
    }

    #[test]
    fn mac_override_plain_dir_completed_to_app() {
        let probe = kinds(vec![("/Applications", PathKind::Dir), (MAC_APP, PathKind::Dir)]);
        let t = resolve_launch_target(Os::Mac, Some("/Applications"), None, None, &probe);
        assert_eq!(t, Some(LaunchTarget::MacApp(PathBuf::from(MAC_APP))));
    }

    #[test]
    fn check_path_verdicts_windows() {
        let probe = kinds(vec![(WIN_DIR, PathKind::Dir), (WIN_EXE, PathKind::File)]);
        assert_eq!(check_client_path(Os::Windows, WIN_EXE, &probe), PathVerdict::Ok);
        assert_eq!(
            check_client_path(Os::Windows, WIN_DIR, &probe),
            PathVerdict::OkDirCompleted(Path::new(WIN_DIR).join("Яндекс Музыка.exe"))
        );
        assert_eq!(check_client_path(Os::Windows, "D:\\gone", &probe), PathVerdict::Missing);
        assert_eq!(check_client_path(Os::Windows, "", &probe), PathVerdict::Missing);

        let probe = kinds(vec![("D:\\Empty", PathKind::Dir)]);
        assert_eq!(check_client_path(Os::Windows, "D:\\Empty", &probe), PathVerdict::DirWithoutClient);
    }

    #[test]
    fn check_path_verdicts_mac() {
        let probe = kinds(vec![
            (MAC_APP, PathKind::Dir),
            (MAC_EXE, PathKind::File),
            ("/Applications", PathKind::Dir),
        ]);
        assert_eq!(check_client_path(Os::Mac, MAC_APP, &probe), PathVerdict::Ok);
        assert_eq!(check_client_path(Os::Mac, MAC_EXE, &probe), PathVerdict::Ok);
        assert_eq!(
            check_client_path(Os::Mac, "/Applications", &probe),
            PathVerdict::OkDirCompleted(PathBuf::from(MAC_APP))
        );
        assert_eq!(check_client_path(Os::Mac, "/gone", &probe), PathVerdict::Missing);

        let probe = kinds(vec![("/opt", PathKind::Dir)]);
        assert_eq!(check_client_path(Os::Mac, "/opt", &probe), PathVerdict::DirWithoutClient);
    }

    #[test]
    fn check_path_trims_input() {
        let probe = kinds(vec![(WIN_EXE, PathKind::File)]);
        assert_eq!(
            check_client_path(Os::Windows, &format!("  {WIN_EXE}  "), &probe),
            PathVerdict::Ok
        );
    }

    #[test]
    fn fs_path_kind_live() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f.txt");
        std::fs::write(&file, b"x").unwrap();
        assert_eq!(fs_path_kind(&file), PathKind::File);
        assert_eq!(fs_path_kind(dir.path()), PathKind::Dir);
        assert_eq!(fs_path_kind(&dir.path().join("nope")), PathKind::Missing);
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
