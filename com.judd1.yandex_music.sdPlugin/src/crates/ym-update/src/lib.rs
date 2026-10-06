use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use ssh_key::{PublicKey, SshSig};

const UA: &str = "ym-plugin-updater";
const GITHUB_API: &str = "https://api.github.com";
const PLUGIN_DIR_NAME: &str = "com.judd1.yandex_music.sdPlugin";
const REPLACE_RETRIES: usize = 5;
const REPLACE_RETRY_DELAY: Duration = Duration::from_millis(250);
const SUMS_NAME: &str = "SHA256SUMS";
const SUMS_SIG_NAME: &str = "SHA256SUMS.sig";
const RATE_LIMIT_RETRIES: usize = 2;
const RATE_LIMIT_MAX_WAIT: Duration = Duration::from_secs(65 * 60);
const STARTUP_RETRY_DELAYS: [Duration; 6] = [
    Duration::from_secs(15),
    Duration::from_secs(30),
    Duration::from_secs(60),
    Duration::from_secs(120),
    Duration::from_secs(300),
    Duration::from_secs(600),
];
use ym_model::attest::{SIG_NAMESPACE, SIGNING_KEYS};

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
    pub body: Option<String>,
    pub html_url: Option<String>,
}

pub struct Asset {
    pub name: String,
    pub url: String,
}

#[derive(Debug, PartialEq)]
pub struct UpdateOutcome {
    pub version: String,
    pub notes: Option<String>,
    pub url: Option<String>,
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
    let body = v
        .get("body")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|s| !s.trim().is_empty());
    let html_url = v.get("html_url").and_then(Value::as_str).map(str::to_owned);
    Ok(Release { tag, assets, body, html_url })
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

fn required_asset<'a>(rel: &'a Release, name: &str) -> Result<&'a Asset> {
    rel.assets
        .iter()
        .find(|a| a.name == name)
        .ok_or_else(|| anyhow!("в релизе {} нет {name} — обновление отклонено", rel.tag))
}

async fn fetch_bytes(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    Ok(client.get(url).send().await?.error_for_status()?.bytes().await?.to_vec())
}

fn verify_sums_with(keys: &[&str], sums: &[u8], sig: &SshSig) -> Result<()> {
    for key in keys {
        let key = PublicKey::from_openssh(key).context("разбор доверенного ключа")?;
        if key.verify(SIG_NAMESPACE, sums, sig).is_ok() {
            return Ok(());
        }
    }
    Err(anyhow!("подпись {SUMS_NAME} не соответствует ни одному доверенному ключу"))
}

fn verify_release_with(keys: &[&str], zip_name: &str, zip: &[u8], sums: &[u8], sig_pem: &[u8]) -> Result<()> {
    let sig = SshSig::from_pem(sig_pem).with_context(|| format!("разбор {SUMS_SIG_NAME}"))?;
    verify_sums_with(keys, sums, &sig)?;
    let text = std::str::from_utf8(sums).with_context(|| format!("{SUMS_NAME} не в UTF-8"))?;
    let expected = ym_model::dist::find_sum(text, zip_name)
        .ok_or_else(|| anyhow!("в {SUMS_NAME} нет строки для {zip_name}"))?;
    let actual = ym_model::dist::hex_lower(&Sha256::digest(zip));
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(anyhow!("sha256 архива {zip_name} не совпадает с подписанным"));
    }
    Ok(())
}

pub fn verify_release(zip_name: &str, zip: &[u8], sums: &[u8], sig_pem: &[u8]) -> Result<()> {
    verify_release_with(SIGNING_KEYS, zip_name, zip, sums, sig_pem)
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

    let mut items: Vec<(usize, String)> = Vec::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        let Some(enclosed) = entry.enclosed_name() else { continue };
        let name = enclosed.to_string_lossy().replace('\\', "/");
        let Some(rel) = name.strip_prefix(&prefix) else { continue };
        if rel.is_empty() || entry.is_dir() {
            continue;
        }
        items.push((i, rel.to_string()));
    }

    let commit_order = |rel: &str| -> u8 {
        if rel.starts_with("bin/ym-bridge.") && !rel.ends_with(".sig") {
            0
        } else if rel == binary_rel {
            1
        } else if rel == "manifest.json" {
            4
        } else {
            3
        }
    };
    items.sort_by_key(|(_, rel)| commit_order(rel));

    for (i, rel) in items {
        let mut entry = archive.by_index(i)?;
        let mut data = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut data)?;
        if rel == binary_rel {
            on_binary(&data)?;
            continue;
        }
        let dest = plugin_dir.join(&rel);
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

async fn fetch_latest(client: &reqwest::Client, api_base: &str, owner: &str, repo: &str, current: &str) -> Result<Release> {
    let url = format!("{api_base}/repos/{owner}/{repo}/releases/latest");
    let started = std::time::Instant::now();
    let resp = client.get(url).send().await?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    if let Some(wait) = rate_limit_wait(resp.status().as_u16(), resp.headers(), now) {
        return Err(RateLimited(wait).into());
    }
    let v: Value = resp.error_for_status()?.json().await?;
    let rel = parse_release(&v)?;
    tracing::info!("{}", check_summary(&rel.tag, current, started.elapsed().as_millis()));
    Ok(rel)
}

fn check_summary(tag: &str, current: &str, ms: u128) -> String {
    let latest = tag.trim().trim_start_matches('v');
    if is_newer(tag, current) {
        format!("ym-update: доступна {latest} (текущая {current}), ответ GitHub за {ms} мс")
    } else {
        format!("ym-update: последняя версия {latest}, обновлений нет (текущая {current}), ответ GitHub за {ms} мс")
    }
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

fn is_transient(e: &anyhow::Error) -> bool {
    e.chain()
        .filter_map(|c| c.downcast_ref::<reqwest::Error>())
        .any(|r| r.is_connect() || r.is_timeout())
}

#[derive(Debug)]
struct RateLimited(Duration);

impl std::fmt::Display for RateLimited {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GitHub ограничил число запросов, сброс через {} с", self.0.as_secs())
    }
}

impl std::error::Error for RateLimited {}

fn rate_limit_wait(status: u16, headers: &reqwest::header::HeaderMap, now: u64) -> Option<Duration> {
    if status != 403 && status != 429 {
        return None;
    }
    let num = |k: &str| headers.get(k)?.to_str().ok()?.trim().parse::<u64>().ok();
    if let Some(s) = num("retry-after") {
        return Some(Duration::from_secs(s));
    }
    if num("x-ratelimit-remaining") == Some(0) {
        return num("x-ratelimit-reset").map(|reset| Duration::from_secs(reset.saturating_sub(now) + 1));
    }
    None
}

fn rate_limited(e: &anyhow::Error) -> Option<Duration> {
    e.chain().find_map(|c| c.downcast_ref::<RateLimited>()).map(|r| r.0)
}

async fn retry_transient<T, F, Fut>(delays: &[Duration], mut op: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut left = delays.iter();
    let mut limited = RATE_LIMIT_RETRIES;
    loop {
        let e = match op().await {
            Err(e) => e,
            r => return r,
        };
        if is_transient(&e) {
            let Some(d) = left.next() else { return Err(e) };
            tracing::info!("ym-update: сеть недоступна ({e:#}), повтор через {} с", d.as_secs());
            tokio::time::sleep(*d).await;
        } else if let Some(w) = rate_limited(&e)
            && limited > 0
        {
            limited -= 1;
            let w = w.min(RATE_LIMIT_MAX_WAIT);
            tracing::info!("ym-update: {e:#}, повтор через {} с", w.as_secs());
            tokio::time::sleep(w).await;
        } else {
            return Err(e);
        }
    }
}

async fn check_and_apply(owner: &str, repo: &str, current: &str) -> Result<Option<UpdateOutcome>> {
    check_and_apply_at(GITHUB_API, owner, repo, current).await
}

async fn check_and_apply_at(api_base: &str, owner: &str, repo: &str, current: &str) -> Result<Option<UpdateOutcome>> {
    let client = reqwest::Client::builder()
        .user_agent(UA)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(180))
        .build()?;
    let rel = fetch_latest(&client, api_base, owner, repo, current).await?;
    if !is_newer(&rel.tag, current) {
        return Ok(None);
    }
    let os = std::env::consts::OS;
    let asset = pick_asset(&rel, os).ok_or_else(|| anyhow!("в релизе {} нет ассета для {os}", rel.tag))?;
    let exact = ym_model::dist::release_zip_name(os, rel.tag.trim().trim_start_matches('v'));
    if asset.name != exact {
        return Err(anyhow!("имя ассета {} не соответствует тегу {} — обновление отклонено", asset.name, rel.tag));
    }
    let bytes = fetch_bytes(&client, &asset.url).await?;
    let sums = fetch_bytes(&client, &required_asset(&rel, SUMS_NAME)?.url).await?;
    let sig = fetch_bytes(&client, &required_asset(&rel, SUMS_SIG_NAME)?.url).await?;
    verify_release(&asset.name, &bytes, &sums, &sig)
        .with_context(|| format!("проверка целостности релиза {}", rel.tag))?;
    tracing::info!("ym-update: подпись релиза {} проверена", rel.tag);
    let dir = plugin_dir()?;
    let version = rel.tag.trim_start_matches('v').to_owned();
    let notes = rel.body.clone();
    let url = rel.html_url.clone();
    tokio::task::spawn_blocking(move || apply_zip(&bytes, &dir, bin_name())).await??;
    Ok(Some(UpdateOutcome { version, notes, url }))
}

pub async fn run(owner: &str, repo: &str, current: &str) -> Option<UpdateOutcome> {
    if !looks_installed() {
        tracing::info!("ym-update: бинарь не в каталоге bin/ — автообновление пропущено");
        return None;
    }
    if let Ok(dir) = plugin_dir() {
        cleanup_stale(&dir);
        cleanup_legacy(&dir);
    }
    match retry_transient(&STARTUP_RETRY_DELAYS, || check_and_apply(owner, repo, current)).await {
        Ok(Some(o)) => {
            tracing::info!("ym-update: применено обновление {} (вступит в силу при следующем запуске)", o.version);
            Some(o)
        }
        Ok(None) => None,
        Err(e) => {
            tracing::warn!("ym-update: {e:#}");
            None
        }
    }
}

pub async fn check(owner: &str, repo: &str, current: &str) -> Option<ym_model::UpdateCheck> {
    let client = reqwest::Client::builder()
        .user_agent(UA)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .ok()?;
    let rel = match fetch_latest(&client, GITHUB_API, owner, repo, current).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("ym-update check: {e:#}");
            return None;
        }
    };
    let latest = rel.tag.trim().trim_start_matches('v').to_owned();
    Some(ym_model::UpdateCheck {
        has_update: is_newer(&rel.tag, current),
        latest,
        notes: rel.body,
        url: rel.html_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::sync::atomic::{AtomicUsize, Ordering};
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
            body: None,
            html_url: None,
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
            body: None,
            html_url: None,
        };
        assert_eq!(pick_asset(&rel, "macos").unwrap().url, "real");
    }

    #[test]
    fn pick_asset_falls_back_to_pattern() {
        let rel = Release {
            tag: "v1.1.0".into(),
            assets: vec![Asset { name: "YandexMusicPlugin-darwin-build7.zip".into(), url: "fb".into() }],
            body: None,
            html_url: None,
        };
        assert_eq!(pick_asset(&rel, "macos").unwrap().url, "fb");
    }

    #[test]
    fn parse_github_release_json() {
        let v = serde_json::json!({
            "tag_name": "v1.2.0",
            "html_url": "https://github.com/x/releases/tag/v1.2.0",
            "body": "• fix A\n• fix B",
            "assets": [
                {"name": "YandexMusicPlugin-darwin-1.2.0.zip", "browser_download_url": "https://x/d.zip"},
                {"name": "YandexMusicPlugin-windows-1.2.0.zip", "browser_download_url": "https://x/w.zip"}
            ]
        });
        let r = parse_release(&v).unwrap();
        assert_eq!(r.tag, "v1.2.0");
        assert_eq!(r.assets.len(), 2);
        assert_eq!(r.assets[0].url, "https://x/d.zip");
        assert_eq!(r.body.as_deref(), Some("• fix A\n• fix B"));
        assert_eq!(r.html_url.as_deref(), Some("https://github.com/x/releases/tag/v1.2.0"));
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

    #[test]
    fn locked_bridge_leaves_no_partial_install() {
        let mut buf = Vec::new();
        {
            let mut zw = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts = SimpleFileOptions::default();
            let p = PLUGIN_DIR_NAME;
            zw.start_file(format!("{p}/manifest.json"), opts).unwrap();
            zw.write_all(b"NEW-MANIFEST").unwrap();
            zw.start_file(format!("{p}/static/img/icon.png"), opts).unwrap();
            zw.write_all(b"NEW-ICON").unwrap();
            zw.start_file(format!("{p}/bin/ym-bridge.dll"), opts).unwrap();
            zw.write_all(b"NEW-BRIDGE").unwrap();
            zw.start_file(format!("{p}/bin/ym-plugin"), opts).unwrap();
            zw.write_all(b"NEW-BIN").unwrap();
            zw.finish().unwrap();
        }

        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("manifest.json"), b"OLD-MANIFEST").unwrap();
        std::fs::create_dir_all(dir.join("static/img")).unwrap();
        std::fs::write(dir.join("static/img/icon.png"), b"OLD-ICON").unwrap();
        std::fs::create_dir_all(dir.join("bin/ym-bridge.dll/keep")).unwrap();

        let binary_applied = std::cell::Cell::new(false);
        let r = apply_zip_with(&buf, dir, "ym-plugin", |_| {
            binary_applied.set(true);
            Ok(())
        });

        assert!(r.is_err(), "замена заблокированного моста должна провалиться");
        assert_eq!(
            std::fs::read(dir.join("manifest.json")).unwrap(),
            b"OLD-MANIFEST",
            "manifest не меняется при провале моста"
        );
        assert_eq!(
            std::fs::read(dir.join("static/img/icon.png")).unwrap(),
            b"OLD-ICON",
            "static не меняется при провале моста"
        );
        assert!(!binary_applied.get(), "бинарь не применяется после провала моста");
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

    #[test]
    fn check_summary_names_verdict_version_and_latency() {
        assert_eq!(
            check_summary("v9.9.9", "1.0.0", 412),
            "ym-update: доступна 9.9.9 (текущая 1.0.0), ответ GitHub за 412 мс"
        );
        assert_eq!(
            check_summary("v9.9.9", "9.9.9", 7),
            "ym-update: последняя версия 9.9.9, обновлений нет (текущая 9.9.9), ответ GitHub за 7 мс"
        );
    }

    #[test]
    fn answered_check_and_skipped_run_are_logged_at_info() {
        let src = include_str!("lib.rs");
        let body = |head: &str| {
            let f = &src[src.find(head).expect(head)..];
            &f[..f.find("\n}\n").expect("конец функции")]
        };
        assert!(
            body("async fn fetch_latest(").contains("tracing::info!(\"{}\", check_summary(&rel.tag, current, "),
            "каждый ответ GitHub обязан оставить строку info с итогом, иначе «проверка прошла» неотличима от «проверка не запускалась»"
        );
        assert!(
            body("pub async fn run(").contains("tracing::info!(\"ym-update: бинарь не в каталоге bin/"),
            "пропуск автообновления обязан быть виден на уровне info"
        );
    }

    use ssh_key::private::Ed25519Keypair;
    use ssh_key::{HashAlg, LineEnding, PrivateKey};

    fn soft_key(seed: u8) -> PrivateKey {
        Ed25519Keypair::from_seed(&[seed; 32]).into()
    }

    fn signed_sums(zip: &[u8], zip_name: &str, key: &PrivateKey) -> (Vec<u8>, Vec<u8>) {
        let hex = ym_model::dist::hex_lower(&Sha256::digest(zip));
        let sums = ym_model::dist::sums_line(&hex, zip_name).into_bytes();
        let sig = key.sign(SIG_NAMESPACE, HashAlg::Sha512, &sums).unwrap();
        (sums, sig.to_pem(LineEnding::LF).unwrap().into_bytes())
    }

    #[test]
    fn pinned_keys_parse_and_are_sk() {
        for k in SIGNING_KEYS {
            let key = PublicKey::from_openssh(k).unwrap();
            assert_eq!(key.algorithm(), ssh_key::Algorithm::SkEd25519);
        }
    }

    #[test]
    fn signature_roundtrip_and_rejections() {
        let key = soft_key(7);
        let pin = key.public_key().to_openssh().unwrap();
        let keys = [pin.as_str()];
        let name = "YandexMusicPlugin-darwin-9.9.9.zip";
        let (sums, sig_pem) = signed_sums(b"ZIPBYTES", name, &key);
        let sig = SshSig::from_pem(&sig_pem).unwrap();

        assert!(verify_sums_with(&keys, &sums, &sig).is_ok());

        let mut tampered = sums.clone();
        tampered[0] ^= 1;
        assert!(verify_sums_with(&keys, &tampered, &sig).is_err());

        let stranger = soft_key(8).public_key().to_openssh().unwrap();
        assert!(verify_sums_with(&[stranger.as_str()], &sums, &sig).is_err());
    }

    #[test]
    fn verify_release_accepts_valid_and_rejects_broken() {
        let key = soft_key(7);
        let pin = key.public_key().to_openssh().unwrap();
        let keys = [pin.as_str()];
        let name = "YandexMusicPlugin-darwin-9.9.9.zip";
        let zip = b"ZIPBYTES";
        let (sums, sig_pem) = signed_sums(zip, name, &key);

        assert!(verify_release_with(&keys, name, zip, &sums, &sig_pem).is_ok());
        assert!(verify_release_with(&keys, name, b"EVILZIP", &sums, &sig_pem).is_err());
        assert!(verify_release_with(&keys, "YandexMusicPlugin-windows-9.9.9.zip", zip, &sums, &sig_pem).is_err());
        assert!(verify_release_with(&keys, name, zip, &sums, b"not a pem").is_err());
        assert!(verify_release_with(SIGNING_KEYS, name, zip, &sums, &sig_pem).is_err());
    }

    #[test]
    fn sk_fixture_verifies_with_pinned_key() {
        let sums = include_bytes!("../fixtures/SHA256SUMS.fixture");
        let sig_pem = include_bytes!("../fixtures/SHA256SUMS.fixture.sig");
        let sig = SshSig::from_pem(sig_pem).unwrap();

        assert!(verify_sums_with(SIGNING_KEYS, sums, &sig).is_ok());

        let mut tampered = sums.to_vec();
        tampered[0] ^= 1;
        assert!(verify_sums_with(SIGNING_KEYS, &tampered, &sig).is_err());

        let stranger = soft_key(9).public_key().to_openssh().unwrap();
        assert!(verify_sums_with(&[stranger.as_str()], sums, &sig).is_err());
    }

    fn spawn_mock(build: impl FnOnce(&str) -> Vec<(String, Vec<u8>)>) -> (String, Arc<Mutex<usize>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let routes = build(&base);
        let hits = Arc::new(Mutex::new(0usize));
        let counter = hits.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { break };
                *counter.lock().unwrap() += 1;
                let mut buf = [0u8; 8192];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = req.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let resp = match routes.iter().find(|(p, _)| *p == path) {
                    Some((_, body)) => {
                        let mut r = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .into_bytes();
                        r.extend_from_slice(body);
                        r
                    }
                    None => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
                };
                let _ = s.write_all(&resp);
            }
        });
        (base, hits)
    }

    fn mock_release_json(base: &str, assets: &[&str]) -> Vec<u8> {
        let assets: Vec<_> = assets
            .iter()
            .map(|name| serde_json::json!({"name": name, "browser_download_url": format!("{base}/dl/{name}")}))
            .collect();
        serde_json::json!({"tag_name": "v9.9.9", "assets": assets}).to_string().into_bytes()
    }

    fn mock_zip_name() -> String {
        ym_model::dist::release_zip_name(std::env::consts::OS, "9.9.9")
    }

    #[tokio::test]
    async fn mock_release_without_sums_is_rejected() {
        let zip_name = mock_zip_name();
        let (base, _hits) = spawn_mock(|base| {
            vec![
                ("/repos/o/r/releases/latest".to_owned(), mock_release_json(base, &[&zip_name])),
                (format!("/dl/{zip_name}"), b"ZIPBYTES".to_vec()),
            ]
        });

        let err = check_and_apply_at(&base, "o", "r", "1.0.0").await.unwrap_err();
        assert!(format!("{err:#}").contains("нет SHA256SUMS"), "ошибка должна называть отсутствующий ассет: {err:#}");
    }

    #[tokio::test]
    async fn mock_release_with_foreign_signature_is_rejected() {
        let zip_name = mock_zip_name();
        let zip = b"ZIPBYTES".to_vec();
        let (sums, sig_pem) = signed_sums(&zip, &zip_name, &soft_key(7));
        let (base, hits) = spawn_mock(|base| {
            vec![
                (
                    "/repos/o/r/releases/latest".to_owned(),
                    mock_release_json(base, &[&zip_name, SUMS_NAME, SUMS_SIG_NAME]),
                ),
                (format!("/dl/{zip_name}"), zip),
                (format!("/dl/{SUMS_NAME}"), sums),
                (format!("/dl/{SUMS_SIG_NAME}"), sig_pem),
            ]
        });

        let err = check_and_apply_at(&base, "o", "r", "1.0.0").await.unwrap_err();
        assert!(
            format!("{err:#}").contains("не соответствует ни одному доверенному ключу"),
            "чужая подпись должна отклоняться: {err:#}"
        );
        assert_eq!(*hits.lock().unwrap(), 4, "latest + zip + SUMS + sig скачаны до вердикта");
    }

    #[tokio::test]
    async fn mock_older_tag_short_circuits_before_downloads() {
        let zip_name = mock_zip_name();
        let (base, hits) = spawn_mock(|base| {
            vec![("/repos/o/r/releases/latest".to_owned(), mock_release_json(base, &[&zip_name]))]
        });

        let out = check_and_apply_at(&base, "o", "r", "99.0.0").await.unwrap();
        assert_eq!(out, None);
        assert_eq!(*hits.lock().unwrap(), 1, "при не-новом теге качается только latest");
    }

    #[tokio::test]
    async fn mock_replayed_release_is_rejected_before_downloads() {
        let replayed = ym_model::dist::release_zip_name(std::env::consts::OS, "2.4.0");
        let (base, hits) = spawn_mock(|base| {
            vec![
                (
                    "/repos/o/r/releases/latest".to_owned(),
                    mock_release_json(base, &[&replayed, SUMS_NAME, SUMS_SIG_NAME]),
                ),
                (format!("/dl/{replayed}"), b"OLDZIP".to_vec()),
            ]
        });

        let err = check_and_apply_at(&base, "o", "r", "1.0.0").await.unwrap_err();
        assert!(
            format!("{err:#}").contains("не соответствует тегу"),
            "replay старого имени под новым тегом должен отклоняться: {err:#}"
        );
        assert_eq!(*hits.lock().unwrap(), 1, "replay отсекается до единого скачивания");
    }

    #[test]
    fn sk_fixture_verifies_with_backup_key() {
        let sums = include_bytes!("../fixtures/SHA256SUMS.fixture");
        let sig = SshSig::from_pem(include_bytes!("../fixtures/SHA256SUMS.fixture.backup.sig")).unwrap();

        assert!(verify_sums_with(SIGNING_KEYS, sums, &sig).is_ok());

        let mut tampered = sums.to_vec();
        tampered[0] ^= 1;
        assert!(verify_sums_with(SIGNING_KEYS, &tampered, &sig).is_err());

        assert!(verify_sums_with(&[SIGNING_KEYS[0]], sums, &sig).is_err(), "подпись именно резервным, не основным");
    }

    #[test]
    fn missing_sums_assets_reject_update() {
        let rel = Release {
            tag: "v9.9.9".into(),
            assets: vec![Asset { name: "YandexMusicPlugin-darwin-9.9.9.zip".into(), url: "z".into() }],
            body: None,
            html_url: None,
        };
        assert!(required_asset(&rel, SUMS_NAME).is_err());
        assert!(required_asset(&rel, SUMS_SIG_NAME).is_err());
        assert_eq!(required_asset(&rel, "YandexMusicPlugin-darwin-9.9.9.zip").unwrap().url, "z");
    }

    fn dead_base() -> String {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        drop(l);
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn startup_retries_network_errors_until_github_answers() {
        let zip_name = mock_zip_name();
        let (live, hits) = spawn_mock(|base| {
            vec![("/repos/o/r/releases/latest".to_owned(), mock_release_json(base, &[&zip_name]))]
        });
        let dead = dead_base();
        let calls = AtomicUsize::new(0);
        let out = retry_transient(&[Duration::ZERO; 5], || {
            let base = if calls.fetch_add(1, Ordering::SeqCst) < 3 { dead.clone() } else { live.clone() };
            async move { check_and_apply_at(&base, "o", "r", "99.0.0").await }
        })
        .await
        .unwrap();
        assert_eq!(out, None);
        assert_eq!(calls.load(Ordering::SeqCst), 4, "три сетевых отказа, четвёртая попытка дошла до релиза");
        assert_eq!(*hits.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn after_network_returns_release_is_fetched_and_verified_once() {
        let zip_name = mock_zip_name();
        let zip = b"ZIPBYTES".to_vec();
        let (sums, sig_pem) = signed_sums(&zip, &zip_name, &soft_key(7));
        let (live, hits) = spawn_mock(|base| {
            vec![
                (
                    "/repos/o/r/releases/latest".to_owned(),
                    mock_release_json(base, &[&zip_name, SUMS_NAME, SUMS_SIG_NAME]),
                ),
                (format!("/dl/{zip_name}"), zip),
                (format!("/dl/{SUMS_NAME}"), sums),
                (format!("/dl/{SUMS_SIG_NAME}"), sig_pem),
            ]
        });
        let dead = dead_base();
        let calls = AtomicUsize::new(0);
        let err = retry_transient(&[Duration::ZERO; 5], || {
            let base = if calls.fetch_add(1, Ordering::SeqCst) < 2 { dead.clone() } else { live.clone() };
            async move { check_and_apply_at(&base, "o", "r", "1.0.0").await }
        })
        .await
        .unwrap_err();
        assert!(format!("{err:#}").contains("не соответствует ни одному доверенному ключу"), "{err:#}");
        assert_eq!(calls.load(Ordering::SeqCst), 3, "отказ подписи не сетевой — повторной загрузки нет");
        assert_eq!(*hits.lock().unwrap(), 4, "после появления сети релиз скачан целиком и дошёл до проверки подписи");
    }

    #[tokio::test]
    async fn startup_retries_give_up_after_schedule() {
        let dead = dead_base();
        let calls = AtomicUsize::new(0);
        let err = retry_transient(&[Duration::ZERO; 2], || {
            calls.fetch_add(1, Ordering::SeqCst);
            let base = dead.clone();
            async move { check_and_apply_at(&base, "o", "r", "1.0.0").await }
        })
        .await
        .unwrap_err();
        assert!(is_transient(&err), "{err:#}");
        assert_eq!(calls.load(Ordering::SeqCst), 3, "первая попытка и два повтора по графику");
    }

    #[test]
    fn rate_limit_wait_reads_retry_after_and_reset_only_for_limit_answers() {
        use reqwest::header::{HeaderMap, HeaderValue};
        let h = |pairs: &[(&'static str, &str)]| {
            let mut m = HeaderMap::new();
            for (k, v) in pairs {
                m.insert(*k, HeaderValue::from_str(v).unwrap());
            }
            m
        };
        let reset = h(&[("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "1030")]);
        assert_eq!(rate_limit_wait(403, &reset, 1000), Some(Duration::from_secs(31)));
        assert_eq!(rate_limit_wait(429, &h(&[("retry-after", "7")]), 1000), Some(Duration::from_secs(7)));
        assert_eq!(rate_limit_wait(403, &reset, 2000), Some(Duration::from_secs(1)), "сброс в прошлом — повтор через секунду");
        assert_eq!(rate_limit_wait(403, &h(&[]), 1000), None, "403 без заголовков лимита — настоящий отказ, не повторяем");
        assert_eq!(rate_limit_wait(403, &h(&[("x-ratelimit-remaining", "12"), ("x-ratelimit-reset", "1030")]), 1000), None);
        assert_eq!(rate_limit_wait(200, &h(&[("retry-after", "7")]), 1000), None);
    }

    #[tokio::test]
    async fn rate_limited_check_waits_for_reset_and_retries_instead_of_giving_up() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let body = mock_release_json(&base, &[]);
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let limited = format!(
            "HTTP/1.1 403 Forbidden\r\nx-ratelimit-remaining: 0\r\nx-ratelimit-reset: {now}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
        .into_bytes();
        let mut ok = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
        ok.extend_from_slice(&body);
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        std::thread::spawn(move || {
            for (stream, resp) in listener.incoming().zip([limited, ok]) {
                let Ok(mut s) = stream else { break };
                counter.fetch_add(1, Ordering::SeqCst);
                let mut buf = [0u8; 8192];
                let _ = s.read(&mut buf);
                let _ = s.write_all(&resp);
            }
        });
        let out = retry_transient(&[], || check_and_apply_at(&base, "o", "r", "99.0.0")).await;
        assert!(matches!(out, Ok(None)), "после сброса лимита проверка обязана повториться: {:?}", out.err());
        assert_eq!(hits.load(Ordering::SeqCst), 2, "403 с лимитом и повтор после сброса");
    }

    #[test]
    fn startup_run_goes_through_network_retry() {
        let src = include_str!("lib.rs");
        let run = &src[src.find("pub async fn run(").expect("нет run")..];
        let run = &run[..run.find("\n}\n").expect("конец run")];
        assert!(
            run.contains("retry_transient(&STARTUP_RETRY_DELAYS, || check_and_apply("),
            "автоустановка при старте обязана повторять сетевые отказы: без сети при входе в систему обновление молча пропадёт до перезапуска Stream Deck"
        );
    }
}
