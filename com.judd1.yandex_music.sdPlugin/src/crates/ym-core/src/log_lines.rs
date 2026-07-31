#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLineParts {
    pub at: Option<String>,
    pub level: Option<String>,
    pub text: String,
}

const LEVELS: [&str; 5] = ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"];

fn timestamp_secs(ts: &str) -> Option<u32> {
    let b = ts.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' || b[b.len() - 1] != b'Z' {
        return None;
    }
    let num = |s: &str| s.parse::<u32>().ok();
    let hh = num(ts.get(11..13)?)?;
    let mm = num(ts.get(14..16)?)?;
    let ss = num(ts.get(17..19)?)?;
    if hh > 23 || mm > 59 || ss > 59 {
        return None;
    }
    Some(hh * 3600 + mm * 60 + ss)
}

fn local_hms(utc_secs: u32, offset_secs: i32) -> String {
    let local = (utc_secs as i64 + offset_secs as i64).rem_euclid(86400) as u32;
    format!("{:02}:{:02}:{:02}", local / 3600, local % 3600 / 60, local % 60)
}

pub fn parse_ring_line(line: &str, offset_secs: i32) -> LogLineParts {
    let Some(sp) = line.find(' ') else {
        return LogLineParts { at: None, level: None, text: line.to_owned() };
    };
    let Some(secs) = timestamp_secs(&line[..sp]) else {
        return LogLineParts { at: None, level: None, text: line.to_owned() };
    };
    let at = Some(local_hms(secs, offset_secs));
    let rest = line[sp..].trim_start();
    let token = rest.split(' ').next().unwrap_or("");
    if LEVELS.contains(&token) {
        LogLineParts {
            at,
            level: Some(token.to_ascii_lowercase()),
            text: rest[token.len()..].trim_start().to_owned(),
        }
    } else {
        LogLineParts { at, level: None, text: rest.to_owned() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_line_with_double_space_alignment() {
        let p = parse_ring_line("2026-07-24T12:34:56.789012Z  INFO Plugin Start (ym-plugin 2.4.0)", 10800);
        assert_eq!(p.at.as_deref(), Some("15:34:56"));
        assert_eq!(p.level.as_deref(), Some("info"));
        assert_eq!(p.text, "Plugin Start (ym-plugin 2.4.0)");
    }

    #[test]
    fn warn_and_error_alignment() {
        let w = parse_ring_line("2026-07-24T00:00:01.000000Z  WARN reconnecting", 0);
        assert_eq!(w.level.as_deref(), Some("warn"));
        assert_eq!(w.text, "reconnecting");
        let e = parse_ring_line("2026-07-24T00:00:01.000000Z ERROR boom happened", 0);
        assert_eq!(e.level.as_deref(), Some("error"));
        assert_eq!(e.text, "boom happened");
    }

    #[test]
    fn offset_wraps_around_midnight() {
        let p = parse_ring_line("2026-07-24T23:30:00.000000Z  INFO late", 10800);
        assert_eq!(p.at.as_deref(), Some("02:30:00"));
        let n = parse_ring_line("2026-07-24T01:30:00.000000Z  INFO early", -10800);
        assert_eq!(n.at.as_deref(), Some("22:30:00"));
    }

    #[test]
    fn line_without_timestamp_falls_back_whole() {
        let p = parse_ring_line("plain message without meta", 10800);
        assert_eq!(p, LogLineParts { at: None, level: None, text: "plain message without meta".to_owned() });
        let z = parse_ring_line("random Z text", 0);
        assert_eq!(z.at, None);
        assert_eq!(z.text, "random Z text");
    }

    #[test]
    fn timestamp_without_level_keeps_at_only() {
        let p = parse_ring_line("2026-07-24T10:00:00.000000Z something odd", 0);
        assert_eq!(p.at.as_deref(), Some("10:00:00"));
        assert_eq!(p.level, None);
        assert_eq!(p.text, "something odd");
    }

    #[test]
    fn level_word_inside_text_not_confused() {
        let p = parse_ring_line("2026-07-24T10:00:00.000000Z  INFO INFO is a word", 0);
        assert_eq!(p.level.as_deref(), Some("info"));
        assert_eq!(p.text, "INFO is a word");
    }

    #[test]
    fn empty_and_single_word_lines() {
        assert_eq!(parse_ring_line("", 0).text, "");
        assert_eq!(parse_ring_line("one-token", 0), LogLineParts { at: None, level: None, text: "one-token".to_owned() });
    }
}
