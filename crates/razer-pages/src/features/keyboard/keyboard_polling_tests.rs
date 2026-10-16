use super::{KeyboardPollingConnection, selection};
use serde_json::json;

#[::core::prelude::v1::test]
fn modern_ble_uses_its_own_field_and_options() {
    let config = json!({
        "POLLING_RATE": [{"content":"1000"}],
        "POLLING_RATE_WIRELESS": [{"content":"2000"}],
        "POLLING_RATE_BLUETOOTH": [{"content":"125"}]
    });
    for pid in [728, 739, 740, 742, 746] {
        let selected = selection(
            pid,
            &config,
            KeyboardPollingConnection {
                is_ble: true,
                ..Default::default()
            },
        );
        assert_eq!(selected.path, "/pollingRateBle");
        assert_eq!(selected.values(&config).unwrap()[0]["content"], "125");
        assert!(selected.bluetooth_info);
    }
}

#[::core::prelude::v1::test]
fn explicitly_unsupported_ble_polling_uses_wireless_not_wired() {
    let config = json!({
        "DeviceInfo": {"supportBluetoothPollingRate": false},
        "POLLING_RATE": [{"content":"1000"}],
        "POLLING_RATE_WIRELESS": [{"content":"2000"}]
    });
    for pid in [728, 739, 740, 742, 746] {
        let selected = selection(
            pid,
            &config,
            KeyboardPollingConnection {
                is_ble: true,
                ..Default::default()
            },
        );
        assert_eq!(selected.path, "/pollingRateWireless");
        assert_eq!(selected.values(&config).unwrap()[0]["content"], "2000");
        assert!(!selected.bluetooth_info);
    }
}

#[::core::prelude::v1::test]
fn legacy_ble_list_is_independent_of_the_wired_setter() {
    let config = json!({
        "POLLING_RATE": [{"content":"1000"}],
        "POLLING_RATE_WIRELESS": [{"content":"2000"}],
        "POLLING_RATE_BLUETOOTH": [{"content":"125"}]
    });
    let selected = selection(
        724,
        &config,
        KeyboardPollingConnection {
            is_ble: true,
            max_rate_hz: Some(1000),
            ..Default::default()
        },
    );
    assert_eq!(selected.path, "/pollingRate");
    assert_eq!(selected.values(&config).unwrap()[0]["content"], "125");
    assert!(!selected.bluetooth_info);
    assert_eq!(selected.max_rate_hz, None);
}

#[::core::prelude::v1::test]
fn missing_ble_options_fall_back_to_wired_options_only() {
    let config = json!({
        "POLLING_RATE": [{"content":"1000"}],
        "POLLING_RATE_WIRELESS": [{"content":"2000"}]
    });
    for pid in [724, 740] {
        let selected = selection(
            pid,
            &config,
            KeyboardPollingConnection {
                is_ble: true,
                ..Default::default()
            },
        );
        assert_eq!(selected.values(&config).unwrap()[0]["content"], "1000");
    }
}

#[::core::prelude::v1::test]
fn explicit_empty_mode_options_remain_empty() {
    let config = json!({
        "POLLING_RATE": [{"content":"1000"}],
        "POLLING_RATE_BLUETOOTH": []
    });
    let selected = selection(
        740,
        &config,
        KeyboardPollingConnection {
            is_ble: true,
            ..Default::default()
        },
    );
    assert!(selected.values(&config).unwrap().is_empty());
}
