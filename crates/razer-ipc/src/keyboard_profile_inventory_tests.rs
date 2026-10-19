use super::*;
use crate::{ServiceRequest, device_tasks::Scope};
use serde_json::json;

const COLLECTION_REQUEST: &[u8] = br#"{"operation":"keyboard_profile_inventory_read","target":{"transport":"collection","node":{"path":[255,0,34,92,128],"vendor_id":5426,"product_id":679,"usage_page":65280,"usage":1,"interface_number":3,"release_number":4660,"serial_number":null,"manufacturer":null,"product":null},"product_id":679,"peer_product_id":null}}"#;

fn target() -> Target {
    let ServiceRequest::KeyboardProfileInventoryRead { target } =
        serde_json::from_slice(COLLECTION_REQUEST).unwrap()
    else {
        panic!("inventory operation changed");
    };
    target
}

fn observation() -> Observation {
    serde_json::from_value(json!({
        "target":target(),
        "inventory":{
            "max_profiles_supported":5,"declared_profile_count":1,
            "slots":[{"slot_id":4,"metadata":{"Observed":{
                "identity":{"guid":"12345678-1234-5678-1234-567812345678",
                    "guid_status":"Recognized","name":"Actual device","owner":"offline"},
                "mapping_hash":"Unobserved"
            }}}]
        }
    }))
    .unwrap()
}

#[test]
fn keyboard_profile_inventory_wire_keeps_full_collection_and_task_scope() {
    let request: ServiceRequest = serde_json::from_slice(COLLECTION_REQUEST).unwrap();
    assert_eq!(serde_json::to_vec(&request).unwrap(), COLLECTION_REQUEST);
    let Target::Collection {
        node,
        product_id,
        peer_product_id,
    } = target()
    else {
        panic!("wrong target kind");
    };
    assert_eq!(node.path, [255, 0, 34, 92, 128]);
    assert_eq!((product_id, peer_product_id), (679, None));
    assert_eq!(
        request.synchronous_device_task().unwrap(),
        Some(Scope::collection(679, &node))
    );
    let value = serde_json::to_value(request).unwrap();
    for field in ["node", "product_id"] {
        let mut incomplete = value.clone();
        incomplete["target"].as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<ServiceRequest>(incomplete).is_err());
    }
}

#[test]
fn keyboard_profile_inventory_container_wire_retains_raw_peer_identity() {
    let bytes = br#"{"operation":"keyboard_profile_inventory_read","target":{"transport":"windows_container","target":{"product_id":756,"physical_product_id":179,"peer_product_id":757,"device_container_id":"source-container","path":"source-hid"}}}"#;
    let request: ServiceRequest = serde_json::from_slice(bytes).unwrap();
    assert_eq!(serde_json::to_vec(&request).unwrap(), bytes);
    assert_eq!(
        request.synchronous_device_task().unwrap(),
        Some(Scope::container(756, "source-container").unwrap())
    );
    let ServiceRequest::KeyboardProfileInventoryRead { target } = request else {
        unreachable!()
    };
    let inventory = razer_device::keyboard_profile_inventory::ProfileInventory {
        max_profiles_supported: 5,
        declared_profile_count: 0,
        slots: Vec::new(),
    };
    let reading = Observation {
        target: target.clone(),
        inventory,
    };
    reading.validate_for(&target).unwrap();
    let Target::WindowsContainer { mut target } = target else {
        unreachable!()
    };
    target.peer_product_id = Some(756);
    assert!(
        reading
            .validate_for(&Target::WindowsContainer { target })
            .is_err()
    );
}

#[test]
fn keyboard_profile_inventory_observation_rejects_missing_or_foreign_data() {
    let reading = observation();
    reading.validate_for(&target()).unwrap();
    let Target::Collection {
        mut node,
        product_id,
        peer_product_id,
    } = target()
    else {
        unreachable!()
    };
    node.release_number += 1;
    assert!(
        reading
            .validate_for(&Target::Collection {
                node,
                product_id,
                peer_product_id
            })
            .is_err()
    );
    let frame = serde_json::to_value(reading).unwrap();
    for field in ["max_profiles_supported", "declared_profile_count", "slots"] {
        let mut incomplete = frame.clone();
        incomplete["inventory"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(serde_json::from_value::<Observation>(incomplete).is_err());
    }
    let mut malformed = frame;
    malformed["inventory"]["declared_profile_count"] = json!(2);
    assert!(
        serde_json::from_value::<Observation>(malformed)
            .unwrap()
            .validate_for(&target())
            .is_err()
    );
}

#[test]
fn keyboard_profile_inventory_rejects_unmounted_products_and_observation_substitutes() {
    let mut reading = observation();
    reading.inventory.slots[0].metadata =
        razer_device::keyboard_profile_inventory::ProfileMetadataObservation::SourceSkippedFirstSlot;
    assert!(reading.validate_for(&target()).is_err());
    let Target::Collection {
        node,
        peer_product_id,
        ..
    } = target()
    else {
        unreachable!()
    };
    assert!(
        Target::Collection {
            node,
            product_id: 515,
            peer_product_id
        }
        .validate()
        .is_err()
    );
}
