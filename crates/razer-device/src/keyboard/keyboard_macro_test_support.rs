use crate::{
    device_reads::DeviceReadCapability,
    keyboard_onboard_profiles::{CompleteReading, Mode},
    keyboard_profile_inventory::ProfileMetadataObservation,
    onboard_assignment::ButtonAssignment,
    onboard_macro::{MacroReading, MacroStorageInfo, MacroWriteRequest},
};
use crate::{
    keyboard_control_test_transport::{Reply, reply},
    keyboard_onboard_profiles::{
        AssignmentBatch, ImportDetails, KeyIdentity, MacroDetail, ModeReading, ProfileReading,
        Reading, Snapshot, settings,
    },
    keyboard_profile_inventory::{
        GuidStatus, MappingHashObservation, ProfileIdentity, ProfileInventory, ProfileSlot,
    },
    keyboard_settings::KeyboardBrightness,
    onboard_macro::{MacroCatalog, MacroInventory, MacroMetadata, MacroSummary},
};

pub(crate) const GUID: &str = "11223344-5566-7788-99aa-bbccddeeff12";

pub(crate) fn cap(product: u32) -> DeviceReadCapability {
    let mut value = crate::keyboard_metadata::capability(product)
        .unwrap()
        .clone();
    value.max_retry_in = 1;
    value.max_retry_out = 1;
    value.sleep_between_out_ms = 0;
    value.sleep_between_out_in_ms = 0;
    value.sleep_between_in_ms = 0;
    value
}

pub(crate) fn storage(available: u32, recyclable: u32) -> Reply {
    let mut value = vec![0x7f, 0xff, 0, 0, 0x20, 0];
    value.extend_from_slice(&available.to_be_bytes());
    value.extend_from_slice(&recyclable.to_be_bytes());
    reply(&value)
}

pub(crate) fn request() -> MacroWriteRequest {
    MacroWriteRequest {
        guid: GUID.into(),
        name: "new".into(),
        num_items: 2,
        timestamp: "current".into(),
        hash: "1234".into(),
        data: vec![1, 4, 2, 4],
    }
}

pub(crate) fn observed_macro() -> MacroReading {
    let mut raw = vec![0; 250];
    raw[..16].copy_from_slice(&[
        0x44, 0x33, 0x22, 0x11, 0x66, 0x55, 0x88, 0x77, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
        0x12,
    ]);
    raw[16] = 4;
    raw[20..23].copy_from_slice(b"old");
    raw[136..139].copy_from_slice(b"old");
    raw[152..158].copy_from_slice(b"abcdef");
    MacroReading {
        metadata: MacroMetadata {
            macro_id: 7,
            guid: GUID.into(),
            num_items: 4,
            name: "old".into(),
            extra_info: [0; 16],
            timestamp: "old".into(),
            hash: "abcdef".into(),
            raw,
            blocks: Vec::new(),
        },
        data: vec![1, 50, 2, 50, 17, 0, 21, 0, 0, 0, 0],
        data_size: 11,
        data_blocks: Vec::new(),
    }
}

pub(crate) fn observation(product: u32, with_macro: bool) -> CompleteReading {
    let keys = crate::keyboard_onboard_inventory::inventory(product, 1)
        .unwrap()
        .iter()
        .map(|key| KeyIdentity {
            input_id: key.input_id.into(),
            firmware_key_id: key.firmware_key_id,
            ignore_normal: key.ignore_normal,
            ignore_hypershift: key.ignore_hypershift,
        })
        .collect::<Vec<_>>();
    let slots = [5, 1, 2];
    let profiles = slots
        .iter()
        .map(|&slot| ProfileReading {
            slot_id: slot,
            brightness: Reading::Observed(KeyboardBrightness {
                profile_id: slot,
                region_id: 5,
                raw_value: 51,
                percent: 20,
            }),
            passes: [Mode::Normal, Mode::Hypershift]
                .into_iter()
                .map(|mode| ModeReading {
                    mode,
                    assignments: keys
                        .iter()
                        .filter(|key| match mode {
                            Mode::Normal => !key.ignore_normal,
                            Mode::Hypershift => !key.ignore_hypershift,
                        })
                        .map(|key| AssignmentBatch {
                            key_ids: vec![key.firmware_key_id],
                            attempts: vec![Reading::Observed(vec![ButtonAssignment {
                                key_id: key.firmware_key_id,
                                mode: u8::from(mode == Mode::Hypershift),
                                function_id: 2,
                                data_size: 2,
                                data: [0, 4, 0, 0, 0],
                            }])],
                        })
                        .collect(),
                    rapid_trigger: Vec::new(),
                    actuation: Vec::new(),
                    legacy_actuation: Vec::new(),
                })
                .collect(),
            rapid_trigger: Vec::new(),
            actuation: Vec::new(),
            kill_switch: Reading::Unobserved,
            keymaps: None,
            settings: settings::Observation {
                polling_rate: Reading::Unobserved,
                bottom_deadzone: Reading::Unobserved,
                debounce: Reading::Unobserved,
                knob_configuration: Reading::Unobserved,
                knob_current: Reading::Unobserved,
                mod_tap: Reading::Unobserved,
                snap_tap: Reading::Unobserved,
                single_key_snap_tap: Reading::Unobserved,
                interrupt_dynamic_keystroke: Reading::Unobserved,
                interrupt_macro: Reading::Unobserved,
                dual_keypress_priority: Reading::Unobserved,
            },
        })
        .collect();
    let macro_reading = observed_macro();
    CompleteReading {
        snapshot: Snapshot {
            product_id: product,
            layout_id: 1,
            key_inventory: keys,
            macros: MacroCatalog {
                inventory: MacroInventory {
                    storage: MacroStorageInfo {
                        max_macro_id: 32767,
                        max_memory: 8192,
                        available_memory: 0,
                        recyclable_memory: 4096,
                    },
                    count: u16::from(with_macro),
                    ids: if with_macro { vec![7] } else { Vec::new() },
                    pages: Vec::new(),
                },
                macros: if with_macro {
                    vec![MacroSummary {
                        metadata: macro_reading.metadata.clone(),
                        data_size: 11,
                        allocated_size_with_name: 512,
                    }]
                } else {
                    Vec::new()
                },
            },
            inventory: ProfileInventory {
                max_profiles_supported: 5,
                declared_profile_count: 3,
                slots: slots
                    .iter()
                    .map(|&slot_id| ProfileSlot {
                        slot_id,
                        metadata: ProfileMetadataObservation::Observed {
                            identity: ProfileIdentity {
                                guid: GUID.into(),
                                guid_status: GuidStatus::Recognized,
                                name: "Observed Device".into(),
                                owner: "owner".into(),
                            },
                            mapping_hash: MappingHashObservation::Observed([3; 6]),
                        },
                    })
                    .collect(),
            },
            profiles,
        },
        import_details: ImportDetails {
            macros: if with_macro {
                vec![MacroDetail {
                    macro_id: 7,
                    content: Reading::Observed(macro_reading),
                }]
            } else {
                Vec::new()
            },
            continuous_rapid_trigger: Vec::new(),
        },
    }
}

pub(crate) fn reconstruction_replies(
    reading: &CompleteReading,
    format_statuses: &[u8],
) -> Vec<Reply> {
    let mut replies = vec![storage(0, 4096)];
    replies.extend((0..if reading.snapshot.product_id == 579 { 3 } else { 2 }).map(|_| reply(&[])));
    replies.push(reply(&[]));
    replies.extend(
        format_statuses
            .iter()
            .map(|&status| reply(&[0, 0, status, 2])),
    );
    for profile in reading
        .snapshot
        .profiles
        .iter()
        .filter(|profile| profile.slot_id != 1)
    {
        // create + four 64-byte name writes + the observed normal/hyper rows.
        let count = 5 + profile
            .passes
            .iter()
            .map(|pass| pass.assignments.len())
            .sum::<usize>();
        replies.extend((0..count).map(|_| reply(&[])));
    }
    replies.extend((0..reading.snapshot.macros.macros.len() * 6).map(|_| reply(&[])));
    replies.push(storage(2048, 0));
    replies.extend((0..6).map(|_| reply(&[])));
    replies.push(storage(1536, 0));
    replies
}
