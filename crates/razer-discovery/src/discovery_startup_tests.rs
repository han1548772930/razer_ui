//! Compile-only startup observation cases; no native queries or real waits.
use super::{receiver_peers, startup_receiver_query_with};
use serde_json::{Value, json};

#[test]
fn mika_host_discovery_keeps_both_product_owners_without_fabricating_online_state() {
    for (physical, keyboard, mouse) in [(2308, 574, 117), (2593, 2595, 2594)] {
        let usb = Ok(
            json!({"complete":true,"devices":[{"vendor_id":5426,"product_id":physical,"device_container_id":"shared-mika","serial_number":"receiver-usb-string"}]}),
        );
        let hid = Ok(json!({"complete":true,"interfaces":[]}));
        let snapshot = super::discover_with(&usb, &hid, &mut |_| {
            panic!("Physical detection does not query a fabricated peer")
        })
        .unwrap();
        assert_eq!(snapshot.devices().len(), 2);
        for product in [keyboard, mouse] {
            let observed = snapshot
                .devices()
                .iter()
                .find(|row| row.product_id() == product)
                .unwrap();
            assert_eq!(observed.physical_product_id(), physical);
            assert_eq!(observed.peer_product_id(), None);
            assert_eq!(
                observed.connection(),
                super::DeviceConnectionObservation::UsbPresent
            );
            assert_eq!(observed.transport(), Some(super::ObservedTransport::Dongle));
            assert!(observed.keyboard_serial().is_none());
        }
        assert!(snapshot.errors().is_empty());
    }
}

fn reply(status: u8) -> Value {
    json!({"devices": [
        {"product_id": 65535, "status": 0},
        {"product_id": 179, "status": 0},
        {"product_id": 183, "status": status}
    ]})
}

#[test]
fn startup_requeries_offline_peer_until_new_online_observation() {
    let mut queries = 0;
    let mut waits = Vec::new();
    let value = startup_receiver_query_with(
        179,
        || {
            queries += 1;
            Ok(reply(if queries < 3 { 0 } else { 1 }))
        },
        |duration| waits.push(duration.as_millis()),
    )
    .unwrap();
    assert_eq!(queries, 3);
    assert_eq!(waits, [400, 600]);
    assert_eq!(receiver_peers(&value).unwrap().last(), Some(&(183, 1)));
}

#[test]
fn startup_requery_is_bounded_and_preserves_real_offline_result() {
    let mut queries = 0;
    let mut waits = Vec::new();
    let value = startup_receiver_query_with(
        179,
        || {
            queries += 1;
            Ok(reply(0))
        },
        |duration| waits.push(duration.as_millis()),
    )
    .unwrap();
    assert_eq!(queries, 6);
    assert_eq!(waits, [400, 600, 800, 1000, 1200]);
    assert_eq!(receiver_peers(&value).unwrap().last(), Some(&(183, 0)));
}

#[test]
fn startup_does_not_retry_empty_unknown_or_already_online_first_peer() {
    for value in [
        json!({"devices": []}),
        reply(2),
        reply(1),
        json!({"devices": [
            {"product_id": 183, "status": 1},
            {"product_id": 180, "status": 0}
        ]}),
    ] {
        let mut queries = 0;
        let result = startup_receiver_query_with(
            179,
            || {
                queries += 1;
                Ok(value.clone())
            },
            |_| panic!("source checks only the first non-sentinel, non-self peer"),
        )
        .unwrap();
        assert_eq!(queries, 1);
        assert_eq!(result, value);
    }
}

#[test]
fn startup_failure_ends_observation_without_reusing_an_old_reply() {
    let mut queries = 0;
    let mut waits = Vec::new();
    let result = startup_receiver_query_with(
        179,
        || {
            queries += 1;
            if queries == 1 {
                Ok(reply(0))
            } else {
                anyhow::bail!("receiver disconnected during startup")
            }
        },
        |duration| waits.push(duration.as_millis()),
    );
    assert!(result.is_err());
    assert_eq!(queries, 2);
    assert_eq!(waits, [400]);
}

#[test]
fn unaudited_startup_callers_do_not_inherit_retry_policy_from_protocol_class() {
    let mut queries = 0;
    let value = startup_receiver_query_with(
        125,
        || {
            queries += 1;
            Ok(reply(0))
        },
        |_| panic!("this caller has no independently audited startup policy"),
    )
    .unwrap();
    assert_eq!(queries, 1);
    assert_eq!(value, reply(0));
}
