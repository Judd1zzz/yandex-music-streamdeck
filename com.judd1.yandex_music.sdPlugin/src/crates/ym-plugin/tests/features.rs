#[cfg(feature = "downloads")]
#[test]
fn injected_api_keeps_download_ui_with_downloads_feature() {
    assert!(!ym_cdp::INJECTED_API_JS.starts_with("window.__ymNoDownloadUi"));
    assert!(ym_cdp::INJECTED_API_JS.contains("_updateDownloadButton"));
}

#[cfg(not(feature = "downloads"))]
#[test]
fn injected_api_disables_download_ui_without_downloads_feature() {
    assert!(ym_cdp::INJECTED_API_JS.starts_with("window.__ymNoDownloadUi=true;"));
}
