//! Product resources are fetched individually and reused from the installation cache.
use gpui_kit::{AssetSource, SharedString};
use std::borrow::Cow;
pub struct SynapseAssets;
mod data;
mod keyboard_source;
pub use keyboard_source::{
    InputCounter, KeyboardSourceGroup, KeyboardSourceInput, keyboard_source_for_layout,
};
mod packed;
mod remote;
pub use remote::{
    ResourceChanged, ResourcePending, is_resource_pending, observe_resources, resource_errors,
    resource_generation, resource_image_cache, retry_resources, start_resources,
};

/// Request one original file by its manifest path, without hydrating its product.
pub fn load_path(path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
    remote::load_path(path).map(|bytes| bytes.map(Cow::Owned))
}
impl SynapseAssets {
    pub fn validate(&self) -> anyhow::Result<()> {
        packed::validate()
    }
}
impl AssetSource for SynapseAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if path.starts_with("assets/") {
            return load_path(path);
        }
        if let Some(bytes) = packed::load(path)? {
            return Ok(Some(Cow::Owned(bytes)));
        }
        gpui_kit::assets::AllAssets.load(path)
    }
    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        let mut items = gpui_kit::assets::AllAssets.list(path)?;
        items.extend(remote::list(path)?.into_iter().map(SharedString::from));
        Ok(items)
    }
}
/// Register cached fonts immediately; missing fonts are requested in the background.
pub fn register_fonts(cx: &gpui_kit::App) -> anyhow::Result<()> {
    let mut fonts = Vec::new();
    for font in &remote::manifest()?.fonts {
        if let Some(bytes) = remote::cached_or_request(font)? {
            fonts.push(Cow::Owned(bytes));
        }
    }
    if !fonts.is_empty() {
        cx.text_system().add_fonts(fonts)?;
    }
    Ok(())
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DeviceImage {
    Product,
    MouseBottom,
    KeyboardWrist,
    KeyboardDial,
    Streamer,
}

type ProductAsset = (u32, u32, u32, DeviceImage, &'static str);
const PRODUCT_IMAGES: &[ProductAsset] =
    include!("../../../assets/synapse/tables/product-images.rs");
const ARMORY_PRODUCT_IMAGES: &[ProductAsset] =
    include!("../../../assets/synapse/tables/armory-product-images.rs");

/// Dashboard and pairing cards use the original PluginImages artwork. An
/// unavailable edition/layout stays unavailable instead of changing its color
/// or substituting Customize artwork. Old keyboard data defaults to layout one.
pub fn dashboard_image(pid: u32, edition_id: u32, layout_id: u32) -> Option<&'static str> {
    const IMAGES: &[(u32, u32, u32, &str)] =
        include!("../../../assets/synapse/tables/dashboard-images.rs");
    const ARMORY_IMAGES: &[(u32, u32, u32, &str)] =
        include!("../../../assets/synapse/tables/armory-dashboard-images.rs");
    let layout_id = match pid {
        653 => layout_id.max(1),
        // Current DualLinkKeyboard's PluginImages path contains the real
        // keyboard layout. An explicit zero has a genuine 404; caller defaults
        // only an absent layout to one, as useYn does in the current UI.
        756 => layout_id,
        _ => 0,
    };
    IMAGES
        .iter()
        .chain(ARMORY_IMAGES)
        .find_map(|&(product, edition, layout, asset)| {
            (product == pid && edition == edition_id && layout == layout_id).then_some(asset)
        })
}

type DashboardPluginAsset = (u32, u32, u32, u32, bool, u32, &'static str);
static CURRENT_DASHBOARD_IMAGES: &[DashboardPluginAsset] =
    include!("../../../assets/synapse/tables/current-dashboard-images.rs");

/// Resolves an exact local PluginImages request without changing its edition,
/// layout, wrist variant or density. Standalone pairing image producers have
/// no Dashboard card retry, so a missing request stays unavailable.
pub fn dashboard_plugin_image(
    pid: u32,
    edition_id: u32,
    filename_edition_id: u32,
    layout_id: u32,
    wrist: bool,
    density: u32,
) -> Option<&'static str> {
    CURRENT_DASHBOARD_IMAGES.iter().find_map(
        |&(product, edition, image_edition, layout, image_wrist, image_density, asset)| {
            (product == pid
                && edition == edition_id
                && image_edition == filename_edition_id
                && layout == layout_id
                && image_wrist == wrist
                && image_density == density)
                .then_some(asset)
        },
    )
}

/// Exact current App.tt requests, with BoxItemDevice.onBoxImageError's final
/// edition-zero/layout-zero/1x retry. This is specific to Dashboard cards;
/// Customize, device keys and standalone pairing retain their own call chains.
pub fn dashboard_card_image(
    pid: u32,
    edition_id: u32,
    filename_edition_id: u32,
    layout_id: Option<u32>,
    wrist: bool,
    scale_factor: f32,
) -> Option<&'static str> {
    let density = if scale_factor > 1. { 3 } else { 1 };
    let lookup = |edition, filename_edition, layout, wrist, density| {
        dashboard_plugin_image(pid, edition, filename_edition, layout, wrist, density)
    };
    layout_id
        .and_then(|layout| lookup(edition_id, filename_edition_id, layout, wrist, density))
        .or_else(|| lookup(0, 0, 0, false, 1))
}

/// Current standalone `Me()` uses the exact category layout and edition.
/// Failed explicit editions retain the original unavailable/placeholder branch.
pub fn pairing_product_image(pid: u32, edition_id: u32, layout_id: u32) -> Option<&'static str> {
    const IMAGES: &[(u32, u32, u32, &str)] =
        include!("../../../assets/synapse/tables/multipairing-product-images.rs");
    IMAGES
        .iter()
        .find_map(|&(product, edition, layout, asset)| {
            (product == pid && edition == edition_id && layout == layout_id).then_some(asset)
        })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedDeviceImage {
    pub asset: &'static str,
    /// The edition and layout of the image that was actually selected.
    pub edition_id: u32,
    pub layout_id: u32,
    /// Whether an image is a preview rather than an exact supported layout.
    pub is_fallback_preview: bool,
}

pub fn device_image(
    pid: u32,
    edition_id: u32,
    layout_id: u32,
    part: DeviceImage,
) -> Option<&'static str> {
    resolve_device_image(pid, edition_id, layout_id, part).map(|image| image.asset)
}

/// Follow km.render's `layoutId || 1`, then ProductImage's edition-zero retry.
/// An explicit unknown layout remains unresolved; do not display another layout
/// or use its input geometry for the requested device.
pub fn resolve_device_image(
    pid: u32,
    edition_id: u32,
    layout_id: u32,
    part: DeviceImage,
) -> Option<ResolvedDeviceImage> {
    let is_keyboard_product = pid == 653 && part == DeviceImage::Product;
    let layout_id = if is_keyboard_product {
        if layout_id == 0 { 1 } else { layout_id }
    } else {
        0
    };
    let lookup = |requested_edition, requested_layout| {
        PRODUCT_IMAGES.iter().chain(ARMORY_PRODUCT_IMAGES).find_map(
            |&(product, edition, layout, purpose, asset)| {
                (product == pid
                    && edition == requested_edition
                    && layout == requested_layout
                    && purpose == part)
                    .then_some(ResolvedDeviceImage {
                        asset,
                        edition_id: edition,
                        layout_id: layout,
                        is_fallback_preview: false,
                    })
            },
        )
    };
    lookup(edition_id, layout_id).or_else(|| lookup(0, layout_id))
}
#[derive(Clone, serde::Deserialize)]
pub struct KeyboardKey {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub functions: Vec<String>,
    pub bounds: [f32; 4],
    pub geometry: KeyboardGeometry,
}

/// IM's original SVG primitives, in its 730 x 340 source coordinate system.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum KeyboardGeometry {
    Path { commands: Vec<KeyboardPathCommand> },
    Circle { center: [f32; 2], radius: f32 },
    Rect { origin: [f32; 2], size: [f32; 2] },
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(tag = "op", content = "points", rename_all = "snake_case")]
pub enum KeyboardPathCommand {
    Move([f32; 2]),
    Line([f32; 2]),
    Curve([[f32; 2]; 3]),
    Close,
}
pub fn keyboard_keys() -> &'static [KeyboardKey] {
    keyboard_keys_for_layout(1)
}

#[derive(serde::Deserialize)]
struct KeyboardLayout {
    layout_id: u32,
    keys: Vec<KeyboardKey>,
}

/// The original Customize layout switch selects groupList independently of
/// edition. Missing legacy layout IDs mean US; unrecognized IDs stay read-only.
pub fn keyboard_keys_for_layout(layout_id: u32) -> &'static [KeyboardKey] {
    static LAYOUTS: std::sync::OnceLock<Vec<KeyboardLayout>> = std::sync::OnceLock::new();
    let layout_id = if layout_id == 0 { 1 } else { layout_id };
    LAYOUTS
        .get_or_init(|| crate::data::layouts!())
        .iter()
        .find(|layout| layout.layout_id == layout_id)
        .map_or(&[], |layout| layout.keys.as_slice())
}

#[cfg(test)]
mod asset_decode_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn registered_image(
        pid: u32,
        edition: u32,
        layout: u32,
        part: DeviceImage,
    ) -> ResolvedDeviceImage {
        let image = resolve_device_image(pid, edition, layout, part)
            .expect("the device variant should have an image");
        assert!(remote::identity(image.asset).is_some(), "{}", image.asset);
        assert_eq!(device_image(pid, edition, layout, part), Some(image.asset));
        image
    }

    #[test]
    fn dashboard_plugin_requests_keep_exact_density_and_missing_variants() {
        assert_eq!(
            dashboard_plugin_image(190, 128, 128, 0, false, 3),
            Some("synapse/current-dashboard-190-128-0-product-3x.png")
        );
        assert!(dashboard_plugin_image(190, 999, 999, 0, false, 3).is_none());
        assert!(dashboard_plugin_image(190, 128, 0, 0, false, 3).is_none());
        assert!(dashboard_plugin_image(190, 128, 128, 0, false, 2).is_none());
        assert!(dashboard_plugin_image(756, 0, 0, 0, false, 3).is_none());
    }

    #[test]
    fn dashboard_current_requests_keep_the_source_retry_and_wrist_variants() {
        let normal = dashboard_card_image(190, 0, 0, Some(0), false, 1.).unwrap();
        assert_eq!(normal, "synapse/current-dashboard-190-0-0-product-1x.png");
        assert_eq!(
            dashboard_card_image(190, 999, 999, Some(99), true, 3.),
            Some(normal),
            "failed requests retry edition-zero/layout-zero/1x"
        );
        assert_eq!(
            dashboard_card_image(190, 0, 0, None, false, 1.),
            Some(normal)
        );
        for density in [1., 2.] {
            let wrist = dashboard_card_image(653, 0, 0, Some(1), true, density).unwrap();
            let expected = format!(
                "synapse/current-dashboard-653-0-1-wrist-{}x.png",
                if density > 1. { 3 } else { 1 }
            );
            assert_eq!(wrist, expected);
            assert!(remote::identity(wrist).is_some());
        }
    }

    #[test]
    fn every_current_dashboard_variant_has_a_registered_resource() {
        type Image = (u32, u32, u32, u32, bool, u32, &'static str);
        const IMAGES: &[Image] =
            include!("../../../assets/synapse/tables/current-dashboard-images.rs");
        assert!(!IMAGES.is_empty());
        for &(pid, directory_edition, filename_edition, layout, wrist, density, alias) in IMAGES {
            let (_, bytes, _) = remote::identity(alias).unwrap_or_else(|| {
                panic!(
                    "missing Dashboard artwork: PID {pid}, directory edition {directory_edition}, \
                         filename edition {filename_edition}, layout {layout}, wrist {wrist}, \
                         density {density}: {alias}"
                )
            });
            assert!(bytes >= 33, "truncated PNG identity: {alias}");
        }
    }

    #[test]
    fn footstep_animation_frames_have_distinct_source_keyframe_identities() {
        for frame in 0..330 {
            let alias = format!("synapse/audio-footstep-{frame:03}.svg");
            assert!(remote::identity(&alias).is_some(), "{alias}");
        }
        let keyframes = [0, 150, 210, 240]
            .map(|frame| {
                remote::identity(&format!("synapse/audio-footstep-{frame:03}.svg"))
                    .unwrap()
                    .2
            })
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(keyframes.len(), 4);
    }

    #[test]
    fn keyboard691_customize_has_a_separate_prepared_resource() {
        let original = remote::identity("synapse/keyboard-product-691.svg").unwrap();
        let customize = remote::identity("synapse/keyboard-customize-691.svg").unwrap();
        assert_ne!(original.0, customize.0);
        assert_ne!(original.2, customize.2);
    }

    #[test]
    fn receiver_dashboard_artwork_resolves_current_identities_and_exact_keyboard_layout() {
        for (pid, layout) in [(164, 0), (241, 0), (756, 1)] {
            let asset = dashboard_image(pid, 0, layout).unwrap();
            assert!(remote::identity(asset).is_some());
        }
        assert!(dashboard_image(756, 0, 0).is_none());
        assert!(dashboard_image(756, 0, 99).is_none());
        assert!(dashboard_image(756, 999, 1).is_none());
    }

    #[test]
    fn mouse_editions_select_distinct_front_and_bottom_images() {
        let front = registered_image(182, 0, 0, DeviceImage::Product);
        let edition_front = registered_image(182, 128, 999, DeviceImage::Product);
        let bottom = registered_image(182, 0, 0, DeviceImage::MouseBottom);
        let edition_bottom = registered_image(182, 128, 999, DeviceImage::MouseBottom);

        assert_eq!(edition_front.edition_id, 128);
        assert_eq!(edition_front.layout_id, 0);
        assert_ne!(front.asset, edition_front.asset);
        assert_ne!(bottom.asset, edition_bottom.asset);
        assert_ne!(edition_front.asset, edition_bottom.asset);
        // The original edition 130 deliberately reuses the standard bottom.
        assert_eq!(
            registered_image(182, 130, 0, DeviceImage::MouseBottom).asset,
            bottom.asset
        );
    }

    #[test]
    fn keyboard_prefers_the_exact_edition_and_layout() {
        let standard = registered_image(653, 0, 1, DeviceImage::Product);
        let white = registered_image(653, 130, 1, DeviceImage::Product);
        let second_layout = registered_image(653, 0, 2, DeviceImage::Product);
        let edition_second_layout = registered_image(653, 128, 2, DeviceImage::Product);

        assert_eq!(standard.asset, "synapse/keyboard-653.png");
        assert_eq!((white.edition_id, white.layout_id), (130, 1));
        assert_ne!(standard.asset, white.asset);
        assert_eq!(second_layout.layout_id, 2);
        assert_ne!(standard.asset, second_layout.asset);
        assert_eq!(edition_second_layout.edition_id, 128);
        assert_ne!(second_layout.asset, edition_second_layout.asset);
        assert!(!white.is_fallback_preview);
    }

    #[test]
    fn missing_keyboard_edition_keeps_the_requested_supported_layout() {
        let standard_second_layout = registered_image(653, 0, 2, DeviceImage::Product);

        // ProductImage retries edition zero while retaining the same layout.
        assert_eq!(
            registered_image(653, 130, 2, DeviceImage::Product),
            standard_second_layout
        );
        assert_eq!(
            registered_image(653, 999, 2, DeviceImage::Product),
            standard_second_layout
        );
        assert!(!standard_second_layout.is_fallback_preview);
    }

    #[test]
    fn explicit_unknown_keyboard_layout_does_not_display_different_geometry() {
        for edition in [0, 130] {
            let first_layout = registered_image(653, edition, 1, DeviceImage::Product);
            let legacy_default = registered_image(653, edition, 0, DeviceImage::Product);

            assert_eq!(legacy_default, first_layout);
            assert!(!legacy_default.is_fallback_preview);
            assert!(resolve_device_image(653, edition, 999, DeviceImage::Product).is_none());
        }
    }

    #[test]
    fn every_supported_product_layout_has_its_original_inputs() {
        for layout in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 15, 16, 17, 18] {
            let image = registered_image(653, 0, layout, DeviceImage::Product);
            assert_eq!(image.layout_id, layout);
            assert!(!image.is_fallback_preview);
            let keys = keyboard_keys_for_layout(image.layout_id);
            let inputs = keyboard_source_for_layout(layout)
                .unwrap()
                .iter()
                .flat_map(KeyboardSourceGroup::inputs);
            assert!(keys.len() >= 118, "layout {layout}");
            for input in inputs {
                let shaped = input.has_geometry();
                let key = keys.iter().find(|key| key.id == input.id());
                assert_eq!(
                    key.is_some(),
                    shaped,
                    "layout {layout}, input {}",
                    input.id()
                );
                if let Some(key) = key {
                    assert_eq!(key.enabled, input.enabled());
                    let functions = input.functions();
                    assert_eq!(
                        key.functions.as_slice(),
                        functions,
                        "layout {layout}, input {}",
                        key.id
                    );
                }
            }
        }
        assert!(std::ptr::eq(keyboard_keys_for_layout(0), keyboard_keys()));
        assert!(std::ptr::eq(
            keyboard_source_for_layout(0).unwrap(),
            keyboard_source_for_layout(1).unwrap()
        ));
        for unknown in [13, 14, 19, 999] {
            assert!(keyboard_keys_for_layout(unknown).is_empty());
            assert!(keyboard_source_for_layout(unknown).is_none());
        }
    }

    #[test]
    fn headset_ignores_layout_and_unknown_products_stay_unavailable() {
        let headset = registered_image(777, 0, 0, DeviceImage::Product);
        assert_eq!(registered_image(777, 0, 999, DeviceImage::Product), headset);
        assert!(!headset.is_fallback_preview);
        assert!(resolve_device_image(12345, 0, 1, DeviceImage::Product).is_none());
        assert!(device_image(12345, 0, 999, DeviceImage::Product).is_none());
    }

    #[test]
    fn accessory_179_has_registered_product_and_dashboard_images() {
        let product = registered_image(179, 0, 0, DeviceImage::Product);
        assert_eq!(product.asset, "synapse/product-179-prd-1x.c84b6fdf.png");
        assert_eq!(
            dashboard_image(179, 0, 0),
            Some("synapse/dashboard-179-0-0.png")
        );
    }
}
