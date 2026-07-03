pub fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| if matches!(c, '/' | '\\' | '?' | '%' | '*' | ':' | '|' | '"' | '<' | '>') { '_' } else { c })
        .collect()
}

pub fn ext_from_codec(codec: &str) -> String {
    let s = codec.replace("he-aac", "m4a").replace("aac", "m4a");
    s.strip_suffix("-mp4").unwrap_or(&s).to_owned()
}

pub fn artists_to_string(names: &[String]) -> String {
    names.iter().map(|n| n.trim()).filter(|n| !n.is_empty()).collect::<Vec<_>>().join(" & ")
}

pub fn track_filename(artists: &str, title: &str, ext: &str) -> String {
    let (a, t) = (artists.trim(), title.trim());
    let base = match (a.is_empty(), t.is_empty()) {
        (false, false) => format!("{a} — {t}"),
        (true, false) => t.to_owned(),
        (false, true) => a.to_owned(),
        (true, true) => "track".to_owned(),
    };
    format!("{}.{ext}", sanitize_filename(&base))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_to_extension() {
        assert_eq!(ext_from_codec("flac"), "flac");
        assert_eq!(ext_from_codec("flac-mp4"), "flac");
        assert_eq!(ext_from_codec("aac"), "m4a");
        assert_eq!(ext_from_codec("he-aac"), "m4a");
        assert_eq!(ext_from_codec("aac-mp4"), "m4a");
        assert_eq!(ext_from_codec("he-aac-mp4"), "m4a");
        assert_eq!(ext_from_codec("mp3"), "mp3");
    }

    #[test]
    fn artists_joined_with_ampersand() {
        assert_eq!(artists_to_string(&["A".into(), "B".into(), "C".into()]), "A & B & C");
        assert_eq!(artists_to_string(&["Solo".into()]), "Solo");
        assert_eq!(artists_to_string(&[]), "");
    }

    #[test]
    fn filename_sanitized_and_formatted() {
        assert_eq!(track_filename("AC/DC", "T:N?", "flac"), "AC_DC — T_N_.flac");
        assert_eq!(track_filename("", "Only Title", "m4a"), "Only Title.m4a");
        assert_eq!(track_filename("Artist", "", "mp3"), "Artist.mp3");
        assert_eq!(track_filename("", "", "flac"), "track.flac");
    }
}
