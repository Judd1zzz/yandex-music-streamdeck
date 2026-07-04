use std::path::{Path, PathBuf};

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientProc {
    pub pid: u32,
    pub parent: Option<u32>,
    pub exe: PathBuf,
    pub is_main: bool,
    pub cmd_unreadable: bool,
    pub debug_port: Option<u16>,
    pub start_time: u64,
}

pub enum MainPick<'a> {
    One(&'a ClientProc),
    NotFound,
    Ambiguous,
}

pub fn scan(sys: &mut System) -> Vec<ClientProc> {
    let kind = ProcessRefreshKind::nothing()
        .with_cmd(UpdateKind::Always)
        .with_exe(UpdateKind::Always);
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
    let mut out = Vec::new();
    for (pid, p) in sys.processes() {
        let Some(exe) = p.exe() else { continue };
        let name = p.name().to_string_lossy();
        if !is_ym_process(exe, &name) {
            continue;
        }
        let cmd: Vec<String> = p.cmd().iter().map(|s| s.to_string_lossy().into_owned()).collect();
        out.push(ClientProc {
            pid: pid.as_u32(),
            parent: p.parent().map(sysinfo::Pid::as_u32),
            exe: exe.to_path_buf(),
            is_main: is_main(&cmd),
            cmd_unreadable: cmd.is_empty(),
            debug_port: parse_debug_port(&cmd),
            start_time: p.start_time(),
        });
    }
    out
}

pub fn is_ym_process(exe: &Path, name: &str) -> bool {
    let path = exe.to_string_lossy().to_lowercase();
    if path.contains("\\windowsapps\\") {
        return false;
    }
    if path.ends_with(".app/contents/macos/яндекс музыка") || path.contains("\\programs\\yandexmusic\\") {
        return true;
    }
    let file = path.rsplit(['/', '\\']).next().unwrap_or("");
    file == "яндекс музыка.exe" || name == "Яндекс Музыка"
}

pub fn is_main(cmd: &[String]) -> bool {
    !cmd.iter().any(|a| a.starts_with("--type="))
}

pub fn parse_debug_port(cmd: &[String]) -> Option<u16> {
    let mut it = cmd.iter();
    while let Some(a) = it.next() {
        if let Some(v) = a.strip_prefix("--remote-debugging-port=") {
            return v.parse().ok().filter(|p| *p != 0);
        }
        if a == "--remote-debugging-port" {
            return it.next().and_then(|v| v.parse().ok()).filter(|p| *p != 0);
        }
    }
    None
}

pub fn main_client(procs: &[ClientProc]) -> MainPick<'_> {
    let mains: Vec<&ClientProc> = procs.iter().filter(|p| p.is_main).collect();
    if let Some(with_port) = mains.iter().find(|p| p.debug_port.is_some()) {
        return MainPick::One(with_port);
    }
    match mains.as_slice() {
        [] => MainPick::NotFound,
        [one] => MainPick::One(one),
        many => {
            let ym_pids: std::collections::HashSet<u32> = procs.iter().map(|p| p.pid).collect();
            let roots: Vec<&&ClientProc> =
                many.iter().filter(|p| p.parent.is_none_or(|pp| !ym_pids.contains(&pp))).collect();
            match roots.as_slice() {
                [one] => MainPick::One(one),
                _ => MainPick::Ambiguous,
            }
        }
    }
}

pub fn app_bundle_from_exe(exe: &Path) -> Option<PathBuf> {
    let macos_dir = exe.parent()?;
    if macos_dir.file_name()? != "MacOS" {
        return None;
    }
    let contents = macos_dir.parent()?;
    if contents.file_name()? != "Contents" {
        return None;
    }
    let app = contents.parent()?;
    if app.extension()? != "app" {
        return None;
    }
    Some(app.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAC_EXE: &str = "/Applications/Яндекс Музыка.app/Contents/MacOS/Яндекс Музыка";
    const WIN_EXE: &str = "C:\\Users\\u\\AppData\\Local\\Programs\\YandexMusic\\Яндекс Музыка.exe";

    fn proc(pid: u32, exe: &str, is_main: bool, debug_port: Option<u16>) -> ClientProc {
        ClientProc {
            pid,
            parent: None,
            exe: PathBuf::from(exe),
            is_main,
            cmd_unreadable: false,
            debug_port,
            start_time: 0,
        }
    }

    fn unreadable(pid: u32, parent: Option<u32>, exe: &str) -> ClientProc {
        ClientProc {
            pid,
            parent,
            exe: PathBuf::from(exe),
            is_main: true,
            cmd_unreadable: true,
            debug_port: None,
            start_time: 0,
        }
    }

    #[test]
    fn ym_process_matching_mac() {
        assert!(is_ym_process(Path::new(MAC_EXE), "Яндекс Музыка"));
        assert!(is_ym_process(
            Path::new("/Applications/Yandex Music Debug.app/Contents/MacOS/Яндекс Музыка"),
            "x"
        ));
        assert!(!is_ym_process(
            Path::new("/Applications/Яндекс Музыка.app/Contents/Frameworks/Яндекс Музыка Helper (Renderer).app/Contents/MacOS/Яндекс Музыка Helper (Renderer)"),
            "Яндекс Музыка Helper (Renderer)"
        ));
        assert!(!is_ym_process(Path::new("/Applications/Safari.app/Contents/MacOS/Safari"), "Safari"));
    }

    #[test]
    fn ym_process_matching_windows() {
        assert!(is_ym_process(Path::new(WIN_EXE), "Яндекс Музыка.exe"));
        assert!(is_ym_process(
            Path::new("c:\\users\\u\\appdata\\local\\programs\\yandexmusic\\ЯНДЕКС МУЗЫКА.EXE"),
            "x"
        ));
        assert!(is_ym_process(Path::new("D:\\Portable\\Яндекс Музыка.exe"), "x"));
        assert!(!is_ym_process(
            Path::new("C:\\Program Files\\WindowsApps\\Yandex.Music_5.0\\Яндекс Музыка.exe"),
            "x"
        ));
        assert!(!is_ym_process(Path::new("C:\\Windows\\explorer.exe"), "explorer.exe"));
    }

    #[test]
    fn main_detection_by_type_arg() {
        assert!(is_main(&[]));
        assert!(is_main(&["--remote-debugging-port=9222".into()]));
        assert!(!is_main(&["--type=renderer".into(), "--user-data-dir=/x".into()]));
        assert!(!is_main(&["--type=gpu-process".into()]));
        assert!(!is_main(&["--type=utility".into()]));
    }

    #[test]
    fn debug_port_parsing() {
        assert_eq!(parse_debug_port(&["--remote-debugging-port=9223".into()]), Some(9223));
        assert_eq!(
            parse_debug_port(&["--remote-debugging-port".into(), "9224".into()]),
            Some(9224)
        );
        assert_eq!(parse_debug_port(&["--lang=ru".into()]), None);
        assert_eq!(parse_debug_port(&["--remote-debugging-port=abc".into()]), None);
        assert_eq!(parse_debug_port(&["--remote-debugging-port=0".into()]), None);
        assert_eq!(parse_debug_port(&["--remote-debugging-port=99999".into()]), None);
        assert_eq!(parse_debug_port(&[]), None);
    }

    #[test]
    fn main_client_prefers_port_and_rejects_ambiguity() {
        let procs = vec![
            proc(1, MAC_EXE, true, None),
            proc(2, MAC_EXE, true, Some(9222)),
            proc(3, MAC_EXE, false, None),
        ];
        assert!(matches!(main_client(&procs), MainPick::One(p) if p.pid == 2));

        let single = vec![proc(1, MAC_EXE, true, None), proc(3, MAC_EXE, false, None)];
        assert!(matches!(main_client(&single), MainPick::One(p) if p.pid == 1));

        let none = vec![proc(3, MAC_EXE, false, None)];
        assert!(matches!(main_client(&none), MainPick::NotFound));
        assert!(matches!(main_client(&[]), MainPick::NotFound));

        let dup = vec![proc(1, MAC_EXE, true, None), proc(2, MAC_EXE, true, None)];
        assert!(matches!(main_client(&dup), MainPick::Ambiguous));
    }

    #[test]
    fn elevated_family_resolves_to_parent_root() {
        let procs = vec![
            unreadable(100, Some(1), WIN_EXE),
            unreadable(101, Some(100), WIN_EXE),
            unreadable(102, Some(100), WIN_EXE),
            unreadable(103, Some(101), WIN_EXE),
        ];
        assert!(matches!(main_client(&procs), MainPick::One(p) if p.pid == 100));
    }

    #[test]
    fn two_independent_elevated_instances_stay_ambiguous() {
        let procs = vec![
            unreadable(100, Some(1), WIN_EXE),
            unreadable(200, Some(2), WIN_EXE),
            unreadable(201, Some(200), WIN_EXE),
        ];
        assert!(matches!(main_client(&procs), MainPick::Ambiguous));
    }

    #[test]
    fn orphaned_parent_counts_as_root() {
        let procs = vec![unreadable(100, None, WIN_EXE), unreadable(101, Some(100), WIN_EXE)];
        assert!(matches!(main_client(&procs), MainPick::One(p) if p.pid == 100));
    }

    #[tokio::test]
    #[ignore]
    async fn live_probe_and_scan() {
        let st = crate::probe::probe(9222).await;
        println!("probe(9222) = {st:?}");
        let mut sys = System::new();
        let procs = scan(&mut sys);
        println!("найдено процессов клиента: {}", procs.len());
        for p in &procs {
            println!(
                "pid={} main={} port={:?} start={} exe={}",
                p.pid,
                p.is_main,
                p.debug_port,
                p.start_time,
                p.exe.display()
            );
        }
        match main_client(&procs) {
            MainPick::One(m) => println!("главный: pid={} port={:?}", m.pid, m.debug_port),
            MainPick::NotFound => println!("главный: не найден"),
            MainPick::Ambiguous => println!("главный: неоднозначно"),
        }
    }

    #[test]
    fn bundle_derivation() {
        assert_eq!(
            app_bundle_from_exe(Path::new(MAC_EXE)),
            Some(PathBuf::from("/Applications/Яндекс Музыка.app"))
        );
        assert_eq!(app_bundle_from_exe(Path::new("/usr/local/bin/ym")), None);
        assert_eq!(app_bundle_from_exe(Path::new("/Applications/Яндекс Музыка.app")), None);
    }
}
