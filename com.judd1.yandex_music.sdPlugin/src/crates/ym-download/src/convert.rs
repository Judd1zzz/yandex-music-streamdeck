use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

use crate::{decode, flac_remux, mp3_encode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    Mp4,
    Flac,
    Mp3,
    Adts,
    Unknown,
}

pub fn sniff(bytes: &[u8]) -> Container {
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        return Container::Mp4;
    }
    if bytes.starts_with(b"fLaC") {
        return Container::Flac;
    }
    if bytes.starts_with(b"ID3") {
        return Container::Mp3;
    }
    if bytes.len() >= 2 && bytes[0] == 0xFF {
        if bytes[1] & 0xF6 == 0xF0 {
            return Container::Adts;
        }
        if bytes[1] & 0xE0 == 0xE0 {
            return Container::Mp3;
        }
    }
    Container::Unknown
}

fn decode_hint(c: Container) -> &'static str {
    match c {
        Container::Mp4 => "mp4",
        Container::Flac => "flac",
        Container::Adts => "aac",
        Container::Mp3 => "mp3",
        Container::Unknown => "",
    }
}

fn unknown_input_err() -> anyhow::Error {
    anyhow!("не удалось распознать скачанный файл — вероятно, он повреждён или не расшифрован")
}

pub async fn process(bytes: Vec<u8>, codec_hint: &str, want_mp3: bool, work: &Path) -> Result<PathBuf> {
    let work = work.to_path_buf();
    let hint = codec_hint.to_owned();
    tokio::task::spawn_blocking(move || process_sync(bytes, &hint, want_mp3, &work))
        .await
        .map_err(|e| anyhow!("конвертация прервана: {e}"))?
}

fn process_sync(bytes: Vec<u8>, codec_hint: &str, want_mp3: bool, work: &Path) -> Result<PathBuf> {
    let container = sniff(&bytes);
    if want_mp3 {
        return match container {
            Container::Mp3 => write_out(work, "mp3", &bytes),
            Container::Unknown => Err(unknown_input_err()),
            _ if codec_hint.trim().to_ascii_lowercase().starts_with("he-aac") => Err(anyhow!(
                "трек доступен только в HE-AAC, конвертация в MP3 невозможна — скачайте в формате Lossless"
            )),
            other => {
                let pcm = decode::decode_all(bytes, decode_hint(other))?;
                let mp3 = mp3_encode::encode_320(&pcm)?;
                write_out(work, "mp3", &mp3)
            }
        };
    }
    match container {
        Container::Flac => write_out(work, "flac", &bytes),
        Container::Mp3 => write_out(work, "mp3", &bytes),
        Container::Adts => write_out(work, "aac", &bytes),
        Container::Mp4 => {
            let m4a = write_out(work, "m4a", &bytes)?;
            match flac_remux::flac_from_mp4_file(&m4a)? {
                Some(flac) => {
                    let out = write_out(work, "flac", &flac)?;
                    let _ = std::fs::remove_file(&m4a);
                    Ok(out)
                }
                None => Ok(m4a),
            }
        }
        Container::Unknown => Err(unknown_input_err()),
    }
}

fn write_out(work: &Path, ext: &str, data: &[u8]) -> Result<PathBuf> {
    let path = work.join(format!("out.{ext}"));
    std::fs::write(&path, data)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniff_detects_containers_by_magic() {
        let mut mp4 = vec![0x00, 0x00, 0x00, 0x20];
        mp4.extend_from_slice(b"ftypisom");
        mp4.extend_from_slice(&[0; 8]);
        assert_eq!(sniff(&mp4), Container::Mp4);

        assert_eq!(sniff(b"fLaC\x00\x00\x00\x22"), Container::Flac);
        assert_eq!(sniff(b"ID3\x04\x00rest"), Container::Mp3);
        assert_eq!(sniff(&[0xFF, 0xFB, 0x90, 0x00]), Container::Mp3);
        assert_eq!(sniff(&[0xFF, 0xF1, 0x50, 0x80]), Container::Adts);
        assert_eq!(sniff(&[0xFF, 0xF9, 0x50, 0x80]), Container::Adts);
        assert_eq!(sniff(b"\x00\x01\x02\x03garbage"), Container::Unknown);
        assert_eq!(sniff(b""), Container::Unknown);
    }

    #[tokio::test]
    async fn he_aac_to_mp3_is_rejected_explicitly() {
        let dir = tempfile::tempdir().unwrap();
        let mut mp4 = vec![0x00, 0x00, 0x00, 0x20];
        mp4.extend_from_slice(b"ftypisom");
        mp4.extend_from_slice(&[0; 8]);
        let err = process(mp4, "he-aac-mp4", true, dir.path()).await.unwrap_err();
        assert!(err.to_string().contains("HE-AAC"), "{err}");
    }

    #[tokio::test]
    async fn garbage_input_fails_without_panic() {
        let dir = tempfile::tempdir().unwrap();
        let garbage = vec![0x11u8; 256];
        assert!(process(garbage.clone(), "flac", false, dir.path()).await.is_err());
        assert!(process(garbage, "flac", true, dir.path()).await.is_err());
    }
}
