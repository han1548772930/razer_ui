use super::*;
use crate::{ServiceRequest, device_tasks::Scope};
use razer_device::keyboard_onboard_profiles::{ImportDetails, KeyIdentity, Snapshot};
use razer_device::keyboard_profile_inventory::ProfileInventory;
use razer_device::onboard_macro::{MacroCatalog, MacroInventory, MacroStorageInfo};

const REQUEST: &[u8] = br#"{"operation":"keyboard_onboard_profiles_read","target":{"transport":"windows_container","target":{"product_id":679,"physical_product_id":679,"peer_product_id":null,"device_container_id":"observed-container","path":"observed-hid"}},"options":{"layout_id":1,"locally_known_macro_guids":["local-macro"]}}"#;

fn request() -> (ServiceRequest, Target, ReadOptions) {
    let request: ServiceRequest = serde_json::from_slice(REQUEST).unwrap();
    let ServiceRequest::KeyboardOnboardProfilesRead { target, options } = &request else {
        panic!("wrong operation")
    };
    (request.clone(), target.clone(), options.clone())
}

fn observation() -> Observation {
    let (_, target, options) = request();
    Observation {
        target,
        options,
        reading: CompleteReading {
            snapshot: Snapshot {
                product_id: 679,
                layout_id: 1,
                key_inventory: razer_device::keyboard_onboard_inventory::current_679_inventory(1)
                    .iter()
                    .map(|key| KeyIdentity {
                        input_id: key.input_id.into(),
                        firmware_key_id: key.firmware_key_id,
                        ignore_normal: key.ignore_normal,
                        ignore_hypershift: key.ignore_hypershift,
                    })
                    .collect(),
                macros: MacroCatalog {
                    inventory: MacroInventory {
                        storage: MacroStorageInfo {
                            max_macro_id: 32767,
                            max_memory: 65536,
                            available_memory: 45678,
                            recyclable_memory: 123,
                        },
                        count: 0,
                        ids: Vec::new(),
                        pages: Vec::new(),
                    },
                    macros: Vec::new(),
                },
                inventory: ProfileInventory {
                    max_profiles_supported: 6,
                    declared_profile_count: 0,
                    slots: Vec::new(),
                },
                profiles: Vec::new(),
            },
            import_details: ImportDetails {
                macros: Vec::new(),
                continuous_rapid_trigger: Vec::new(),
            },
        },
    }
}

#[test]
fn keyboard_onboard_profiles_wire_keeps_request_identity_and_exclusive_scope() {
    let (request, target, options) = request();
    assert_eq!(serde_json::to_vec(&request).unwrap(), REQUEST);
    assert_eq!(
        request.synchronous_device_task().unwrap(),
        Some(Scope::container(679, "observed-container").unwrap())
    );
    observation().validate_for(&target, &options).unwrap();
    for name in ["layout_id", "locally_known_macro_guids"] {
        let mut malformed = serde_json::to_value(&request).unwrap();
        malformed["options"].as_object_mut().unwrap().remove(name);
        assert!(serde_json::from_value::<ServiceRequest>(malformed).is_err());
    }
}

#[test]
fn keyboard_onboard_profiles_wire_rejects_foreign_owner_layout_and_incomplete_snapshot() {
    let (_, target, options) = request();
    let reading = observation();
    assert_eq!(
        reading
            .reading
            .snapshot
            .macros
            .inventory
            .storage
            .available_memory,
        45678
    );
    let mut wrong_options = options.clone();
    wrong_options.layout_id = 2;
    assert!(reading.validate_for(&target, &wrong_options).is_err());
    let mut wrong_owner = target.clone();
    let Target::WindowsContainer { target: foreign } = &mut wrong_owner else {
        unreachable!()
    };
    foreign.device_container_id = "another-container".into();
    assert!(reading.validate_for(&wrong_owner, &options).is_err());
    let mut missing_keys = observation();
    missing_keys.reading.snapshot.key_inventory.pop();
    assert!(missing_keys.validate_for(&target, &options).is_err());
    let mut missing_profile = observation();
    missing_profile
        .reading
        .snapshot
        .inventory
        .declared_profile_count = 1;
    assert!(missing_profile.validate_for(&target, &options).is_err());
    let mut fake_details = observation();
    fake_details
        .reading
        .import_details
        .continuous_rapid_trigger
        .push(
            razer_device::keyboard_onboard_profiles::ContinuousRapidTrigger {
                slot_id: 1,
                raw_enabled: razer_device::keyboard_onboard_profiles::Reading::Observed(1),
            },
        );
    assert!(fake_details.validate_for(&target, &options).is_err());
}

#[test]
fn keyboard_onboard_profiles_wire_never_claims_unmigrated_product_or_radio_branches() {
    let (request, _, _) = request();
    for physical in [756, 757, 179] {
        let mut malformed = serde_json::to_value(&request).unwrap();
        malformed["target"]["target"]["product_id"] = serde_json::json!(756);
        malformed["target"]["target"]["physical_product_id"] = serde_json::json!(physical);
        let request: ServiceRequest = serde_json::from_value(malformed).unwrap();
        assert!(request.synchronous_device_task().is_err());
    }
}
