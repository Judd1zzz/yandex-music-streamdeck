use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use crate::{is_runtime_only_icon, make_executable, plugin_dir, repo_root, run, rust_dir, MAC_TARGETS, PKG};

pub const ELGATO_UUID: &str = "com.judd1.yandex-music";
pub const ELGATO_DIR_NAME: &str = "com.judd1.yandex-music.sdPlugin";
const OVERLAY: &str = include_str!("../assets/elgato.json");
const EXCLUDED_SUFFIXES: [&str; 2] = [".action.download", ".action.volume_knob"];
const EXPECTED_ACTIONS: usize = 11;

pub fn run_task(sub: Option<&str>) -> Result<()> {
    match sub.unwrap_or("all") {
        "icons" => crate::icons::generate(&plugin_dir()),
        "stage" => stage().map(|_| ()),
        "pack" => pack(),
        "validate" => validate(),
        "all" => {
            crate::icons::generate(&plugin_dir())?;
            stage()?;
            validate()?;
            pack()
        }
        other => {
            bail!("xtask elgato: неизвестная подзадача {other:?}. Доступно: icons, stage, pack, validate, all")
        }
    }
}

fn uuid_charset_ok(uuid: &str) -> bool {
    !uuid.is_empty()
        && uuid.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
}

pub fn to_four_segments(version: &str) -> Result<String> {
    let parts: Vec<&str> = version.trim().split('.').collect();
    if !(3..=4).contains(&parts.len()) || parts.iter().any(|p| p.parse::<u64>().is_err()) {
        bail!("версия {version:?} не похожа на 3-4 числовых сегмента");
    }
    if parts.len() == 4 {
        return Ok(parts.join("."));
    }
    Ok(format!("{}.0", parts.join(".")))
}

fn icon_basename(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_owned()
}

pub fn transform_manifest(src: &Value, overlay: &Value, version: &str) -> Result<Value> {
    let mut m = src.clone();
    let obj = m.as_object_mut().context("manifest не объект")?;

    let actions = obj
        .get("Actions")
        .and_then(Value::as_array)
        .context("manifest без Actions")?
        .iter()
        .filter(|a| {
            let uuid = a.get("UUID").and_then(Value::as_str).unwrap_or_default();
            !EXCLUDED_SUFFIXES.iter().any(|s| uuid.ends_with(s))
        })
        .cloned()
        .map(|mut a| {
            let ao = a.as_object_mut().context("action не объект")?;
            let uuid = ao.get("UUID").and_then(Value::as_str).context("action без UUID")?.replace('_', "-");
            ao.insert("UUID".into(), json!(uuid));
            ao.insert("Controllers".into(), json!(["Keypad"]));
            ao.remove("UserTitleEnabled");
            ao.remove("Settings");
            if let Some(icon) = ao.get("Icon").and_then(Value::as_str) {
                let base = icon_basename(icon);
                ao.insert("Icon".into(), json!(format!("static/img/elgato/{base}")));
            }
            if let Some(states) = ao.get_mut("States").and_then(Value::as_array_mut) {
                for st in states {
                    let Some(so) = st.as_object_mut() else { continue };
                    if so.contains_key("Image") {
                        so.insert("Image".into(), json!("static/img/elgato/key-empty"));
                    }
                    if let Some(fs) = so.get("FontSize").and_then(Value::as_str)
                        && let Ok(n) = fs.trim().parse::<u32>()
                    {
                        so.insert("FontSize".into(), json!(n));
                    }
                }
            }
            Ok(a)
        })
        .collect::<Result<Vec<Value>>>()?;
    obj.insert("Actions".into(), Value::Array(actions));

    for key in ["Name", "Description", "Category", "Software", "OS"] {
        let v = overlay.get(key).with_context(|| format!("в оверлее нет {key}"))?;
        obj.insert(key.into(), v.clone());
    }
    obj.insert("UUID".into(), json!(ELGATO_UUID));
    obj.insert("SDKVersion".into(), json!(3));
    obj.insert("Icon".into(), json!("static/img/elgato/plugin-icon"));
    obj.insert("CategoryIcon".into(), json!("static/img/elgato/category-icon"));
    obj.insert("CodePath".into(), json!("bin/ym-plugin"));
    obj.insert("CodePathWin".into(), json!("bin/ym-plugin.exe"));
    obj.remove("CodePathMac");
    obj.insert("Version".into(), json!(to_four_segments(version)?));

    let actions = m["Actions"].as_array().expect("Actions только что записаны");
    if actions.len() != EXPECTED_ACTIONS {
        bail!("ожидалось {EXPECTED_ACTIONS} действий, получилось {}", actions.len());
    }
    for a in actions {
        let uuid = a["UUID"].as_str().unwrap_or_default();
        if !uuid_charset_ok(uuid) || !uuid.starts_with(ELGATO_UUID) {
            bail!("невалидный UUID действия для Elgato: {uuid:?}");
        }
    }
    Ok(m)
}

pub fn overlay() -> Result<Value> {
    serde_json::from_str(OVERLAY).context("разбор assets/elgato.json")
}

fn staging_dir() -> PathBuf {
    repo_root().join("release").join("elgato").join(ELGATO_DIR_NAME)
}

fn win_exe_drop() -> PathBuf {
    repo_root().join("release").join("elgato-in").join("ym-plugin.exe")
}

fn win_exe_cross_target() -> PathBuf {
    rust_dir().join("target").join("x86_64-pc-windows-gnu").join("release").join("ym-plugin.exe")
}

const MARKETPLACE_MARKER: &[u8] = b"__ymNoDownloadUi=true;";

pub fn binary_has_marketplace_marker(bytes: &[u8]) -> bool {
    bytes.windows(MARKETPLACE_MARKER.len()).any(|w| w == MARKETPLACE_MARKER)
}

fn pick_win_exe() -> Result<PathBuf> {
    let drop = win_exe_drop();
    let cross = win_exe_cross_target();
    let src = if drop.is_file() {
        drop
    } else if cross.is_file() {
        cross
    } else {
        bail!(
            "нет Windows-бинаря для Elgato: соберите кроссом (cargo build --release -p ym-plugin --target x86_64-pc-windows-gnu --no-default-features) или положите no-default сборку в {}",
            win_exe_drop().display()
        );
    };
    let bytes = std::fs::read(&src).with_context(|| format!("чтение {}", src.display()))?;
    if !binary_has_marketplace_marker(&bytes) {
        bail!(
            "{} собран с полным набором фич (download/self-update) — для Elgato пересоберите с --no-default-features",
            src.display()
        );
    }
    Ok(src)
}

fn copy_tree(src_dir: &Path, dst_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dst_dir).with_context(|| format!("создание {}", dst_dir.display()))?;
    let in_img_root = src_dir.ends_with("static/img");
    let mut entries: Vec<_> = std::fs::read_dir(src_dir)
        .with_context(|| format!("чтение {}", src_dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name();
        let name_str = name.to_string_lossy().into_owned();
        if name_str.starts_with('.') {
            continue;
        }
        let ft = entry.file_type()?;
        if ft.is_file() && (is_runtime_only_icon(&name_str) || in_img_root) {
            continue;
        }
        if ft.is_dir() {
            copy_tree(&entry.path(), &dst_dir.join(&name_str))?;
        } else {
            std::fs::copy(entry.path(), dst_dir.join(&name_str))
                .with_context(|| format!("копирование {}", entry.path().display()))?;
        }
    }
    Ok(())
}

fn build_mac_universal(staging_bin: &Path) -> Result<()> {
    let rust = rust_dir();
    for t in MAC_TARGETS {
        let _ = Command::new("rustup").args(["target", "add", t]).status();
        println!("xtask: elgato release-сборка {PKG} (--no-default-features) для {t}");
        run(Command::new("cargo").current_dir(&rust).args([
            "build",
            "--release",
            "-p",
            PKG,
            "--no-default-features",
            "--target",
            t,
        ]))?;
    }
    let out = staging_bin.join("ym-plugin");
    let mut lipo = Command::new("lipo");
    lipo.args(["-create", "-output"]).arg(&out);
    for t in MAC_TARGETS {
        lipo.arg(rust.join("target").join(t).join("release").join("ym-plugin"));
    }
    run(&mut lipo)?;
    make_executable(&out)?;
    println!("xtask: codesign ad-hoc {}", out.display());
    run(Command::new("codesign").args(["-s", "-", "--force"]).arg(&out))?;
    Ok(())
}

pub fn stage() -> Result<PathBuf> {
    if !cfg!(target_os = "macos") {
        bail!("elgato stage собирается на macOS (universal binary + codesign)");
    }
    let plugin = plugin_dir();
    let staging = staging_dir();
    let _ = std::fs::remove_dir_all(&staging);
    let bin_dir = staging.join("bin");
    std::fs::create_dir_all(&bin_dir).with_context(|| format!("создание {}", bin_dir.display()))?;

    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(plugin.join("manifest.json")).context("чтение manifest.json")?,
    )
    .context("разбор manifest.json")?;
    let transformed = transform_manifest(&manifest, &overlay()?, env!("CARGO_PKG_VERSION"))?;
    std::fs::write(staging.join("manifest.json"), serde_json::to_string_pretty(&transformed)?)
        .context("запись elgato manifest.json")?;

    copy_tree(&plugin.join("static"), &staging.join("static"))?;

    build_mac_universal(&bin_dir)?;

    let win_src = pick_win_exe()?;
    println!("xtask: windows-бинарь для Elgato ← {}", win_src.display());
    std::fs::copy(&win_src, bin_dir.join("ym-plugin.exe")).context("копирование ym-plugin.exe")?;

    println!("xtask: elgato staging → {}", staging.display());
    Ok(staging)
}

fn npx_cli(args: &[&str]) -> Result<()> {
    let mut cmd = Command::new("npx");
    cmd.args(["--yes", "@elgato/cli@latest"]).args(args);
    run(&mut cmd)
}

pub fn scrub_ds_store(dir: &Path) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("чтение {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            scrub_ds_store(&path)?;
        } else if entry.file_name().to_string_lossy() == ".DS_Store" {
            std::fs::remove_file(&path).with_context(|| format!("удаление {}", path.display()))?;
            println!("xtask: удалён Finder-мусор {}", path.display());
        }
    }
    Ok(())
}

pub fn validate() -> Result<()> {
    let staging = staging_dir();
    if !staging.is_dir() {
        bail!("staging не собран: сначала cargo run -p xtask -- elgato stage");
    }
    scrub_ds_store(&staging)?;
    npx_cli(&["validate", &staging.to_string_lossy()])
}

pub fn pack() -> Result<()> {
    let staging = staging_dir();
    if !staging.is_dir() {
        bail!("staging не собран: сначала cargo run -p xtask -- elgato stage");
    }
    scrub_ds_store(&staging)?;
    let out_dir = repo_root().join("release").join("elgato");
    npx_cli(&["pack", &staging.to_string_lossy(), "--output", &out_dir.to_string_lossy(), "--force"])?;
    println!("xtask: elgato-пакет в {}", out_dir.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real_manifest() -> Value {
        let raw = std::fs::read_to_string(plugin_dir().join("manifest.json")).unwrap();
        serde_json::from_str(&raw).unwrap()
    }

    #[test]
    fn copy_tree_drops_legacy_top_level_img_but_keeps_elgato_set() -> anyhow::Result<()> {
        let tmp = std::env::temp_dir().join(format!("elgato_copy_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let src = tmp.join("static");
        std::fs::create_dir_all(src.join("img").join("elgato"))?;
        std::fs::create_dir_all(src.join("css"))?;
        std::fs::write(src.join("property_inspector.html"), b"html")?;
        std::fs::write(src.join("css").join("inspector.css"), b"css")?;
        std::fs::write(src.join("img").join("ym.png"), b"legacy")?;
        std::fs::write(src.join("img").join("yandex_music_like.png"), b"legacy")?;
        std::fs::write(src.join("img").join("btn_yandex_music_next_v1.png"), b"runtime")?;
        std::fs::write(src.join("img").join("elgato").join("plugin-icon.png"), b"elgato")?;

        let dst = tmp.join("staging_static");
        copy_tree(&src, &dst)?;

        assert!(dst.join("property_inspector.html").is_file());
        assert!(dst.join("css").join("inspector.css").is_file());
        assert!(dst.join("img").join("elgato").join("plugin-icon.png").is_file());
        assert!(!dst.join("img").join("ym.png").exists(), "легаси-иконки не должны попадать в Elgato-staging");
        assert!(!dst.join("img").join("yandex_music_like.png").exists());
        assert!(!dst.join("img").join("btn_yandex_music_next_v1.png").exists());
        std::fs::remove_dir_all(&tmp).ok();
        Ok(())
    }

    #[test]
    fn marketplace_marker_detection() {
        let with_marker = [b"prefix ".as_slice(), MARKETPLACE_MARKER, b" suffix"].concat();
        assert!(binary_has_marketplace_marker(&with_marker));
        assert!(!binary_has_marketplace_marker(b"ordinary full-featured binary contents"));
        assert!(!binary_has_marketplace_marker(b"__ymNoDownloadUi=false;"));
    }

    #[test]
    fn scrub_removes_ds_store_recursively() -> anyhow::Result<()> {
        let tmp = std::env::temp_dir().join(format!("elgato_scrub_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("static/img"))?;
        std::fs::write(tmp.join(".DS_Store"), b"junk")?;
        std::fs::write(tmp.join("static/.DS_Store"), b"junk")?;
        std::fs::write(tmp.join("static/img/.DS_Store"), b"junk")?;
        std::fs::write(tmp.join("manifest.json"), b"{}")?;

        scrub_ds_store(&tmp)?;

        assert!(!tmp.join(".DS_Store").exists());
        assert!(!tmp.join("static/.DS_Store").exists());
        assert!(!tmp.join("static/img/.DS_Store").exists());
        assert!(tmp.join("manifest.json").exists());
        std::fs::remove_dir_all(&tmp).ok();
        Ok(())
    }

    #[test]
    fn four_segment_versions() {
        assert_eq!(to_four_segments("2.2.0").unwrap(), "2.2.0.0");
        assert_eq!(to_four_segments("2.2.0.7").unwrap(), "2.2.0.7");
        assert!(to_four_segments("2.2").is_err());
        assert!(to_four_segments("2.2.x").is_err());
    }

    #[test]
    fn transform_real_manifest_meets_elgato_rules() {
        let out = transform_manifest(&real_manifest(), &overlay().unwrap(), "2.2.0").unwrap();

        assert_eq!(out["UUID"], ELGATO_UUID);
        assert_eq!(out["Version"], "2.2.0.0");
        assert_eq!(out["Name"], "Yandex Music Integration");
        assert_eq!(out["Category"], "Yandex Music Integration");
        assert_eq!(out["Software"]["MinimumVersion"], "6.9");
        assert_eq!(out["SDKVersion"], 3);
        assert_eq!(out["Icon"], "static/img/elgato/plugin-icon");
        assert_eq!(out["CategoryIcon"], "static/img/elgato/category-icon");
        assert_eq!(out["CodePath"], "bin/ym-plugin");
        assert_eq!(out["CodePathWin"], "bin/ym-plugin.exe");
        assert!(out.get("CodePathMac").is_none());
        assert_eq!(out["OS"][0]["MinimumVersion"], "12");

        let actions = out["Actions"].as_array().unwrap();
        assert_eq!(actions.len(), EXPECTED_ACTIONS);
        for a in actions {
            let uuid = a["UUID"].as_str().unwrap();
            assert!(uuid_charset_ok(uuid), "{uuid}");
            assert!(!uuid.contains('_'), "{uuid}");
            assert_eq!(a["Controllers"], json!(["Keypad"]));
            assert!(a.get("UserTitleEnabled").is_none());
            assert!(a.get("Settings").is_none());
            assert!(a["Icon"].as_str().unwrap().starts_with("static/img/elgato/"), "{:?}", a["Icon"]);
            for st in a["States"].as_array().unwrap() {
                assert_eq!(st["Image"], "static/img/elgato/key-empty");
                if let Some(fs) = st.get("FontSize") {
                    assert!(fs.is_number(), "FontSize должен стать числом, получили {fs:?}");
                }
            }
        }
        let uuids: Vec<&str> = actions.iter().map(|a| a["UUID"].as_str().unwrap()).collect();
        assert!(!uuids.iter().any(|u| u.ends_with(".download") || u.ends_with(".volume-knob")));
        assert!(uuids.contains(&"com.judd1.yandex-music.action.volume-display"));
        assert!(uuids.contains(&"com.judd1.yandex-music.action.playpause"));
    }

    #[test]
    fn transform_uses_workspace_version() {
        let out =
            transform_manifest(&real_manifest(), &overlay().unwrap(), env!("CARGO_PKG_VERSION")).unwrap();
        let want = format!("{}.0", env!("CARGO_PKG_VERSION"));
        assert_eq!(out["Version"].as_str().unwrap(), want);
    }

    #[test]
    fn action_icon_paths_map_to_generated_names() {
        let out = transform_manifest(&real_manifest(), &overlay().unwrap(), "2.2.0").unwrap();
        for a in out["Actions"].as_array().unwrap() {
            let icon = a["Icon"].as_str().unwrap();
            let base = icon.rsplit('/').next().unwrap();
            assert!(
                crate::icons::ACTION_ICONS.contains(&base) || base == "yandex_music_info",
                "иконка {base} должна генерироваться в elgato-наборе"
            );
        }
    }
}
