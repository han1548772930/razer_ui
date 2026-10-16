use crate::model::{Device, LocalizedText};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Deserialize)]
pub(crate) struct Translation {
    pub product: String,
    #[serde(default)]
    pub dashboard: String,
    #[serde(default)]
    pub edition: String,
}

type Catalog = BTreeMap<u32, BTreeMap<u32, BTreeMap<String, Translation>>>;

pub(crate) fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| read_catalog_catalog())
}

pub fn product_locale(locale: &str) -> &'static str {
    if locale.replace('_', "-").eq_ignore_ascii_case("zh-cn") {
        "zh-cn"
    } else {
        "en"
    }
}

pub(crate) fn translation(pid: u32, edition: u32, locale: &str) -> Option<&'static Translation> {
    let values = catalog().get(&pid)?.get(&edition)?;
    values
        .get(&locale.replace('_', "-").to_ascii_lowercase())
        .or_else(|| values.get("en"))
}

pub fn localized_names(pid: u32, edition: u32) -> (LocalizedText, LocalizedText) {
    let mut names = LocalizedText::default();
    let mut products = LocalizedText::default();
    if let Some(values) = catalog()
        .get(&pid)
        .and_then(|editions| editions.get(&edition))
    {
        for (locale, value) in values {
            if !value.dashboard.is_empty() {
                names.values.insert(locale.clone(), value.dashboard.clone());
            }
            if !value.product.is_empty() {
                products
                    .values
                    .insert(locale.clone(), value.product.clone());
            }
        }
    }
    if let Some(product) = razer_catalog::registered(pid) {
        names
            .values
            .entry("en".into())
            .or_insert_with(|| product.name().into());
        products
            .values
            .entry("en".into())
            .or_insert_with(|| product.name().into());
    }
    (names, products)
}

pub fn edition_name(pid: u32, edition: u32) -> Option<LocalizedText> {
    let values = catalog()
        .get(&pid)?
        .get(&edition)?
        .iter()
        .filter(|(_, translation)| !translation.edition.is_empty())
        .map(|(locale, translation)| (locale.clone(), translation.edition.clone()))
        .collect::<BTreeMap<_, _>>();
    (!values.is_empty()).then_some(LocalizedText { values })
}

fn is_registered_english_name(device: &Device, names: &LocalizedText) -> bool {
    let english = names.exact("en").unwrap_or("");
    if english.is_empty()
        || names
            .values
            .iter()
            .any(|(locale, value)| !value.is_empty() && !locale.eq_ignore_ascii_case("en"))
    {
        return false;
    }
    razer_catalog::registered(device.product_id).is_some_and(|product| english == product.name())
        || translation(device.product_id, device.edition_id, "en")
            .is_some_and(|value| english == value.dashboard || english == value.product)
}

pub(crate) fn legacy_dashboard_name(device: &Device, locale: &str) -> Option<&'static str> {
    if !is_registered_english_name(device, &device.name) {
        return None;
    }
    let value = catalog()
        .get(&device.product_id)?
        .get(&device.edition_id)?
        .get(locale)?;
    (!value.dashboard.is_empty()).then_some(value.dashboard.as_str())
}

pub(crate) fn restore_localized_names(device: &mut Device) {
    let (names, products) = localized_names(device.product_id, device.edition_id);
    if is_registered_english_name(device, &device.name) {
        for (locale, value) in names.values {
            device.name.values.entry(locale).or_insert(value);
        }
    }
    if is_registered_english_name(device, &device.product_name) {
        for (locale, value) in products.values {
            device.product_name.values.entry(locale).or_insert(value);
        }
    }
}

pub(crate) fn apply_observed_variant(device: &mut Device, edition: u32, layout: u32) {
    let (previous_names, previous_products) = localized_names(device.product_id, device.edition_id);
    let previous_edition = edition_name(device.product_id, device.edition_id);
    if device.dashboard.edition_name.is_none()
        || device
            .dashboard
            .edition_name
            .as_ref()
            .zip(previous_edition.as_ref())
            .is_some_and(|(current, previous)| current.values == previous.values)
    {
        device.dashboard.edition_name = edition_name(device.product_id, edition);
    }
    let matches_source = |names: &LocalizedText, source: &LocalizedText| {
        !names.values.is_empty()
            && names.values.iter().all(|(locale, value)| {
                value.is_empty()
                    || source.exact(locale) == Some(value.as_str())
                    || (locale.eq_ignore_ascii_case("en")
                        && razer_catalog::registered(device.product_id)
                            .is_some_and(|product| product.name() == value))
            })
    };
    let replace_name = matches_source(&device.name, &previous_names);
    let replace_product = matches_source(&device.product_name, &previous_products);
    let (names, products) = localized_names(device.product_id, edition);
    if replace_name {
        device.name = names;
    }
    if replace_product {
        device.product_name = products;
    }
    device.edition_id = edition;
    device.layout_id = layout;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::localized;

    #[test]
    fn source_names_follow_middleware_language_and_edition_maps() {
        let (names, products) = localized_names(182, 0);
        assert_eq!(names.get("ZH_CN"), "Razer 炼狱蝰蛇 V3专业版");
        assert_eq!(products.get("en"), "Razer Deathadder V3 Pro");
        let (names, products) = localized_names(182, 128);
        assert_eq!(names.get("en"), "Razer Deathadder V3 Pro");
        assert_eq!(products.get("en"), "Razer Deathadder V3 Pro White Edition");
        assert_eq!(
            localized_names(756, 0).0.get("zh-CN"),
            "Razer 隐蛛迷你版 65%"
        );
        assert_eq!(
            localized_names(3886, 0).0.get("zh-CN"),
            "Razer Chroma Wireless ARGB Controller"
        );
        assert!(localized_names(162, 0).0.values.is_empty());
        assert!(razer_catalog::registry().iter().all(|product| {
            !localized_names(
                product.id(),
                product.edition_ids().first().copied().unwrap_or(0),
            )
            .0
            .values
            .is_empty()
        }));
    }

    #[test]
    fn english_only_persisted_device_relocalizes_without_changing_identity() {
        let mut device = crate::test_devices::mouse();
        device.name = localized(&[("en", razer_catalog::registered(182).unwrap().name())]);
        device.product_name = device.name.clone();
        let before = serde_json::to_value(&device).unwrap();
        assert_eq!(
            device.display_name_for_locale("zh-CN"),
            "Razer 炼狱蝰蛇 V3专业版"
        );
        restore_localized_names(&mut device);
        let mut after = serde_json::to_value(&device).unwrap();
        after["name"] = before["name"].clone();
        after["product_name"] = before["product_name"].clone();
        assert_eq!(after, before);
        for locale in ["zh-CN", "zh-cn", "ZH_cn"] {
            assert_eq!(
                device.display_name_for_locale(locale),
                "Razer 炼狱蝰蛇 V3专业版"
            );
        }
        assert_eq!(device.display_name_for_locale("en"), device.name.get("en"));
        assert_eq!(device.display_name_for_locale("ja"), device.name.get("en"));
        assert_eq!(
            device.display_name_for_locale("zh-CN"),
            device.name.get("zh-cn")
        );
        let saved = serde_json::to_string(&device).unwrap();
        let restored: Device = serde_json::from_str(&saved).unwrap();
        assert_eq!(
            restored.display_name_for_locale("zh-CN"),
            device.name.get("zh-cn")
        );
    }

    #[test]
    fn custom_names_and_original_dashboard_fallback_order_are_preserved() {
        let mut device = crate::test_devices::mouse();
        device.name = localized(&[("en", "My office mouse")]);
        device.product_name = device.name.clone();
        restore_localized_names(&mut device);
        assert_eq!(device.name.values.len(), 1);
        assert_eq!(device.display_name_for_locale("zh-CN"), "My office mouse");
        device.dashboard.base_product_name = Some("Base device".into());
        assert_eq!(device.display_name_for_locale("zh-CN"), "Base device");
        device.name.values.insert("zh-CN".into(), "我的鼠标".into());
        assert_eq!(device.display_name_for_locale("ZH_CN"), "我的鼠标");
        assert_eq!(device.display_name_for_locale("en"), "My office mouse");
    }

    #[test]
    fn missing_source_does_not_invent_a_translation_or_device_observation() {
        let mut device = crate::test_devices::mouse();
        device.product_id = 999_999;
        device.name = localized(&[("en", "Unknown model")]);
        restore_localized_names(&mut device);
        assert_eq!(device.display_name_for_locale("zh-CN"), "Unknown model");
        assert!(localized_names(device.product_id, 0).0.values.is_empty());
        device.name.values.clear();
        assert_eq!(device.display_name_for_locale("zh-CN"), "productId 999999");
    }

    #[test]
    fn observed_variant_updates_only_known_source_names_and_variant_metadata() {
        let mut device = crate::test_devices::mouse();
        (device.name, device.product_name) = localized_names(182, 0);
        apply_observed_variant(&mut device, 128, 3);
        assert_eq!(device.edition_id, 128);
        assert_eq!(device.layout_id, 3);
        assert_eq!(
            device.product_name.get("en"),
            "Razer Deathadder V3 Pro White Edition"
        );
        assert_eq!(
            device.dashboard.edition_name.as_ref().unwrap().get("zh-CN"),
            "白色"
        );
        device.name = crate::model::localized(&[("en", "My own device")]);
        apply_observed_variant(&mut device, 0, 1);
        assert_eq!(device.name.get("en"), "My own device");
        assert!(device.dashboard.edition_name.is_none());
    }
}

// Generated catalog reader: razer-model/catalog.
fn read_catalog_catalog() -> Catalog {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::collections::BTreeMap<
            u32,
            std::collections::BTreeMap<u32, std::collections::BTreeMap<String, Translation>>,
        >,
    > {
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
    ) -> anyhow::Result<
        std::collections::BTreeMap<u32, std::collections::BTreeMap<String, Translation>>,
    > {
        let value = match r.tag()? {
            0 => r.map(decode_1, decode_3)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, Translation>> {
        let value = match r.tag()? {
            0 => r.map(decode_4, decode_5)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Translation> {
        let value = match r.tag()? {
            0 => Translation {
                product: r.child(decode_4)?,
                dashboard: r.child(decode_4)?,
                edition: r.child(decode_4)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_MODEL_CATALOG,
        0,
        "3a11464df15869b897e00c1b63fba5d6adbae6de332e1d3bebe902d0816b5181",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod catalog_catalog_tests {
    #[test]
    fn packaged_catalog_catalog_decodes() {
        let _ = super::read_catalog_catalog();
    }
}
// End generated catalog reader: razer-model/catalog.
