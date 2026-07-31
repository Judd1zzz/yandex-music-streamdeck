#[cfg(feature = "downloads")]
#[test]
fn injected_api_keeps_download_ui_with_downloads_feature() {
    assert!(!ym_cdp::INJECTED_API_JS.starts_with(ym_model::dist::MARKETPLACE_MARKER));
    assert!(ym_cdp::INJECTED_API_JS.contains("_updateDownloadButton"));
}

#[cfg(not(feature = "downloads"))]
#[test]
fn injected_api_disables_download_ui_without_downloads_feature() {
    assert!(ym_cdp::INJECTED_API_JS.starts_with(ym_model::dist::MARKETPLACE_MARKER));

    let js = ym_cdp::JsHandles::random().injected_api();
    assert!(
        !js.contains("__YM_NODL_FLAG__") && !js.contains("__ymNoDownloadUi"),
        "флаг обязан быть рандомизирован, иначе чужой плагин выключит фичу извне"
    );
}
