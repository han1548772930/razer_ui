use super::*;
use std::{cell::Cell, sync::Mutex};

const GUID: &str = "11223344-5566-7788-99aa-bbccddeeff12";
struct Mock {
    sent: Mutex<Vec<Vec<u8>>>,
    fail_slot: Option<u8>,
}
impl FeatureTransport for Mock {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        self.sent.lock().unwrap().push(report.to_vec());
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        let sent = self.sent.lock().unwrap().last().unwrap().clone();
        let data = match &sent[7..9] {
            [5, 129] => vec![2, 1, 3],
            [5, 136] => {
                let id = sent[9];
                let offset = u16::from_be_bytes([sent[10], sent[11]]) as usize;
                let count = (250 - offset).min(64);
                let mut metadata = [0; 250];
                metadata[..16].copy_from_slice(&[
                    0x44,
                    0x33,
                    0x22,
                    0x11,
                    0x66,
                    0x55,
                    0x88,
                    0x77,
                    0x99,
                    0xaa,
                    0xbb,
                    0xcc,
                    0xdd,
                    0xee,
                    0xff,
                    if id == 3 { 0x12 } else { 0x13 },
                ]);
                let mut data = vec![id, sent[10], sent[11], 0, count as u8];
                data.extend_from_slice(&metadata[offset..offset + count]);
                data
            }
            [0, 218] => vec![sent[9], 0xa5, 0xa1],
            _ => vec![],
        };
        report.fill(0);
        report[1] = if sent[7] == 2 && sent[8] < 128 && self.fail_slot == Some(sent[9]) {
            4
        } else {
            2
        };
        report[2] = sent[2];
        report[6] = data.len() as u8;
        report[7..9].copy_from_slice(&sent[7..9]);
        report[9..9 + data.len()].copy_from_slice(&data);
        Ok(91)
    }
    fn metadata(&self) -> Value {
        serde_json::json!({"simulated":true})
    }
}

#[test]
fn custom_gaming_preserves_active_selection_and_only_updates_observed_guid_slots() {
    let mock = Mock {
        sent: Mutex::new(vec![]),
        fail_slot: None,
    };
    let settings = serde_json::json!({"combos":[
        {"id":1,"enabled":true,"key1":"KEY_A","key2":""},
        {"id":2,"enabled":false,"key1":"KEY_B","key2":"KEY_C"}
    ]});
    let receipt = submit(
        &mock,
        742,
        GUID,
        Intent::CustomGamingCombinations { settings },
        sequence(),
        || Ok(()),
    )
    .unwrap();
    assert!(receipt.completed);
    assert_eq!(receipt.matched_slots, vec![3]);
    assert_eq!(
        receipt
            .submissions
            .iter()
            .map(|step| step.command)
            .collect::<Vec<_>>(),
        vec![
            [10, 0, 93],
            [3, 0, 218],
            [3, 0, 90],
            [10, 0, 93],
            [3, 0, 90]
        ]
    );
    assert_eq!(
        receipt.submissions[0].payload,
        vec![1, 1, 31, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(receipt.submissions[2].payload, vec![1, 0xa5, 0xa3]);
    assert_eq!(receipt.submissions[4].payload, vec![3, 0xa5, 0xa3]);
}

#[test]
fn local_custom_gaming_reducer_preserves_normal_gaming_state() {
    let mut profile = serde_json::json!({"gamingMode":{"state":2,"isAltTabDisabled":true}});
    let message = serde_json::json!({"type":"ON_SET_GAME_MODE_COMBINATION_KEYS","payload":{"combos":[{"id":1,"enabled":false}]}});
    persist_local(&message, &mut profile).unwrap();
    assert_eq!(profile["gamingMode"]["state"], 2);
    assert_eq!(profile["gamingMode"]["customGameMode"], message["payload"]);
}
fn sequence() -> impl FnMut() -> anyhow::Result<u8> {
    let mut next = 0;
    move || {
        let value = next;
        next = (next + 1) % 31;
        Ok(value)
    }
}

#[test]
fn magnetic_assignments_precede_ordered_analog_batches_and_keep_changed_key_resets() {
    let mock = Mock {
        sent: Mutex::new(vec![]),
        fail_slot: None,
    };
    let receipt = submit(
        &mock,
        740,
        GUID,
        Intent::MappingChanges {
            assignments: vec![
                PreparedAssignment::Button(ButtonAssignment {
                    key_id: 31,
                    mode: 0,
                    function_id: 2,
                    data_size: 2,
                    data: [0, 4, 0, 0, 0],
                }),
                PreparedAssignment::Button(ButtonAssignment {
                    key_id: 31,
                    mode: 0,
                    function_id: 2,
                    data_size: 2,
                    data: [0, 5, 0, 0, 0],
                }),
            ],
            points: vec![
                ActuationPoint {
                    key_id: 31,
                    threshold_high: 256,
                    threshold_low: 19656,
                },
                ActuationPoint {
                    key_id: 31,
                    threshold_high: 1638,
                    threshold_low: 3276,
                },
            ],
            keys: vec![RapidTriggerKey {
                key_id: 31,
                enabled: 0,
                up_sensitivity: 1638,
                down_sensitivity: 3276,
            }],
            macro_sync: None,
            single_key_snap_tap: None,
        },
        sequence(),
        || Ok(()),
    )
    .unwrap();
    assert!(receipt.completed);
    assert_eq!(receipt.matched_slots, [3]);
    assert_eq!(
        receipt
            .submissions
            .iter()
            .map(|step| (step.profile_id, step.command, step.payload.clone()))
            .collect::<Vec<_>>(),
        vec![
            (3, [80, 2, 13], vec![3, 31, 0, 2, 2, 0, 4, 0, 0, 0]),
            (1, [80, 2, 13], vec![1, 31, 0, 2, 2, 0, 4, 0, 0, 0]),
            (3, [80, 2, 13], vec![3, 31, 0, 2, 2, 0, 5, 0, 0, 0]),
            (1, [80, 2, 13], vec![1, 31, 0, 2, 2, 0, 5, 0, 0, 0]),
            (3, [80, 2, 25], vec![0, 3, 31, 1, 0, 0x4c, 0xc8]),
            (3, [80, 2, 25], vec![0, 3, 31, 6, 0x66, 0x0c, 0xcc]),
            (1, [80, 2, 25], vec![0, 1, 31, 1, 0, 0x4c, 0xc8]),
            (1, [80, 2, 25], vec![0, 1, 31, 6, 0x66, 0x0c, 0xcc]),
            (3, [80, 2, 26], vec![0, 3, 31, 0, 6, 0x66, 0x0c, 0xcc]),
            (1, [80, 2, 26], vec![0, 1, 31, 0, 6, 0x66, 0x0c, 0xcc]),
        ]
    );
    assert!(
        receipt
            .submissions
            .iter()
            .all(|step| { step.attempted && step.acknowledgement == Some(Vec::new()) })
    );
    let sent = mock.sent.lock().unwrap();
    for packet in sent.iter().filter(|packet| packet[7..9] == [2, 13]) {
        assert_eq!(packet[6], 10);
    }
}

#[test]
fn magnetic_assignment_cancellation_retains_the_attempt_without_inventing_ack() {
    let mock = Mock {
        sent: Mutex::new(vec![]),
        fail_slot: None,
    };
    let failure = submit(
        &mock,
        746,
        GUID,
        Intent::MappingChanges {
            assignments: vec![PreparedAssignment::Button(ButtonAssignment {
                key_id: 31,
                mode: 1,
                function_id: 0,
                data_size: 0,
                data: [0; 5],
            })],
            points: vec![],
            keys: vec![],
            macro_sync: None,
            single_key_snap_tap: None,
        },
        sequence(),
        || {
            ensure!(
                !mock
                    .sent
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|packet| packet[7..9] == [2, 13]),
                "canceled after actual assignment send"
            );
            Ok(())
        },
    )
    .unwrap_err();
    assert!(!failure.receipt.completed);
    assert_eq!(failure.receipt.submissions.len(), 1);
    assert!(failure.receipt.submissions[0].attempted);
    assert!(failure.receipt.submissions[0].acknowledgement.is_none());
    assert_eq!(failure.receipt.submissions[0].profile_id, 3);
}

#[test]
fn source_select_all_mapping_is_local_and_cannot_claim_a_hardware_receipt() {
    let message = serde_json::json!({"type":"ON_SET_KEYMAPPING","payload":{"isSelectAll":true,"mappingList":[]}});
    assert_eq!(
        intent_with_profile(740, &message, &Value::Null, &Value::Null, 1).unwrap(),
        Intent::MappingDraftOnly
    );
    let mock = Mock {
        sent: Mutex::new(vec![]),
        fail_slot: None,
    };
    assert!(
        submit(
            &mock,
            740,
            GUID,
            Intent::MappingDraftOnly,
            sequence(),
            || Ok(())
        )
        .is_err()
    );
    assert!(mock.sent.lock().unwrap().is_empty());
}

#[test]
fn magnetic_bottom_deadzone_uses_current_product_thresholds_and_medium_fallback() {
    for product in [740, 746] {
        for (name, raw) in [
            ("responsive", 3),
            ("RESPONSIVE", 3),
            ("Responsive", 3),
            ("medium", 5),
            ("stable", 8),
            ("STABLE", 8),
            ("other", 5),
            ("", 5),
            (" responsive ", 5),
        ] {
            let mock = Mock {
                sent: Mutex::new(vec![]),
                fail_slot: None,
            };
            let receipt = submit(
                &mock,
                product,
                GUID,
                Intent::BottomDeadzone { value: name.into() },
                sequence(),
                || Ok(()),
            )
            .unwrap();
            assert_eq!(
                receipt
                    .submissions
                    .iter()
                    .map(|step| (step.command, step.payload.clone()))
                    .collect::<Vec<_>>(),
                vec![([2, 2, 40], vec![1, raw]), ([2, 2, 40], vec![3, raw])]
            );
            assert!(!receipt.intent.publishes_mapping());
        }
    }
}

#[test]
fn magnetic_gamepad_controls_capture_engine_mode_and_disabled_override_only_persists() {
    let mut profile = crate::keyboard_magnetic_mapping::default_profile(740).unwrap();
    profile["guid"] = serde_json::json!(GUID);
    let disabled = serde_json::json!({"type":"ON_SET_ENABLE_KEYBOARD_KEYS","payload":{"isEnableKeyboardKeys":false}});
    let (saved, intent) = prepare_local(740, 1, &disabled, &profile, &profile).unwrap();
    assert_eq!(saved["isEnableKeyboardKeys"], false);
    assert_eq!(saved["_magneticEnginePreparation"], "gamepad");
    assert_eq!(intent, Intent::AppEngineOnly);
    assert!(!intent.requires_device());
    assert!(intent.publishes_mapping());
    let override_message = serde_json::json!({"type":"ON_SET_GAMEPAD_OVERRIDE_KEYS","payload":{"isGamepadOverride":false}});
    let (overridden, intent) = prepare_local(740, 1, &override_message, &saved, &saved).unwrap();
    assert_eq!(overridden["isGamepadOverride"], false);
    assert_eq!(overridden["mappings"], profile["mappings"]);
    assert_eq!(intent, Intent::ProfileDraftOnly);
    assert!(!intent.publishes_mapping());
    assert!(!intent.requires_device());
    let enabled = serde_json::json!({"type":"ON_SET_ENABLE_KEYBOARD_KEYS","payload":{"isEnableKeyboardKeys":true}});
    let (enabled, _) = prepare_local(740, 1, &enabled, &overridden, &overridden).unwrap();
    let (_, intent) = prepare_local(740, 1, &override_message, &enabled, &enabled).unwrap();
    assert_eq!(intent, Intent::AppEngineOnly);
    let mock = Mock {
        sent: Mutex::new(vec![]),
        fail_slot: None,
    };
    assert!(submit(&mock, 740, GUID, intent, sequence(), || Ok(())).is_err());
    assert!(mock.sent.lock().unwrap().is_empty());
    assert!(prepare_local(742, 1, &disabled, &profile, &profile).is_err());
}

#[test]
fn magnetic_interrupt_settings_preserve_dedicated_keys_on_any_and_write_both_families() {
    let profile = serde_json::json!({"guid":GUID,"mappings":[],"interruptKey":{"isEnabled":false,"interruptionKey":"dedicated","interrupts":["KEY_C"]}});
    let message = serde_json::json!({"type":"ON_SET_INTERRUPT_KEYS","payload":{"isEnabled":true,"interruptionKey":"dedicated","interrupts":["KEY_A","KEY_D","KEY_A"]}});
    let (saved, intent) = prepare_local(746, 1, &message, &profile, &profile).unwrap();
    assert_eq!(
        saved["interruptKey"]["interrupts"],
        serde_json::json!(["KEY_A", "KEY_D"])
    );
    assert!(!intent.publishes_mapping());
    let mock = Mock {
        sent: Mutex::new(vec![]),
        fail_slot: None,
    };
    let receipt = submit(&mock, 746, GUID, intent, sequence(), || Ok(())).unwrap();
    assert_eq!(
        receipt
            .submissions
            .iter()
            .map(|step| (step.command, step.payload.clone()))
            .collect::<Vec<_>>(),
        vec![
            ([7, 2, 45], vec![1, 1, 1, 31, 33, 0, 0]),
            ([7, 2, 45], vec![3, 1, 1, 31, 33, 0, 0]),
            ([7, 2, 46], vec![1, 1, 1, 31, 33, 0, 0]),
            ([7, 2, 46], vec![3, 1, 1, 31, 33, 0, 0]),
        ]
    );
    assert!(
        receipt
            .submissions
            .iter()
            .all(|step| step.attempted && step.acknowledgement == Some(vec![]))
    );
    let any = serde_json::json!({"type":"ON_SET_INTERRUPT_KEYS","payload":{"isEnabled":true,"interruptionKey":"any","interrupts":["KEY_ANY"]}});
    let (any_saved, any_intent) = prepare_local(746, 1, &any, &saved, &saved).unwrap();
    assert_eq!(
        any_saved["interruptKey"]["interrupts"],
        serde_json::json!(["KEY_A", "KEY_D"])
    );
    let mock = Mock {
        sent: Mutex::new(vec![]),
        fail_slot: None,
    };
    let receipt = submit(&mock, 746, GUID, any_intent, sequence(), || Ok(())).unwrap();
    assert_eq!(receipt.submissions[0].payload, vec![1, 1, 0, 0, 0, 0, 0]);
    assert!(prepare_local(742, 1, &any, &saved, &saved).is_err());
}

#[test]
fn magnetic_interrupt_partial_failure_keeps_independent_mapping_task_and_real_acknowledgements() {
    let settings = InterruptSettings::parse(
        &serde_json::json!({"isEnabled":true,"interruptionKey":"dedicated","interrupts":["KEY_A"]}),
    )
    .unwrap();
    let intent = Intent::InterruptKeys {
        settings,
        mapping: Some(Box::new(Intent::MappingChanges {
            assignments: vec![PreparedAssignment::Button(ButtonAssignment {
                key_id: 31,
                mode: 0,
                function_id: 2,
                data_size: 2,
                data: [0, 5, 0, 0, 0],
            })],
            points: vec![],
            keys: vec![],
            macro_sync: None,
            single_key_snap_tap: None,
        })),
    };
    let mock = Mock {
        sent: Mutex::new(vec![]),
        fail_slot: Some(3),
    };
    let failure = submit(&mock, 740, GUID, intent, sequence(), || Ok(())).unwrap_err();
    assert!(!failure.receipt.completed);
    assert_eq!(failure.receipt.submissions.len(), 2);
    assert_eq!(failure.receipt.submissions[0].acknowledgement, Some(vec![]));
    assert!(failure.receipt.submissions[1].attempted);
    assert!(failure.receipt.submissions[1].acknowledgement.is_none());
    let mapping = failure.receipt.mapping_followup.as_ref().unwrap();
    assert!(!mapping.completed);
    assert_eq!(mapping.submissions[0].command, [80, 2, 13]);
    assert_eq!(mapping.submissions[0].profile_id, 3);
    assert!(mapping.submissions[0].attempted);
    assert!(mapping.submissions[0].acknowledgement.is_none());
}

#[test]
fn magnetic_interrupt_cancellation_stops_both_families_and_the_enqueued_mapping() {
    let settings = InterruptSettings::parse(
        &serde_json::json!({"isEnabled":true,"interruptionKey":"any","interrupts":["KEY_ANY"]}),
    )
    .unwrap();
    let intent = Intent::InterruptKeys {
        settings,
        mapping: Some(Box::new(Intent::MappingChanges {
            assignments: vec![],
            points: vec![],
            keys: vec![],
            macro_sync: None,
            single_key_snap_tap: None,
        })),
    };
    let mock = Mock {
        sent: Mutex::new(vec![]),
        fail_slot: None,
    };
    let failure = submit(&mock, 740, GUID, intent, sequence(), || {
        anyhow::ensure!(
            !mock
                .sent
                .lock()
                .unwrap()
                .iter()
                .any(|report| report[7..9] == [2, 45]),
            "canceled"
        );
        Ok(())
    })
    .unwrap_err();
    assert_eq!(failure.receipt.submissions.len(), 1);
    assert!(failure.receipt.submissions[0].attempted);
    assert!(failure.receipt.submissions[0].acknowledgement.is_none());
    assert!(failure.receipt.mapping_followup.is_none());
}

#[test]
fn magnetic_dks_cleanup_persists_reindexed_keys_and_writes_source_single_key_snap_slots() {
    let profile = serde_json::json!({"guid":GUID,"mappings":[],"singleKeySnapTap":{"isEnabled":true,"keyList":[
        {"id":1,"keyMapMake":"KEY_A","keyMapBreak":"KEY_B"},
        {"id":5,"keyMapMake":"KEY_C","keyMapBreak":"KEY_D"}]}});
    let message = serde_json::json!({"type":"ON_SET_KEYMAPPING","payload":{"isSelectAll":true,"mappingList":[{
        "inputID":"KEY_A","inputType":"AnalogInput","isHyperShift":false,"mapping":[
            {"outputType":"dynamicKeyStrokeGroup","dynamicKeyStrokeGroup":{},"actuationPoint":{"0":7000,"1":256}},
            {"outputType":"defaultGroup","actuationPoint":{"0":7000,"1":256}}]}]}});
    let (saved, intent) = prepare_local(740, 1, &message, &profile, &profile).unwrap();
    assert_eq!(
        saved["singleKeySnapTap"]["keyList"],
        serde_json::json!([
        {"id":1,"keyMapMake":"KEY_C","keyMapBreak":"KEY_D"}])
    );
    let Intent::SingleKeySnapTap { settings, global } = &intent else {
        panic!("source DKS cleanup must retain its hardware task")
    };
    assert!(!global);
    assert_eq!(settings.keys, [[48, 7]]);
    let mock = Mock {
        sent: Mutex::new(vec![]),
        fail_slot: None,
    };
    let receipt = submit(&mock, 740, GUID, intent, sequence(), || Ok(())).unwrap();
    assert!(receipt.completed);
    assert_eq!(
        receipt
            .submissions
            .iter()
            .map(|step| (step.command, step.payload.clone()))
            .collect::<Vec<_>>(),
        vec![
            ([11, 2, 44], vec![1, 1, 1, 48, 7, 0, 0, 0, 0, 0, 0]),
            ([11, 2, 44], vec![3, 1, 1, 48, 7, 0, 0, 0, 0, 0, 0])
        ]
    );
    assert!(
        receipt
            .submissions
            .iter()
            .all(|step| step.attempted && step.acknowledgement == Some(vec![]))
    );
}

#[test]
fn magnetic_single_key_snap_toggle_uses_global_class_and_retains_partial_failure() {
    // Current746 MW6737 module34737 ri uses make.analogKeyID and break.HID.
    // Its module24592 key table gives W analog18 (scan0x11) and S HID22.
    let profile = serde_json::json!({"guid":GUID,"mappings":[],"singleKeySnapTap":{"isEnabled":false,
        "keyList":[{"id":1,"keyMapMake":"KEY_W","keyMapBreak":"KEY_S"}]}});
    let message =
        serde_json::json!({"type":"ON_TOGGLE_SINGLE_KEY_SNAP_TAP","payload":{"isEnabled":true}});
    let (saved, intent) = prepare_local(746, 1, &message, &profile, &profile).unwrap();
    assert_eq!(saved["singleKeySnapTap"]["isEnabled"], true);
    let mock = Mock {
        sent: Mutex::new(vec![]),
        fail_slot: Some(3),
    };
    let failure = submit(&mock, 746, GUID, intent, sequence(), || Ok(())).unwrap_err();
    assert!(!failure.receipt.completed);
    assert_eq!(failure.receipt.submissions.len(), 2);
    assert_eq!(
        failure.receipt.submissions[0].payload,
        vec![0, 1, 1, 18, 22, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(failure.receipt.submissions[0].acknowledgement, Some(vec![]));
    assert_eq!(
        failure.receipt.submissions[1].payload,
        vec![3, 1, 1, 18, 22, 0, 0, 0, 0, 0, 0]
    );
    assert!(failure.receipt.submissions[1].attempted);
    assert!(failure.receipt.submissions[1].acknowledgement.is_none());
    assert_eq!(saved["singleKeySnapTap"]["isEnabled"], true);
    let empty = serde_json::json!({"guid":GUID,"mappings":[],"singleKeySnapTap":{"isEnabled":false,"keyList":[]}});
    assert!(prepare_local(746, 1, &message, &empty, &empty).is_err());
}
#[test]
fn writes_active_then_only_observed_matching_guid_slot_without_readback() {
    let mock = Mock {
        sent: Mutex::new(vec![]),
        fail_slot: None,
    };
    let receipt = submit(
        &mock,
        742,
        GUID,
        Intent::BottomDeadzone {
            value: "STABLE".into(),
        },
        sequence(),
        || Ok(()),
    )
    .unwrap();
    assert!(receipt.completed);
    assert_eq!(receipt.matched_slots, vec![3]);
    assert_eq!(
        receipt
            .submissions
            .iter()
            .map(|step| step.payload.clone())
            .collect::<Vec<_>>(),
        vec![vec![1, 14], vec![3, 14]]
    );
    assert!(
        receipt
            .submissions
            .iter()
            .all(|step| step.attempted && step.acknowledgement == Some(vec![]))
    );
    let sent = mock.sent.lock().unwrap();
    assert!(sent.iter().all(|packet| packet[8] != 168));
}
#[test]
fn failure_retains_active_ack_and_attempted_obm_write() {
    let mock = Mock {
        sent: Mutex::new(vec![]),
        fail_slot: Some(3),
    };
    let failure = submit(
        &mock,
        742,
        GUID,
        Intent::ContinuousRapidTrigger { enabled: true },
        sequence(),
        || Ok(()),
    )
    .unwrap_err();
    assert!(!failure.receipt.completed);
    assert_eq!(failure.receipt.submissions.len(), 2);
    assert_eq!(failure.receipt.submissions[0].acknowledgement, Some(vec![]));
    assert!(failure.receipt.submissions[1].attempted);
    assert!(failure.receipt.submissions[1].acknowledgement.is_none());
}
#[test]
fn cancellation_stops_before_second_setter_and_679_has_no_new_deadzone_gate() {
    let mock = Mock {
        sent: Mutex::new(vec![]),
        fail_slot: None,
    };
    let completed = Cell::new(false);
    let result = submit(
        &mock,
        742,
        GUID,
        Intent::BottomDeadzone {
            value: "responsive".into(),
        },
        sequence(),
        || {
            let sent = mock.sent.lock().unwrap();
            if sent.iter().any(|packet| packet[7..9] == [2, 40]) {
                completed.set(true);
                anyhow::bail!("canceled");
            }
            Ok(())
        },
    );
    assert!(completed.get());
    assert!(result.is_err());
    assert_eq!(
        mock.sent
            .lock()
            .unwrap()
            .iter()
            .filter(|packet| packet[7..9] == [2, 40])
            .count(),
        1
    );
    assert!(
        validate(
            679,
            &Intent::BottomDeadzone {
                value: "medium".into()
            }
        )
        .is_err()
    );
}

struct SnapMock {
    base: Mock,
    current: [u8; 3],
    wrong_getter_slot: bool,
}

impl SnapMock {
    fn new(current: [u8; 3]) -> Self {
        Self {
            base: Mock {
                sent: Mutex::new(vec![]),
                fail_slot: None,
            },
            current,
            wrong_getter_slot: false,
        }
    }
}

impl FeatureTransport for SnapMock {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        self.base.send_feature(report)
    }

    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        let count = self.base.get_feature(report)?;
        let sent = self.base.sent.lock().unwrap();
        let sent = sent.last().unwrap();
        if sent[7..9] == [2, 161] {
            report[6] = 4;
            report[9] = sent[9] ^ u8::from(self.wrong_getter_slot);
            report[10..13].copy_from_slice(&self.current);
        }
        Ok(count)
    }

    fn metadata(&self) -> Value {
        serde_json::json!({"simulated_snap_tap":true})
    }
}

fn snap_message() -> Value {
    serde_json::json!({"type":"ON_SET_SNAP_TAP","payload":{
        "isEnabled":true,"keyList":[{"id":1,"key1":"KEY_A","key2":"KEY_D"}],
        "pressedKeys":[{"inputID":"KEY_A","snaptap":true}]
    }})
}

#[test]
fn snap_tap_routes_source_input_ids_and_does_not_persist_pressed_observations() {
    let message = snap_message();
    let intent = intent(679, &message).unwrap();
    let Intent::SnapTap { settings } = intent else {
        panic!("Snap Tap was not routed to the device task");
    };
    let mut profile = serde_json::json!({"gamingMode":{"state":2},"snapTap":{"isEnabled":false}});
    persist_local(&message, &mut profile).unwrap();
    assert_eq!(profile["snapTap"], settings);
    assert_eq!(profile["gamingMode"]["state"], 2);
    assert!(profile["snapTap"].get("pressedKeys").is_none());
    for invalid in [
        serde_json::json!({"type":"ON_SET_SNAP_TAP","payload":{"isEnabled":true}}),
        serde_json::json!({"type":"ON_SET_SNAP_TAP","payload":{"isEnabled":true,"keyList":[{"key1":"KEY_A","key2":"UNKNOWN"}]}}),
    ] {
        assert!(super::intent(679, &invalid).is_err());
    }
}

#[test]
fn snap_tap_writes_only_first_guid_matched_slot_and_preserves_setter_ack() {
    let mock = SnapMock::new([0, 31, 33]);
    let receipt = submit(
        &mock,
        679,
        GUID,
        intent(679, &snap_message()).unwrap(),
        sequence(),
        || Ok(()),
    )
    .unwrap();
    assert!(receipt.completed);
    assert_eq!(receipt.matched_slots, vec![3]);
    assert_eq!(receipt.submissions.len(), 2);
    assert_eq!(receipt.submissions[0].command, [4, 2, 161]);
    assert_eq!(receipt.submissions[0].payload, vec![3]);
    assert_eq!(
        receipt.submissions[0].acknowledgement,
        Some(vec![3, 0, 31, 33])
    );
    assert_eq!(receipt.submissions[1].command, [4, 2, 33]);
    assert_eq!(receipt.submissions[1].payload, vec![3, 1, 31, 33]);
    assert_eq!(receipt.submissions[1].acknowledgement, Some(vec![]));
    assert!(receipt.submissions.iter().all(|step| step.attempted));
}

#[test]
fn snap_tap_without_guid_match_uses_active_slot_and_matching_getter_skips_mutation() {
    let mock = SnapMock::new([1, 31, 33]);
    let receipt = submit(
        &mock,
        679,
        "22334455-6677-8899-aabb-ccddeeff1122",
        intent(679, &snap_message()).unwrap(),
        sequence(),
        || Ok(()),
    )
    .unwrap();
    assert!(receipt.completed);
    assert!(receipt.matched_slots.is_empty());
    assert_eq!(receipt.submissions.len(), 1);
    assert_eq!(receipt.submissions[0].command, [4, 2, 161]);
    assert_eq!(receipt.submissions[0].payload, vec![1]);
    assert!(
        !mock
            .base
            .sent
            .lock()
            .unwrap()
            .iter()
            .any(|packet| packet[7..9] == [2, 33])
    );
}

#[test]
fn snap_tap_rejects_wrong_getter_identity_before_setter() {
    let mut mock = SnapMock::new([0, 31, 33]);
    mock.wrong_getter_slot = true;
    let failure = submit(
        &mock,
        679,
        GUID,
        intent(679, &snap_message()).unwrap(),
        sequence(),
        || Ok(()),
    )
    .unwrap_err();
    assert!(!failure.receipt.completed);
    assert_eq!(failure.receipt.submissions.len(), 1);
    assert!(
        !mock
            .base
            .sent
            .lock()
            .unwrap()
            .iter()
            .any(|packet| packet[7..9] == [2, 33])
    );
}

#[test]
fn snap_tap_cancellation_during_getter_does_not_write_or_acknowledge() {
    let mock = SnapMock::new([0, 31, 33]);
    let failure = submit(
        &mock,
        679,
        GUID,
        intent(679, &snap_message()).unwrap(),
        sequence(),
        || {
            ensure!(
                !mock
                    .base
                    .sent
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|packet| packet[7..9] == [2, 161]),
                "Snap Tap scope ended"
            );
            Ok(())
        },
    )
    .unwrap_err();
    assert!(!failure.receipt.completed);
    assert_eq!(failure.receipt.submissions.len(), 1);
    assert!(failure.receipt.submissions[0].attempted);
    assert!(failure.receipt.submissions[0].acknowledgement.is_none());
    assert!(
        !mock
            .base
            .sent
            .lock()
            .unwrap()
            .iter()
            .any(|packet| packet[7..9] == [2, 33])
    );
}

#[test]
fn snap_tap_setter_failure_keeps_getter_and_never_rolls_back_local_draft() {
    let mut mock = SnapMock::new([0, 31, 33]);
    mock.base.fail_slot = Some(3);
    let message = snap_message();
    let mut profile = serde_json::json!({});
    persist_local(&message, &mut profile).unwrap();
    let failure = submit(
        &mock,
        679,
        GUID,
        intent(679, &message).unwrap(),
        sequence(),
        || Ok(()),
    )
    .unwrap_err();
    assert!(!failure.receipt.completed);
    // Rejection is explicit; no getter or setter is invented as success.
    assert_eq!(failure.receipt.submissions.len(), 2);
    assert_eq!(
        failure.receipt.submissions[0].acknowledgement,
        Some(vec![3, 0, 31, 33])
    );
    assert!(failure.receipt.submissions[1].attempted);
    assert!(failure.receipt.submissions[1].acknowledgement.is_none());
    assert_eq!(profile["snapTap"]["isEnabled"], true);
}
