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
    DATA.get_or_init(|| make_source_identity_table())
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

// Generated typed table: razer-model/source_identity_data.
#[rustfmt::skip]
fn make_source_identity_table() -> Vec<Source> {
    vec![
        Source{
            product_id: 515u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                10u32,
                11u32,
                12u32,
                15u32,
                16u32
            ],
            available_edition_ids: vec![0u32,1u32,2u32,3u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 521u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,6u32,7u32,16u32,17u32,18u32],
            available_edition_ids: vec![0u32,1u32,128u32,129u32,130u32,131u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 529u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,5u32,6u32,7u32],
            available_edition_ids: vec![0u32,1u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 534u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                11u32,
                12u32,
                16u32,
                17u32
            ],
            available_edition_ids: vec![0u32,128u32,129u32,130u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 542u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                15u32,
                16u32,
                17u32,
                18u32
            ],
            available_edition_ids: vec![0u32,128u32,129u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 545u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                15u32,
                16u32,
                17u32,
                18u32
            ],
            available_edition_ids: vec![0u32,128u32,129u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 550u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,5u32,6u32,7u32,9u32,11u32,12u32,16u32],
            available_edition_ids: vec![0u32,128u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 551u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                9u32,
                10u32,
                11u32,
                12u32,
                15u32,
                16u32,
                17u32,
                18u32
            ],
            available_edition_ids: vec![0u32,128u32,129u32,130u32,131u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 552u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                0u32,
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                15u32,
                16u32,
                17u32,
                18u32
            ],
            available_edition_ids: vec![0u32,128u32,129u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 554u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                15u32,
                16u32,
                17u32,
                18u32
            ],
            available_edition_ids: vec![0u32,128u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 555u32,
            policy: "replace_product".into(),
            category: "KEYPAD".into(),
            available_layout_ids: vec![1u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 556u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 565u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,6u32,12u32],
            available_edition_ids: vec![0u32,128u32,129u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: false
        },
        Source{
            product_id: 567u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                15u32,
                16u32,
                17u32,
                18u32
            ],
            available_edition_ids: vec![0u32,130u32,131u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: false
        },
        Source{
            product_id: 574u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 575u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                16u32,
                17u32,
                18u32,
                19u32
            ],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 577u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                15u32,
                16u32,
                17u32,
                18u32,
                19u32
            ],
            available_edition_ids: vec![0u32,128u32,129u32,130u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 579u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,5u32,6u32,7u32,12u32,16u32],
            available_edition_ids: vec![0u32,128u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 580u32,
            policy: "replace_product".into(),
            category: "KEYPAD".into(),
            available_layout_ids: vec![1u32],
            available_edition_ids: vec![0u32,128u32],
            runtime_default_layout_id: 0u32,
            is_chroma_device: true
        },
        Source{
            product_id: 585u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: false
        },
        Source{
            product_id: 590u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                15u32,
                16u32,
                17u32,
                18u32,
                19u32
            ],
            available_edition_ids: vec![0u32,128u32,129u32,130u32,131u32,132u32,133u32,134u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 591u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32],
            available_edition_ids: vec![0u32,128u32,129u32,130u32,131u32,132u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: false
        },
        Source{
            product_id: 592u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: false
        },
        Source{
            product_id: 593u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: false
        },
        Source{
            product_id: 599u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                9u32,
                10u32,
                11u32,
                12u32,
                15u32,
                16u32,
                17u32,
                18u32
            ],
            available_edition_ids: vec![0u32,128u32,129u32,130u32,131u32,132u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 600u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,5u32,6u32,7u32,8u32,9u32,12u32,15u32,16u32],
            available_edition_ids: vec![0u32,128u32,129u32,130u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 602u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                15u32,
                16u32,
                17u32,
                18u32
            ],
            available_edition_ids: vec![0u32,128u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 605u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                13u32,
                14u32,
                15u32,
                16u32,
                17u32,
                18u32,
                19u32,
                20u32
            ],
            available_edition_ids: vec![0u32,128u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 606u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                10u32,
                11u32,
                12u32,
                15u32,
                16u32,
                17u32,
                18u32
            ],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 614u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,5u32,6u32,7u32,12u32,15u32,16u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 617u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![12u32],
            available_edition_ids: vec![0u32,128u32,129u32,130u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 619u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                12u32,
                14u32,
                15u32,
                16u32
            ],
            available_edition_ids: vec![0u32,128u32,129u32,130u32,132u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 620u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,5u32,6u32,7u32,8u32,12u32,15u32,16u32],
            available_edition_ids: vec![0u32,128u32,129u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 631u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,6u32,7u32,8u32,9u32,12u32,16u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: false
        },
        Source{
            product_id: 642u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,6u32,7u32,8u32,9u32,12u32],
            available_edition_ids: vec![0u32,128u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 647u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                15u32,
                16u32,
                17u32,
                18u32
            ],
            available_edition_ids: vec![0u32,128u32,129u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 653u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                0u32,
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                15u32,
                16u32,
                17u32,
                18u32
            ],
            available_edition_ids: vec![0u32,128u32,129u32,130u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 654u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: false
        },
        Source{
            product_id: 655u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                13u32,
                14u32,
                15u32,
                16u32,
                17u32,
                18u32,
                19u32,
                20u32
            ],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 658u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                15u32,
                16u32,
                17u32,
                18u32
            ],
            available_edition_ids: vec![0u32,128u32,129u32,130u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 659u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                14u32,
                15u32,
                16u32,
                17u32,
                18u32,
                20u32
            ],
            available_edition_ids: vec![0u32,128u32,129u32,130u32,131u32,132u32,133u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 660u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                13u32,
                14u32,
                15u32,
                16u32,
                17u32,
                18u32,
                19u32,
                20u32
            ],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 661u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                15u32,
                16u32,
                17u32,
                18u32
            ],
            available_edition_ids: vec![0u32,128u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 664u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                15u32,
                16u32,
                17u32,
                18u32
            ],
            available_edition_ids: vec![0u32,128u32,130u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 673u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                13u32,
                14u32,
                15u32,
                16u32,
                17u32,
                18u32,
                19u32,
                20u32,
                21u32
            ],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 674u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                13u32,
                14u32,
                15u32,
                16u32,
                17u32,
                18u32,
                19u32,
                20u32,
                21u32
            ],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 675u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                7u32,
                6u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                13u32,
                14u32,
                15u32,
                16u32,
                17u32,
                18u32,
                21u32
            ],
            available_edition_ids: vec![
                0u32,
                128u32,
                129u32,
                130u32,
                131u32,
                132u32,
                133u32,
                134u32,
                135u32,
                136u32,
                137u32
            ],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 677u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,6u32,8u32,9u32,12u32],
            available_edition_ids: vec![0u32,128u32,129u32,130u32,131u32,132u32,133u32,134u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 678u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,6u32,7u32,8u32,9u32,10u32,12u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 679u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,6u32,7u32,8u32,9u32,12u32],
            available_edition_ids: vec![0u32,128u32,130u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 688u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,6u32,7u32,8u32,9u32,12u32],
            available_edition_ids: vec![0u32,128u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 689u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,2u32,4u32,8u32,10u32,16u32,18u32],
            available_edition_ids: vec![0u32,128u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 691u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,6u32],
            available_edition_ids: vec![0u32,128u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 697u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,5u32,6u32,7u32,8u32,9u32,12u32,15u32,16u32],
            available_edition_ids: vec![0u32,128u32,129u32,130u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 708u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 716u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,5u32,6u32,7u32,8u32,9u32,12u32,15u32,16u32],
            available_edition_ids: vec![0u32,128u32,129u32,130u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 717u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                15u32,
                16u32,
                17u32,
                18u32
            ],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 719u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,6u32,9u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 720u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,6u32,7u32,9u32,12u32],
            available_edition_ids: vec![0u32,128u32,129u32,130u32,131u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 721u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,6u32,9u32,12u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 724u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,6u32,7u32,9u32,12u32,16u32],
            available_edition_ids: vec![0u32,128u32,129u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 727u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,6u32,9u32,16u32],
            available_edition_ids: vec![0u32,129u32,130u32,131u32,132u32,133u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 728u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,6u32,7u32,8u32,9u32,10u32,12u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 739u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 740u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,6u32,7u32,9u32,12u32,15u32,16u32,20u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 741u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,8u32,9u32,15u32,16u32,18u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 742u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 746u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32,3u32,4u32,6u32,7u32,8u32,9u32,12u32,20u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 747u32,
            policy: "replace_product".into(),
            category: "KEYPAD".into(),
            available_layout_ids: vec![1u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 0u32,
            is_chroma_device: true
        },
        Source{
            product_id: 752u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 756u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 2595u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![1u32],
            available_edition_ids: vec![0u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        },
        Source{
            product_id: 2596u32,
            policy: "replace_product".into(),
            category: "KEYBOARD".into(),
            available_layout_ids: vec![
                1u32,
                2u32,
                3u32,
                4u32,
                5u32,
                6u32,
                7u32,
                8u32,
                9u32,
                10u32,
                11u32,
                12u32,
                15u32,
                16u32,
                17u32,
                18u32
            ],
            available_edition_ids: vec![0u32,128u32,130u32,131u32],
            runtime_default_layout_id: 1u32,
            is_chroma_device: true
        }
    ]
}
