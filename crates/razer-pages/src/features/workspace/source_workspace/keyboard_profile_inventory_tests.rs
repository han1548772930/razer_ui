use super::*;
use razer_device::{
    backend::HidNode,
    keyboard_profile_inventory::{
        GuidStatus, MappingHashObservation, ProfileIdentity, ProfileMetadataObservation,
        ProfileSlot,
    },
};

fn target(pid: u32, path: &[u8]) -> inventory::Target {
    let cap = razer_device::keyboard_metadata::hid_capability(pid, pid).unwrap();
    inventory::Target::Collection {
        node: HidNode {
            path: path.to_vec(),
            vendor_id: cap.vendor_id,
            product_id: pid as u16,
            interface_number: i32::from(cap.claim_interface_for(pid).unwrap()),
            usage_page: 0xff00,
            usage: 1,
            release_number: 0,
            serial_number: None,
            manufacturer: None,
            product: None,
        },
        product_id: pid,
        peer_product_id: None,
    }
}

fn request(owner: &mut Owner, pid: u32, generation: u64) -> (uuid::Uuid, Arc<AtomicBool>) {
    let id = uuid::Uuid::new_v4();
    let canceled = Arc::new(AtomicBool::new(false));
    owner.begin(Request {
        id,
        generation,
        product_id: pid,
        options: None,
        canceled: canceled.clone(),
    });
    (id, canceled)
}

fn observation() -> inventory::Observation {
    inventory::Observation {
        target: target(550, b"physical-keyboard"),
        inventory: ProfileInventory {
            max_profiles_supported: 7,
            declared_profile_count: 2,
            slots: vec![
                ProfileSlot {
                    slot_id: 9,
                    metadata: ProfileMetadataObservation::SourceSkippedFirstSlot,
                },
                ProfileSlot {
                    slot_id: 4,
                    metadata: ProfileMetadataObservation::Observed {
                        identity: ProfileIdentity {
                            guid: "12345678-1234-5678-1234-567812345678".into(),
                            guid_status: GuidStatus::Recognized,
                            name: "Actual device name".into(),
                            owner: "actual owner".into(),
                        },
                        mapping_hash: MappingHashObservation::Unobserved,
                    },
                },
            ],
        },
    }
}

#[::core::prelude::v1::test]
fn keyboard_inventory_owner_publishes_actual_device_values_and_returned_slot_order() {
    let mut owner = Owner::default();
    let (id, _) = request(&mut owner, 550, 2);
    assert!(matches!(owner.observation, Observation::Loading));
    assert!(owner.inventory().is_none());
    assert!(owner.finish(id, 2, Ok((observation(), None))));
    let observed = owner.inventory().unwrap();
    assert_eq!(observed.max_profiles_supported, 7);
    assert_eq!(
        observed
            .slots
            .iter()
            .map(|slot| slot.slot_id)
            .collect::<Vec<_>>(),
        [9, 4]
    );
    let ProfileMetadataObservation::Observed { identity, .. } = &observed.slots[1].metadata else {
        panic!("actual metadata lost")
    };
    assert_eq!(identity.name, "Actual device name");
    assert_eq!(identity.owner, "actual owner");
}

#[::core::prelude::v1::test]
fn keyboard_inventory_failure_clears_previous_observation_and_rejects_old_completion() {
    let mut owner = Owner::default();
    let (old, old_guard) = request(&mut owner, 550, 1);
    assert!(owner.finish(old, 1, Ok((observation(), None))));
    let (next, _) = request(&mut owner, 550, 2);
    assert!(old_guard.load(Ordering::Acquire));
    assert!(owner.inventory().is_none());
    assert!(!owner.finish(old, 1, Ok((observation(), None))));
    assert!(owner.finish(next, 2, Err("actual read failed".into())));
    assert_eq!(owner.error(), Some("actual read failed"));
    assert!(owner.inventory().is_none());
    assert!(owner.slots().is_none());
}

#[::core::prelude::v1::test]
fn keyboard_inventory_unknown_guid_and_empty_device_list_do_not_create_local_profiles() {
    let mut owner = Owner::default();
    let (id, _) = request(&mut owner, 550, 1);
    let mut actual = observation();
    let ProfileMetadataObservation::Observed { identity, .. } =
        &mut actual.inventory.slots[1].metadata
    else {
        panic!("metadata missing")
    };
    identity.guid = "ffffffff-ffff-ffff-ffff-ffffffffffff".into();
    identity.guid_status = GuidStatus::Unrecognized;
    assert!(owner.finish(id, 1, Ok((actual, None))));
    assert_eq!(owner.slots().unwrap().len(), 2);
    let (id, _) = request(&mut owner, 550, 2);
    let mut actual = observation();
    actual.inventory.declared_profile_count = 0;
    actual.inventory.slots.clear();
    assert!(owner.finish(id, 2, Ok((actual, None))));
    assert_eq!(owner.slots(), Some(&[][..]));
    assert!(owner.error().is_none());
}

#[::core::prelude::v1::test]
fn keyboard_inventory_disconnect_cancel_and_generation_change_hide_confirmed_slots() {
    let mut owner = Owner::default();
    let (id, guard) = request(&mut owner, 550, 1);
    assert!(!owner.finish(id, 2, Ok((observation(), None))));
    assert!(owner.finish(id, 1, Ok((observation(), None))));
    guard.store(true, Ordering::Release);
    assert!(owner.inventory().is_none());
    assert!(!owner.finish(id, 1, Ok((observation(), None))));
    owner.invalidate(false);
    assert!(matches!(owner.observation, Observation::Disconnected));
    let (next, _) = request(&mut owner, 550, 3);
    assert!(!owner.finish(id, 3, Err("late canceled result".into())));
    assert!(owner.finish(next, 3, Ok((observation(), None))));
}

#[::core::prelude::v1::test]
fn keyboard_inventory_owner_rejects_foreign_product_and_malformed_observation() {
    let mut owner = Owner::default();
    let (id, _) = request(&mut owner, 551, 1);
    assert!(owner.finish(id, 1, Ok((observation(), None))));
    assert!(owner.inventory().is_none());
    assert!(owner.error().unwrap().contains("another product"));
    let (id, _) = request(&mut owner, 550, 2);
    let mut actual = observation();
    actual.inventory.declared_profile_count = 1;
    assert!(owner.finish(id, 2, Ok((actual, None))));
    assert!(owner.inventory().is_none());
    assert!(owner.error().unwrap().contains("count differs"));
}

#[::core::prelude::v1::test]
fn keyboard_inventory_partial_756_slots_never_fabricate_maximum_or_count() {
    let mut owner = Owner::default();
    let (id, _) = request(&mut owner, 756, 1);
    let slot = ProfileSlot {
        slot_id: 6,
        metadata: ProfileMetadataObservation::Observed {
            identity: ProfileIdentity {
                guid: "12345678-1234-5678-1234-567812345678".into(),
                guid_status: GuidStatus::Recognized,
                name: "Observed 756 profile".into(),
                owner: "other local owner".into(),
            },
            mapping_hash: MappingHashObservation::Observed([1, 2, 3, 4, 5, 6]),
        },
    };
    assert!(owner.finish_slots(
        id,
        1,
        Ok(inventory::SlotsObservation {
            target: target(756, b"current-756"),
            slots: vec![slot.clone()]
        })
    ));
    assert_eq!(owner.slots(), Some([slot].as_slice()));
    assert!(owner.inventory().is_none());
    let (new, _) = request(&mut owner, 756, 2);
    assert!(!owner.finish_slots(id, 1, Err("stale 756 failure".into())));
    assert!(owner.finish_slots(new, 2, Err("756 query failed".into())));
    assert!(owner.slots().is_none());
    assert_eq!(owner.error(), Some("756 query failed"));
}
