use std::process::Command;

use lofty::prelude::*;
use ym_download::ffmpeg::{convert, ffmpeg_path};
use ym_download::tag::{write_tags, TrackTags};

fn ffmpeg_ok() -> bool {
    Command::new(ffmpeg_path()).arg("-version").output().map(|o| o.status.success()).unwrap_or(false)
}

#[tokio::test]
async fn remux_lossless_and_transcode_mp3_then_tag() {
    if !ffmpeg_ok() {
        eprintln!("ffmpeg недоступен — тест пропущен");
        return;
    }
    let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/silence.m4a");
    let dir = std::env::temp_dir().join(format!("ym_dl_pipe_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let m4a = dir.join("out.m4a");
    convert(&input, &m4a, false).await.expect("remux m4a");
    assert!(m4a.exists() && std::fs::metadata(&m4a).unwrap().len() > 0);

    let mp3 = dir.join("out.mp3");
    convert(&input, &mp3, true).await.expect("transcode mp3");
    assert!(mp3.exists() && std::fs::metadata(&mp3).unwrap().len() > 0);

    let cover = vec![0xFF, 0xD8, 0xFF, 0xD9];
    for p in [&m4a, &mp3] {
        write_tags(
            p,
            &TrackTags {
                title: "TestTitle".to_owned(),
                artist: "TestArtist".to_owned(),
                album: Some("TestAlbum".to_owned()),
                year: Some(2021),
                genre: Some("rock".to_owned()),
                cover_jpeg: Some(cover.clone()),
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

    let _ = std::fs::remove_dir_all(&dir);
}
