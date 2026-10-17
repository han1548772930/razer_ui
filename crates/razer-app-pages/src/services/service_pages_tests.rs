use super::{MODULES, ModulePage};

#[test]
fn service_description_pending_keeps_the_primary_image_without_requesting_fallback() {
    let mut state = super::ServiceDescriptionImageState::default();
    let mut requested = Vec::new();
    let pending: Option<Result<(), ()>> = state.load(Some("primary"), "fallback", |alias| {
        requested.push(alias.to_owned());
        None
    });
    assert_eq!(pending, None);
    assert_eq!(requested, ["primary"]);
    let ready: Option<Result<_, ()>> = state.load(Some("primary"), "fallback", |alias| {
        requested.push(alias.to_owned());
        Some(Ok(42))
    });
    assert_eq!(ready, Some(Ok(42)));
    assert_eq!(requested, ["primary", "primary"]);
}

#[test]
fn service_description_failed_primary_stays_on_fallback_across_row_renders() {
    let mut state = super::ServiceDescriptionImageState::default();
    let mut requested = Vec::new();
    let first: Option<Result<_, &str>> = state.load(Some("primary"), "fallback", |alias| {
        requested.push(alias.to_owned());
        Some(if alias == "primary" { Err("decode") } else { Ok(7) })
    });
    assert_eq!(first, Some(Ok(7)));
    // The same row owner survives collapse and a later expanded image child.
    let next: Option<Result<_, &str>> = state.load(Some("changed-primary"), "fallback", |alias| {
        requested.push(alias.to_owned());
        Some(Ok(8))
    });
    assert_eq!(next, Some(Ok(8)));
    assert_eq!(requested, ["primary", "fallback", "fallback"]);
}

#[test]
fn service_description_fallback_preserves_pending_and_real_errors() {
    let mut state = super::ServiceDescriptionImageState::default();
    let pending: Option<Result<(), &str>> = state.load(None, "fallback", |_| None);
    assert_eq!(pending, None);
    let failed: Option<Result<(), &str>> = state.load(None, "fallback", |_| Some(Err("missing")));
    assert_eq!(failed, Some(Err("missing")));
}

#[test]
fn service_description_rejects_unverified_image_identity_and_origin() {
    for invalid in [
        "https://unverified.invalid/synapse/products/229/ui/229_0/PluginImages/229_0_0_dashboard1x.avif",
        "/synapse/products/229/ui/239_0/PluginImages/229_0_0_dashboard1x.avif",
        "/synapse/products/229/ui/229_0/PluginImages/239_0_0_dashboard1x.avif",
        "/synapse/products/229/ui/229_0/PluginImages/229_0_0_dashboard1x.png",
        "/synapse/products/229/ui/229_0/PluginImages/229_0_0_dashboard2x.avif",
        "/synapse/products/229/ui/229_0/PluginImages/229_0_0_extra_dashboard1x.avif",
        "/synapse/products/229/ui/229_0/PluginImages/229_0_0_dashboard1x.avif/extra",
    ] {
        assert_eq!(super::service_description_plugin_image(invalid), None, "{invalid}");
    }
}

#[test]
fn service_description_preserves_exact_product_edition_layout_and_density() {
    assert_eq!(
        super::service_description_plugin_image(
            "https://apps.razer.com/synapse/products/190/ui/190_128/PluginImages/190_128_0_dashboard3x.avif",
        ),
        razer_assets::dashboard_plugin_image(190, 128, 128, 0, false, 3),
    );
    assert!(super::service_description_plugin_image(
        "/synapse/products/190/ui/190_128/PluginImages/190_128_0_dashboard3x.avif",
    ).is_some());
    assert!(super::service_description_plugin_image(
        "/synapse/products/190/ui/190_999/PluginImages/190_999_0_dashboard3x.avif",
    ).is_none());
    assert!(super::service_description_plugin_image(
        "/synapse/products/190/ui/190_128/PluginImages/190_0_0_dashboard3x.avif",
    ).is_none());
}

#[test]
fn implemented_dashboard_modules_keep_their_named_host_tab_destinations() {
    let page_for = |id| {
        MODULES
            .iter()
            .find(|module| module.id == id)
            .and_then(|module| module.native_page)
    };

    assert_eq!(page_for("alexa"), None);
    assert_eq!(page_for("macro"), Some(ModulePage::Macro));
    assert_eq!(page_for("linked-games"), Some(ModulePage::Profiles));
    assert_eq!(page_for("armory"), None);
    assert_eq!(page_for("feedback"), None);
}
