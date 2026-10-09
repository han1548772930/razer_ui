//! Compile-only pure projection regressions; no native calls, timers or fixtures
//! are injected into the live application by these cases.
use super::*;

const CONTAINER: &str = "{7A1F7C04-2B84-4BA3-A66C-17023B63E99F}";

fn reply(receiver_pid: u32, peers: &[(u32, u8)]) -> Value {
    let capability = capability(receiver_pid).unwrap();
    json!({
        "query":"receiver_wireless_status_v2",
        "vendor_id":capability.vendor_id,
        "product_id":receiver_pid,
        "claim_interface":capability.claim_interface,
        "report_bytes":capability.report_bytes,
        "path":"observed-hid-collection",
        "device_instance_id":"observed-device-instance",
        "device_container_id":CONTAINER.to_ascii_lowercase(),
        "device_count":peers.len(),
        "devices":peers.iter().map(|(pid,status)| {
            json!({"product_id":pid,"status":status})
        }).collect::<Vec<_>>()
    })
}

#[test]
fn real_peer_projection_preserves_receiver_scope_and_raw_status() {
    let value = reply(179, &[(183, 1)]);
    let unchanged = value.clone();
    let projection = project_receiver_query(179, CONTAINER, &value).unwrap();
    assert_eq!(value, unchanged);
    assert_eq!(projection.receiver_product_id(), 179);
    assert_eq!(projection.container(), CONTAINER);
    assert!(projection.errors().is_empty());
    let [peer] = projection.devices() else {
        panic!("one queried peer expected")
    };
    assert_eq!(peer.product_id(), 182);
    assert_eq!(peer.container(), CONTAINER);
    assert_eq!(peer.physical_product_id(), 179);
    assert_eq!(peer.peer_product_id(), Some(183));
    assert_eq!(peer.real_product_id, 183);
    assert_eq!(
        peer.connection(),
        DeviceConnectionObservation::ReceiverPeer(1)
    );
    assert!(matches!(peer.transport(), Some(ObservedTransport::Dongle)));
    assert!(peer.read_values().is_none());
    assert!(peer.serial.is_empty());
    let payload = projection.pairing_payload().unwrap();
    assert_eq!(payload[0]["productId"], 182);
    assert_eq!(payload[0]["dongleId"], 183);
    assert_eq!(payload[0]["status"], 1);
    for field in ["editionId", "layoutId", "serialNumber", "setupStatus"] {
        assert!(payload[0].get(field).is_none());
    }
    let device = peer.clone().into_device().unwrap();
    assert!(device.serial_number.is_empty());
    assert!(matches!(
        device.setup_status,
        razer_model::model::SetupStatus::Unknown
    ));
    assert!(device.profiles.is_empty());
    assert!(!device.features_initialized);
}

#[test]
fn empty_sentinel_and_standalone_self_rows_do_not_create_peers() {
    for peers in [vec![], vec![(65535, 1), (179, 1), (65535, 0)]] {
        let projection = project_receiver_query(179, CONTAINER, &reply(179, &peers)).unwrap();
        assert!(projection.devices().is_empty());
        assert!(projection.errors().is_empty());
        assert_eq!(projection.pairing_payload().unwrap(), json!([]));
    }
}

#[test]
fn an_actual_product_dongle_self_row_is_its_wireless_peer() {
    let value = reply(227, &[(227, 1)]);
    let projection = project_receiver_query(227, CONTAINER, &value).unwrap();
    let [peer] = projection.devices() else {
        panic!("source dongle identity must survive the standalone-self filter")
    };
    assert_eq!(peer.product_id(), 226);
    assert_eq!(peer.physical_product_id(), 227);
    assert_eq!(peer.peer_product_id(), Some(227));
    assert_eq!(projection.pairing_payload().unwrap()[0]["productId"], 226);
    assert_eq!(pairing_payload(227, &value).unwrap()[0]["dongleId"], 227);
}

#[test]
fn offline_and_unknown_statuses_remain_observations_without_telemetry() {
    for status in [0, 2, 3, 255] {
        let projection =
            project_receiver_query(179, CONTAINER, &reply(179, &[(183, status)])).unwrap();
        assert_eq!(
            projection.devices()[0].connection(),
            DeviceConnectionObservation::ReceiverPeer(status)
        );
        assert!(projection.devices()[0].read_values().is_none());
        assert_eq!(projection.pairing_payload().unwrap()[0]["status"], status);
    }
}

#[test]
fn unknown_and_ambiguous_catalog_rows_keep_other_real_peers_partial() {
    let value = reply(179, &[(183, 1), (60000, 1), (2593, 1)]);
    let projection = project_receiver_query(179, CONTAINER, &value).unwrap();
    assert_eq!(projection.devices().len(), 1);
    assert_eq!(projection.devices()[0].product_id(), 182);
    assert_eq!(projection.errors().len(), 2);
    assert!(projection.pairing_payload().is_err());
}

#[test]
fn distinct_raw_rows_cannot_choose_one_status_for_the_same_product() {
    // This keyboard capability accepts a product PID as well as scalar dongle
    // PID: 182 and 183 both map to product 182, while 227 remains product 226.
    let projection =
        project_receiver_query(625, CONTAINER, &reply(625, &[(182, 0), (183, 1), (227, 1)]))
            .unwrap();
    assert_eq!(projection.devices().len(), 1);
    assert_eq!(projection.devices()[0].product_id(), 226);
    assert_eq!(projection.errors().len(), 1);
    assert!(projection.pairing_payload().is_err());
}

#[test]
fn mismatched_scope_and_unverified_interface_envelopes_are_rejected() {
    for (field, invalid) in [
        ("query", json!("other_query")),
        ("vendor_id", json!(1)),
        ("product_id", json!(227)),
        ("claim_interface", json!(7)),
        ("report_bytes", json!(90)),
        (
            "device_container_id",
            json!("{5A1F7C04-2B84-4BA3-A66C-17023B63E99F}"),
        ),
        ("path", json!("")),
        ("device_instance_id", Value::Null),
        ("device_count", json!(0)),
    ] {
        let mut value = reply(179, &[(183, 1)]);
        value[field] = invalid;
        assert!(
            project_receiver_query(179, CONTAINER, &value).is_err(),
            "{field}"
        );
    }
    for invalid in [
        "",
        "PREVIEW-receiver",
        "{00000000-0000-0000-0000-000000000000}",
    ] {
        assert!(project_receiver_query(179, invalid, &reply(179, &[])).is_err());
    }
    assert!(project_receiver_query(60000, CONTAINER, &reply(179, &[])).is_err());
}

#[test]
fn malformed_or_duplicate_rows_never_become_successful_empty_observations() {
    for rows in [
        json!([{"product_id":183}]),
        json!([{"product_id":183,"status":256}]),
        json!([{"product_id":0,"status":1}]),
        json!([{"product_id":65536,"status":1}]),
        json!([{"product_id":183,"status":1},{"product_id":183,"status":0}]),
    ] {
        let mut value = reply(179, &[]);
        value["device_count"] = json!(rows.as_array().unwrap().len());
        value["devices"] = rows;
        assert!(project_receiver_query(179, CONTAINER, &value).is_err());
    }
}
