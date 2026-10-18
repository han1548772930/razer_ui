use super::*;
use crate::lighting_engine::LedInputType;

#[test]
fn complete_physical_and_edition_identities_keep_monitoring_and_docking_separate() {
    let expected = [
        (214, 0),
        (214, 128),
        (214, 129),
        (215, 0),
        (215, 128),
        (215, 129),
        (216, 0),
        (216, 128),
        (216, 129),
        (221, 0),
        (226, 0),
        (226, 128),
        (227, 0),
        (227, 128),
        (228, 0),
        (228, 128),
        (231, 0),
        (232, 0),
        (233, 0),
        (234, 0),
        (235, 0),
        (236, 0),
        (45066, 0),
        (45067, 0),
        (45068, 0),
    ];
    let mut actual = peer_identities().to_vec();
    actual.sort_unstable();
    assert_eq!(actual, expected);
    let expected_v2 = [
        (215, 0),
        (215, 128),
        (215, 129),
        (221, 0),
        (227, 0),
        (227, 128),
        (232, 0),
        (234, 0),
        (45067, 0),
    ];
    for (product, edition) in expected {
        let peer = peer_support(product, edition).unwrap();
        assert_eq!(peer.source_product_id, product);
        assert_eq!(peer.edition, edition);
        assert_eq!(peer.vendor_id, 5426);
        assert_eq!(peer.category, PeerCategory::Mouse);
        assert!(peer.support_battery_monitoring);
        assert!(!peer.support_docking_station);
        assert_eq!(
            peer.support_docking_v2,
            expected_v2.contains(&(product, edition))
        );
    }
}

#[test]
fn exact_edition_is_required_while_non_keyboard_layout_follows_source_fallback() {
    for (product, edition) in [(214, 1), (221, 128), (226, 129), (45067, 128), (241, 0)] {
        assert!(peer_support(product, edition).is_err());
        assert!(peer_manifest(product, edition, 0).is_err());
    }
    let fallback = peer_manifest(215, 129, 99).unwrap();
    assert_eq!(fallback, peer_manifest(215, 129, 0).unwrap());
    assert_eq!(fallback.led.edition, 129);
    assert_eq!(fallback.led.layout, 0);
}

#[test]
fn source_physical_manifest_and_led_ids_are_independent() {
    let edition = peer_manifest(227, 128, 0).unwrap();
    assert_eq!(edition.support.source_product_id, 227);
    assert_eq!(edition.support.manifest_product_id, 226);
    assert_eq!(
        edition.support.rz_device_name,
        PeerDeviceFactory::RzDevice25
    );
    assert_eq!(edition.led.product_id, 226);
    assert!(edition.support.support_docking_v2);
    let mouse = peer_manifest(232, 0, 0).unwrap();
    assert_eq!(mouse.support.manifest_product_id, 232);
    assert_eq!(mouse.led.product_id, 231);
    assert_eq!(peer_manifest(233, 0, 0).unwrap().led.product_id, 231);
    let hyperspeed = peer_support(234, 0).unwrap();
    assert_eq!(hyperspeed.wired_product_id, 235);
    assert_eq!(hyperspeed.dongle_product_id, 234);
    assert_eq!(hyperspeed.ble_product_id, Some(236));
}

#[test]
fn source_transport_factories_and_claim_interfaces_are_not_inferred_from_pid_order() {
    for (product, edition, factory, claim) in [
        (214, 0, PeerDeviceFactory::RzDevice25, 0),
        (215, 128, PeerDeviceFactory::RzDevice25DualLinkMouse, 0),
        (216, 129, PeerDeviceFactory::RzDevice30, 0),
        (221, 0, PeerDeviceFactory::RzDevice25Krakoff, 3),
        (227, 0, PeerDeviceFactory::RzDevice25DualLinkMouse, 3),
        (227, 128, PeerDeviceFactory::RzDevice25, 3),
        (228, 0, PeerDeviceFactory::RzDevice30, 0),
        (233, 0, PeerDeviceFactory::RzDevice30, 3),
        (234, 0, PeerDeviceFactory::RzDevice25DualLinkMouse, 3),
        (236, 0, PeerDeviceFactory::RzDevice30, 0),
        (45067, 0, PeerDeviceFactory::RzDevice25DualLinkMouse, 0),
    ] {
        let support = peer_support(product, edition).unwrap();
        assert_eq!(support.rz_device_name, factory);
        assert_eq!(support.claim_interface, claim);
        assert_eq!((support.max_retry_in, support.max_retry_out), (30, 20));
        assert_eq!(
            (
                support.sleep_time_between_in_ms,
                support.sleep_time_between_out_ms,
                support.sleep_time_between_out_in_ms
            ),
            (5, 5, 5)
        );
    }
    assert_eq!(peer_support(221, 0).unwrap().ble_product_id, None);
}

#[test]
fn complete_geometry_preserves_regions_input_entries_and_sparse_device_slots() {
    let mut input_count = 0;
    let mut region_count = 0;
    let mut region_position_count = 0;
    for &(product, edition) in peer_identities() {
        let manifest = peer_manifest(product, edition, 0).unwrap();
        input_count += manifest.led.led_input_map.len();
        region_count += manifest.led.led_regions.len();
        region_position_count += manifest
            .led
            .led_regions
            .iter()
            .map(|region| region.region_matrix.len())
            .sum::<usize>();
    }
    assert_eq!(
        (input_count, region_count, region_position_count),
        (338, 56, 296)
    );
    let basilisk = peer_manifest(215, 0, 0).unwrap().led;
    assert_eq!((basilisk.matrix_max_row, basilisk.matrix_max_col), (7, 5));
    assert_eq!((basilisk.device_max_row, basilisk.device_max_col), (1, 13));
    assert_eq!(basilisk.led_matrix.len(), 12);
    assert!(
        basilisk
            .led_matrix
            .iter()
            .all(|led| led.device_pos != [0, 12])
    );
    assert_eq!(basilisk.led_input_map[0].input_type, LedInputType::Dkm);
    assert_eq!(basilisk.led_input_map[0].input_data, [81, 16]);
    assert_eq!(basilisk.led_input_map[0].matrix_pos, [65535, 65535]);
    assert!(
        peer_manifest(232, 0, 0)
            .unwrap()
            .led
            .led_input_map
            .iter()
            .any(|mapping| mapping.input_type == LedInputType::Keyboard)
    );
}

#[test]
fn krakoff_regions_keep_source_placeholder_coordinates_and_native_json_names() {
    let led = peer_manifest(221, 0, 0).unwrap().led;
    assert_eq!(led.led_matrix.len(), 17);
    assert_eq!(
        led.led_regions
            .iter()
            .map(|region| region.region_id)
            .collect::<Vec<_>>(),
        [17, 16]
    );
    for region in &led.led_regions {
        assert_eq!(region.num_led, 49);
        assert_eq!(region.region_matrix.len(), 49);
        assert_eq!((region.max_row, region.max_col), (7, 7));
    }
    assert_eq!(led.led_regions[0].region_matrix[0].matrix_pos, [7, 7]);
    let json = serde_json::to_value(led).unwrap();
    assert!(json.get("LedRegions").is_none());
    assert_eq!(json["LedRegion"][0]["NumLed"], 49);
    assert_eq!(
        json["LedRegion"][0]["RegionMatrix"][0]["MatrixPos"],
        serde_json::json!([7, 7])
    );
    assert_eq!(json["LedInputMap"][0]["InputType"], "mou");
    assert_eq!(
        json["LedInputMap"][0]["MatrixPos"],
        serde_json::json!([65535, 65535])
    );
}

#[test]
fn native_exit_effect_uses_manifest_color_for_anniversary_family() {
    assert_eq!(
        peer_support(215, 0).unwrap().quick_effect_on_exit,
        PeerExitEffect::Spectrum
    );
    for product in [45066, 45067, 45068] {
        assert_eq!(
            peer_support(product, 0).unwrap().quick_effect_on_exit,
            PeerExitEffect::Static {
                color: [0xa3, 0xe0, 0x1f]
            }
        );
    }
}

#[test]
fn default_profile_effect_uses_the_current_middleware_profile_for_every_peer() {
    for &(product, edition) in peer_identities() {
        let support = peer_support(product, edition).unwrap();
        let expected = if [45066, 45067, 45068].contains(&product) {
            serde_json::json!({
                "selectedEffectId": 1,
                "selectedEffectSetting": {"color1": "#a3e01f"}
            })
        } else {
            serde_json::json!({"selectedEffectId": 3})
        };
        assert_eq!(
            serde_json::to_value(support.default_quick_effect()).unwrap(),
            expected
        );
    }
}

#[test]
fn default_profile_effect_is_independent_from_the_native_exit_effect() {
    let mut support = peer_support(214, 0).unwrap();
    support.quick_effect_on_exit = PeerExitEffect::Static { color: [1, 2, 3] };
    assert_eq!(
        serde_json::to_value(support.default_quick_effect()).unwrap(),
        serde_json::json!({"selectedEffectId": 3})
    );
    support.default_profile_effect = PeerProfileEffect::Static {
        color: [0x12, 0x34, 0xab],
    };
    support.quick_effect_on_exit = PeerExitEffect::Spectrum;
    assert_eq!(
        serde_json::to_value(support.default_quick_effect()).unwrap(),
        serde_json::json!({
            "selectedEffectId": 1,
            "selectedEffectSetting": {"color1": "#1234ab"}
        })
    );
}

#[test]
fn krakoff_monitor_flags_and_inclusive_effect_intervals_are_source_owned() {
    let peer = peer_manifest(221, 0, 0).unwrap();
    assert!(peer.support.skip_regions_for_charging_effect);
    assert!(peer.support.use_parent_container_id_to_set_device_handle);
    assert_eq!(peer.support.create_preview_engine_main_device, Some(false));
    assert_eq!(peer.support.send_chroma_frames_to_dock, Some(false));
    let battery = peer.battery_level_effects.unwrap();
    let charging = peer.charging_effects.unwrap();
    for (level, color) in [
        (0, 16711680),
        (24, 16711680),
        (25, 16729344),
        (49, 16729344),
        (50, 16776960),
        (74, 16776960),
        (75, 65280),
        (99, 65280),
        (100, 65280),
    ] {
        let effect = battery.for_level(level).unwrap();
        assert_eq!(effect.effect_id, 6);
        assert_eq!(effect.color, color);
        let effect = charging.for_level(level).unwrap();
        assert_eq!(effect.effect_id, if level == 100 { 6 } else { 5 });
        assert_eq!(effect.color, color);
    }
    assert!(battery.for_level(101).is_err());
    for product in [215, 227, 232, 234, 45067] {
        let ordinary = peer_manifest(product, 0, 0).unwrap();
        assert_eq!(ordinary.battery_level_effects, None);
        assert_eq!(ordinary.charging_effects, None);
        assert!(!ordinary.support.skip_regions_for_charging_effect);
        assert!(
            !ordinary
                .support
                .use_parent_container_id_to_set_device_handle
        );
        assert_eq!(ordinary.support.create_preview_engine_main_device, None);
        assert_eq!(ordinary.support.send_chroma_frames_to_dock, None);
    }
}

#[test]
fn battery_effect_construction_rejects_overlapping_or_missing_intervals() {
    assert!(PeerBatteryEffect::new(75, 101, 6, 65280).is_err());
    assert!(PeerBatteryEffect::new(75, 99, 6, 0x100_0000).is_err());
    let source = peer_manifest(221, 0, 0)
        .unwrap()
        .battery_level_effects
        .unwrap();
    let overlap = PeerBatteryEffect::new(24, 49, 6, 16729344).unwrap();
    assert!(
        PeerBatteryEffects::new(
            source.full,
            source.high,
            source.mid,
            overlap,
            source.critical
        )
        .is_err()
    );
    let gap = PeerBatteryEffect::new(26, 49, 6, 16729344).unwrap();
    assert!(
        PeerBatteryEffects::new(source.full, source.high, source.mid, gap, source.critical)
            .is_err()
    );
    assert_eq!(
        PeerBatteryEffects::new(
            source.full,
            source.high,
            source.mid,
            source.low,
            source.critical
        )
        .unwrap(),
        source
    );
}
