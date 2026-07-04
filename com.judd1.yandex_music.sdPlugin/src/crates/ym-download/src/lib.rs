pub mod convert;
pub mod crypto;
pub mod decode;
pub mod flac_remux;
pub mod mp3_encode;
pub mod naming;
pub mod sign;
pub mod tag;

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde_json::Value;

const BASE: &str = "https://api.music.yandex.net/";
const CLIENT_HEADER: &str = "YandexMusicDesktopAppWindows/5.85.0";
const CODECS: [&str; 7] = ["flac", "aac", "he-aac", "mp3", "flac-mp4", "aac-mp4", "he-aac-mp4"];
const TRANSPORTS: [&str; 1] = ["encraw"];
const QUALITY: &str = "lossless";

struct DownloadInfo {
    url: String,
    codec: String,
    transport: String,
    key: String,
}

struct TrackMeta {
    title: String,
    artists: Vec<String>,
    album: Option<String>,
    year: Option<u32>,
    genre: Option<String>,
    cover_uri: Option<String>,
}

fn unix_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn unwrap_result(v: &Value) -> &Value {
    v.get("result").unwrap_or(v)
}

fn headers(rb: reqwest::RequestBuilder, token: &str) -> reqwest::RequestBuilder {
    rb.header("Authorization", format!("OAuth {token}"))
        .header("x-yandex-music-client", CLIENT_HEADER)
        .header("accept-language", "ru")
        .header("x-yandex-music-without-invocation-info", "1")
}

fn parse_meta(v: &Value) -> TrackMeta {
    let artists = v
        .get("artists")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.get("name").and_then(Value::as_str).map(str::to_owned)).collect())
        .unwrap_or_default();
    let album0 = v.get("albums").and_then(Value::as_array).and_then(|a| a.first());
    TrackMeta {
        title: v.get("title").and_then(Value::as_str).unwrap_or("Unknown").to_owned(),
        artists,
        album: album0.and_then(|a| a.get("title")).and_then(Value::as_str).map(str::to_owned),
        year: album0.and_then(|a| a.get("year")).and_then(Value::as_u64).map(|y| y as u32),
        genre: album0.and_then(|a| a.get("genre")).and_then(Value::as_str).map(str::to_owned),
        cover_uri: v.get("coverUri").and_then(Value::as_str).map(str::to_owned),
    }
}

fn parse_download_info(v: &Value) -> Result<DownloadInfo> {
    let di = unwrap_result(v).get("downloadInfo").cloned().ok_or_else(|| anyhow!("no downloadInfo in response"))?;
    let url = di.get("url").and_then(Value::as_str).unwrap_or_default().to_owned();
    if url.is_empty() {
        return Err(anyhow!("empty download url"));
    }
    Ok(DownloadInfo {
        url,
        codec: di.get("codec").and_then(Value::as_str).unwrap_or("mp3").to_owned(),
        transport: di.get("transport").and_then(Value::as_str).unwrap_or_default().to_owned(),
        key: di.get("key").and_then(Value::as_str).unwrap_or_default().to_owned(),
    })
}

async fn get_file_info(client: &reqwest::Client, base: &str, token: &str, track_id: &str) -> Result<DownloadInfo> {
    let ts = unix_secs();
    let sign_str = sign::file_info_sign_str(ts, track_id, QUALITY, &CODECS, &TRANSPORTS);
    let sg = sign::sign(sign::SECRET, &sign_str);
    let (ts_s, codecs_s, transports_s) = (ts.to_string(), CODECS.join(","), TRANSPORTS.join(","));
    let resp = headers(client.get(format!("{base}get-file-info")), token)
        .query(&[
            ("trackId", track_id),
            ("ts", ts_s.as_str()),
            ("quality", QUALITY),
            ("codecs", codecs_s.as_str()),
            ("transports", transports_s.as_str()),
            ("sign", sg.as_str()),
        ])
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    parse_download_info(&resp)
}

async fn get_track_meta(client: &reqwest::Client, base: &str, token: &str, track_id: &str) -> Result<TrackMeta> {
    let form = reqwest::multipart::Form::new()
        .text("trackIds", track_id.to_owned())
        .text("removeDuplicates", "false")
        .text("withProgress", "false");
    let resp = headers(client.post(format!("{base}tracks")), token)
        .multipart(form)
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    let first = unwrap_result(&resp).as_array().and_then(|a| a.first()).ok_or_else(|| anyhow!("empty tracks meta"))?;
    Ok(parse_meta(first))
}

async fn fetch_cover(client: &reqwest::Client, cover_uri: Option<&str>) -> Option<Vec<u8>> {
    let uri = cover_uri?.trim();
    if uri.is_empty() {
        return None;
    }
    let url = format!("https://{}", uri.replace("%%", "400x400"));
    let bytes = client.get(url).send().await.ok()?.error_for_status().ok()?.bytes().await.ok()?;
    Some(bytes.to_vec())
}

pub fn resolve_dir(setting: &str) -> PathBuf {
    let s = setting.trim();
    if !s.is_empty() {
        return PathBuf::from(s);
    }
    let home = std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).ok();
    match home.filter(|h| !h.is_empty()) {
        Some(h) => Path::new(&h).join("Music"),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

fn make_workdir(track_id: &str) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("ym_dl_{}_{}", std::process::id(), naming::sanitize_filename(track_id)));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

async fn move_file(from: &Path, to: &Path) -> Result<()> {
    if tokio::fs::rename(from, to).await.is_ok() {
        return Ok(());
    }
    tokio::fs::copy(from, to).await?;
    let _ = tokio::fs::remove_file(from).await;
    Ok(())
}

pub fn is_mp3_format(format: &str) -> bool {
    format.trim().eq_ignore_ascii_case("mp3")
}

async fn download_track_with_base(base: &str, track_id: &str, token: &str, dest_dir: &Path, mp3: bool) -> Result<PathBuf> {
    let client = reqwest::Client::new();
    let info = get_file_info(&client, base, token, track_id).await?;
    let meta = get_track_meta(&client, base, token, track_id).await?;

    let mut bytes = client.get(&info.url).send().await?.error_for_status()?.bytes().await?.to_vec();
    if info.transport == "encraw" {
        crypto::decrypt_ctr(&info.key, &mut bytes).map_err(|e| anyhow!("decrypt: {e}"))?;
    }

    let work = make_workdir(track_id)?;
    let out = convert::process(bytes, &info.codec, mp3, &work).await?;
    let ext = out.extension().and_then(|e| e.to_str()).unwrap_or("bin").to_owned();
    let artists = naming::artists_to_string(&meta.artists);
    let filename = naming::track_filename(&artists, &meta.title, &ext);

    let cover = fetch_cover(&client, meta.cover_uri.as_deref()).await;
    let tags = tag::TrackTags {
        title: meta.title,
        artist: artists,
        album: meta.album,
        year: meta.year,
        genre: meta.genre,
        cover_jpeg: cover,
    };
    if let Err(e) = tag::write_tags(&out, &tags) {
        tracing::warn!("tagging failed for {}: {e}", out.display());
    }

    tokio::fs::create_dir_all(dest_dir).await?;
    let final_path = dest_dir.join(&filename);
    move_file(&out, &final_path).await?;
    let _ = tokio::fs::remove_dir_all(&work).await;
    Ok(final_path)
}

pub async fn download_track(track_id: &str, token: &str, dest_dir: &Path, format: &str) -> Result<PathBuf> {
    download_track_with_base(BASE, track_id, token, dest_dir, is_mp3_format(format)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_meta_extracts_fields() {
        let v = json!({
            "title": "Faded",
            "artists": [{"name": "Alan Walker"}, {"name": "Iselin"}],
            "albums": [{"title": "Different World", "year": 2018, "genre": "electronic"}],
            "coverUri": "avatars.yandex.net/x/%%"
        });
        let m = parse_meta(&v);
        assert_eq!(m.title, "Faded");
        assert_eq!(m.artists, vec!["Alan Walker".to_owned(), "Iselin".to_owned()]);
        assert_eq!(m.album.as_deref(), Some("Different World"));
        assert_eq!(m.year, Some(2018));
        assert_eq!(m.genre.as_deref(), Some("electronic"));
        assert_eq!(m.cover_uri.as_deref(), Some("avatars.yandex.net/x/%%"));
    }

    #[test]
    fn parse_download_info_unwraps_result_and_validates_url() {
        let direct = json!({"downloadInfo": {"url": "https://x/f", "codec": "flac", "transport": "encraw", "key": "ab"}});
        let di = parse_download_info(&direct).unwrap();
        assert_eq!(di.url, "https://x/f");
        assert_eq!(di.codec, "flac");
        assert_eq!(di.transport, "encraw");
        assert_eq!(di.key, "ab");

        let wrapped = json!({"result": {"downloadInfo": {"url": "https://y/f", "codec": "mp3"}}});
        assert_eq!(parse_download_info(&wrapped).unwrap().url, "https://y/f");

        assert!(parse_download_info(&json!({"downloadInfo": {"url": ""}})).is_err());
        assert!(parse_download_info(&json!({})).is_err());
    }

    #[test]
    fn resolve_dir_prefers_setting_then_home_music() {
        assert_eq!(resolve_dir("  /custom/path  "), PathBuf::from("/custom/path"));
        let dir = resolve_dir("");
        assert!(dir.ends_with("Music") || dir.is_absolute() || dir == Path::new("."));
    }

    #[test]
    fn mp3_format_detection() {
        assert!(is_mp3_format("mp3"));
        assert!(is_mp3_format(" MP3 "));
        assert!(!is_mp3_format("lossless"));
        assert!(!is_mp3_format(""));
    }
}
