pub const RELEASE_BASE: &str = "YandexMusicPlugin";

pub fn platform_tag(os: &str) -> &'static str {
    match os {
        "macos" => "darwin",
        "windows" => "windows",
        _ => "linux",
    }
}

pub fn release_zip_name(os: &str, version: &str) -> String {
    format!("{RELEASE_BASE}-{}-{version}.zip", platform_tag(os))
}

pub fn is_release_asset(name: &str, os: &str) -> bool {
    let needle = format!("-{}", platform_tag(os));
    name.starts_with(RELEASE_BASE) && name.contains(&needle) && name.ends_with(".zip")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_symmetric() {
        assert_eq!(release_zip_name("macos", "1.1.0"), "YandexMusicPlugin-darwin-1.1.0.zip");
        assert_eq!(release_zip_name("windows", "1.1.0"), "YandexMusicPlugin-windows-1.1.0.zip");
        assert_eq!(release_zip_name("linux", "2.0.0"), "YandexMusicPlugin-linux-2.0.0.zip");
    }

    #[test]
    fn matcher_accepts_what_producer_emits() {
        for os in ["macos", "windows", "linux"] {
            for v in ["1.1.0", "1.2.3", "10.0.0"] {
                assert!(is_release_asset(&release_zip_name(os, v), os));
            }
        }
    }

    #[test]
    fn matcher_rejects_wrong_platform_and_base() {
        assert!(!is_release_asset("YandexMusicPlugin-windows-1.1.0.zip", "macos"));
        assert!(!is_release_asset("YandexMusicPlugin-darwin-1.1.0.zip", "windows"));
        assert!(!is_release_asset("com.judd1.yandex_music.sdPlugin-darwin.zip", "macos"));
        assert!(!is_release_asset("YandexMusicPlugin-darwin-1.1.0.txt", "macos"));
    }
}
