//! Embedded, converted reference assets; provenance is in assets/synapse/manifest.json.
use gpui_kit::{AssetSource, SharedString};
use std::borrow::Cow;
pub struct SynapseAssets;
const ASSETS: &[(&str, &[u8])] = include!("../../../assets/synapse/embedded.rs");
const MODULE_SERVICE_ASSETS: &[(&str, &[u8])] =
    include!("../../../assets/synapse/module-service-embedded.rs");
const AUDIO_OLED_HOME_ASSETS: &[(&str, &[u8])] =
    include!("../../../assets/synapse/audio-oled-home-embedded.rs");
const AUDIO_OLED_ARTWORK_ASSETS: &[(&str, &[u8])] =
    include!("../../../assets/synapse/audio-oled-artwork-embedded.rs");
const AUDIO_OLED_BANNER_ASSETS: &[(&str, &[u8])] =
    include!("../../../assets/synapse/audio-oled-banner-embedded.rs");
const AUDIO_OLED_SYSTEM_ASSETS: &[(&str, &[u8])] =
    include!("../../../assets/synapse/audio-oled-system-embedded.rs");
const TRAY_ACCOUNT_ASSETS: &[(&str, &[u8])] =
    include!("../../../assets/synapse/tray-account-embedded.rs");
const TRAY_WIDGET_ASSETS: &[(&str, &[u8])] =
    include!("../../../assets/synapse/tray-widget-embedded.rs");
const SETTINGS_WINDOW_ASSETS: &[(&str, &[u8])] =
    include!("../../../assets/synapse/settings-window-embedded.rs");
const CHROMA_SETTINGS_ASSETS: &[(&str, &[u8])] =
    include!("../../../assets/synapse/chroma-settings-embedded.rs");
const CHROMA_STUDIO_ASSETS: &[(&str, &[u8])] =
    include!("../../../assets/synapse/chroma-studio-embedded.rs");
const CHROMA_STUDIO_COLOR_ASSETS: &[(&str, &[u8])] =
    include!("../../../assets/synapse/chroma-studio-color-embedded.rs");
const KITSUNE_ASSETS: &[(&str, &[u8])] = include!("../../../assets/synapse/kitsune-embedded.rs");
const GAMEPAD_DIALOG_ASSETS: &[(&str, &[u8])] =
    include!("../../../assets/synapse/gamepad-2636-embedded.rs");
const GAMEPAD_CALIBRATION_ASSETS: &[(&str, &[u8])] =
    include!("../../../assets/synapse/gamepad-2636-calibration-embedded.rs");
const PROFILES_TRANSFER_ASSETS: &[(&str, &[u8])] =
    include!("../../../assets/synapse/profiles-transfer-embedded.rs");
const MONITOR_INPUT_ASSETS: &[(&str, &[u8])] =
    include!("../../../assets/synapse/monitor-input-embedded.rs");
const STREAM_MIXER_ASSETS: &[(&str, &[u8])] =
    include!("../../../assets/synapse/stream-mixer-embedded.rs");
const SNAP_TAP_ASSETS: &[(&str, &[u8])] = include!("../../../assets/synapse/snap-tap-embedded.rs");
const KEYBOARD_PROPERTIES_ASSETS: &[(&str, &[u8])] =
    include!("../../../assets/synapse/keyboard-properties-embedded.rs");
const KEYBOARD_ACTUATION_ASSETS: &[(&str, &[u8])] =
    include!("../../../assets/synapse/keyboard-actuation-embedded.rs");
const CHROMA_STUDIO_HOST_ASSETS: &[(&str, &[u8])] = &[(
    "synapse/host-chroma-studio-favicon.svg",
    include_bytes!("../../../assets/synapse/host-chroma-studio-favicon.svg"),
)];
const AUDIO_OLED_RUNTIME_ASSETS: &[(&str, &[u8])] = &[(
    "synapse/audio-oled-runtime-warning.svg",
    include_bytes!("../../../assets/synapse/audio-oled-runtime-warning.svg"),
)];
mod audio_demo_controls {
    include!("../../../assets/synapse/audio-demo-controls-embedded.rs");
}
const MOUSE_POLLING_ASSETS: &[(&str, &[u8])] = &[(
    "synapse/polling-info.svg",
    include_bytes!("../../../assets/synapse/polling-info.svg"),
)];
const RECEIVER_PARENT_ASSETS: &[(&str, &[u8])] = &[
    (
        "synapse/receiver/connection-skeleton-base.svg",
        include_bytes!("../../../assets/synapse/receiver/connection-skeleton-base.svg"),
    ),
    (
        "synapse/receiver/connection-skeleton-sweep.svg",
        include_bytes!("../../../assets/synapse/receiver/connection-skeleton-sweep.svg"),
    ),
];
impl AssetSource for SynapseAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = audio_demo_controls::audio_demo_controls_load(path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        if let Some((_, bytes)) = ASSETS
            .iter()
            .chain(MODULE_SERVICE_ASSETS)
            .chain(AUDIO_OLED_HOME_ASSETS)
            .chain(AUDIO_OLED_ARTWORK_ASSETS)
            .chain(AUDIO_OLED_BANNER_ASSETS)
            .chain(AUDIO_OLED_SYSTEM_ASSETS)
            .chain(TRAY_ACCOUNT_ASSETS)
            .chain(TRAY_WIDGET_ASSETS)
            .chain(SETTINGS_WINDOW_ASSETS)
            .chain(CHROMA_SETTINGS_ASSETS)
            .chain(CHROMA_STUDIO_ASSETS)
            .chain(CHROMA_STUDIO_COLOR_ASSETS)
            .chain(KITSUNE_ASSETS)
            .chain(GAMEPAD_DIALOG_ASSETS)
            .chain(GAMEPAD_CALIBRATION_ASSETS)
            .chain(PROFILES_TRANSFER_ASSETS)
            .chain(MONITOR_INPUT_ASSETS)
            .chain(STREAM_MIXER_ASSETS)
            .chain(SNAP_TAP_ASSETS)
            .chain(KEYBOARD_PROPERTIES_ASSETS)
            .chain(KEYBOARD_ACTUATION_ASSETS)
            .chain(CHROMA_STUDIO_HOST_ASSETS)
            .chain(AUDIO_OLED_RUNTIME_ASSETS)
            .chain(MOUSE_POLLING_ASSETS)
            .chain(RECEIVER_PARENT_ASSETS)
            .find(|(key, _)| *key == path)
        {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        gpui_kit::assets::AllAssets.load(path)
    }
    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        let mut items = gpui_kit::assets::AllAssets.list(path)?;
        items.extend(audio_demo_controls::audio_demo_controls_list(path));
        items.extend(
            ASSETS
                .iter()
                .chain(MODULE_SERVICE_ASSETS)
                .chain(AUDIO_OLED_HOME_ASSETS)
                .chain(AUDIO_OLED_ARTWORK_ASSETS)
                .chain(AUDIO_OLED_BANNER_ASSETS)
                .chain(AUDIO_OLED_SYSTEM_ASSETS)
                .chain(TRAY_ACCOUNT_ASSETS)
                .chain(TRAY_WIDGET_ASSETS)
                .chain(SETTINGS_WINDOW_ASSETS)
                .chain(CHROMA_SETTINGS_ASSETS)
                .chain(CHROMA_STUDIO_ASSETS)
                .chain(CHROMA_STUDIO_COLOR_ASSETS)
                .chain(KITSUNE_ASSETS)
                .chain(GAMEPAD_DIALOG_ASSETS)
                .chain(GAMEPAD_CALIBRATION_ASSETS)
                .chain(PROFILES_TRANSFER_ASSETS)
                .chain(MONITOR_INPUT_ASSETS)
                .chain(STREAM_MIXER_ASSETS)
                .chain(SNAP_TAP_ASSETS)
                .chain(KEYBOARD_PROPERTIES_ASSETS)
                .chain(KEYBOARD_ACTUATION_ASSETS)
                .chain(CHROMA_STUDIO_HOST_ASSETS)
                .chain(AUDIO_OLED_RUNTIME_ASSETS)
                .chain(MOUSE_POLLING_ASSETS)
                .chain(RECEIVER_PARENT_ASSETS)
                .filter(|(key, _)| key.starts_with(path))
                .map(|(key, _)| SharedString::from(*key)),
        );
        Ok(items)
    }
}
pub fn register_fonts(cx: &gpui_kit::App) -> anyhow::Result<()> {
    cx.text_system().add_fonts(vec![
        Cow::Borrowed(include_bytes!("../../../assets/synapse/Roboto-Light.ttf")),
        Cow::Borrowed(include_bytes!("../../../assets/synapse/Roboto-Regular.ttf")),
        Cow::Borrowed(include_bytes!("../../../assets/synapse/Roboto-Medium.ttf")),
        Cow::Borrowed(include_bytes!("../../../assets/synapse/Roboto-Bold.ttf")),
        Cow::Borrowed(include_bytes!("../../../assets/synapse/RazerF5-Thin.ttf")),
        Cow::Borrowed(include_bytes!(
            "../../../assets/synapse/RazerF5-Regular.ttf"
        )),
        Cow::Borrowed(include_bytes!(
            "../../../assets/synapse/RazerF5-SemiBold.ttf"
        )),
        Cow::Borrowed(include_bytes!("../../../assets/synapse/RazerF5-Bold.ttf")),
    ])
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
const PRODUCT_IMAGES: &[ProductAsset] = include!("../../../assets/synapse/product-images.rs");
const ARMORY_PRODUCT_IMAGES: &[ProductAsset] =
    include!("../../../assets/synapse/armory-product-images.rs");

/// Dashboard and pairing cards use the original PluginImages artwork. An
/// unavailable edition/layout stays unavailable instead of changing its color
/// or substituting Customize artwork. Old keyboard data defaults to layout one.
pub fn dashboard_image(pid: u32, edition_id: u32, layout_id: u32) -> Option<&'static str> {
    const IMAGES: &[(u32, u32, u32, &str)] =
        include!("../../../assets/synapse/dashboard-images.rs");
    const ARMORY_IMAGES: &[(u32, u32, u32, &str)] =
        include!("../../../assets/synapse/armory-dashboard-images.rs");
    let layout_id = if pid == 653 { layout_id.max(1) } else { 0 };
    IMAGES
        .iter()
        .chain(ARMORY_IMAGES)
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
    /// An unknown keyboard layout is shown as a generic preview. Its image must
    /// not be used to enable layout-specific input hit regions.
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
/// If a nonzero keyboard layout is unknown, this app also supplies a layout-one
/// preview. That last fallback is local behavior, not a claim about the device.
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
    let lookup_layout = |layout| lookup(edition_id, layout).or_else(|| lookup(0, layout));
    lookup_layout(layout_id).or_else(|| {
        if is_keyboard_product && layout_id != 1 {
            lookup_layout(1).map(|mut image| {
                image.is_fallback_preview = true;
                image
            })
        } else {
            None
        }
    })
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
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../../../assets/synapse/keyboard-653-layouts.json"
            ))
            .expect("bundled Customize layout geometry")
        })
        .iter()
        .find(|layout| layout.layout_id == layout_id)
        .map_or(&[], |layout| layout.keys.as_slice())
}

/// Full groupList data includes invisible dial sub-inputs required by the drawer.
pub fn keyboard_source_for_layout(layout_id: u32) -> Option<&'static str> {
    const SOURCES: &[(u32, &str)] = include!("../../../assets/synapse/keyboard-sources.rs");
    let layout_id = if layout_id == 0 { 1 } else { layout_id };
    SOURCES
        .iter()
        .find_map(|&(layout, source)| (layout == layout_id).then_some(source))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn embedded_image(
        pid: u32,
        edition: u32,
        layout: u32,
        part: DeviceImage,
    ) -> ResolvedDeviceImage {
        let image = resolve_device_image(pid, edition, layout, part)
            .expect("the device variant should have an image");
        assert!(
            ASSETS
                .iter()
                .any(|(path, bytes)| *path == image.asset && !bytes.is_empty()),
            "resolved image must be embedded: {}",
            image.asset
        );
        assert_eq!(device_image(pid, edition, layout, part), Some(image.asset));
        image
    }

    #[test]
    fn mouse_editions_select_distinct_front_and_bottom_images() {
        let front = embedded_image(182, 0, 0, DeviceImage::Product);
        let edition_front = embedded_image(182, 128, 999, DeviceImage::Product);
        let bottom = embedded_image(182, 0, 0, DeviceImage::MouseBottom);
        let edition_bottom = embedded_image(182, 128, 999, DeviceImage::MouseBottom);

        assert_eq!(edition_front.edition_id, 128);
        assert_eq!(edition_front.layout_id, 0);
        assert_ne!(front.asset, edition_front.asset);
        assert_ne!(bottom.asset, edition_bottom.asset);
        assert_ne!(edition_front.asset, edition_bottom.asset);
        // The original edition 130 deliberately reuses the standard bottom.
        assert_eq!(
            embedded_image(182, 130, 0, DeviceImage::MouseBottom).asset,
            bottom.asset
        );
    }

    #[test]
    fn keyboard_prefers_the_exact_edition_and_layout() {
        let standard = embedded_image(653, 0, 1, DeviceImage::Product);
        let white = embedded_image(653, 130, 1, DeviceImage::Product);
        let second_layout = embedded_image(653, 0, 2, DeviceImage::Product);
        let edition_second_layout = embedded_image(653, 128, 2, DeviceImage::Product);

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
        let standard_second_layout = embedded_image(653, 0, 2, DeviceImage::Product);

        // Edition 130 has only layout one; use edition zero's layout two before
        // considering a layout-one preview, even though its color differs.
        assert_eq!(
            embedded_image(653, 130, 2, DeviceImage::Product),
            standard_second_layout
        );
        assert_eq!(
            embedded_image(653, 999, 2, DeviceImage::Product),
            standard_second_layout
        );
        assert!(!standard_second_layout.is_fallback_preview);
    }

    #[test]
    fn legacy_default_and_unknown_keyboard_layouts_have_different_editability() {
        for edition in [0, 130] {
            let first_layout = embedded_image(653, edition, 1, DeviceImage::Product);
            let legacy_default = embedded_image(653, edition, 0, DeviceImage::Product);
            let unknown_layout = embedded_image(653, edition, 999, DeviceImage::Product);

            assert_eq!(legacy_default, first_layout);
            assert!(!legacy_default.is_fallback_preview);
            assert_eq!(unknown_layout.asset, first_layout.asset);
            assert_eq!(
                (unknown_layout.edition_id, unknown_layout.layout_id),
                (edition, 1)
            );
            assert!(unknown_layout.is_fallback_preview);
        }
    }

    #[test]
    fn every_supported_product_layout_has_its_original_inputs() {
        for layout in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 15, 16, 17, 18] {
            let image = embedded_image(653, 0, layout, DeviceImage::Product);
            assert_eq!(image.layout_id, layout);
            assert!(!image.is_fallback_preview);
            let keys = keyboard_keys_for_layout(image.layout_id);
            let source: serde_json::Value =
                serde_json::from_str(keyboard_source_for_layout(layout).unwrap()).unwrap();
            let inputs = source
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|group| group["group"]["buttonList"].as_array().unwrap().iter());
            assert!(keys.len() >= 118, "layout {layout}");
            for input in inputs {
                let shaped = input.get("d").is_some()
                    || input.get("r").is_some()
                    || input.get("x").is_some();
                let key = keys
                    .iter()
                    .find(|key| key.id == input["inputID"].as_str().unwrap());
                assert_eq!(
                    key.is_some(),
                    shaped,
                    "layout {layout}, input {}",
                    input["inputID"]
                );
                if let Some(key) = key {
                    assert_eq!(key.enabled, input["isEnabled"].as_bool().unwrap_or(true));
                    let functions: Vec<String> =
                        input.get("functionList").map_or_else(Vec::new, |value| {
                            serde_json::from_value(value.clone()).unwrap()
                        });
                    assert_eq!(
                        key.functions, functions,
                        "layout {layout}, input {}",
                        key.id
                    );
                }
            }
        }
        assert!(std::ptr::eq(keyboard_keys_for_layout(0), keyboard_keys()));
        assert_eq!(keyboard_source_for_layout(0), keyboard_source_for_layout(1));
        for unknown in [13, 14, 19, 999] {
            assert!(keyboard_keys_for_layout(unknown).is_empty());
            assert!(keyboard_source_for_layout(unknown).is_none());
        }
    }

    #[test]
    fn headset_ignores_layout_and_unknown_products_stay_unavailable() {
        let headset = embedded_image(777, 0, 0, DeviceImage::Product);
        assert_eq!(embedded_image(777, 0, 999, DeviceImage::Product), headset);
        assert!(!headset.is_fallback_preview);
        assert!(resolve_device_image(12345, 0, 1, DeviceImage::Product).is_none());
        assert!(device_image(12345, 0, 999, DeviceImage::Product).is_none());
    }

    #[test]
    fn accessory_179_has_embedded_product_and_dashboard_images() {
        let product = embedded_image(179, 0, 0, DeviceImage::Product);
        assert_eq!(product.asset, "synapse/product-179-prd-1x.c84b6fdf.png");
        assert_eq!(
            dashboard_image(179, 0, 0),
            Some("synapse/dashboard-179-0-0.png")
        );
    }
}
