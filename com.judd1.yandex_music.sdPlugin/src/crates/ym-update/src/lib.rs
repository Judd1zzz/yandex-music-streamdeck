use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde_json::Value;

const UA: &str = "ym-plugin-updater";
const PLUGIN_DIR_NAME: &str = "com.judd1.yandex_music.sdPlugin";
const REPLACE_RETRIES: usize = 5;
const REPLACE_RETRY_DELAY: Duration = Duration::from_millis(250);

fn with_retries_delayed<T>(
    what: &str,
    delay: Duration,
    mut op: impl FnMut() -> std::io::Result<T>,
) -> Result<T> {
    let mut last: Option<std::io::Error> = None;
    for attempt in 0..REPLACE_RETRIES {
        match op() {
            Ok(v) => return Ok(v),
            Err(e) => {
                last = Some(e);
                if attempt + 1 < REPLACE_RETRIES {
                    std::thread::sleep(delay);
                }
            }
        }
    }
    Err(anyhow!("{what}: {}", last.expect("последняя ошибка ретраев")))
}

fn with_retries<T>(what: &str, op: impl FnMut() -> std::io::Result<T>) -> Result<T> {
    with_retries_delayed(what, REPLACE_RETRY_DELAY, op)
}

pub struct Release {
    pub tag: String,
    pub assets: Vec<Asset>,
}

pub struct Asset {
    pub name: String,
    pub url: String,
}

pub fn parse_release(v: &Value) -> Result<Release> {
    let tag = v.get("tag_name").and_then(Value::as_str).ok_or_else(|| anyhow!("нет tag_name"))?.to_owned();
    let assets = v
        .get("assets")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| {
                    let name = x.get("name").and_then(Value::as_str)?.to_owned();
                    let url = x.get("browser_download_url").and_then(Value::as_str)?.to_owned();
                    Some(Asset { name, url })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Release { tag, assets })
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    let norm = |s: &str| s.trim().trim_start_matches('v').to_owned();
    match (semver::Version::parse(&norm(latest)), semver::Version::parse(&norm(current))) {
        (Ok(l), Ok(c)) => l.pre.is_empty() && l > c,
        _ => false,
    }
}

pub fn bin_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "ym-plugin.exe"
    } else {
        "ym-plugin"
    }
}

pub fn pick_asset<'a>(rel: &'a Release, os: &str) -> Option<&'a Asset> {
    let exact = ym_model::dist::release_zip_name(os, rel.tag.trim().trim_start_matches('v'));
    rel.assets
        .iter()
        .find(|a| a.name == exact)
        .or_else(|| rel.assets.iter().find(|a| ym_model::dist::is_release_asset(&a.name, os)))
}

fn looks_installed() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.file_name().map(|n| n == "bin").unwrap_or(false)))
        .unwrap_or(false)
}

fn plugin_dir() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("current_exe")?;
    exe.parent()
        .and_then(|b| b.parent())
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("не определить каталог плагина от {}", exe.display()))
}

#[cfg(unix)]
fn set_exec(p: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perm = std::fs::metadata(p)?.permissions();
    perm.set_mode(0o755);
    std::fs::set_permissions(p, perm)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_exec(_p: &Path) -> Result<()> {
    Ok(())
}

pub fn apply_zip_with<F>(bytes: &[u8], plugin_dir: &Path, bin_name: &str, mut on_binary: F) -> Result<()>
where
    F: FnMut(&[u8]) -> Result<()>,
{
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).context("чтение zip")?;
    let prefix = format!("{PLUGIN_DIR_NAME}/");
    let binary_rel = format!("bin/{bin_name}");
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(enclosed) = entry.enclosed_name() else { continue };
        let name = enclosed.to_string_lossy().replace('\\', "/");
        let Some(rel) = name.strip_prefix(&prefix) else { continue };
        if rel.is_empty() || entry.is_dir() {
            continue;
        }
        let mut data = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut data)?;
        if rel == binary_rel {
            on_binary(&data)?;
            continue;
        }
        let dest = plugin_dir.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = dest.with_extension("ymtmp");
        std::fs::write(&tmp, &data).with_context(|| format!("запись {}", tmp.display()))?;
        if entry.unix_mode().is_some_and(|m| m & 0o111 != 0) {
            set_exec(&tmp)?;
        }
        with_retries(&format!("замена {}", dest.display()), || std::fs::rename(&tmp, &dest))?;
    }
    Ok(())
}

fn apply_zip(bytes: &[u8], dir: &Path, bin: &str) -> Result<()> {
    let bin_path = dir.join("bin").join(bin);
    apply_zip_with(bytes, dir, bin, |data| {
        let tmp = bin_path.with_extension("new");
        std::fs::write(&tmp, data).with_context(|| format!("запись {}", tmp.display()))?;
        set_exec(&tmp)?;
        with_retries("self_replace бинаря", || self_replace::self_replace(&tmp))?;
        let _ = std::fs::remove_file(&tmp);
        Ok(())
    })
}

async fn fetch_latest(client: &reqwest::Client, owner: &str, repo: &str) -> Result<Release> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let v: Value = client.get(url).send().await?.error_for_status()?.json().await?;
    parse_release(&v)
}

fn cleanup_stale(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            cleanup_stale(&path);
        } else if path.extension().is_some_and(|e| e == "ymtmp" || e == "new")
            && std::fs::remove_file(&path).is_ok()
        {
            tracing::debug!("ym-update: удалён недописанный файл {}", path.display());
        }
    }
}

fn cleanup_legacy(dir: &Path) {
    for name in ["ffmpeg", "ffmpeg.exe"] {
        let p = dir.join("bin").join(name);
        if p.is_file() && std::fs::remove_file(&p).is_ok() {
            tracing::info!("ym-update: удалён устаревший {}", p.display());
        }
    }
}

async fn check_and_apply(owner: &str, repo: &str, current: &str) -> Result<Option<String>> {
    let client = reqwest::Client::builder()
        .user_agent(UA)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(180))
        .build()?;
    let rel = fetch_latest(&client, owner, repo).await?;
    if !is_newer(&rel.tag, current) {
        return Ok(None);
    }
    let os = std::env::consts::OS;
    let asset = pick_asset(&rel, os).ok_or_else(|| anyhow!("в релизе {} нет ассета для {os}", rel.tag))?;
    let bytes = client.get(&asset.url).send().await?.error_for_status()?.bytes().await?.to_vec();
    let dir = plugin_dir()?;
    let version = rel.tag.trim_start_matches('v').to_owned();
    tokio::task::spawn_blocking(move || apply_zip(&bytes, &dir, bin_name())).await??;
    Ok(Some(version))
}

pub async fn run(owner: &str, repo: &str, current: &str) -> Option<String> {
    if !looks_installed() {
        tracing::debug!("ym-update: бинарь не в каталоге bin/ — автообновление пропущено");
        return None;
    }
    if let Ok(dir) = plugin_dir() {
        cleanup_stale(&dir);
        cleanup_legacy(&dir);
    }
    match check_and_apply(owner, repo, current).await {
        Ok(Some(v)) => {
            tracing::info!("ym-update: применено обновление {v} (вступит в силу при следующем запуске)");
            Some(v)
        }
        Ok(None) => {
            tracing::debug!("ym-update: обновлений нет (текущая {current})");
            None
        }
        Err(e) => {
            tracing::warn!("ym-update: {e:#}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::sync::{Arc, Mutex};
    use zip::write::SimpleFileOptions;

    #[test]
    fn retries_succeed_after_transient_lock() {
        let calls = Arc::new(Mutex::new(0usize));
        let c = calls.clone();
        let res = with_retries_delayed("тест", Duration::from_millis(1), move || {
            let mut n = c.lock().unwrap();
            *n += 1;
            if *n < 3 {
                Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "AV держит файл"))
            } else {
                Ok(())
            }
        });
        assert!(res.is_ok());
        assert_eq!(*calls.lock().unwrap(), 3);
    }

    #[test]
    fn retries_give_up_after_limit() {
        let calls = Arc::new(Mutex::new(0usize));
        let c = calls.clone();
        let res: Result<()> = with_retries_delayed("замена файла", Duration::from_millis(1), move || {
            *c.lock().unwrap() += 1;
            Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "вечный лок"))
        });
        let err = res.unwrap_err().to_string();
        assert!(err.contains("замена файла"), "ошибка должна называть операцию: {err}");
        assert_eq!(*calls.lock().unwrap(), REPLACE_RETRIES);
    }

    #[test]
    fn semver_comparison() {
        assert!(is_newer("1.1.0", "1.0.0"));
        assert!(is_newer("v1.1.0", "1.0.0"));
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.1.0"));
        assert!(!is_newer("garbage", "1.0.0"));
    }

    #[test]
    fn prerelease_tags_never_auto_update() {
        assert!(!is_newer("v2.1.0-beta.1", "2.0.0"));
        assert!(!is_newer("2.1.0-rc.1", "2.0.0"));
        assert!(is_newer("2.1.0", "2.0.0"));
        assert!(is_newer("2.0.0", "2.0.0-rc.1"));
    }

    #[test]
    fn pick_asset_matches_platform() {
        let rel = Release {
            tag: "v1.1.0".into(),
            assets: vec![
                Asset { name: "YandexMusicPlugin-windows-1.1.0.zip".into(), url: "win".into() },
                Asset { name: "YandexMusicPlugin-darwin-1.1.0.zip".into(), url: "mac".into() },
            ],
        };
        assert_eq!(pick_asset(&rel, "macos").unwrap().url, "mac");
        assert_eq!(pick_asset(&rel, "windows").unwrap().url, "win");
        assert!(pick_asset(&rel, "linux").is_none());
    }

    #[test]
    fn pick_asset_prefers_exact_tag_version() {
        let rel = Release {
            tag: "v1.1.0".into(),
            assets: vec![
                Asset { name: "YandexMusicPlugin-darwin-1.1.0.sha256.zip".into(), url: "junk".into() },
                Asset { name: "YandexMusicPlugin-darwin-1.1.0.zip".into(), url: "real".into() },
            ],
        };
        assert_eq!(pick_asset(&rel, "macos").unwrap().url, "real");
    }

    #[test]
    fn pick_asset_falls_back_to_pattern() {
        let rel = Release {
            tag: "v1.1.0".into(),
            assets: vec![Asset { name: "YandexMusicPlugin-darwin-build7.zip".into(), url: "fb".into() }],
        };
        assert_eq!(pick_asset(&rel, "macos").unwrap().url, "fb");
    }

    #[test]
    fn parse_github_release_json() {
        let v = serde_json::json!({
            "tag_name": "v1.2.0",
            "assets": [
                {"name": "YandexMusicPlugin-darwin-1.2.0.zip", "browser_download_url": "https://x/d.zip"},
                {"name": "YandexMusicPlugin-windows-1.2.0.zip", "browser_download_url": "https://x/w.zip"}
            ]
        });
        let r = parse_release(&v).unwrap();
        assert_eq!(r.tag, "v1.2.0");
        assert_eq!(r.assets.len(), 2);
        assert_eq!(r.assets[0].url, "https://x/d.zip");
    }

    fn make_zip() -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zw = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts = SimpleFileOptions::default();
            let p = PLUGIN_DIR_NAME;
            zw.start_file(format!("{p}/manifest.json"), opts).unwrap();
            zw.write_all(b"{\"Version\":\"1.2.0\"}").unwrap();
            zw.start_file(format!("{p}/static/img/icon.png"), opts).unwrap();
            zw.write_all(b"PNGDATA").unwrap();
            zw.start_file(format!("{p}/bin/ym-plugin"), opts).unwrap();
            zw.write_all(b"NEWBINARY").unwrap();
            zw.finish().unwrap();
        }
        buf
    }

    #[test]
    fn apply_zip_writes_files_and_routes_binary() {
        let zip = make_zip();
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let captured: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let cap = captured.clone();
        apply_zip_with(&zip, dir, "ym-plugin", move |data| {
            *cap.lock().unwrap() = data.to_vec();
            Ok(())
        })
        .unwrap();

        assert_eq!(std::fs::read(dir.join("manifest.json")).unwrap(), b"{\"Version\":\"1.2.0\"}");
        assert_eq!(std::fs::read(dir.join("static/img/icon.png")).unwrap(), b"PNGDATA");
        assert!(!dir.join("bin/ym-plugin").exists(), "бинарь не пишется напрямую, идёт в on_binary");
        assert_eq!(&*captured.lock().unwrap(), b"NEWBINARY");
    }

    #[cfg(unix)]
    #[test]
    fn apply_zip_preserves_executable_bit() {
        use std::os::unix::fs::PermissionsExt;
        let mut buf = Vec::new();
        {
            let mut zw = zip::ZipWriter::new(Cursor::new(&mut buf));
            let p = PLUGIN_DIR_NAME;
            zw.start_file(format!("{p}/bin/helper"), SimpleFileOptions::default().unix_permissions(0o755))
                .unwrap();
            zw.write_all(b"HELPER").unwrap();
            zw.start_file(format!("{p}/manifest.json"), SimpleFileOptions::default().unix_permissions(0o644))
                .unwrap();
            zw.write_all(b"{}").unwrap();
            zw.finish().unwrap();
        }
        let tmp = tempfile::tempdir().unwrap();
        apply_zip_with(&buf, tmp.path(), "ym-plugin", |_| Ok(())).unwrap();

        let helper = std::fs::metadata(tmp.path().join("bin/helper")).unwrap().permissions().mode();
        assert_eq!(helper & 0o111, 0o111, "исполняемый файл должен остаться исполняемым");
        let manifest = std::fs::metadata(tmp.path().join("manifest.json")).unwrap().permissions().mode();
        assert_eq!(manifest & 0o111, 0, "обычный файл не должен становиться исполняемым");
    }

    #[test]
    fn cleanup_removes_stale_files_recursively() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::create_dir_all(dir.join("static/img")).unwrap();
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::write(dir.join("manifest.json"), b"{}").unwrap();
        std::fs::write(dir.join("manifest.ymtmp"), b"junk").unwrap();
        std::fs::write(dir.join("static/img/icon.ymtmp"), b"junk").unwrap();
        std::fs::write(dir.join("bin/ym-plugin.new"), b"junk").unwrap();
        std::fs::write(dir.join("bin/helper"), b"keep").unwrap();

        cleanup_stale(dir);

        assert!(dir.join("manifest.json").exists());
        assert!(dir.join("bin/helper").exists());
        assert!(!dir.join("manifest.ymtmp").exists());
        assert!(!dir.join("static/img/icon.ymtmp").exists());
        assert!(!dir.join("bin/ym-plugin.new").exists());
    }

    #[test]
    fn cleanup_legacy_removes_only_old_ffmpeg() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::write(dir.join("bin/ffmpeg"), b"old").unwrap();
        std::fs::write(dir.join("bin/ffmpeg.exe"), b"old").unwrap();
        std::fs::write(dir.join("bin/ym-plugin"), b"keep").unwrap();
        std::fs::write(dir.join("manifest.json"), b"{}").unwrap();

        cleanup_legacy(dir);

        assert!(!dir.join("bin/ffmpeg").exists(), "устаревший ffmpeg должен удаляться");
        assert!(!dir.join("bin/ffmpeg.exe").exists());
        assert!(dir.join("bin/ym-plugin").exists());
        assert!(dir.join("manifest.json").exists());

        cleanup_legacy(dir);
        assert!(dir.join("bin/ym-plugin").exists(), "повторный вызов безопасен");
    }

    #[tokio::test]
    async fn run_returns_none_when_not_installed() {
        assert!(!looks_installed(), "тестовый бинарь не должен выглядеть установленным");
        assert_eq!(run("owner", "repo", "0.0.1").await, None);
    }
}
