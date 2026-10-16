//! Current app-menu Root module 569 U -> b/I/P -> Y, with module 141 categories.
//! Source acquisition and complete bodies: tools/workspace.py prepare-app-picker.
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Deserialize)]
struct NativeProductIcon {
    asset: String,
}

#[derive(Deserialize)]
struct Icons {
    subcategory_categories: Vec<String>,
    icons: BTreeMap<String, String>,
    products: BTreeMap<u32, NativeProductIcon>,
}

fn icons() -> &'static Icons {
    static ICONS: OnceLock<Icons> = OnceLock::new();
    ICONS.get_or_init(|| {
        serde_json::from_str(include_str!("app_picker_icons_data.json"))
            .expect("verified current app menu category vectors and local catalog icons")
    })
}

/// Observed fields follow the source U selector, including its absent branch.
/// Missing category observations use the separately verified current product
/// favicon classification for local presentation, without modifying device state.
pub fn device_icon(
    product_id: u32,
    category: Option<&str>,
    sub_category: Option<&str>,
) -> Option<&'static str> {
    let source = icons();
    let Some(category) = category else {
        return source
            .products
            .get(&product_id)
            .map(|product| product.asset.as_str());
    };
    let sub_category = sub_category.filter(|value| !value.is_empty());
    let requested = if source
        .subcategory_categories
        .iter()
        .any(|name| name == category)
    {
        sub_category.unwrap_or(category)
    } else {
        category
    };
    source
        .icons
        .get(requested)
        .or_else(|| sub_category.and_then(|_| source.icons.get(category)))
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{AssetSource, SvgRenderer};
    use std::{collections::BTreeSet, sync::Arc};

    #[test]
    fn current_app_picker_category_selector_preserves_source_subcategory_and_absent_branches() {
        assert_eq!(
            device_icon(579, Some("KEYBOARD"), None),
            Some("synapse/app-picker-category/keyboard.svg")
        );
        assert_eq!(
            device_icon(1422, Some("BROADCASTER"), Some("MICROPHONE")),
            Some("synapse/app-picker-category/microphone.svg")
        );
        assert_eq!(
            device_icon(0, Some("IOT"), Some("KEY_LIGHT")),
            Some("synapse/app-picker-category/key-light.svg")
        );
        assert_eq!(
            device_icon(0, Some("GAMEPAD"), Some("ARCADE_CONTROLLER")),
            Some("synapse/app-picker-category/gamepad.svg")
        );
        assert_eq!(
            device_icon(0, Some("CAMERA"), Some("MICROPHONE")),
            Some("synapse/app-picker-category/camera.svg")
        );
        assert_eq!(device_icon(579, Some("UNKNOWN"), Some("KEYBOARD")), None);
        assert_eq!(device_icon(579, Some("keyboard"), None), None);
        assert_eq!(device_icon(579, Some(""), None), None);
        assert_eq!(device_icon(u32::MAX, None, None), None);
    }

    #[test]
    fn current_app_picker_category_vectors_decode_at_source_size_with_visible_pixels() {
        let renderer = SvgRenderer::new(Arc::new(razer_assets::SynapseAssets));
        let mut aliases = BTreeSet::new();
        for product in razer_catalog::registry() {
            aliases.insert(
                device_icon(product.id(), None, None).expect("current native product icon"),
            );
        }
        aliases.extend(icons().icons.values().map(String::as_str));
        for alias in aliases {
            let bytes = razer_assets::SynapseAssets
                .load(alias)
                .unwrap()
                .expect(alias);
            let image = renderer.render_single_frame(&bytes, 1.).expect(alias);
            let size = image.size(0);
            let mut bounds = [size.width.0, size.height.0, 0, 0];
            let mut pixels = 0;
            for (index, pixel) in image.as_bytes(0).unwrap().chunks_exact(4).enumerate() {
                if pixel[3] == 0 {
                    continue;
                }
                let index = i32::try_from(index).expect("decoded icon pixel index fits i32");
                let x = index % size.width.0;
                let y = index / size.width.0;
                bounds[0] = bounds[0].min(x);
                bounds[1] = bounds[1].min(y);
                bounds[2] = bounds[2].max(x + 1);
                bounds[3] = bounds[3].max(y + 1);
                pixels += 1;
            }
            assert!(pixels > 0, "blank source category icon: {alias}");
            if alias.starts_with("synapse/app-picker-category/") {
                assert_eq!((size.width.0, size.height.0), (40, 40), "{alias}");
            }
            if alias == "synapse/app-picker-category/keyboard.svg" {
                assert_eq!(bounds, [0, 12, 40, 30]);
            }
            if alias == "synapse/app-picker-category/mouse.svg" {
                assert!(bounds[3] - bounds[1] >= 36);
            }
            eprintln!(
                "app picker icon {alias}: {}x{} visible={bounds:?}, pixels={pixels}",
                size.width.0, size.height.0
            );
        }
    }
}
