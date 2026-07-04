use std::path::{Path, PathBuf};

use lofty::file::FileType;
use lofty::prelude::*;
use ym_download::convert::process;
use ym_download::decode::decode_all;
use ym_download::tag::{TrackTags, write_tags};

// Фикстуры сгенерированы вендореным минимальным ffmpeg (WAV-тишина 0.2с 44100 stereo):
//   silence.flac:     ffmpeg -i silence.wav -c:a flac silence.flac
//   silence_flac.mp4: ffmpeg -i silence.flac -c:a copy -strict -2 -f mp4 silence_flac.mp4
//   silence.mp3:      ffmpeg -i silence.wav -c:a libmp3lame -b:a 320k silence.mp3
//   silence.m4a:      ffmpeg -i silence.wav -c:a aac silence.m4a

fn fixture(name: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("фикстура {}: {e}", p.display()))
}

fn workdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ym_conv_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn tag_roundtrip(p: &Path) {
    let cover = vec![0xFF, 0xD8, 0xFF, 0xD9];
    write_tags(
        p,
        &TrackTags {
            title: "TestTitle".to_owned(),
            artist: "TestArtist".to_owned(),
            album: Some("TestAlbum".to_owned()),
            year: Some(2021),
            genre: Some("rock".to_owned()),
            cover_jpeg: Some(cover),
        },
    )
    .unwrap_or_else(|e| panic!("tag {}: {e}", p.display()));
    let f = lofty::read_from_path(p).unwrap();
    let t = f.primary_tag().expect("primary tag");
    assert_eq!(t.title().as_deref(), Some("TestTitle"));
    assert_eq!(t.artist().as_deref(), Some("TestArtist"));
    assert_eq!(t.album().as_deref(), Some("TestAlbum"));
    assert_eq!(t.year(), Some(2021));
}

#[tokio::test]
async fn flac_mp4_remuxes_to_native_flac_and_tags() {
    let dir = workdir("flacmp4");
    let out = process(fixture("silence_flac.mp4"), "flac-mp4", false, &dir).await.expect("remux");
    assert_eq!(out.extension().and_then(|e| e.to_str()), Some("flac"));
    let bytes = std::fs::read(&out).unwrap();
    assert_eq!(&bytes[..4], b"fLaC");

    let f = lofty::read_from_path(&out).unwrap();
    assert_eq!(f.file_type(), FileType::Flac);
    assert_eq!(f.properties().sample_rate(), Some(44100));

    tag_roundtrip(&out);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn m4a_passes_through_as_m4a_and_tags() {
    let dir = workdir("m4a");
    let src = fixture("silence.m4a");
    let out = process(src.clone(), "aac-mp4", false, &dir).await.expect("m4a");
    assert_eq!(out.extension().and_then(|e| e.to_str()), Some("m4a"));
    assert_eq!(std::fs::read(&out).unwrap(), src, "aac-mp4 сохраняется без изменений");
    tag_roundtrip(&out);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn native_flac_passes_through_byte_identical() {
    let dir = workdir("flac");
    let src = fixture("silence.flac");
    let out = process(src.clone(), "flac", false, &dir).await.expect("flac");
    assert_eq!(out.extension().and_then(|e| e.to_str()), Some("flac"));
    assert_eq!(std::fs::read(&out).unwrap(), src);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn m4a_and_flac_mp4_transcode_to_mp3_320() {
    for (name, hint, dur_ms) in
        [("silence.m4a", "aac-mp4", 700..1500), ("silence_flac.mp4", "flac-mp4", 100..600)]
    {
        let dir = workdir(&format!("mp3_{name}"));
        let out = process(fixture(name), hint, true, &dir).await.expect("transcode");
        assert_eq!(out.extension().and_then(|e| e.to_str()), Some("mp3"));

        let f = lofty::read_from_path(&out).unwrap();
        assert_eq!(f.file_type(), FileType::Mpeg, "вход {name}");
        let dur = f.properties().duration().as_millis();
        assert!(dur_ms.contains(&dur), "длительность фикстуры, получили {dur}мс ({name})");

        tag_roundtrip(&out);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[tokio::test]
async fn mp3_input_with_mp3_format_is_copied_verbatim() {
    let dir = workdir("mp3copy");
    let src = fixture("silence.mp3");
    let out = process(src.clone(), "mp3", true, &dir).await.expect("copy");
    assert_eq!(std::fs::read(&out).unwrap(), src, "mp3 не перекодируется повторно");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn decode_reads_flac_inside_mp4_directly() {
    let pcm = decode_all(fixture("silence_flac.mp4"), "mp4").expect("decode flac-in-mp4");
    assert_eq!(pcm.rate, 44100);
    assert_eq!(pcm.channels, 2);
    let frames = pcm.interleaved.len() / pcm.channels;
    assert!((6000..12000).contains(&frames), "~8820 фреймов тишины, получили {frames}");
    assert!(pcm.interleaved.iter().all(|s| s.abs() < 0.01), "тишина должна декодироваться в нули");
}
