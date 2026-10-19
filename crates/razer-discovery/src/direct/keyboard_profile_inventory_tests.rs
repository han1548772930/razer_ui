use super::*;
use razer_device::backend::HidNode;
use serde_json::json;

fn target() -> Target {
    Target::Collection {
        product_id: 679,
        peer_product_id: None,
        node: HidNode {
            path: b"observed-keyboard".to_vec(),
            vendor_id: 0x1532,
            product_id: 679,
            interface_number: 3,
            usage_page: 0xff00,
            usage: 1,
            release_number: 0,
            serial_number: None,
            manufacturer: None,
            product: None,
        },
    }
}

fn device_inventory(target: &Target) -> serde_json::Value {
    json!({"target":target,"inventory":{
        "max_profiles_supported":7,"declared_profile_count":1,
        "slots":[{"slot_id":4,"metadata":{"Observed":{
            "identity":{"guid":"12345678-1234-5678-1234-567812345678",
            "guid_status":"Recognized","name":"Read from device","owner":"local-owner"},
            "mapping_hash":"Unobserved"
        }}}]
    }})
}

#[test]
fn keyboard_profile_inventory_discovery_publishes_actual_values_without_active_selection() {
    let target = target();
    let result = read_with(&target, |request| {
        let ServiceRequest::KeyboardProfileInventoryRead { target: requested } = request else {
            panic!("wrong inventory request");
        };
        assert_eq!(requested, target);
        Ok(device_inventory(&requested))
    })
    .unwrap();
    assert_eq!(result.max_profiles_supported, 7);
    assert_eq!(result.slots[0].slot_id, 4);
    let razer_device::keyboard_profile_inventory::ProfileMetadataObservation::Observed {
        identity,
        ..
    } = &result.slots[0].metadata
    else {
        panic!("missing actual metadata")
    };
    assert_eq!(identity.name, "Read from device");
}

#[test]
fn keyboard_profile_inventory_discovery_rejects_failure_missing_and_stale_results() {
    let target = target();
    assert!(read_with(&target, |_| anyhow::bail!("disconnected")).is_err());
    for missing in [json!(null), json!({"target":target})] {
        assert!(read_with(&target, |_| Ok(missing.clone())).is_err());
    }
    let mut stale = device_inventory(&target);
    stale["target"]["node"]["path"] = json!([7]);
    assert!(read_with(&target, |_| Ok(stale.clone())).is_err());
    let mut mismatch = device_inventory(&target);
    mismatch["inventory"]["declared_profile_count"] = json!(2);
    assert!(read_with(&target, |_| Ok(mismatch.clone())).is_err());
}

#[test]
fn keyboard_profile_inventory_discovery_keeps_successful_empty_inventory() {
    let target = target();
    let result = read_with(&target, |_| {
        Ok(json!({"target":target,"inventory":{
            "max_profiles_supported":5,"declared_profile_count":0,"slots":[]
        }}))
    })
    .unwrap();
    assert_eq!(result.max_profiles_supported, 5);
    assert!(result.slots.is_empty());
}
