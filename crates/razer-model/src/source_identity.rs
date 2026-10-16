use razer_device::keyboard_metadata::KeyboardMetadataReading;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Deserialize)]
struct Source {
    product_id: u32,
    policy: String,
    category: String,
    available_layout_ids: Vec<u32>,
    available_edition_ids: Vec<u32>,
    runtime_default_layout_id: u32,
    is_chroma_device: bool,
}

fn source(product_id: u32) -> Option<&'static Source> {
    static DATA: OnceLock<Vec<Source>> = OnceLock::new();
    DATA.get_or_init(|| crate::data::source_identity_data!())
        .iter()
        .find(|source| source.product_id == product_id)
}

pub fn variant(product_id: u32, edition: u32, layout: u32, previous_layout: u32) -> (u32, u32) {
    let Some(source) = source(product_id) else {
        return (edition, layout);
    };
    (
        if source.available_edition_ids.contains(&edition) {
            edition
        } else {
            0
        },
        if source.available_layout_ids.contains(&layout) {
            layout
        } else if source.available_layout_ids.contains(&previous_layout) {
            previous_layout
        } else {
            source.runtime_default_layout_id
        },
    )
}

pub fn keyboard_publication(
    product_id: u32,
    serial: &str,
    reading: &KeyboardMetadataReading,
    previous_layout: u32,
    use_ble: bool,
) -> Option<(&'static str, Value)> {
    let source = source(product_id)?;
    if serial.is_empty() {
        return None;
    }
    let (edition, layout) = variant(
        product_id,
        reading.edition_id.into(),
        reading.keyboard_layout.into(),
        previous_layout,
    );
    let translations = crate::source_names::catalog()
        .get(&product_id)?
        .get(&edition)?;
    let names = |field: fn(&crate::source_names::Translation) -> &str| {
        translations
            .iter()
            .map(|(locale, text)| (locale.clone(), field(text).to_owned()))
            .collect::<BTreeMap<_, _>>()
    };
    Some((
        &source.policy,
        json!({
            "productId":product_id, "serialNumber":serial, "editionId":edition, "layoutId":layout,
            "category":source.category, "isChromaDevice":source.is_chroma_device && !use_ble, "name":names(|text| &text.dashboard),
            "productName":names(|text| &text.product), "editionName":names(|text| &text.edition),
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::{keyboard_publication, variant};
    use razer_device::keyboard_metadata::KeyboardMetadataReading;

    #[test]
    fn current_keyboard_source_normalizes_presentation_without_rewriting_raw_read() {
        let reading = KeyboardMetadataReading {
            keyboard_layout: 255,
            edition_id: 255,
            firmware_id: None,
        };
        assert_eq!(variant(691, 255, 255, 6), (0, 6));
        assert_eq!(variant(691, 255, 255, 0), (0, 1));
        let (_, record) =
            keyboard_publication(691, "real-source-serial", &reading, 6, false).unwrap();
        assert_eq!(record["layoutId"], 6);
        assert_eq!(record["editionId"], 0);
        assert_eq!(record["category"], "KEYBOARD");
        assert_eq!(reading.keyboard_layout, 255);
        assert_eq!(record["isChromaDevice"], true);
        assert_eq!(
            keyboard_publication(691, "real-source-serial", &reading, 6, true)
                .unwrap()
                .1["isChromaDevice"],
            false
        );
        for key in ["setupStatus", "connected"] {
            assert!(record.get(key).is_none());
        }
        assert!(keyboard_publication(691, "", &reading, 0, false).is_none());
        assert!(keyboard_publication(u32::MAX, "serial", &reading, 0, false).is_none());
    }
}
