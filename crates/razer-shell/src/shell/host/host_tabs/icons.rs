//! Host TabUI consumes the page's first favicon, independent of module logos.
use std::{collections::BTreeMap, sync::OnceLock};

pub(super) fn device_favicon(pid: u32) -> Option<&'static str> {
    static ICONS: OnceLock<BTreeMap<u32, Option<String>>> = OnceLock::new();
    ICONS
        .get_or_init(|| crate::data::host_device_favicons::host_device_favicons!())
        .get(&pid)
        .and_then(|icon| icon.as_deref())
}

#[cfg(test)]
mod tests {
    use super::device_favicon;
    use gpui_kit::AssetSource;
    use std::collections::BTreeSet;

    #[test]
    fn every_registered_product_tab_has_a_manifested_source_favicon() {
        let mut images = BTreeSet::new();
        for product in razer_catalog::registry() {
            let icon = device_favicon(product.id())
                .unwrap_or_else(|| panic!("Missing source favicon for PID {}", product.id()));
            images.insert(icon);
        }
        assert!(!images.is_empty());
        for icon in images {
            assert!(
                razer_assets::SynapseAssets
                    .list(icon)
                    .unwrap()
                    .iter()
                    .any(|alias| alias.as_ref() == icon),
                "{icon}"
            );
        }
        assert!(device_favicon(u32::MAX).is_none());
    }
}
