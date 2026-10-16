use super::*;
use crate::backend::FeatureTransport;
use crate::device_reads::DeviceReadCapability;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

enum Step {
    Send(Vec<u8>),
    SendFails(Vec<u8>),
    Get(Vec<u8>),
    GetFails,
    ShortGet(Vec<u8>),
}

struct Transport {
    steps: RefCell<VecDeque<Step>>,
}

impl Transport {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            steps: RefCell::new(steps.into_iter().collect()),
        }
    }

    fn finish(&self) {
        assert!(self.steps.borrow().is_empty(), "synthetic I/O not consumed");
    }
}

impl FeatureTransport for Transport {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        match self.steps.borrow_mut().pop_front().expect("unexpected OUT") {
            Step::Send(expected) => assert_eq!(report, expected),
            Step::SendFails(expected) => {
                assert_eq!(report, expected);
                anyhow::bail!("simulated OUT error");
            }
            _ => panic!("OUT where IN was expected"),
        };
        Ok(())
    }

    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        // Interface selection is checked across products in device_query_tests.
        assert_eq!(report.len(), 91);
        assert_eq!(&report[1..], [0; 90]);
        match self.steps.borrow_mut().pop_front().expect("unexpected IN") {
            Step::Get(expected) => {
                report.copy_from_slice(&expected);
                Ok(91)
            }
            Step::ShortGet(expected) => {
                report[..expected.len()].copy_from_slice(&expected);
                Ok(expected.len())
            }
            Step::GetFails => anyhow::bail!("simulated IN error"),
            _ => panic!("IN where OUT was expected"),
        }
    }

    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

fn cap(product_id: u32) -> DeviceReadCapability {
    let mut cap = crate::device_reads::capability(product_id).unwrap().clone();
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap.max_retry_out = 1;
    cap.max_retry_in = 1;
    cap
}

fn packet(header: [u8; 3], payload: &[u8], transaction: u8, status: u8) -> Vec<u8> {
    let mut report = vec![0; 91];
    report[1] = status;
    report[2] = transaction;
    report[6..9].copy_from_slice(&header);
    report[6] = payload.len() as u8;
    report[9..9 + payload.len()].copy_from_slice(payload);
    report[89] = report[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    report
}

fn send(header: [u8; 3], payload: &[u8], transaction: u8) -> Step {
    let mut report = vec![0; 91];
    report[2] = transaction;
    report[6..9].copy_from_slice(&header);
    report[9..9 + payload.len()].copy_from_slice(payload);
    report[89] = report[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    Step::Send(report)
}

fn receive(header: [u8; 3], payload: &[u8], transaction: u8) -> Step {
    Step::Get(packet(header, payload, transaction, 2))
}

fn target() -> ButtonTarget {
    ButtonTarget {
        profile_id: 3,
        button_id: 9,
        mode: AssignmentMode::Normal,
    }
}

fn txs() -> impl FnMut() -> anyhow::Result<u8> {
    let mut value = 0;
    move || {
        value += 1;
        Ok(value)
    }
}

#[test]
fn all_product_descriptors_gate_perfect180_to_the_three_supported_products() {
    for product in [170, 204, 226] {
        let encoded = encode(
            product,
            ButtonAction::Perfect180 {
                direction: Perfect180Direction::Left,
                report_count: 0x1234,
                enabled: true,
            },
        )
        .unwrap();
        assert_eq!(encoded, (22, 4, [1, 0x12, 0x34, 1, 0]));
    }
    assert!(
        encode(
            45066,
            ButtonAction::Perfect180 {
                direction: Perfect180Direction::Right,
                report_count: 1,
                enabled: false,
            }
        )
        .is_err()
    );
}

#[test]
fn encodes_mouse_keyboard_and_turbo_big_endian_fields() {
    assert_eq!(
        encode(
            170,
            ButtonAction::Mouse(MouseAction::Button(MouseButton::Left))
        )
        .unwrap(),
        (1, 1, [1, 0, 0, 0, 0])
    );
    assert_eq!(
        encode(
            170,
            ButtonAction::Mouse(MouseAction::TurboButton {
                button: MouseButton::Right,
                keys_per_second: Some(7),
            })
        )
        .unwrap(),
        (14, 3, [2, 0, 142, 0, 0])
    );
    assert_eq!(
        encode(
            170,
            ButtonAction::Keyboard(KeyboardAction {
                hid: 0x04,
                modifiers: 0x02,
                turbo: true,
                keys_per_second: Some(10),
            })
        )
        .unwrap(),
        (13, 4, [2, 4, 0, 100, 0])
    );
}

#[test]
fn encodes_each_typed_converter_and_preserves_valid_zero_macro_id() {
    assert_eq!(
        encode(
            170,
            ButtonAction::Dpi(DpiAction::Clutch { x: 800, y: 1600 })
        )
        .unwrap(),
        (6, 5, [5, 3, 32, 6, 64])
    );
    assert_eq!(
        encode(
            170,
            ButtonAction::Macro(MacroAction {
                macro_id: 0x1234,
                playback: MacroPlayback::NTimes(3),
            })
        )
        .unwrap(),
        (3, 3, [0x12, 0x34, 3, 0, 0])
    );
    assert_eq!(
        encode(170, ButtonAction::Profile(ProfileAction::Specific(7))).unwrap(),
        (7, 2, [3, 7, 0, 0, 0])
    );
    assert_eq!(
        encode(170, ButtonAction::Consumer(ConsumerAction::VolumeUp)).unwrap(),
        (10, 2, [0, 233, 0, 0, 0])
    );
    assert_eq!(
        encode(170, ButtonAction::ScrollMode(ScrollModeAction::CycleDown)).unwrap(),
        (18, 1, [5, 0, 0, 0, 0])
    );
    assert_eq!(
        encode(
            170,
            ButtonAction::Macro(MacroAction {
                macro_id: 0,
                playback: MacroPlayback::Once,
            })
        )
        .unwrap(),
        (3, 3, [0, 0, 1, 0, 0])
    );
}

#[test]
fn source_double_click_turbo_branch_is_disable_and_repeat_scroll_uses_default_period() {
    assert_eq!(
        encode(
            170,
            ButtonAction::Mouse(MouseAction::LegacyStringTurboDoubleClick)
        )
        .unwrap(),
        (0, 0, [0; 5])
    );
    assert_eq!(
        encode(
            170,
            ButtonAction::Mouse(MouseAction::RepeatScroll(MouseButton::ScrollDown))
        )
        .unwrap(),
        (14, 3, [10, 0, 142, 0, 0])
    );
    assert!(
        encode(
            170,
            ButtonAction::Mouse(MouseAction::RepeatScroll(MouseButton::Left))
        )
        .is_err()
    );
}

#[test]
fn read_uses_actual_report_data_size_ten_and_preserves_padding() {
    let target = target();
    let response = [
        target.profile_id,
        target.button_id,
        0,
        2,
        2,
        0x12,
        0x34,
        0xaa,
        0xbb,
        0xcc,
    ];
    let transport = Transport::new([
        send([10, 2, 140], &[target.profile_id, target.button_id, 0], 1),
        receive([10, 2, 140], &response, 1),
    ]);
    let observed = MouseButtonMapping::new(&transport, &cap(170))
        .unwrap()
        .read(target, txs(), || Ok(()))
        .unwrap();
    assert_eq!(observed.assignment.function_id, 2);
    assert_eq!(observed.assignment.data_size, 2);
    assert_eq!(observed.raw_data, [0x12, 0x34, 0xaa, 0xbb, 0xcc]);
    transport.finish();
}

#[test]
fn apply_sends_fixed_ten_byte_set_payload_and_optional_readback_verifies_meaningful_bytes() {
    let target = target();
    let action = ButtonAction::Keyboard(KeyboardAction {
        hid: 0x04,
        modifiers: 1,
        turbo: false,
        keys_per_second: None,
    });
    let requested = assignment(170, target, action).unwrap();
    let payload = [
        target.profile_id,
        target.button_id,
        0,
        requested.function_id,
        requested.data_size,
        1,
        4,
        0,
        0,
        0,
    ];
    let transport = Transport::new([
        send([10, 2, 12], &payload, 1),
        receive([10, 2, 12], &[], 1),
        send([10, 2, 140], &[target.profile_id, target.button_id, 0], 2),
        receive(
            [10, 2, 140],
            &[target.profile_id, target.button_id, 0, 2, 2, 1, 4, 9, 9, 9],
            2,
        ),
    ]);
    let report = MouseButtonMapping::new(&transport, &cap(204))
        .unwrap()
        .apply(target, action, true, txs(), || Ok(()))
        .unwrap();
    assert!(report.verified());
    assert_eq!(report.steps[0].acknowledgment_data, Vec::<u8>::new());
    assert_eq!(
        report.steps[0].observed.as_ref().unwrap().raw_data[2..],
        [9, 9, 9]
    );
    transport.finish();
}

#[test]
fn ack_without_readback_is_explicitly_unverified() {
    let target = target();
    let requested = assignment(226, target, ButtonAction::Disabled).unwrap();
    let payload = [
        target.profile_id,
        target.button_id,
        0,
        requested.function_id,
        0,
        0,
        0,
        0,
        0,
        0,
    ];
    let transport = Transport::new([send([10, 2, 12], &payload, 1), receive([10, 2, 12], &[], 1)]);
    let report = MouseButtonMapping::new(&transport, &cap(226))
        .unwrap()
        .apply(target, ButtonAction::Disabled, false, txs(), || Ok(()))
        .unwrap();
    assert!(report.source_completed);
    assert!(!report.verified());
    transport.finish();
}

#[test]
fn mismatched_identity_and_mismatched_readback_are_errors_without_fabricated_success() {
    let target = target();
    let wrong_identity = [
        target.profile_id,
        target.button_id + 1,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ];
    let transport = Transport::new([
        send([10, 2, 140], &[target.profile_id, target.button_id, 0], 1),
        receive([10, 2, 140], &wrong_identity, 1),
    ]);
    assert!(
        MouseButtonMapping::new(&transport, &cap(45066))
            .unwrap()
            .read(target, txs(), || Ok(()))
            .is_err()
    );
    transport.finish();

    let requested = assignment(170, target, ButtonAction::Disabled).unwrap();
    let payload = [target.profile_id, target.button_id, 0, 0, 0, 0, 0, 0, 0, 0];
    let transport = Transport::new([
        send([10, 2, 12], &payload, 1),
        receive([10, 2, 12], &[], 1),
        send([10, 2, 140], &[target.profile_id, target.button_id, 0], 2),
        receive(
            [10, 2, 140],
            &[target.profile_id, target.button_id, 0, 1, 1, 2, 0, 0, 0, 0],
            2,
        ),
    ]);
    let failure = MouseButtonMapping::new(&transport, &cap(170))
        .unwrap()
        .apply(target, ButtonAction::Disabled, true, txs(), || Ok(()))
        .expect_err("different readback must fail");
    assert_eq!(failure.phase, ButtonFailurePhase::Readback);
    assert!(failure.device_state_may_have_changed());
    assert!(failure.progress.steps[0].observed.is_some());
    assert_eq!(requested.function_id, 0);
    transport.finish();
}

#[test]
fn cancellation_before_write_preserves_unattempted_state() {
    let target = target();
    let failure = MouseButtonMapping::new(&Transport::new([]), &cap(170))
        .unwrap()
        .apply(target, ButtonAction::Disabled, false, txs(), || {
            anyhow::bail!("cancel")
        })
        .expect_err("cancellation must stop before OUT");
    assert_eq!(failure.phase, ButtonFailurePhase::Cancelled);
    assert!(!failure.device_state_may_have_changed());
}

fn disabled_payload(target: ButtonTarget) -> [u8; 10] {
    [
        target.profile_id,
        target.button_id,
        target.mode.raw(),
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ]
}

#[test]
fn transport_out_failure_retains_attempt_and_never_claims_ack() {
    let target = target();
    let Step::Send(expected) = send([10, 2, 12], &disabled_payload(target), 1) else {
        unreachable!()
    };
    let transport = Transport::new([Step::SendFails(expected)]);
    let failure = MouseButtonMapping::new(&transport, &cap(170))
        .unwrap()
        .apply(target, ButtonAction::Disabled, true, txs(), || Ok(()))
        .expect_err("OUT failure must propagate");
    assert_eq!(failure.phase, ButtonFailurePhase::Write);
    assert!(failure.device_state_may_have_changed());
    assert!(!failure.progress.source_completed);
    assert!(!failure.progress.verified());
    assert!(!failure.progress.steps[0].acknowledgment_received);
    assert!(failure.progress.steps[0].observed.is_none());
    assert!(failure.to_string().contains("simulated OUT error"));
    transport.finish();
}

#[test]
fn transport_in_failure_and_truncated_ack_do_not_claim_completion() {
    let target = target();
    for incoming in [Step::GetFails, Step::ShortGet(vec![0; 90])] {
        let transport = Transport::new([send([10, 2, 12], &disabled_payload(target), 1), incoming]);
        let failure = MouseButtonMapping::new(&transport, &cap(204))
            .unwrap()
            .apply(target, ButtonAction::Disabled, false, txs(), || Ok(()))
            .expect_err("IN failure or truncated packet must propagate");
        assert_eq!(failure.phase, ButtonFailurePhase::Write);
        assert!(failure.device_state_may_have_changed());
        assert!(!failure.progress.source_completed);
        assert!(!failure.progress.steps[0].acknowledgment_received);
        transport.finish();
    }
}

#[test]
fn invalid_transaction_and_cancellation_at_presend_do_not_mark_attempted() {
    let target = target();
    let transport = Transport::new([]);
    let failure = MouseButtonMapping::new(&transport, &cap(170))
        .unwrap()
        .apply(target, ButtonAction::Disabled, false, || Ok(31), || Ok(()))
        .expect_err("transaction 31 is outside modulus 31");
    assert_eq!(failure.phase, ButtonFailurePhase::Write);
    assert!(!failure.device_state_may_have_changed());
    transport.finish();

    let calls = Cell::new(0);
    let failure = MouseButtonMapping::new(&transport, &cap(170))
        .unwrap()
        .apply(target, ButtonAction::Disabled, false, txs(), || {
            calls.set(calls.get() + 1);
            anyhow::ensure!(calls.get() != 2, "cancel at pre-send guard");
            Ok(())
        })
        .expect_err("pre-send cancellation must preserve unattempted state");
    assert_eq!(failure.phase, ButtonFailurePhase::Cancelled);
    assert!(!failure.device_state_may_have_changed());
    transport.finish();
}

#[test]
fn cancellation_after_ack_retains_ack_and_stops_before_readback() {
    let target = target();
    let transport = Transport::new([
        send([10, 2, 12], &disabled_payload(target), 1),
        receive([10, 2, 12], &[0x42, 0x53], 1),
    ]);
    let calls = Cell::new(0);
    let failure = MouseButtonMapping::new(&transport, &cap(170))
        .unwrap()
        .apply(target, ButtonAction::Disabled, true, txs(), || {
            calls.set(calls.get() + 1);
            anyhow::ensure!(calls.get() != 6, "cancel after ACK");
            Ok(())
        })
        .expect_err("cancel after ACK must not issue a GET");
    assert_eq!(failure.phase, ButtonFailurePhase::Cancelled);
    assert!(failure.device_state_may_have_changed());
    assert!(failure.progress.steps[0].acknowledgment_received);
    assert_eq!(failure.progress.steps[0].acknowledgment_data, [0x42, 0x53]);
    assert!(failure.progress.steps[0].observed.is_none());
    assert!(!failure.progress.verified());
    transport.finish();
}

#[test]
fn busy_then_ack_polls_without_resending_assignment() {
    let target = target();
    let transport = Transport::new([
        send([10, 2, 12], &disabled_payload(target), 1),
        Step::Get(packet([10, 2, 12], &[], 1, 1)),
        receive([10, 2, 12], &[], 1),
    ]);
    let mut cap = cap(170);
    cap.max_retry_in = 2;
    let report = MouseButtonMapping::new(&transport, &cap)
        .unwrap()
        .apply(target, ButtonAction::Disabled, false, txs(), || Ok(()))
        .unwrap();
    assert!(report.source_completed);
    assert!(report.steps[0].acknowledgment_received);
    assert!(!report.verified());
    transport.finish();
}

#[test]
fn unknown_function_size_and_extra_bytes_are_preserved_as_raw_observation() {
    let target = target();
    let response = [3, 9, 0, 0xee, 0xfe, 1, 2, 3, 4, 5, 0xa5, 0x5a];
    let transport = Transport::new([
        send([10, 2, 140], &[3, 9, 0], 1),
        receive([10, 2, 140], &response, 1),
    ]);
    let observed = MouseButtonMapping::new(&transport, &cap(170))
        .unwrap()
        .read(target, txs(), || Ok(()))
        .unwrap();
    assert_eq!(observed.assignment.function_id, 0xee);
    assert_eq!(observed.assignment.data_size, 0xfe);
    assert_eq!(observed.raw_data, [1, 2, 3, 4, 5]);
    assert_eq!(observed.raw_payload, response);
    transport.finish();
}

#[test]
fn remaining_source_converter_families_have_exact_firmware_fields() {
    for (action, expected) in [
        (ButtonAction::HyperShift, (12, 1, [1, 0, 0, 0, 0])),
        (
            ButtonAction::Razer(RazerKey::FnKey),
            (17, 1, [1, 0, 0, 0, 0]),
        ),
        (
            ButtonAction::Power(PowerAction::SystemSleep),
            (9, 1, [130, 0, 0, 0, 0]),
        ),
        (
            ButtonAction::Windows(WindowsShortcut::XboxGameBar),
            (21, 1, [100, 0, 0, 0, 0]),
        ),
        (
            ButtonAction::Consumer(ConsumerAction::CycleApps),
            (10, 2, [2, 159, 0, 0, 0]),
        ),
        (
            ButtonAction::Mouse(MouseAction::DoubleClick),
            (11, 1, [1, 0, 0, 0, 0]),
        ),
        (
            ButtonAction::Mouse(MouseAction::ScrollModeSwitch),
            (18, 1, [1, 0, 0, 0, 0]),
        ),
        (
            ButtonAction::Keyboard(KeyboardAction {
                hid: 0x04,
                modifiers: 0x81,
                turbo: false,
                keys_per_second: None,
            }),
            (2, 2, [0x81, 4, 0, 0, 0]),
        ),
        (
            ButtonAction::Macro(MacroAction {
                macro_id: 0x0102,
                playback: MacroPlayback::Once,
            }),
            (3, 3, [1, 2, 1, 0, 0]),
        ),
        (
            ButtonAction::Macro(MacroAction {
                macro_id: 0x0102,
                playback: MacroPlayback::Held,
            }),
            (4, 2, [1, 2, 0, 0, 0]),
        ),
        (
            ButtonAction::Macro(MacroAction {
                macro_id: 0x0102,
                playback: MacroPlayback::Toggle,
            }),
            (5, 2, [1, 2, 0, 0, 0]),
        ),
        (
            ButtonAction::Macro(MacroAction {
                macro_id: 0x0102,
                playback: MacroPlayback::Sequence,
            }),
            (15, 2, [1, 2, 0, 0, 0]),
        ),
        (ButtonAction::Dpi(DpiAction::Up), (6, 1, [1, 0, 0, 0, 0])),
        (ButtonAction::Dpi(DpiAction::Down), (6, 1, [2, 0, 0, 0, 0])),
        (
            ButtonAction::Dpi(DpiAction::CycleUp),
            (6, 1, [6, 0, 0, 0, 0]),
        ),
        (
            ButtonAction::Dpi(DpiAction::CycleDown),
            (6, 1, [7, 0, 0, 0, 0]),
        ),
        (
            ButtonAction::Profile(ProfileAction::Up),
            (7, 1, [1, 0, 0, 0, 0]),
        ),
        (
            ButtonAction::Profile(ProfileAction::Down),
            (7, 1, [2, 0, 0, 0, 0]),
        ),
        (
            ButtonAction::Profile(ProfileAction::CycleUp),
            (7, 1, [4, 0, 0, 0, 0]),
        ),
        (
            ButtonAction::Profile(ProfileAction::CycleDown),
            (7, 1, [5, 0, 0, 0, 0]),
        ),
    ] {
        for product in [170, 204, 226, 45066] {
            assert_eq!(encode(product, action).unwrap(), expected);
        }
    }
}

#[test]
fn invalid_converter_values_fail_before_any_device_request() {
    let transport = Transport::new([]);
    for action in [
        ButtonAction::Mouse(MouseAction::TurboButton {
            button: MouseButton::Left,
            keys_per_second: Some(0),
        }),
        ButtonAction::Keyboard(KeyboardAction {
            hid: 4,
            modifiers: 0,
            turbo: true,
            keys_per_second: Some(0),
        }),
        ButtonAction::Macro(MacroAction {
            macro_id: 1,
            playback: MacroPlayback::NTimes(0),
        }),
        ButtonAction::Profile(ProfileAction::Specific(0)),
    ] {
        let failure = MouseButtonMapping::new(&transport, &cap(170))
            .unwrap()
            .apply(target(), action, true, txs(), || Ok(()))
            .expect_err("invalid typed action must be rejected before OUT");
        assert_eq!(failure.phase, ButtonFailurePhase::Prepare);
        assert!(!failure.device_state_may_have_changed());
    }
    transport.finish();
}

#[test]
fn raw_restore_preserves_unknown_function_and_padding_and_verifies_independently() {
    let target = target();
    let payload = [3, 9, 0, 0xee, 3, 1, 2, 3, 0xa5, 0x5a];
    let observed = parse_assignment(170, target, &payload).unwrap();
    let transport = Transport::new([
        send([10, 2, 12], &payload, 1),
        receive([10, 2, 12], &[], 1),
        send([10, 2, 140], &[3, 9, 0], 2),
        receive([10, 2, 140], &payload, 2),
    ]);
    let report = MouseButtonMapping::new(&transport, &cap(170))
        .unwrap()
        .restore_assignment(&observed, true, txs(), || Ok(()))
        .unwrap();
    assert!(report.verified());
    assert_eq!(report.steps[0].requested.function_id, 0xee);
    assert_eq!(report.steps[0].requested.data, [1, 2, 3, 0xa5, 0x5a]);
    transport.finish();
}

#[test]
fn raw_restore_rejects_oversized_or_inconsistent_or_nonproduct_observations() {
    let target = target();
    let observed = parse_assignment(170, target, &[3, 9, 0, 1, 1, 2, 0, 0, 0, 0]).unwrap();
    let mut oversized = observed.clone();
    oversized.assignment.data_size = 6;
    oversized.raw_payload[4] = 6;
    let mut contradicted = observed.clone();
    contradicted.raw_data[0] = 3;
    let mut wrong_target = observed.clone();
    wrong_target.assignment.target.button_id = 64;
    wrong_target.raw_payload[1] = 64;
    let transport = Transport::new([]);
    for observation in [oversized, contradicted, wrong_target] {
        let failure = MouseButtonMapping::new(&transport, &cap(170))
            .unwrap()
            .restore_assignment(&observation, true, txs(), || Ok(()))
            .expect_err("invalid raw restore must not issue OUT");
        assert_eq!(failure.phase, ButtonFailurePhase::Prepare);
        assert!(!failure.device_state_may_have_changed());
    }
    transport.finish();
}

#[test]
fn rejects_unknown_product_transport_and_zero_targets() {
    assert!(descriptor(999).is_err());
    let mut cap = cap(170);
    cap.source_class = "other".into();
    assert!(MouseButtonMapping::new(&Transport::new([]), &cap).is_err());
    assert!(
        assignment(
            170,
            ButtonTarget {
                profile_id: 0,
                button_id: 1,
                mode: AssignmentMode::Normal,
            },
            ButtonAction::Disabled,
        )
        .is_err()
    );
}
