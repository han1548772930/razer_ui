//! Host TabUI consumes the page's first favicon, independent of module logos.
use std::{collections::BTreeMap, sync::OnceLock};

pub(super) fn device_favicon(pid: u32) -> Option<&'static str> {
    static ICONS: OnceLock<BTreeMap<u32, Option<String>>> = OnceLock::new();
    ICONS
        .get_or_init(|| read_host_device_favicons_catalog())
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

// Generated catalog reader: razer-shell/host_device_favicons.
fn read_host_device_favicons_catalog() -> BTreeMap<u32, Option<String>> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<u32, std::option::Option<String>>> {
        let value = match r.tag()? {
            0 => r.map(decode_1, decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<String>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_3)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_SHELL_HOST_DEVICE_FAVICONS,
        0,
        "d7200394fef94caa1677d2f62ae3e18b2c904ed25541b88f34c83b04bb74a07c",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod host_device_favicons_catalog_tests {
    #[test]
    fn packaged_catalog_host_device_favicons_decodes() {
        let _ = super::read_host_device_favicons_catalog();
    }
}
// End generated catalog reader: razer-shell/host_device_favicons.
