pub fn epoch_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
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
}
