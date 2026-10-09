//! Source product names are presentation data, never discovery observations.
use crate::{i18n, model::LocalizedText};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Deserialize)]
struct Translation {
    product: String,
    #[serde(default)]
    dashboard: String,
    #[serde(default)]
    edition: String,
}
type Catalog = BTreeMap<u32, BTreeMap<u32, BTreeMap<String, Translation>>>;

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("preview_names.json"))
            .expect("validated current preview product names")
    })
}

fn translation(pid: u32, edition: u32, locale: &str) -> Option<&'static Translation> {
    let values = catalog().get(&pid)?.get(&edition)?;
    values
        .get(&locale.to_lowercase())
        .or_else(|| values.get("en"))
}

fn default_edition(pid: u32) -> u32 {
    crate::product::registered(pid)
        .and_then(|product| product.edition_ids().first().copied())
        .unwrap_or(0)
}

pub(crate) fn product_name(pid: u32, edition: u32, locale: &str) -> String {
    translation(pid, edition, locale)
        .map(|value| value.product.clone())
        .or_else(|| crate::product::registered(pid).map(|product| product.name().to_owned()))
        .unwrap_or_else(|| pid.to_string())
}

/// Keep both source names searchable; numeric product identity is unchanged.
pub(crate) fn preview_product_label(pid: u32) -> String {
    let edition = default_edition(pid);
    let mut names = vec![product_name(pid, edition, &i18n::locale())];
    for locale in ["en", "zh-cn"] {
        let name = product_name(pid, edition, locale);
        if !names.contains(&name) {
            names.push(name);
        }
    }
    format!("{pid} · {}", names.join(" · "))
}

pub(crate) fn preview_edition_label(pid: u32, edition: u32) -> String {
    if let Some(value) = translation(pid, edition, &i18n::locale()) {
        if !value.edition.is_empty() {
            return format!("{edition} · {}", value.edition);
        }
        if value.product != product_name(pid, default_edition(pid), &i18n::locale()) {
            return format!("{edition} · {}", value.product);
        }
    }
    if edition == 0 {
        if i18n::locale().eq_ignore_ascii_case("zh-cn") {
            "默认 (0)".into()
        } else {
            "Default (0)".into()
        }
    } else {
        edition.to_string()
    }
}

/// Apply source display strings to an already explicit preview identity only.
/// This does not touch readiness, runtime state, hardware features or profiles.
pub(crate) fn apply_preview_names(device: &mut crate::model::Device) {
    if !device.serial_number.starts_with("PREVIEW-")
        || !device.device_container_id.starts_with("preview-")
    {
        return;
    }
    let pid = device.product_id;
    let edition = device.edition_id;
    let mut names = BTreeMap::new();
    let mut products = BTreeMap::new();
    let mut editions = BTreeMap::new();
    let mut locales = crate::model::LOCALES.to_vec();
    locales.push("zh-tw");
    for locale in locales {
        let product = product_name(pid, edition, locale);
        let source = translation(pid, edition, locale);
        let dashboard = source
            .filter(|value| !value.dashboard.is_empty())
            .map_or(product.as_str(), |value| value.dashboard.as_str());
        let marker = if locale.starts_with("zh") {
            "预览"
        } else {
            "preview"
        };
        // Preserve variant identity even where the official edition has no label.
        let variant = if edition != default_edition(pid) || device.layout_id != 0 {
            format!(" · edition {edition} / layout {}", device.layout_id)
        } else {
            String::new()
        };
        names.insert(locale.into(), format!("{dashboard} · {marker}{variant}"));
        products.insert(locale.into(), format!("{product} · {marker}{variant}"));
        if let Some(source) = source.filter(|value| !value.edition.is_empty()) {
            editions.insert(locale.into(), source.edition.clone());
        }
    }
    device.name = LocalizedText { values: names };
    device.product_name = LocalizedText { values: products };
    device.dashboard.edition_name =
        (!editions.is_empty()).then_some(LocalizedText { values: editions });
}
