mod elgato;
mod icons;

use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const PKG: &str = "ym-plugin";
const PLUGIN_DIR_NAME: &str = "com.judd1.yandex_music.sdPlugin";
const MAC_TARGETS: [&str; 2] = ["x86_64-apple-darwin", "aarch64-apple-darwin"];

fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref().unwrap_or("") {
        "dist" => dist(),
        "package" => package(std::env::args().nth(2).as_deref()),
        "store" => store(),
        "clean" => clean(),
        "elgato" => elgato::run_task(std::env::args().nth(2).as_deref()),
        other => {
            eprintln!("xtask: неизвестная задача {other:?}. Доступно: dist, package [os], store, clean, elgato");
            Ok(())
        }
    }
}

fn rust_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask внутри workspace").to_path_buf()
}

fn plugin_dir() -> PathBuf {
    rust_dir().parent().expect("src внутри пакета плагина").to_path_buf()
}

fn plugin_bin_dir() -> PathBuf {
    plugin_dir().join("bin")
}

fn repo_root() -> PathBuf {
    plugin_dir().parent().expect("пакет внутри репозитория").to_path_buf()
}

fn bin_filename(os: &str) -> &'static str {
    if os == "windows" {
        "ym-plugin.exe"
    } else {
        "ym-plugin"
    }
}

fn run(cmd: &mut Command) -> Result<()> {
    let status = cmd.status().with_context(|| format!("запуск {cmd:?}"))?;
    if !status.success() {
        bail!("команда завершилась с ошибкой ({status}): {cmd:?}");
    }
    Ok(())
}

fn sweep_days(arg: Option<&str>) -> u32 {
    arg.and_then(|s| s.trim().parse::<u32>().ok()).filter(|d| *d > 0).unwrap_or(7)
}

fn clean() -> Result<()> {
    let rust = rust_dir();
    let days = sweep_days(std::env::args().nth(2).as_deref());
    let has_sweep = Command::new("cargo")
        .args(["sweep", "--version"])
        .current_dir(&rust)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !has_sweep {
        println!("xtask: cargo-sweep не установлен — ничего не удалено.");
        println!("       Установите один раз: cargo install cargo-sweep, затем повторите.");
        return Ok(());
    }
    println!("xtask: cargo sweep --time {days} (удаляю артефакты старше {days} дн. в {})", rust.join("target").display());
    run(Command::new("cargo").current_dir(&rust).args(["sweep", "--time", &days.to_string()]))
}

fn dist() -> Result<()> {
    let rust = rust_dir();
    let bin_dir = plugin_bin_dir();
    std::fs::create_dir_all(&bin_dir).with_context(|| format!("создание {}", bin_dir.display()))?;

    if cfg!(target_os = "macos") {
        dist_macos(&rust, &bin_dir)
    } else if cfg!(target_os = "windows") {
        dist_copy(&rust, &bin_dir, "ym-plugin.exe")
    } else {
        dist_copy(&rust, &bin_dir, "ym-plugin")
    }
}

fn dist_macos(rust: &Path, bin_dir: &Path) -> Result<()> {
    for t in MAC_TARGETS {
        let _ = Command::new("rustup").args(["target", "add", t]).status();
        println!("xtask: release-сборка {PKG} для {t}");
        run(Command::new("cargo")
            .current_dir(rust)
            .args(["build", "--release", "-p", PKG, "--target", t]))?;
    }
    let out = bin_dir.join("ym-plugin");
    println!("xtask: lipo universal2 → {}", out.display());
    let mut lipo = Command::new("lipo");
    lipo.args(["-create", "-output"]).arg(&out);
    for t in MAC_TARGETS {
        lipo.arg(rust.join("target").join(t).join("release").join("ym-plugin"));
    }
    run(&mut lipo)?;
    make_executable(&out)?;
    println!("xtask: готов universal2 бинарь {}", out.display());
    Ok(())
}

fn dist_copy(rust: &Path, bin_dir: &Path, bin_name: &str) -> Result<()> {
    println!("xtask: release-сборка {PKG}");
    run(Command::new("cargo").current_dir(rust).args(["build", "--release", "-p", PKG]))?;
    let src = rust.join("target").join("release").join(bin_name);
    let dst = bin_dir.join(bin_name);
    std::fs::copy(&src, &dst).with_context(|| format!("копирование {} → {}", src.display(), dst.display()))?;
    make_executable(&dst)?;
    println!("xtask: готов {}", dst.display());
    Ok(())
}

fn package(os_arg: Option<&str>) -> Result<()> {
    let os = match os_arg {
        Some(os) if ["windows", "macos", "linux"].contains(&os) => os.to_owned(),
        Some(other) => bail!("xtask package: неизвестная ОС {other:?} (windows|macos|linux)"),
        None => {
            dist()?;
            std::env::consts::OS.to_owned()
        }
    };
    let bin = bin_filename(&os);
    let bin_path = plugin_bin_dir().join(bin);
    if !bin_path.is_file() {
        bail!("нет бинаря {}: соберите его (xtask dist или кросс-сборкой) и повторите", bin_path.display());
    }

    let release = repo_root().join("release");
    std::fs::create_dir_all(&release).with_context(|| format!("создание {}", release.display()))?;
    let zip_path = release.join(ym_model::dist::release_zip_name(&os, env!("CARGO_PKG_VERSION")));

    let entries = write_release_zip(&zip_path, &plugin_dir(), &[bin])?;
    print_package_summary(&format!("{} — {os}", plugin_display_name(&plugin_dir())), &zip_path, &entries);
    println!("xtask: релиз → {}", zip_path.display());
    Ok(())
}

fn store() -> Result<()> {
    let plugin = plugin_dir();
    for bin in ["ym-plugin", "ym-plugin.exe"] {
        let p = plugin.join("bin").join(bin);
        if !p.is_file() {
            bail!("для store-пакета нужны оба бинаря; отсутствует {}", p.display());
        }
    }
    let release = repo_root().join("release");
    std::fs::create_dir_all(&release).with_context(|| format!("создание {}", release.display()))?;
    let out = release.join(PLUGIN_DIR_NAME);
    let entries = write_release_zip(&out, &plugin, &["ym-plugin", "ym-plugin.exe"])?;
    print_package_summary(&format!("{} — StreamDock Store", plugin_display_name(&plugin)), &out, &entries);
    println!("xtask: store-пакет (zip с обёрткой, оба бинаря) → {}", out.display());
    Ok(())
}

fn write_release_zip(zip_path: &Path, plugin: &Path, bins: &[&str]) -> Result<Vec<(String, u64)>> {
    let file = File::create(zip_path).with_context(|| format!("создание {}", zip_path.display()))?;
    let mut zw = ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let exec_opts = opts.unix_permissions(0o755);
    let mut entries = Vec::new();

    add_file(&mut zw, &plugin.join("manifest.json"), &format!("{PLUGIN_DIR_NAME}/manifest.json"), opts, &mut entries)?;
    add_tree(&mut zw, &plugin.join("static"), &format!("{PLUGIN_DIR_NAME}/static"), opts, &mut entries)?;

    for bin in bins {
        add_file(&mut zw, &plugin.join("bin").join(bin), &format!("{PLUGIN_DIR_NAME}/bin/{bin}"), exec_opts, &mut entries)?;
    }

    zw.finish().context("финализация zip")?;
    Ok(entries)
}

fn human_bytes(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1} MB", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1} kB", n as f64 / 1_000.0)
    } else {
        format!("{n} B")
    }
}

fn plugin_display_name(plugin: &Path) -> String {
    std::fs::read_to_string(plugin.join("manifest.json"))
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|m| m.get("Name").and_then(|n| n.as_str()).map(str::to_owned))
        .unwrap_or_else(|| "ym-plugin".to_owned())
}

fn print_package_summary(title: &str, zip_path: &Path, entries: &[(String, u64)]) {
    println!();
    println!("📦 {title} (v{})", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Содержимое пакета");
    let width = entries.iter().map(|(_, s)| human_bytes(*s).len()).max().unwrap_or(0);
    for (i, (name, size)) in entries.iter().enumerate() {
        let branch = if i + 1 == entries.len() { "└─" } else { "├─" };
        println!("{branch}  {:>width$}  {name}", human_bytes(*size));
    }
    println!();
    println!("Детали пакета");
    println!("  Всего файлов:          {}", entries.len());
    println!("  Распакованный размер:  {}", human_bytes(entries.iter().map(|(_, s)| s).sum()));
    let packed = std::fs::metadata(zip_path).map(|m| m.len()).unwrap_or(0);
    println!("  Размер архива:         {}", human_bytes(packed));
    println!("  Файл:                  {}", zip_path.display());
    println!();
}

fn add_file(
    zw: &mut ZipWriter<File>,
    src: &Path,
    name: &str,
    opts: SimpleFileOptions,
    entries: &mut Vec<(String, u64)>,
) -> Result<()> {
    let data = std::fs::read(src).with_context(|| format!("чтение {}", src.display()))?;
    zw.start_file(name, opts).with_context(|| format!("zip start_file {name}"))?;
    zw.write_all(&data).with_context(|| format!("zip write {name}"))?;
    entries.push((name.to_owned(), data.len() as u64));
    Ok(())
}

fn is_runtime_only_icon(name: &str) -> bool {
    name.starts_with("btn_") || name == "emptiness_black.png"
}

fn add_tree(
    zw: &mut ZipWriter<File>,
    src_dir: &Path,
    prefix: &str,
    opts: SimpleFileOptions,
    entries: &mut Vec<(String, u64)>,
) -> Result<()> {
    let mut dir_entries: Vec<_> = std::fs::read_dir(src_dir)
        .with_context(|| format!("чтение {}", src_dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    dir_entries.sort_by_key(|e| e.file_name());
    for entry in dir_entries {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') {
            continue;
        }
        let ft = entry.file_type()?;
        if ft.is_file() && is_runtime_only_icon(&name_str) {
            continue;
        }
        if ft.is_dir() && name_str == "elgato" && prefix.ends_with("/static/img") {
            continue;
        }
        let entry_name = format!("{prefix}/{name_str}");
        if ft.is_dir() {
            add_tree(zw, &entry.path(), &entry_name, opts, entries)?;
        } else {
            add_file(zw, &entry.path(), &entry_name, opts, entries)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::io::Read;

    #[test]
    fn human_bytes_units() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(6_100), "6.1 kB");
        assert_eq!(human_bytes(8_939_248), "8.9 MB");
    }

    #[test]
    fn bin_names() {
        assert_eq!(bin_filename("macos"), "ym-plugin");
        assert_eq!(bin_filename("windows"), "ym-plugin.exe");
        assert_eq!(bin_filename("linux"), "ym-plugin");
    }

    #[test]
    fn sweep_days_default_and_parse() {
        assert_eq!(sweep_days(None), 7);
        assert_eq!(sweep_days(Some("14")), 14);
        assert_eq!(sweep_days(Some(" 3 ")), 3);
        assert_eq!(sweep_days(Some("0")), 7);
        assert_eq!(sweep_days(Some("x")), 7);
    }

    #[test]
    fn release_zip_has_clean_layout_and_skips_dotfiles() -> Result<()> {
        let tmp = std::env::temp_dir().join(format!("xtask_pkg_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let plugin = tmp.join("plugin");
        std::fs::create_dir_all(plugin.join("static").join("sub"))?;
        std::fs::create_dir_all(plugin.join("bin"))?;
        std::fs::write(plugin.join("manifest.json"), b"{}")?;
        std::fs::write(plugin.join("static").join("icon.png"), b"png")?;
        std::fs::write(plugin.join("static").join("sub").join("inspector.js"), b"js")?;
        std::fs::write(plugin.join("static").join(".DS_Store"), b"junk")?;
        std::fs::write(plugin.join("bin").join("ym-plugin"), b"binary")?;

        let zip_path = tmp.join("out.zip");
        write_release_zip(&zip_path, &plugin, &["ym-plugin"])?;

        let mut archive = zip::ZipArchive::new(File::open(&zip_path)?)?;
        let names: HashSet<String> = archive.file_names().map(String::from).collect();
        let p = PLUGIN_DIR_NAME;
        assert!(names.contains(&format!("{p}/manifest.json")), "нет manifest");
        assert!(names.contains(&format!("{p}/static/icon.png")), "нет static/icon.png");
        assert!(names.contains(&format!("{p}/static/sub/inspector.js")), "нет вложенного static");
        assert!(names.contains(&format!("{p}/bin/ym-plugin")), "нет bin/ym-plugin");
        assert!(!names.iter().any(|n| n.contains("DS_Store")), "dotfiles должны пропускаться");

        let mut f = archive.by_name(&format!("{p}/manifest.json"))?;
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        assert_eq!(s, "{}");
        drop(f);

        std::fs::remove_dir_all(&tmp).ok();
        Ok(())
    }

    #[test]
    fn release_zip_excludes_runtime_only_icons() -> Result<()> {
        let tmp = std::env::temp_dir().join(format!("xtask_icons_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let plugin = tmp.join("plugin");
        let img = plugin.join("static").join("img");
        std::fs::create_dir_all(&img)?;
        std::fs::create_dir_all(plugin.join("bin"))?;
        std::fs::write(plugin.join("manifest.json"), b"{}")?;
        for f in [
            "btn_yandex_music_next_v1.png",
            "emptiness_black.png",
            "emptiness.png",
            "yandex_music_next.png",
            "ym.png",
        ] {
            std::fs::write(img.join(f), b"png")?;
        }
        std::fs::create_dir_all(img.join("elgato"))?;
        std::fs::write(img.join("elgato").join("plugin-icon.png"), b"png")?;
        std::fs::write(plugin.join("bin").join("ym-plugin"), b"binary")?;

        let zip_path = tmp.join("out.zip");
        write_release_zip(&zip_path, &plugin, &["ym-plugin"])?;

        let archive = zip::ZipArchive::new(File::open(&zip_path)?)?;
        let names: HashSet<String> = archive.file_names().map(String::from).collect();
        let p = PLUGIN_DIR_NAME;
        for host in ["emptiness.png", "yandex_music_next.png", "ym.png"] {
            assert!(names.contains(&format!("{p}/static/img/{host}")), "host-иконка {host} должна быть в zip");
        }
        for runtime in ["btn_yandex_music_next_v1.png", "emptiness_black.png"] {
            assert!(!names.iter().any(|n| n.ends_with(runtime)), "рантайм-иконка {runtime} не должна попадать в zip");
        }
        assert!(
            !names.iter().any(|n| n.contains("/img/elgato/")),
            "elgato-набор иконок не должен попадать в StreamDock/GitHub-zip"
        );

        std::fs::remove_dir_all(&tmp).ok();
        Ok(())
    }

    #[test]
    fn release_zip_binary_is_last_entry() -> Result<()> {
        let tmp = std::env::temp_dir().join(format!("xtask_order_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let plugin = tmp.join("plugin");
        std::fs::create_dir_all(plugin.join("static"))?;
        std::fs::create_dir_all(plugin.join("bin"))?;
        std::fs::write(plugin.join("manifest.json"), b"{}")?;
        std::fs::write(plugin.join("static").join("icon.png"), b"png")?;
        std::fs::write(plugin.join("bin").join("ym-plugin"), b"binary")?;

        let zip_path = tmp.join("out.zip");
        write_release_zip(&zip_path, &plugin, &["ym-plugin"])?;

        let mut archive = zip::ZipArchive::new(File::open(&zip_path)?)?;
        let mut ordered = Vec::new();
        for i in 0..archive.len() {
            ordered.push(archive.by_index(i)?.name().to_owned());
        }
        let p = PLUGIN_DIR_NAME;
        assert_eq!(ordered.last().unwrap(), &format!("{p}/bin/ym-plugin"), "бинарь — commit point, строго последняя запись");
        assert!(
            !ordered.iter().any(|n| n.contains("ffmpeg")),
            "ffmpeg больше не поставляется в релизе"
        );

        std::fs::remove_dir_all(&tmp).ok();
        Ok(())
    }

    #[test]
    fn store_zip_contains_both_binaries_wrapped() -> Result<()> {
        let tmp = std::env::temp_dir().join(format!("xtask_store_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let plugin = tmp.join("plugin");
        std::fs::create_dir_all(plugin.join("static"))?;
        std::fs::create_dir_all(plugin.join("bin"))?;
        std::fs::write(plugin.join("manifest.json"), b"{}")?;
        std::fs::write(plugin.join("static").join("icon.png"), b"png")?;
        std::fs::write(plugin.join("static").join(".DS_Store"), b"junk")?;
        std::fs::write(plugin.join("bin").join("ym-plugin"), b"mac")?;
        std::fs::write(plugin.join("bin").join("ym-plugin.exe"), b"win")?;

        let zip_path = tmp.join("com.judd1.yandex_music.sdPlugin");
        write_release_zip(&zip_path, &plugin, &["ym-plugin", "ym-plugin.exe"])?;

        let archive = zip::ZipArchive::new(File::open(&zip_path)?)?;
        let names: HashSet<String> = archive.file_names().map(String::from).collect();
        let p = PLUGIN_DIR_NAME;
        assert!(names.contains(&format!("{p}/manifest.json")));
        assert!(names.contains(&format!("{p}/bin/ym-plugin")));
        assert!(names.contains(&format!("{p}/bin/ym-plugin.exe")));
        assert!(names.iter().all(|n| n.starts_with(&format!("{p}/"))), "все записи внутри папки-обёртки");
        assert!(!names.iter().any(|n| n.contains("DS_Store")));

        std::fs::remove_dir_all(&tmp).ok();
        Ok(())
    }

    #[test]
    fn manifest_version_matches_workspace_version() -> Result<()> {
        let manifest = std::fs::read_to_string(plugin_dir().join("manifest.json"))?;
        let v: serde_json::Value = serde_json::from_str(&manifest)?;
        assert_eq!(
            v["Version"].as_str().unwrap_or_default(),
            env!("CARGO_PKG_VERSION"),
            "Version в manifest.json обязан совпадать с workspace-версией"
        );
        Ok(())
    }
}
