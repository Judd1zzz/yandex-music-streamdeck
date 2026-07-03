use std::path::{Path, PathBuf};

use tokio::process::Command;

pub fn ffmpeg_path() -> PathBuf {
    if let Ok(p) = std::env::var("YM_FFMPEG") {
        let p = p.trim();
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    let name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let bundled = dir.join(name);
        if bundled.exists() {
            return bundled;
        }
    }
    PathBuf::from(name)
}

pub async fn convert(input: &Path, output: &Path, mp3: bool) -> Result<(), String> {
    let ff = ffmpeg_path();
    let mut cmd = Command::new(&ff);
    cmd.arg("-y").arg("-loglevel").arg("error").arg("-i").arg(input).arg("-map").arg("0:a").arg("-vn");
    if mp3 {
        cmd.args(["-c:a", "libmp3lame", "-b:a", "320k"]);
    } else {
        cmd.args(["-c:a", "copy"]);
    }
    cmd.arg(output);
    let out = cmd.output().await.map_err(|e| format!("не удалось запустить ffmpeg ({}): {e}", ff.display()))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let tail: Vec<&str> = err.lines().rev().take(4).collect();
        let tail: String = tail.into_iter().rev().collect::<Vec<_>>().join(" | ");
        return Err(format!("ffmpeg завершился с ошибкой ({}): {tail}", out.status));
    }
    Ok(())
}
