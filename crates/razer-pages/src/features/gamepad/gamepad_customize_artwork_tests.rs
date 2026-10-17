//! Pure source selection/resource closure coverage; no window or device runs.
use super::super::resource_fixtures as fixtures;
use super::*;

#[::core::prelude::v1::test]
fn all_current_controller_images_have_manifest_entries_and_source_dimensions() {
    // Only original PNG IHDR bytes/SVG root tags are needed for these geometry
    // assertions. Full resource closure is checked by the resource manifest;
    // pure tests never queue a runtime download or inspect an installation cache.
    let entries = razer_assets::list_resources("synapse/")
        .unwrap()
        .into_iter()
        .map(|key| key.to_string())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(atlas().products.len(), 8);
    for product in &atlas().products {
        let controls = &source_product(product.product_id).unwrap().button_controls;
        assert_eq!(product.lines.len(), controls.len());
        let all_buttons: Vec<_> = product
            .left_buttons
            .iter()
            .chain(&product.right_buttons)
            .collect();
        assert_eq!(all_buttons.len(), controls.len());
        for control in controls {
            let id = control["inputID"].as_str().unwrap();
            assert_eq!(
                all_buttons
                    .iter()
                    .filter(|button| button.as_str() == id)
                    .count(),
                1
            );
            assert_eq!(
                product
                    .lines
                    .iter()
                    .filter(|line| line.input_id == id)
                    .count(),
                1
            );
        }
        for image in product.images.iter().chain(&product.mkeys) {
            assert!(entries.contains(&image.asset), "{}", image.asset);
            let loaded = fixtures::GAMEPAD_HEADERS
                .iter()
                .find(|(alias, _)| *alias == image.asset)
                .expect("original image header fixture")
                .1;
            assert!(image.width > 0. && image.height > 0.);
            if image.asset.ends_with(".png") {
                assert!(loaded.starts_with(b"\x89PNG\r\n\x1a\n"), "{}", image.asset);
                let width = u32::from_be_bytes(loaded[16..20].try_into().unwrap());
                let height = u32::from_be_bytes(loaded[20..24].try_into().unwrap());
                assert_eq!((width as f32, height as f32), (image.width, image.height));
            } else {
                assert!(
                    std::str::from_utf8(&loaded).unwrap().contains("<svg"),
                    "{}",
                    image.asset
                );
            }
        }
    }
}

#[::core::prelude::v1::test]
fn numeric_layout_and_new_gamepad_prd_branches_follow_the_actual_producer() {
    for pid in [2629, 2636, 2647, 2650, 4133] {
        let product = product(pid);
        assert!(!product.normalize_to_prd);
        let normal = product.image(0, 0, 1.).unwrap();
        let large = product.image(0, 0, 2.).unwrap();
        assert_eq!(normal.resolution, 1);
        assert_eq!(large.resolution, 3);
        assert_ne!(normal.asset, large.asset);
        assert_eq!(product.image(9999, 0, 1.).unwrap().asset, normal.asset);
        assert!(product.image(0, 1, 1.).is_none());
        assert!(product.image(0, 9999, 1.).is_none());
    }
    for pid in [2676, 2684, 4144] {
        let product = product(pid);
        assert!(product.normalize_to_prd);
        let image = product.image(0, 0, 1.).unwrap();
        assert!(image.asset.ends_with(".svg"));
        assert!(image.layout_id.is_none());
        assert_eq!(product.image(9999, 99, 2.).unwrap().asset, image.asset);
    }
}

#[::core::prelude::v1::test]
fn mkeys_use_the_source_nonzero_and_requested_edition_rules_independently() {
    for pid in [2629, 2636, 2647, 2650, 4133] {
        let product = product(pid);
        assert_ne!(
            product.mkeys(0).unwrap().asset,
            product.mkeys(128).unwrap().asset
        );
        assert_eq!(
            product.mkeys(129).unwrap().asset,
            product.mkeys(128).unwrap().asset
        );
        assert_eq!(
            product.mkeys(9999).unwrap().asset,
            product.mkeys(128).unwrap().asset
        );
    }
    for pid in [2676, 2684, 4144] {
        let product = product(pid);
        assert_eq!(
            product.mkeys(9999).unwrap().asset,
            product.mkeys(0).unwrap().asset
        );
    }
}

#[::core::prelude::v1::test]
fn share_callouts_keep_product_specific_columns_bends_and_layers() {
    for (pid, bend) in [(2647, 90.), (2650, 60.)] {
        let product = product(pid);
        assert!(product.two_canvas_layers);
        assert_eq!(product.right_buttons.last().unwrap(), "SHARE_BUTTON");
        let share = product
            .lines
            .iter()
            .find(|line| line.input_id == "SHARE_BUTTON")
            .unwrap();
        assert_eq!(
            (share.side.as_str(), share.start_y, share.bend),
            ("right", 286., 120.)
        );
        assert!(above(share, false));
        for line in product
            .lines
            .iter()
            .filter(|line| line.input_id != "SHARE_BUTTON")
        {
            assert_eq!(line.bend, bend);
            assert!(!above(line, false));
            assert!(above(line, true));
        }
    }
    for pid in [2676, 2684] {
        let product = product(pid);
        assert!(!product.two_canvas_layers);
        assert_eq!(
            product.left_buttons,
            ["BUTTON_M1", "SHARE_BUTTON", "BUTTON_M3", "BUTTON_M5"]
        );
        let share = product
            .lines
            .iter()
            .find(|line| line.input_id == "SHARE_BUTTON")
            .unwrap();
        assert_eq!(
            (share.side.as_str(), share.start_y, share.x, share.y),
            ("left", 136., 200., 158.)
        );
        assert_eq!(product.geometry.left_bottom_y, 120.);
    }
    let signature = product(4144);
    assert_eq!(
        (signature.geometry.mkeys_left, signature.geometry.mkeys_top),
        (263., 37.)
    );
    let top = signature
        .lines
        .iter()
        .find(|line| line.input_id == "BUTTON_M1")
        .unwrap();
    assert_eq!((top.x, top.y, top.bend), (125., 50., 55.));
}
