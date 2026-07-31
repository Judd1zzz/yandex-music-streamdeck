pub const RELEASE_BASE: &str = "YandexMusicPlugin";

pub const MARKETPLACE_MARKER: &str = "window.__YM_NODL_FLAG__=true;";

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

pub fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sums_line(hex: &str, name: &str) -> String {
    format!("{hex}  {name}\n")
}

pub fn find_sum<'a>(sums: &'a str, name: &str) -> Option<&'a str> {
    sums.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        match (parts.next(), parts.next()) {
            (Some(hex), Some(n)) if n == name => Some(hex),
            _ => None,
        }
    })
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

    #[test]
    fn find_sum_tolerates_crlf() {
        let name = "YandexMusicPlugin-darwin-2.4.0.zip";
        let hex = "c".repeat(64);
        let sums = format!("{hex}  {name}\r\n");
        assert_eq!(find_sum(&sums, name), Some(hex.as_str()));
    }

    #[test]
    fn hex_lower_encodes() {
        assert_eq!(hex_lower(&[0x00, 0xff, 0x1a]), "00ff1a");
    }

    #[test]
    fn sums_roundtrip() {
        let mac = release_zip_name("macos", "2.4.0");
        let win = release_zip_name("windows", "2.4.0");
        let hex_mac = "a".repeat(64);
        let hex_win = "b".repeat(64);
        let sums = format!("{}{}", sums_line(&hex_mac, &mac), sums_line(&hex_win, &win));
        assert_eq!(find_sum(&sums, &mac), Some(hex_mac.as_str()));
        assert_eq!(find_sum(&sums, &win), Some(hex_win.as_str()));
        assert!(find_sum(&sums, &release_zip_name("linux", "2.4.0")).is_none());
        assert!(find_sum("", &mac).is_none());
    }
}
