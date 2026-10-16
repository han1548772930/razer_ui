use super::*;
use crate::settings_page::preview_layouts;
use std::collections::BTreeSet;

#[::core::prelude::v1::test]
fn all_products_catalog_contains_every_registered_product_once() {
    let catalog = catalog();
    let expected = razer_catalog::registry()
        .iter()
        .map(|product| product.id())
        .collect::<BTreeSet<_>>();
    let actual = catalog
        .iter()
        .map(|product| product.pid)
        .collect::<BTreeSet<_>>();
    assert_eq!(catalog.len(), actual.len());
    assert_eq!(actual, expected);
    eprintln!(
        "All Products: {} registered, {} with local pages, {} without local pages, {} missing page routes",
        catalog.len(),
        catalog
            .iter()
            .filter(|product| !product.available_pages.is_empty())
            .count(),
        catalog
            .iter()
            .filter(|product| product.available_pages.is_empty())
            .count(),
        catalog
            .iter()
            .map(|product| product.missing_pages.len())
            .sum::<usize>()
    );
    for product in &catalog {
        assert_eq!(
            first_local_page(product.pid).is_some(),
            !product.available_pages.is_empty()
        );
        assert!(
            product
                .available_pages
                .iter()
                .all(|key| !product.missing_pages.contains(key))
        );
    }
    assert!(first_local_page(u32::MAX).is_none());
    for product in razer_catalog::registry() {
        assert!(!razer_pages::features::has_local_product_page(
            product.id(),
            "unimplemented-page"
        ));
        assert!(!razer_pages::features::has_local_product_page(
            product.id(),
            "HELP"
        ));
    }
}

#[::core::prelude::v1::test]
fn all_products_connected_matching_requires_exact_variant_and_live_observation() {
    use razer_model::model::DeviceConnectionObservation;
    let mut device = preview_device(70, 0, 0).unwrap();
    assert!(!matching_connected_device(&device, 70, 0, 0));
    device.dashboard.connection_observation = Some(DeviceConnectionObservation::UsbPresent);
    assert!(!matching_connected_device(&device, 70, 0, 0));
    device.serial_number = "physical-test-device".into();
    assert!(matching_connected_device(&device, 70, 0, 0));
    assert!(!matching_connected_device(&device, 71, 0, 0));
    assert!(!matching_connected_device(&device, 70, 128, 0));
    assert!(!matching_connected_device(&device, 70, 0, 1));
    device.dashboard.connection_observation = Some(DeviceConnectionObservation::ReceiverPeer(0));
    assert!(!matching_connected_device(&device, 70, 0, 0));
    device.dashboard.connection_observation = Some(DeviceConnectionObservation::ReceiverPeer(1));
    assert!(matching_connected_device(&device, 70, 0, 0));
    device.dashboard.connection_observation = None;
    assert!(!matching_connected_device(&device, 70, 0, 0));
}

#[::core::prelude::v1::test]
fn local_preview_artwork_is_owned_by_the_catalog_variant_not_connection_state() {
    use razer_model::model::DeviceConnectionObservation;
    let mut device = preview_device(563, 0, 0).unwrap();
    assert_eq!(local_preview_artwork(&device), artwork(563, 0, 0));
    assert!(local_preview_artwork(&device).is_some());

    let mut restored: Device =
        serde_json::from_value(serde_json::to_value(&device).unwrap()).unwrap();
    restored.begin_local_session();
    assert_eq!(
        local_preview_artwork(&restored),
        local_preview_artwork(&device)
    );
    device.dashboard.connection_observation = Some(DeviceConnectionObservation::UsbPresent);
    assert!(local_preview_artwork(&device).is_none());
    device.dashboard.connection_observation = None;
    device.serial_number = "physical-device".into();
    assert!(local_preview_artwork(&device).is_none());
    device.serial_number = "PREVIEW-563".into();
    device.layout_id = u32::MAX;
    assert!(local_preview_artwork(&device).is_none());
}

#[::core::prelude::v1::test]
fn persisted_legacy_keyboard_previews_use_current_default_artwork_without_mutating_identity() {
    for pid in [579, 689, 720] {
        // This is the original persisted constructor, not the new selectable
        // variant constructor which intentionally rejects layout zero.
        let legacy = product_preview::registered_preview(pid).unwrap();
        assert_eq!((legacy.edition_id, legacy.layout_id), (0, 0));
        assert!(preview_device(pid, 0, 0).is_none());
        let mut restored: Device =
            serde_json::from_slice(&serde_json::to_vec(&legacy).unwrap()).unwrap();
        restored.begin_local_session();
        let identity = (
            restored.serial_number.clone(),
            restored.device_container_id.clone(),
        );
        assert_eq!(local_preview_artwork(&restored), artwork(pid, 0, 1));
        assert!(local_preview_artwork(&restored).is_some(), "PID {pid}");
        assert_eq!((restored.edition_id, restored.layout_id), (0, 0));
        assert_eq!(
            (
                restored.serial_number.clone(),
                restored.device_container_id.clone()
            ),
            identity
        );
        assert!(restored.dashboard.connection_observation.is_none());
        assert!(restored.dashboard.readonly_values.is_none());
        restored.layout_id = u32::MAX;
        assert!(local_preview_artwork(&restored).is_none());
        restored.layout_id = 0;
        restored.serial_number = format!("PREVIEW-{pid}-0-0");
        assert!(local_preview_artwork(&restored).is_none());
    }
}

#[::core::prelude::v1::test]
fn all_products_filters_search_names_ids_and_exact_categories() {
    let catalog = catalog();
    assert_eq!(filtered(&catalog, "", "").len(), catalog.len());
    assert!(filtered(&catalog, "not-a-real-product-identity", "").is_empty());
    assert!(filtered(&catalog, "", "not-a-category").is_empty());
    for product in &catalog {
        assert!(
            filtered(&catalog, &product.pid.to_string(), "")
                .iter()
                .any(|index| catalog[*index].pid == product.pid)
        );
        for category in &product.categories {
            let results = filtered(&catalog, "", category);
            assert!(
                results
                    .iter()
                    .any(|index| catalog[*index].pid == product.pid)
            );
            assert!(
                results
                    .iter()
                    .all(|index| catalog[*index].categories.contains(category))
            );
        }
    }
    let lower = filtered(&catalog, "razer", "");
    assert_eq!(lower, filtered(&catalog, " RAZER ", ""));
    for locale in ["en", "zh-cn"] {
        let name = product_preview::source_product_name(70, preview_editions(70)[0], locale);
        assert!(
            filtered(&catalog, &name, "")
                .iter()
                .any(|index| catalog[*index].pid == 70)
        );
    }
    for product in razer_catalog::registry() {
        for edition in preview_editions(product.id()) {
            for locale in razer_model::model::LOCALES.iter().copied().chain(["zh-tw"]) {
                let name = product_preview::source_product_name(product.id(), edition, locale);
                assert!(
                    filtered(&catalog, &name, "")
                        .iter()
                        .any(|index| catalog[*index].pid == product.id()),
                    "PID {} edition {edition} locale {locale}",
                    product.id()
                );
            }
        }
    }
}

#[::core::prelude::v1::test]
fn all_products_categories_are_explicit_localized_and_never_internal_keys() {
    let locales = [
        serde_json::from_str::<serde_json::Value>(include_str!("../../../../locales/en.json"))
            .unwrap(),
        serde_json::from_str::<serde_json::Value>(include_str!("../../../../locales/zh-CN.json"))
            .unwrap(),
    ];
    assert_eq!(
        product_categories(&[]),
        vec!["LOCAL_PRODUCTS_CATEGORY_OTHER"]
    );
    assert_eq!(
        product_categories(&["productCategoryIcon", "unknown-category"]),
        vec!["LOCAL_PRODUCTS_CATEGORY_OTHER"]
    );
    assert_eq!(
        product_categories(&["productCategoryIcon", "KEYBOARD"]),
        vec!["LOCAL_PRODUCTS_CATEGORY_KEYBOARDS"]
    );
    assert_eq!(
        product_categories(&["AUDIO", "AUDIO_SPEAKER_HEAD_CUSHION_CLIO_X"]),
        vec!["LOCAL_PRODUCTS_CATEGORY_AUDIO"]
    );
    assert_eq!(
        product_categories(&["HUE", "HUE_HUE"]),
        vec!["LOCAL_PRODUCTS_CATEGORY_HUE"]
    );
    assert_eq!(
        category_key("BROADCASTER_MICROPHONE"),
        "LOCAL_PRODUCTS_CATEGORY_MICROPHONES"
    );
    assert_eq!(
        category_key("ACCESSORY_MAINBOARD"),
        "LOCAL_PRODUCTS_CATEGORY_MAINBOARDS"
    );
    assert_eq!(
        category_key("MOUSEPLUSMAT"),
        "LOCAL_PRODUCTS_CATEGORY_MOUSE_MAT_SETS"
    );
    for product in razer_catalog::registry() {
        let categories = product_categories(product.categories());
        assert!(!categories.is_empty());
        for key in categories {
            assert!(key.starts_with("LOCAL_PRODUCTS_CATEGORY_"));
            for locale in &locales {
                let label = locale[key].as_str().expect("explicit category translation");
                assert!(!label.is_empty());
                assert!(!label.contains('_'));
                assert_ne!(label, "productCategoryIcon");
                assert_ne!(label, key);
            }
        }
    }
}

#[::core::prelude::v1::test]
fn all_products_variants_have_stable_distinct_offline_identity() {
    let mut serials = BTreeSet::new();
    let mut containers = BTreeSet::new();
    let mut profiles = BTreeSet::new();
    for product in razer_catalog::registry() {
        for edition in preview_editions(product.id()) {
            for layout in preview_layouts_for_edition(product.id(), edition) {
                let device = preview_device(product.id(), edition, layout).unwrap();
                assert_eq!(device.product_id, product.id());
                assert_eq!(device.real_product_id, product.id());
                assert_eq!(device.edition_id, edition);
                assert_eq!(device.layout_id, layout);
                assert!(serials.insert(device.serial_number.clone()));
                assert!(containers.insert(device.device_container_id.clone()));
                assert!(profiles.insert(device.active_profile.clone()));
                assert!(device.serial_number.starts_with("PREVIEW-"));
                assert!(device.device_container_id.starts_with("preview-"));
                assert!(device.power_status.is_none());
                assert!(device.dashboard.connection_observation.is_none());
                assert!(
                    device
                        .profiles
                        .iter()
                        .any(|profile| profile.id == device.active_profile)
                );
                assert_eq!(
                    device.serial_number,
                    preview_device(product.id(), edition, layout)
                        .unwrap()
                        .serial_number
                );
            }
        }
        assert!(preview_device(product.id(), u32::MAX, 0).is_none());
        assert!(
            preview_device(product.id(), preview_editions(product.id())[0], u32::MAX).is_none()
        );
    }
    assert!(preview_device(u32::MAX, 0, 0).is_none());
}

#[::core::prelude::v1::test]
fn all_products_artwork_never_substitutes_unknown_editions() {
    for product in razer_catalog::registry() {
        assert!(artwork(product.id(), u32::MAX, 0).is_none());
    }
    assert!(artwork(70, 0, 0).is_some());
}

#[::core::prelude::v1::test]
fn all_products_artwork_covers_every_default_and_declared_variant_without_hiding_gaps() {
    use gpui_kit::AssetSource as _;
    let mut variants = 0;
    let mut available = 0;
    let mut missing = Vec::new();
    let mut default_missing = Vec::new();
    let mut empty_domains = Vec::new();
    for product in razer_catalog::registry() {
        let pid = product.id();
        let editions = preview_editions(pid);
        for (edition_index, edition) in editions.iter().copied().enumerate() {
            let layouts = preview_layouts_for_edition(pid, edition);
            if layouts.is_empty() {
                empty_domains.push((pid, edition));
            }
            for (layout_index, layout) in layouts.iter().copied().enumerate() {
                variants += 1;
                assert!(preview_device(pid, edition, layout).is_some());
                if let Some(asset) = artwork(pid, edition, layout) {
                    available += 1;
                    assert!(
                        razer_assets::SynapseAssets.load(asset).unwrap().is_some(),
                        "PID {pid} edition {edition} layout {layout}: {asset}"
                    );
                } else {
                    missing.push((pid, edition, layout));
                    if edition_index == 0 && layout_index == 0 {
                        default_missing.push((pid, edition, layout));
                    }
                }
            }
        }
    }
    assert_eq!(variants, available + missing.len());
    assert!(available > 0);
    let unavailable: serde_json::Value = serde_json::from_str(include_str!(
        "../../testdata/product-image-unavailability.json"
    ))
    .unwrap();
    let classified = unavailable["variants"].as_array().unwrap();
    let mut verified_unavailable = Vec::new();
    for variant in classified {
        assert_eq!(
            variant["classification"],
            "official-edition-image-unavailable"
        );
        assert!(!variant["reason"].as_str().unwrap().is_empty());
        let producer = &variant["producer"];
        assert_eq!(producer["input"]["sha256"].as_str().unwrap().len(), 64);
        let function = producer["complete_function"].as_str().unwrap();
        assert!(function.contains("this.props.editionId"));
        assert!(!function.contains("catch") && !function.contains("onError"));
        let requests = variant["dashboard_requests"].as_array().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests
                .iter()
                .map(|request| request["density"].as_u64().unwrap())
                .collect::<std::collections::BTreeSet<_>>(),
            std::collections::BTreeSet::from([1, 3]),
        );
        for request in requests {
            assert_eq!(request["product_id"], variant["product_id"]);
            assert_eq!(request["edition_id"], variant["edition_id"]);
            assert_eq!(request["layout_id"], variant["layout_id"]);
            assert!(
                request["original"]["error"]
                    .as_str()
                    .unwrap()
                    .contains("HTTP Error 404")
            );
        }
        assert!(
            variant["available_resource_editions"]
                .as_array()
                .unwrap()
                .iter()
                .all(|edition| edition != &variant["edition_id"])
        );
        assert!(!variant["resource_contexts"].as_array().unwrap().is_empty());
        verified_unavailable.push((
            variant["product_id"].as_u64().unwrap() as u32,
            variant["edition_id"].as_u64().unwrap() as u32,
            variant["layout_id"].as_u64().unwrap() as u32,
        ));
    }
    verified_unavailable.sort_unstable();
    missing.sort_unstable();
    eprintln!(
        "{}",
        serde_json::json!({
            "registered_products": razer_catalog::registry().len(),
            "declared_preview_variants": variants,
            "available_images": available,
            "missing_images": missing,
            "default_missing_images": default_missing,
            "empty_source_domains": empty_domains,
            "verified_source_unavailable": verified_unavailable,
            "runtime_acceptance": "unverified",
        })
    );
    assert!(
        empty_domains.is_empty(),
        "Unresolved source layout domains: {empty_domains:?}"
    );
    assert!(
        default_missing.is_empty(),
        "Missing default product images: {default_missing:?}"
    );
    assert_eq!(
        missing, verified_unavailable,
        "Unclassified missing image or stale unavailable evidence"
    );
}

#[::core::prelude::v1::test]
fn all_products_keyboard_layouts_belong_to_their_own_source_product() {
    assert!(preview_layouts(756).contains(&1));
    assert!(!preview_layouts(756).contains(&0));
    assert_eq!(preview_layouts(70), vec![0]);
    assert!(preview_layouts(u32::MAX).is_empty());
    for product in razer_catalog::registry() {
        let layouts =
            razer_pages::features::keyboard_products::source_preview_layouts(product.id());
        if product.id() != 653 && !layouts.is_empty() {
            assert_eq!(preview_layouts(product.id()), layouts);
        }
    }
    assert_eq!(
        preview_device(756, 0, 1).unwrap().serial_number,
        "PREVIEW-756-0-1"
    );
    assert_eq!(
        preview_device(653, 0, 1).unwrap().serial_number,
        "PREVIEW-653"
    );
}

#[::core::prelude::v1::test]
fn all_products_edition_changes_preserve_only_source_valid_layout_pairs() {
    for product in razer_catalog::registry() {
        let pid = product.id();
        for edition in preview_editions(pid) {
            let layouts = preview_layouts_for_edition(pid, edition);
            assert_eq!(
                selected_preview_variant(pid, edition, None),
                layouts.first().copied().map(|layout| (edition, layout))
            );
            for previous in preview_layouts(pid).into_iter().chain([u32::MAX]) {
                let selected = selected_preview_variant(pid, edition, Some(previous));
                if layouts.contains(&previous) {
                    assert_eq!(selected, Some((edition, previous)));
                } else {
                    assert_eq!(
                        selected,
                        layouts.first().copied().map(|layout| (edition, layout))
                    );
                    assert!(preview_device(pid, edition, previous).is_none());
                }
                if let Some((selected_edition, selected_layout)) = selected {
                    assert!(preview_device(pid, selected_edition, selected_layout).is_some());
                }
            }
            for next_edition in preview_editions(pid) {
                for previous in &layouts {
                    let next = preview_layouts_for_edition(pid, next_edition);
                    let selected = selected_preview_variant(pid, next_edition, Some(*previous));
                    assert_eq!(
                        selected.map(|variant| variant.1),
                        next.contains(previous)
                            .then_some(*previous)
                            .or_else(|| next.first().copied())
                    );
                }
            }
        }
        assert!(selected_preview_variant(pid, u32::MAX, None).is_none());
    }
    assert!(selected_preview_variant(u32::MAX, 0, None).is_none());
}

#[::core::prelude::v1::test]
fn all_products_selection_survives_filtering_by_domain_identity() {
    let catalog = Arc::new(catalog());
    let pid = catalog[0].pid;
    let mut list = ProductsList {
        catalog,
        visible: Vec::new(),
        query: String::new(),
        category: String::new(),
        selected: Some(pid),
    };
    let initial = list.refilter().unwrap();
    assert_eq!(list.product(initial).unwrap().pid, pid);
    list.query = "not-a-real-product-identity".into();
    assert!(list.refilter().is_none());
    assert_eq!(list.selected, Some(pid));
    list.query.clear();
    let restored = list.refilter().unwrap();
    assert_eq!(list.product(restored).unwrap().pid, pid);
}
