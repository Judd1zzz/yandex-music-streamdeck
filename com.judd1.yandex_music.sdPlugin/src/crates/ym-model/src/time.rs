pub fn epoch_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn capture_local_offset() -> i32 {
    time::UtcOffset::current_local_offset().map(|o| o.whole_seconds()).unwrap_or(0)
}

pub fn day_index(epoch_secs: f64, offset_secs: i32) -> i64 {
    ((epoch_secs + offset_secs as f64) / 86400.0).floor() as i64
}

pub fn hour_of_day(epoch_secs: f64, offset_secs: i32) -> u8 {
    let local = epoch_secs + offset_secs as f64;
    (local.rem_euclid(86400.0) / 3600.0) as u8
}

pub fn weekday_from_day(day: i64) -> u8 {
    (day + 3).rem_euclid(7) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_secs_is_unix_seconds_scale() {
        let now = epoch_secs();
        assert!(
            (1.7e9..4e9).contains(&now),
            "ожидались unix-секунды (не миллисекунды), получено {now}"
        );
    }

    #[test]
    fn day_index_utc_new_year_2021() {
        assert_eq!(day_index(1_609_459_200.0, 0), 18628);
        assert_eq!(day_index(1_609_459_200.0 + 86_399.0, 0), 18628);
    }

    #[test]
    fn day_index_shifts_with_offset() {
        assert_eq!(day_index(1_609_459_200.0 - 3600.0, 10800), 18628);
        assert_eq!(day_index(1_609_459_200.0 - 3600.0, 0), 18627);
    }

    #[test]
    fn hour_of_day_utc_and_offset() {
        assert_eq!(hour_of_day(1_609_459_200.0 + 54_000.0, 0), 15);
        assert_eq!(hour_of_day(1_609_459_200.0, 10800), 3);
        assert_eq!(hour_of_day(1_609_459_200.0, 0), 0);
    }

    #[test]
    fn weekday_epoch_day_is_thursday() {
        assert_eq!(weekday_from_day(0), 3);
        assert_eq!(weekday_from_day(18628), 4);
        assert_eq!(weekday_from_day(18631), 0);
    }
}
