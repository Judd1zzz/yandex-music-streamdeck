use std::process::Command;

#[test]
fn binary_logs_to_file_never_stdout_and_exits_clean() {
    let dir = tempfile::tempdir().unwrap();
    let lock = dir.path().join("lock.pid");
    let cdp_free = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        p
    };

    let out = Command::new(env!("CARGO_BIN_EXE_ym-plugin"))
        .env("YM_LOG_DIR", dir.path())
        .env("YM_LOCK_PATH", &lock)
        .env("YM_HOST_BACKOFF_MS", "10")
        .env("YM_CDP_PORT", cdp_free.to_string())
        .args([
            "-port", "28196",
            "-pluginUUID", "ABC123",
            "-registerEvent", "registerPlugin",
            "-info", "{}",
        ])
        .output()
        .expect("запуск бинаря");

    assert!(out.status.success(), "ненулевой код выхода: {:?}", out.status);
    assert!(
        out.stdout.is_empty(),
        "stdout должен быть пуст (Stream Deck резервирует его), а там: {:?}",
        String::from_utf8_lossy(&out.stdout)
    );

    let log = std::fs::read_to_string(dir.path().join("plugin.log")).expect("plugin.log");
    assert!(log.contains("Plugin Start"), "нет 'Plugin Start' в логе");
    assert!(log.contains("registration args parsed"), "нет разбора argv в логе");
}
