#[cfg(feature = "downloads")]
#[test]
fn remote_bundle_keeps_download_ui_with_downloads_feature() {
    let m = ym_remote_controller::remote_module();
    assert!(!m.source.starts_with(ym_model::dist::MARKETPLACE_MARKER));
    assert!(m.kinds.contains(&"download"), "фича downloads обязана дотянуться до JS-пульта");
}

#[cfg(not(feature = "downloads"))]
#[test]
fn remote_bundle_drops_download_ui_without_downloads_feature() {
    let m = ym_remote_controller::remote_module();
    assert!(
        m.source.starts_with(ym_model::dist::MARKETPLACE_MARKER),
        "маркетплейс-сборка обязана нести маркер, который xtask ищет внутри .exe"
    );
    assert!(!m.kinds.contains(&"download"));
    assert!(!m.source.contains("ym-dl-btn"));
}
