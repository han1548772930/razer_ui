use super::*;
use crate::backend::ReportLengths;
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

#[derive(Debug)]
struct ScriptedTerminalFailure;

impl std::fmt::Display for ScriptedTerminalFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("scripted typed BlackShark native failure")
    }
}

impl std::error::Error for ScriptedTerminalFailure {}

fn terminal_failure(outcome: Option<NativeIoOutcome>) -> anyhow::Error {
    let error = ScriptedTerminalFailure.into();
    match outcome {
        Some(outcome) => crate::backend::native_io_failure(outcome, error),
        None => error,
    }
}

#[test]
fn help_factory_reset1398_mic_observer_keeps_the_actual_status_and_wire_header() {
    for (byte, expected) in [
        (0, BlackSharkMicStatus::Unmuted),
        (1, BlackSharkMicStatus::Muted),
        (7, BlackSharkMicStatus::Unknown(7)),
    ] {
        let (mut firmware, state) = session(1398, 1398, vec![reply(85, 2, &[byte])]);
        let observed = firmware.exchange(
            BlackSharkFirmwareCommand::GetMicStatus1398,
            |_| Ok(()),
            |_| Ok(()),
        );
        assert!(observed.succeeded());
        assert!(observed.acknowledged());
        assert_eq!(observed.observed_mic_status(), Some(expected));
        let state = state.borrow();
        assert_eq!(state.outputs.len(), 1);
        assert_eq!(state.reads, 1);
        assert!(state.replies.is_empty());
        assert_eq!(&state.outputs[0][6..14], &[4, 0, 0, 0, 85, 0, 0, 0]);
        assert_eq!(state.outputs[0][62], 0x33);
    }
    let (mut firmware, state) = session(1398, 1398, vec![reply(85, 4, &[1]); 3]);
    let observed = firmware.exchange(
        BlackSharkFirmwareCommand::GetMicStatus1398,
        |_| Ok(()),
        |_| Ok(()),
    );
    assert_eq!(observed.observed_mic_status(), None);
    assert!(!observed.succeeded());
    assert!(!observed.acknowledged());
    assert_eq!(
        observed.failure.as_ref().unwrap().kind,
        BlackSharkFirmwareFailureKind::DeviceRejected
    );
    assert_eq!(observed.attempts.len(), 3);
    let mut output = vec![0; 64];
    output[0] = 2;
    output[2] = 0x60;
    output[6] = 4;
    output[10] = 85;
    output[62] = 0x33;
    for attempt in &observed.attempts {
        assert_eq!(attempt.output, output);
        assert!(attempt.output_attempted && attempt.output_completed);
        assert_eq!(attempt.inputs, [reply(85, 4, &[1])]);
        assert!(attempt.rejected_inputs.is_empty());
        assert_eq!(attempt.reply.as_ref().unwrap().status, 4);
    }
    let state = state.borrow();
    assert_eq!(state.outputs, vec![output; 3]);
    assert_eq!(state.reads, 3);
    assert!(state.replies.is_empty());
}

#[test]
fn help_factory_reset1398_uses_the_source_general_report_and_rejects_foreign_owners() {
    for connected in [1398, 1399] {
        let (mut firmware, state) = session(1398, connected, vec![reply(239, 2, &[0])]);
        let reset = firmware.exchange(
            BlackSharkFirmwareCommand::FactoryReset1398,
            |_| Ok(()),
            |_| Ok(()),
        );
        assert!(reset.succeeded());
        assert!(reset.acknowledged());
        let state = state.borrow();
        assert_eq!(&state.outputs[0][..3], &[2, 0, 0x60]);
        assert_eq!(&state.outputs[0][6..14], &[5, 0, 0, 0, 239, 0, 1, 0]);
        assert_eq!(state.outputs[0][62], 0x89);
    }
    let (mut firmware, state) = session(1401, 1401, vec![]);
    let reset = firmware.exchange(
        BlackSharkFirmwareCommand::FactoryReset1398,
        |_| Ok(()),
        |_| Ok(()),
    );
    assert!(!reset.succeeded());
    assert_eq!(
        reset.failure.unwrap().kind,
        BlackSharkFirmwareFailureKind::InvalidRequest
    );
    assert!(state.borrow().outputs.is_empty());
}

#[test]
fn help_factory_reset1398_keeps_real_ack_after_cancel_and_never_retries_an_uncertain_write() {
    let (mut firmware, state) = session(1398, 1398, vec![reply(239, 2, &[0])]);
    let result = firmware.exchange(
        BlackSharkFirmwareCommand::FactoryReset1398,
        |_| {
            ensure!(state.borrow().reads == 0, "cancel after input");
            Ok(())
        },
        |_| Ok(()),
    );
    assert!(!result.succeeded());
    assert!(result.acknowledged());
    assert_eq!(state.borrow().outputs.len(), 1);
    assert_eq!(
        result.failure.unwrap().kind,
        BlackSharkFirmwareFailureKind::Validation
    );
}

#[test]
fn roller_current_six_owner_routes_use_actual_get_set_replies() {
    for (owner, pid) in [
        (1398, 1398),
        (1401, 1401),
        (2638, 2638),
        (2641, 2641),
        (4124, 4124),
        (4126, 4126),
    ] {
        let (mut firmware, state) =
            session(owner, pid, vec![reply(0x6a, 2, &[3]), reply(0xea, 2, &[0])]);
        let read = crate::blackshark_roller_mapping::execute(
            None,
            |command| Ok(firmware.exchange(command, |_| Ok(()), |_| Ok(()))),
            || Ok(()),
        );
        assert_eq!(read.observed_index, Some(3));
        assert_eq!(
            read.observed_mappings.as_ref().unwrap()[0]["audioGroup"]["audioAssignment"],
            "BluetoothVolume"
        );
        let set = crate::blackshark_roller_mapping::execute(
            Some(&[]),
            |command| Ok(firmware.exchange(command, |_| Ok(()), |_| Ok(()))),
            || Ok(()),
        );
        assert_eq!(set.source_result, Some(true));
        assert!(set.commands[0].acknowledged());
        let state = state.borrow();
        assert_eq!(&state.outputs[0][10..13], &[0x6a, 0, 0]);
        assert_eq!(&state.outputs[1][10..14], &[0xea, 0, 1, 0]);
        assert_eq!(state.outputs[1][9], if owner == 4126 { 0x80 } else { 0 });
    }
}

#[test]
fn roller_rejection_continues_in_source_order_and_retains_partial_ack() {
    let (mut firmware, state) = session(
        1398,
        1398,
        vec![
            reply(0xea, 3, &[0]),
            reply(0xea, 3, &[0]),
            reply(0xea, 3, &[0]),
            reply(0xea, 2, &[0]),
        ],
    );
    let mappings = vec![
        json!({"outputType":"audioGroup","audioGroup":{"audioAssignment":"SidetoneLevel"}}),
        json!({"outputType":"disableGroup"}),
    ];
    let progress = crate::blackshark_roller_mapping::execute(
        Some(&mappings),
        |command| Ok(firmware.exchange(command, |_| Ok(()), |_| Ok(()))),
        || Ok(()),
    );
    assert_eq!(progress.source_result, Some(false));
    assert_eq!(progress.commands.len(), 2);
    assert!(!progress.commands[0].acknowledged());
    assert!(progress.commands[1].acknowledged());
    assert_eq!(
        state
            .borrow()
            .outputs
            .iter()
            .map(|output| output[13])
            .collect::<Vec<_>>(),
        vec![1, 1, 1, 47]
    );
}

#[test]
fn roller_cancel_after_real_ack_retains_receipt_and_stops_next_write() {
    let (mut firmware, state) = session(1398, 1398, vec![reply(0xea, 2, &[0])]);
    let mappings = vec![
        json!({"outputType":"disableGroup"}),
        json!({"outputType":"disableGroup"}),
    ];
    let progress = crate::blackshark_roller_mapping::execute(
        Some(&mappings),
        |command| Ok(firmware.exchange(command, |_| Ok(()), |_| Ok(()))),
        || {
            ensure!(state.borrow().reads == 0, "canceled after reply");
            Ok(())
        },
    );
    assert_eq!(progress.commands.len(), 1);
    assert!(progress.commands[0].acknowledged());
    assert!(progress.source_result.is_none());
    assert!(progress.error.as_deref().unwrap().contains("canceled"));
}

#[test]
fn roller_failed_getter_does_not_claim_default_was_observed() {
    let (mut firmware, _) = session(1398, 1398, vec![reply(0x6a, 3, &[0]); 3]);
    let progress = crate::blackshark_roller_mapping::execute(
        None,
        |command| Ok(firmware.exchange(command, |_| Ok(()), |_| Ok(()))),
        || Ok(()),
    );
    assert!(progress.observed_index.is_none());
    assert!(progress.observed_mappings.is_none());
    assert_eq!(progress.source_mappings, Some(vec![]));
    assert!(progress.error.is_some());
}

#[derive(Default)]
struct MockState {
    outputs: Vec<Vec<u8>>,
    output_returns: VecDeque<Result<usize>>,
    replies: VecDeque<Result<Vec<u8>>>,
    input_returns: VecDeque<Result<usize>>,
    waits: Vec<u64>,
    reads: usize,
    fail_write: bool,
    cancel_after_read: bool,
    cancel_after_write: bool,
}
struct MockHid {
    state: Rc<RefCell<MockState>>,
    lengths: ReportLengths,
}
impl FeatureTransport for MockHid {
    fn send_feature(&self, _: &[u8]) -> Result<()> {
        panic!("BlackShark must use HID output, not feature reports")
    }
    fn get_feature(&self, _: &mut [u8]) -> Result<usize> {
        panic!("BlackShark must use interrupt input, not feature reports")
    }
    fn get_input(&self, _: &mut [u8]) -> Result<usize> {
        panic!("BlackShark must not substitute control input")
    }
    fn write_output(&self, report: &[u8]) -> Result<()> {
        let mut state = self.state.borrow_mut();
        state.outputs.push(report.to_vec());
        ensure!(!state.fail_write, "simulated output exception");
        Ok(())
    }
    fn write_output_counted(&self, report: &[u8]) -> Result<usize> {
        let scripted = self.state.borrow_mut().output_returns.pop_front();
        if let Some(Err(error)) = &scripted {
            if native_io_outcome(error) == Some(NativeIoOutcome::NotCalled) {
                return scripted.expect("a scripted preflight error is present");
            }
        }
        self.write_output(report)?;
        match scripted {
            Some(result) => result,
            None => Ok(report.len()),
        }
    }
    fn read_interrupt(&self, report: &mut [u8], timeout_ms: u32) -> Result<usize> {
        assert_eq!(timeout_ms, BLACKSHARK_INPUT_TIMEOUT_MS);
        let mut state = self.state.borrow_mut();
        state.reads += 1;
        let bytes = state
            .replies
            .pop_front()
            .expect("scripted HID input required")?;
        let count = bytes.len();
        if count <= report.len() {
            report[..count].copy_from_slice(&bytes);
        }
        match state.input_returns.pop_front() {
            Some(result) => result,
            None => Ok(count),
        }
    }
    fn report_lengths(&self) -> Result<ReportLengths> {
        Ok(self.lengths.clone())
    }
    fn metadata(&self) -> Value {
        json!({"transport":"simulated"})
    }
}
fn report_lengths() -> ReportLengths {
    ReportLengths {
        input: [(2, 64)].into(),
        output: [(2, 64)].into(),
        feature: Default::default(),
    }
}
fn node(pid: u16) -> HidNode {
    HidNode {
        path: b"simulated-collection-path".to_vec(),
        vendor_id: 0x1532,
        product_id: pid,
        usage_page: 0xff00,
        usage: 1,
        interface_number: 5,
        release_number: 0x0100,
        serial_number: Some("mock-serial".into()),
        manufacturer: None,
        product: None,
    }
}
fn identity(owner: u32, pid: u16) -> BlackSharkFirmwareIdentity {
    BlackSharkFirmwareIdentity::new(owner, node(pid), "mock-container", 7).unwrap()
}
fn session(
    owner: u32,
    pid: u16,
    packets: Vec<Vec<u8>>,
) -> (BlackSharkFirmwareSession, Rc<RefCell<MockState>>) {
    let state = Rc::new(RefCell::new(MockState {
        replies: packets.into_iter().map(Ok).collect(),
        ..Default::default()
    }));
    let transport = Box::new(MockHid {
        state: state.clone(),
        lengths: report_lengths(),
    });
    (
        BlackSharkFirmwareSession::new(identity(owner, pid), transport).unwrap(),
        state,
    )
}
// This fixture uses the observed raw offsets, independently of the Rust packer
// and decoder. Status=2, transaction=0x60, command bytes at 10/11, data at 13.
fn reply(id: u8, status: u8, data: &[u8]) -> Vec<u8> {
    let mut report = vec![0; 64];
    report[0] = 2;
    report[1] = status;
    report[2] = 0x60;
    report[10] = id;
    report[11] = 1;
    report[12] = data.len() as u8;
    report[13..13 + data.len()].copy_from_slice(data);
    report
}
fn waits(state: Rc<RefCell<MockState>>) -> impl FnMut(u64) -> Result<()> {
    move |milliseconds| {
        state.borrow_mut().waits.push(milliseconds);
        Ok(())
    }
}
fn values(owner: u32) -> Vec<Value> {
    default_blackshark_local_eq_presets(owner)
        .unwrap()
        .iter()
        .map(|preset| serde_json::to_value(preset).unwrap())
        .collect()
}

#[test]
fn all_six_routes_require_current_connected_pid_and_4126_is_always_dongle() {
    for (owner, dongle) in [
        (1398, 1399),
        (1401, 1402),
        (2638, 2645),
        (2641, 2648),
        (4124, 4123),
        (4126, 4126),
    ] {
        assert_eq!(
            blackshark_firmware_route(owner, dongle).unwrap(),
            BlackSharkFirmwareConnection::Dongle
        );
        if owner != 4126 {
            assert_eq!(
                blackshark_firmware_route(owner, owner as u16).unwrap(),
                BlackSharkFirmwareConnection::Wired
            );
        }
        assert!(blackshark_firmware_route(owner, 1397).is_err());
    }
    assert!(blackshark_firmware_route(1390, 1390).is_err());
    for modify in 0..4 {
        let mut selected = node(1398);
        match modify {
            0 => selected.vendor_id = 0x046d,
            1 => selected.product_id = 1401,
            2 => selected.interface_number = 4,
            _ => selected.path.clear(),
        };
        assert!(BlackSharkFirmwareIdentity::new(1398, selected, "container", 0).is_err());
    }
    assert!(BlackSharkFirmwareIdentity::new(1398, node(1398), "", 0).is_err());
    assert!(BlackSharkFirmwareIdentity::new(1398, node(1398), "a\0b", 0).is_err());
}

#[test]
fn descriptor_contract_requires_report2_input_output_exact64() {
    for lengths in [
        ReportLengths::default(),
        ReportLengths {
            input: [(2, 63)].into(),
            ..report_lengths()
        },
        ReportLengths {
            output: [(2, 65)].into(),
            ..report_lengths()
        },
        ReportLengths {
            input: [(0, 64)].into(),
            ..report_lengths()
        },
    ] {
        assert!(
            BlackSharkFirmwareSession::new(
                identity(1398, 1398),
                Box::new(MockHid {
                    state: Default::default(),
                    lengths
                })
            )
            .is_err()
        );
    }
}

#[test]
fn wire_goldens_use_source_header_checksum_and_sign_magnitude_bands() {
    let cases = [
        (
            BlackSharkFirmwareCommand::SetPresetEqEnabled(true),
            vec![2, 0, 0x60, 0, 0, 0, 5, 0, 0, 0, 0x9e, 0, 1, 1],
        ),
        (
            BlackSharkFirmwareCommand::GetPresetEqStatus,
            vec![2, 0, 0x60, 0, 0, 0, 4, 0, 0, 0, 0x1e, 0, 0],
        ),
        (
            BlackSharkFirmwareCommand::BatchStart,
            vec![2, 0, 0x60, 0, 0, 0, 5, 0, 0, 0, 0xe1, 0, 1, 1],
        ),
        (
            BlackSharkFirmwareCommand::BatchStop,
            vec![2, 0, 0x60, 0, 0, 0, 5, 0, 0, 0, 0xe1, 0, 1, 2],
        ),
        (
            BlackSharkFirmwareCommand::GetBands { index: 3 },
            vec![2, 0, 0x60, 0, 0, 0, 5, 0, 0, 0, 0x15, 0, 1, 3],
        ),
        (
            BlackSharkFirmwareCommand::GetOrder { index: 2 },
            vec![2, 0, 0x60, 0, 0, 0, 5, 0, 0, 0, 0x60, 0, 1, 2],
        ),
    ];
    for (command, prefix) in cases {
        let packed =
            pack_blackshark_general_report(&command, BlackSharkFirmwareConnection::Wired, false)
                .unwrap();
        assert_eq!(&packed[..prefix.len()], prefix);
        assert!(packed[prefix.len()..62].iter().all(|byte| *byte == 0));
        assert_eq!(packed[62], prefix.iter().fold(0, |sum, byte| sum ^ byte));
        assert_eq!(packed[63], 0);
    }
    let command = BlackSharkFirmwareCommand::SetBands {
        index: 3,
        bands: [3, 3, 3, -1, -4, -4, 2, 3, 3, 3],
    };
    let packed =
        pack_blackshark_general_report(&command, BlackSharkFirmwareConnection::Dongle, false)
            .unwrap();
    assert_eq!(packed[6], 15);
    assert_eq!(packed[9], 0x80);
    assert_eq!(
        &packed[10..24],
        &[0x95, 0, 11, 3, 3, 3, 3, 129, 132, 132, 2, 3, 3, 3]
    );
    assert_eq!(packed[62], 0xf3);
    let order = BlackSharkFirmwareCommand::SetOrder(BlackSharkEqOrder {
        index: 3,
        voice_prompt_index: 3,
        enabled: true,
        active: false,
        cloud_equalizer_id: 3,
        custom: false,
    });
    let packed =
        pack_blackshark_general_report(&order, BlackSharkFirmwareConnection::Wired, false).unwrap();
    assert_eq!(&packed[10..19], &[0xe0, 0, 6, 3, 3, 1, 0, 3, 0]);
    assert_eq!(packed[6], 10);
}

#[test]
fn integer_band_codec_round_trips_source_byte_domain() {
    for level in -127..=128 {
        assert_eq!(
            decode_blackshark_band(encode_blackshark_band(level).unwrap()),
            level
        );
    }
    assert_eq!(decode_blackshark_band(128), 128);
    assert_eq!(decode_blackshark_band(255), -127);
    assert!(encode_blackshark_band(-128).is_err());
    assert!(encode_blackshark_band(129).is_err());
    assert_eq!(encode_blackshark_band(-4).unwrap(), 132);
}

#[test]
fn matching_reply_requires_length_id_transaction_and_first_two_command_bytes() {
    let good = reply(0x9e, 2, &[222]);
    for field in [0, 2, 10, 11] {
        let mut bad = good.clone();
        bad[field] ^= 1;
        assert!(decode_blackshark_general_reply(&bad, [0x9e, 1, 1]).is_err());
    }
    assert!(decode_blackshark_general_reply(&good[..63], [0x9e, 1, 1]).is_err());
    let mut source_accepted = good.clone();
    source_accepted[6] = 255;
    source_accepted[9] = 128;
    source_accepted[12] = 0;
    source_accepted[62] = 255;
    source_accepted[63] = 17;
    let decoded = decode_blackshark_general_reply(&source_accepted, [0x9e, 1, 1]).unwrap();
    assert!(decoded.source_success());
    assert_eq!(decoded.data, vec![222]);
    assert!(decode_blackshark_general_reply(&good, [0x9e, 1, 60]).is_err());
    for status in [0, 1, 3, 4, 5, 255] {
        let packet = reply(0x9e, status, &[0]);
        let decoded = decode_blackshark_general_reply(&packet, [0x9e, 1, 1]).unwrap();
        assert!(!decoded.source_success());
        assert_eq!(decoded.resend_requested, matches!(status, 1 | 3));
    }
}

#[test]
fn getter_decoders_preserve_signed_levels_and_source_boolean_semantics() {
    assert_eq!(
        decode_blackshark_eq_bands(&[3, 0, 128, 129, 132, 255, 2, 3, 4, 5, 6], 3).unwrap(),
        [0, 128, -1, -4, -127, 2, 3, 4, 5, 6]
    );
    assert!(decode_blackshark_eq_bands(&[3; 10], 3).is_err());
    assert!(decode_blackshark_eq_bands(&[3; 11], 4).is_err());
    let order = decode_blackshark_eq_order(&[3, 7, 1, 2, 14, 255], 3).unwrap();
    assert!(order.enabled);
    assert!(!order.active);
    assert!(!order.custom);
    assert_eq!(order.cloud_equalizer_id, 14);
    assert!(decode_blackshark_eq_order(&[3; 5], 3).is_err());
    assert!(decode_blackshark_eq_order(&[3; 6], 4).is_err());
}

#[test]
fn wired_setter_waits_but_does_not_invent_readback() {
    let (mut firmware, state) = session(1398, 1398, vec![reply(0x9e, 2, &[99])]);
    let selected = firmware.identity().clone();
    let progress = firmware.set_preset_eq_enabled(
        true,
        |current| {
            assert_eq!(current, &selected);
            Ok(())
        },
        waits(state.clone()),
    );
    assert!(progress.succeeded());
    assert!(progress.acknowledged());
    assert_eq!(progress.source_status(), "SUCCESS");
    assert_eq!(progress.reply().unwrap().data, vec![99]);
    assert_eq!(state.borrow().reads, 1);
    assert_eq!(state.borrow().waits, vec![10, 5]);
    assert_eq!(state.borrow().outputs[0][13], 1);
    assert_eq!(firmware.last_command(), Some(&progress));
}

#[test]
fn dongle_constructor_state_and_explicit_target_reset_are_preserved() {
    let (mut firmware, state) = session(4126, 4126, vec![reply(0x9e, 2, &[0]); 4]);
    assert!(
        firmware
            .set_preset_eq_enabled(false, |_| Ok(()), waits(state.clone()))
            .succeeded()
    );
    assert!(
        firmware
            .set_preset_eq_enabled(true, |_| Ok(()), waits(state.clone()))
            .succeeded()
    );
    assert!(
        firmware
            .exchange_to_dongle(
                BlackSharkFirmwareCommand::SetPresetEqEnabled(false),
                |_| Ok(()),
                waits(state.clone())
            )
            .succeeded()
    );
    assert!(
        firmware
            .set_preset_eq_enabled(true, |_| Ok(()), waits(state.clone()))
            .succeeded()
    );
    assert_eq!(
        state
            .borrow()
            .outputs
            .iter()
            .map(|bytes| bytes[9])
            .collect::<Vec<_>>(),
        vec![0, 128, 0, 128]
    );
    assert_eq!(state.borrow().waits, vec![10, 30, 10, 30, 10, 30, 10, 30]);
}

#[test]
fn constructor_flag_resets_even_when_first_send_throws() {
    let (mut firmware, state) = session(1398, 1399, vec![reply(0x9e, 2, &[0])]);
    state.borrow_mut().fail_write = true;
    let failed = firmware.set_preset_eq_enabled(false, |_| Ok(()), waits(state.clone()));
    assert_eq!(
        failed.failure.unwrap().kind,
        BlackSharkFirmwareFailureKind::Transport
    );
    state.borrow_mut().fail_write = false;
    let result = firmware.set_preset_eq_enabled(true, |_| Ok(()), waits(state.clone()));
    assert!(result.succeeded());
    assert_eq!(state.borrow().outputs[0][9], 0);
    assert_eq!(state.borrow().outputs[1][9], 128);
}

#[test]
fn source_retries_all_non_success_statuses_including_no_resend_flag() {
    for status in [0, 1, 3, 4, 5, 255] {
        let (mut firmware, state) = session(1398, 1398, vec![reply(0x9e, status, &[0]); 3]);
        let result = firmware.set_preset_eq_enabled(false, |_| Ok(()), waits(state.clone()));
        assert!(!result.succeeded());
        assert!(!result.acknowledged());
        assert_eq!(result.source_status(), "FAIL");
        assert_eq!(
            result.failure.unwrap().kind,
            BlackSharkFirmwareFailureKind::DeviceRejected
        );
        assert_eq!(state.borrow().outputs.len(), 3);
        assert_eq!(state.borrow().reads, 3);
    }
    let (mut firmware, state) =
        session(1398, 1398, vec![reply(0x9e, 1, &[0]), reply(0x9e, 2, &[0])]);
    let result = firmware.set_preset_eq_enabled(true, |_| Ok(()), waits(state.clone()));
    assert!(result.succeeded());
    assert_eq!(result.attempts.len(), 2);
}

#[test]
fn unrelated_packets_are_bounded_and_never_count_as_success() {
    let (mut firmware, state) = session(1398, 1398, vec![reply(0x15, 2, &[0]); 15]);
    let result = firmware.set_preset_eq_enabled(true, |_| Ok(()), waits(state.clone()));
    assert!(!result.succeeded());
    assert_eq!(
        result.failure.unwrap().kind,
        BlackSharkFirmwareFailureKind::InputExhausted
    );
    assert_eq!(result.attempts.len(), 3);
    assert!(
        result
            .attempts
            .iter()
            .all(|attempt| attempt.inputs.len() == 5 && attempt.rejected_inputs.len() == 5)
    );
    assert_eq!(state.borrow().reads, 15);
}

#[test]
fn empty_and_truncated_packets_consume_budget_and_matching_packet_finishes() {
    let mut wrong_tx = reply(0x9e, 2, &[0]);
    wrong_tx[2] = 0x61;
    let (mut firmware, state) = session(
        1398,
        1398,
        vec![
            vec![],
            reply(0x9e, 2, &[0])[..63].to_vec(),
            wrong_tx,
            reply(0x9e, 2, &[0]),
        ],
    );
    let result = firmware.set_preset_eq_enabled(true, |_| Ok(()), waits(state.clone()));
    assert!(result.succeeded());
    assert_eq!(result.attempts[0].inputs.len(), 4);
    assert_eq!(result.attempts[0].rejected_inputs.len(), 3);
    assert_eq!(state.borrow().outputs.len(), 1);
}

#[test]
fn cancellation_before_and_during_io_preserves_attempts_and_observed_ack() {
    let (mut firmware, state) = session(1398, 1398, vec![]);
    let result = firmware.set_preset_eq_enabled(
        true,
        |_| anyhow::bail!("stale identity"),
        waits(state.clone()),
    );
    assert_eq!(
        result.failure.unwrap().kind,
        BlackSharkFirmwareFailureKind::Validation
    );
    assert!(state.borrow().outputs.is_empty());
    let (mut firmware, state) = session(1398, 1398, vec![]);
    state.borrow_mut().cancel_after_write = true;
    let guard_state = state.clone();
    let result = firmware.set_preset_eq_enabled(
        true,
        move |_| {
            ensure!(
                !guard_state.borrow().cancel_after_write || guard_state.borrow().outputs.is_empty(),
                "canceled after write"
            );
            Ok(())
        },
        waits(state.clone()),
    );
    assert_eq!(
        result.failure.unwrap().kind,
        BlackSharkFirmwareFailureKind::Validation
    );
    assert_eq!(state.borrow().outputs.len(), 1);
    assert_eq!(state.borrow().reads, 0);
    assert!(result.attempts[0].output_completed);
    assert_eq!(result.attempts[0].output_returned_bytes, Some(64));
    let (mut firmware, state) = session(1398, 1398, vec![reply(0x9e, 2, &[1])]);
    state.borrow_mut().cancel_after_read = true;
    let guard_state = state.clone();
    let result = firmware.set_preset_eq_enabled(
        true,
        move |_| {
            ensure!(
                !guard_state.borrow().cancel_after_read || guard_state.borrow().reads == 0,
                "device disconnected"
            );
            Ok(())
        },
        waits(state.clone()),
    );
    assert!(!result.succeeded());
    assert!(!result.completed);
    assert!(result.acknowledged());
    assert_eq!(result.attempts[0].inputs, [reply(0x9e, 2, &[1])]);
    assert!(
        result
            .failure
            .as_ref()
            .unwrap()
            .message
            .contains("HID exchange owner validation also failed")
    );
    assert_eq!(
        result.failure.unwrap().kind,
        BlackSharkFirmwareFailureKind::Validation
    );
}

#[test]
fn counted_output_retries_normal_short_returns_before_reading_the_actual_reply() {
    for (owner, connected, delay) in [
        (1398, 1398, 5),
        (1398, 1399, 30),
        (1401, 1401, 5),
        (1401, 1402, 30),
        (2638, 2638, 5),
        (2638, 2645, 30),
        (2641, 2641, 5),
        (2641, 2648, 30),
        (4124, 4124, 5),
        (4124, 4123, 30),
        (4126, 4126, 30),
    ] {
        let acknowledged = reply(0x9e, 2, &[1]);
        let (mut firmware, state) = session(owner, connected, vec![acknowledged.clone()]);
        state
            .borrow_mut()
            .output_returns
            .extend([Ok(0), Ok(63), Ok(64)]);
        let result = firmware.set_preset_eq_enabled(true, |_| Ok(()), waits(state.clone()));
        assert!(result.succeeded());
        assert!(result.acknowledged());
        assert_eq!(result.attempts.len(), 3);
        let mut expected = vec![0; 64];
        expected[..14].copy_from_slice(&[2, 0, 0x60, 0, 0, 0, 5, 0, 0, 0, 0x9e, 0, 1, 1]);
        expected[62] = 0xf9;
        for (attempt, count) in result.attempts.iter().zip([0, 63, 64]) {
            assert_eq!(attempt.output, expected);
            assert!(attempt.output_attempted);
            assert_eq!(attempt.output_returned_bytes, Some(count));
            assert_eq!(attempt.output_completed, count == 64);
            if count == 64 {
                assert_eq!(attempt.inputs, [acknowledged.clone()]);
                assert!(attempt.reply.as_ref().unwrap().source_success());
            } else {
                assert!(attempt.inputs.is_empty());
                assert!(attempt.reply.is_none());
            }
        }
        let state = state.borrow();
        assert_eq!(state.outputs, vec![expected; 3]);
        assert_eq!(state.reads, 1);
        assert_eq!(state.waits, [10, delay]);
        assert!(state.output_returns.is_empty());
        assert!(state.replies.is_empty());
    }
}

#[test]
fn counted_output_normal_oversized_count_retries_exactly_until_64_for_every_source_route() {
    for (owner, connected, delay) in [
        (1398, 1398, 5),
        (1398, 1399, 30),
        (1401, 1401, 5),
        (1401, 1402, 30),
        (2638, 2638, 5),
        (2638, 2645, 30),
        (2641, 2641, 5),
        (2641, 2648, 30),
        (4124, 4124, 5),
        (4124, 4123, 30),
        (4126, 4126, 30),
    ] {
        for complete_after_retry in [false, true] {
            let acknowledged = reply(0x9e, 2, &[1]);
            let packets = if complete_after_retry {
                vec![acknowledged.clone()]
            } else {
                vec![]
            };
            let (mut firmware, state) = session(owner, connected, packets);
            state
                .borrow_mut()
                .output_returns
                .extend(if complete_after_retry {
                    vec![Ok(65), Ok(64)]
                } else {
                    vec![Ok(65), Ok(65), Ok(65)]
                });
            let result = firmware.set_preset_eq_enabled(true, |_| Ok(()), waits(state.clone()));
            assert_eq!(result.succeeded(), complete_after_retry);
            assert_eq!(result.acknowledged(), complete_after_retry);
            assert_eq!(
                result.attempts.len(),
                if complete_after_retry { 2 } else { 3 }
            );
            let mut expected = vec![0; 64];
            expected[..14].copy_from_slice(&[2, 0, 0x60, 0, 0, 0, 5, 0, 0, 0, 0x9e, 0, 1, 1]);
            expected[62] = 0xf9;
            for (ordinal, attempt) in result.attempts.iter().enumerate() {
                let complete = complete_after_retry && ordinal == 1;
                assert_eq!(attempt.output, expected);
                assert!(attempt.output_attempted);
                assert_eq!(
                    attempt.output_returned_bytes,
                    Some(if complete { 64 } else { 65 })
                );
                assert_eq!(attempt.output_completed, complete);
                if complete {
                    assert_eq!(attempt.inputs, [acknowledged.clone()]);
                } else {
                    assert!(attempt.inputs.is_empty());
                    assert!(attempt.reply.is_none());
                }
            }
            if !complete_after_retry {
                assert_eq!(
                    result.failure.unwrap().kind,
                    BlackSharkFirmwareFailureKind::Transport
                );
            }
            let state = state.borrow();
            assert_eq!(
                state.outputs,
                vec![expected; if complete_after_retry { 2 } else { 3 }]
            );
            assert_eq!(state.reads, usize::from(complete_after_retry));
            assert_eq!(
                state.waits,
                if complete_after_retry {
                    vec![10, delay]
                } else {
                    vec![10]
                }
            );
            assert!(state.output_returns.is_empty());
            assert!(state.replies.is_empty());
        }
    }
}

#[test]
fn counted_output_exhausts_unequal_returns_without_reading_or_fabricating_acknowledgement() {
    for count in [0, 1, 63, 65] {
        let (mut firmware, state) = session(1398, 1398, vec![]);
        state
            .borrow_mut()
            .output_returns
            .extend((0..3).map(|_| Ok(count)));
        let result = firmware.set_preset_eq_enabled(true, |_| Ok(()), waits(state.clone()));
        assert!(!result.succeeded());
        assert!(!result.acknowledged());
        assert_eq!(result.attempts.len(), 3);
        for attempt in &result.attempts {
            assert!(attempt.output_attempted);
            assert_eq!(attempt.output_returned_bytes, Some(count));
            assert!(!attempt.output_completed);
            assert!(attempt.inputs.is_empty());
            assert!(attempt.reply.is_none());
        }
        assert_eq!(state.borrow().outputs.len(), 3);
        assert_eq!(state.borrow().reads, 0);
        assert_eq!(state.borrow().waits, [10]);
        assert!(state.borrow().output_returns.is_empty());
        assert_eq!(
            result.failure.unwrap().kind,
            BlackSharkFirmwareFailureKind::Transport
        );
    }
}

#[test]
fn counted_output_cancellation_after_a_short_return_prevents_the_next_send() {
    let (mut firmware, state) = session(1398, 1398, vec![]);
    state.borrow_mut().output_returns.extend([Ok(63), Ok(64)]);
    let result = firmware.set_preset_eq_enabled(
        true,
        |_| {
            ensure!(
                state.borrow().outputs.is_empty(),
                "canceled after a short output"
            );
            Ok(())
        },
        waits(state.clone()),
    );
    assert!(!result.succeeded());
    assert!(!result.acknowledged());
    assert_eq!(result.attempts.len(), 1);
    assert_eq!(result.attempts[0].output_returned_bytes, Some(63));
    assert!(!result.attempts[0].output_completed);
    assert!(result.attempts[0].inputs.is_empty());
    assert_eq!(
        result.failure.unwrap().kind,
        BlackSharkFirmwareFailureKind::Validation
    );
    let state = state.borrow();
    assert_eq!(state.outputs.len(), 1);
    assert_eq!(state.reads, 0);
    assert_eq!(state.waits, [10]);
    assert_eq!(state.output_returns.len(), 1);
}

#[test]
fn counted_output_preserves_failed_returns_without_reading_or_retrying() {
    for (outcome, attempted) in [
        (None, false),
        (Some(NativeIoOutcome::NotCalled), false),
        (Some(NativeIoOutcome::Rejected), true),
        (Some(NativeIoOutcome::Returned { byte_count: 0 }), true),
        (Some(NativeIoOutcome::Returned { byte_count: 63 }), true),
        (Some(NativeIoOutcome::Returned { byte_count: 64 }), true),
        (Some(NativeIoOutcome::Returned { byte_count: 65 }), true),
    ] {
        let (mut firmware, state) = session(1398, 1398, vec![]);
        state
            .borrow_mut()
            .output_returns
            .push_back(Err(terminal_failure(outcome)));
        let result = firmware.set_preset_eq_enabled(true, |_| Ok(()), waits(state.clone()));
        let expected_count = match outcome {
            Some(NativeIoOutcome::Returned { byte_count }) => Some(byte_count),
            _ => None,
        };
        assert!(!result.succeeded());
        assert!(!result.acknowledged());
        assert_eq!(result.attempts.len(), 1);
        assert_eq!(result.attempts[0].output_attempted, attempted);
        assert_eq!(result.attempts[0].output_returned_bytes, expected_count);
        assert_eq!(
            result.attempts[0].output_completed,
            expected_count == Some(64)
        );
        assert_eq!(state.borrow().reads, 0);
        assert_eq!(
            state.borrow().outputs.len(),
            if outcome == Some(NativeIoOutcome::NotCalled) {
                0
            } else {
                1
            }
        );
        let failure = result.failure.unwrap();
        assert_eq!(failure.kind, BlackSharkFirmwareFailureKind::Transport);
        assert!(
            failure
                .message
                .contains("scripted typed BlackShark native failure")
        );
    }
}

#[test]
fn native_failure_and_release_failure_are_both_retained_without_more_io() {
    struct FailingRelease {
        inner: MockHid,
        releases: Rc<Cell<usize>>,
    }
    struct Lease<'a>(&'a FailingRelease);
    impl crate::backend::ExchangeLease for Lease<'_> {
        fn transport(&self) -> &dyn FeatureTransport {
            &self.0.inner
        }
        fn retry_release(&self) -> Result<()> {
            panic!("a terminal native failure must not retry release")
        }
        fn finish(self: Box<Self>) -> Result<()> {
            self.0.releases.set(self.0.releases.get() + 1);
            Err(anyhow::anyhow!("scripted independent release failure"))
        }
    }
    impl FeatureTransport for FailingRelease {
        fn begin_exchange(
            &self,
            _: crate::backend::ExchangeOptions,
        ) -> Result<Box<dyn crate::backend::ExchangeLease + '_>> {
            Ok(Box::new(Lease(self)))
        }
        fn send_feature(&self, _: &[u8]) -> Result<()> {
            panic!("BlackShark must use its retained Output lease")
        }
        fn get_feature(&self, _: &mut [u8]) -> Result<usize> {
            panic!("BlackShark must use its retained interrupt lease")
        }
        fn report_lengths(&self) -> Result<ReportLengths> {
            self.inner.report_lengths()
        }
        fn metadata(&self) -> Value {
            self.inner.metadata()
        }
    }

    for during_input in [false, true] {
        let state = Rc::new(RefCell::new(MockState::default()));
        if during_input {
            state
                .borrow_mut()
                .replies
                .push_back(Err(terminal_failure(None)));
        } else {
            state
                .borrow_mut()
                .output_returns
                .push_back(Err(terminal_failure(None)));
        }
        let releases = Rc::new(Cell::new(0));
        let mut firmware = BlackSharkFirmwareSession::new(
            identity(1398, 1398),
            Box::new(FailingRelease {
                inner: MockHid {
                    state: state.clone(),
                    lengths: report_lengths(),
                },
                releases: releases.clone(),
            }),
        )
        .unwrap();
        let result = firmware.set_preset_eq_enabled(true, |_| Ok(()), waits(state.clone()));
        assert!(!result.succeeded());
        assert!(!result.acknowledged());
        let failure = result.failure.unwrap();
        assert_eq!(failure.kind, BlackSharkFirmwareFailureKind::Transport);
        assert!(
            failure
                .message
                .contains("scripted typed BlackShark native failure")
        );
        assert!(failure.message.contains("HID exchange release also failed"));
        assert!(
            failure
                .message
                .contains("scripted independent release failure")
        );
        assert_eq!(state.borrow().outputs.len(), 1);
        assert_eq!(state.borrow().reads, usize::from(during_input));
        assert_eq!(releases.get(), 1);
    }
}

#[test]
fn interrupt_diagnostics_require_trusted_bytes_and_a_complete_protocol_reply() {
    for outcome in [
        None,
        Some(NativeIoOutcome::NotCalled),
        Some(NativeIoOutcome::Rejected),
        Some(NativeIoOutcome::Returned { byte_count: 0 }),
        Some(NativeIoOutcome::Returned { byte_count: 63 }),
        Some(NativeIoOutcome::Returned { byte_count: 64 }),
        Some(NativeIoOutcome::Returned { byte_count: 65 }),
    ] {
        for cancel_after_read in [false, true] {
            let (mut firmware, state) = session(1398, 1398, vec![reply(0x9e, 2, &[1])]);
            state
                .borrow_mut()
                .input_returns
                .push_back(Err(terminal_failure(outcome)));
            let validation_state = state.clone();
            let result = firmware.set_preset_eq_enabled(
                true,
                |_| {
                    ensure!(
                        !cancel_after_read || validation_state.borrow().reads == 0,
                        "owner changed while reading an untrusted terminal failure"
                    );
                    Ok(())
                },
                waits(state.clone()),
            );
            assert!(!result.succeeded());
            assert!(!result.completed);
            assert!(!result.acknowledged());
            assert!(result.attempts[0].inputs.is_empty());
            assert!(result.attempts[0].reply.is_none());
            assert_eq!(state.borrow().outputs.len(), 1);
            assert_eq!(state.borrow().reads, 1);
            let failure = result.failure.unwrap();
            assert_eq!(
                failure.kind,
                if cancel_after_read {
                    BlackSharkFirmwareFailureKind::Validation
                } else {
                    BlackSharkFirmwareFailureKind::Transport
                }
            );
            assert!(
                failure
                    .message
                    .contains("scripted typed BlackShark native failure")
            );
        }
    }

    for count in [0, 1, 63, 64, 65] {
        let mut packet = reply(0x9e, 2, &[1]);
        packet.truncate(count);
        let (mut firmware, state) = session(1398, 1398, vec![packet.clone()]);
        if count == 65 {
            state.borrow_mut().input_returns.push_back(Ok(65));
        }
        let validation_state = state.clone();
        let result = firmware.set_preset_eq_enabled(
            true,
            |_| {
                ensure!(
                    validation_state.borrow().reads == 0,
                    "owner changed during read"
                );
                Ok(())
            },
            waits(state.clone()),
        );
        assert!(!result.succeeded());
        assert!(!result.completed);
        assert_eq!(result.acknowledged(), count == 64);
        assert_eq!(
            result.attempts[0].inputs,
            if count <= 64 { vec![packet] } else { vec![] }
        );
        assert_eq!(result.attempts[0].output_returned_bytes, Some(64));
        assert_eq!(state.borrow().outputs.len(), 1);
        assert_eq!(state.borrow().reads, 1);
        assert_eq!(
            result.failure.unwrap().kind,
            BlackSharkFirmwareFailureKind::Validation
        );
    }

    let (mut firmware, state) = session(1398, 1398, vec![reply(0x95, 2, &[1])]);
    let validation_state = state.clone();
    let result = firmware.set_preset_eq_enabled(
        true,
        |_| {
            ensure!(
                validation_state.borrow().reads == 0,
                "owner changed during read"
            );
            Ok(())
        },
        waits(state.clone()),
    );
    assert!(!result.acknowledged());
    assert_eq!(result.attempts[0].inputs, [reply(0x95, 2, &[1])]);
    assert_eq!(result.attempts[0].rejected_inputs.len(), 1);
    assert_eq!(state.borrow().outputs.len(), 1);
    assert_eq!(state.borrow().reads, 1);
}

#[test]
fn scheduling_failure_prevents_output_and_impossible_read_count_is_transport_error() {
    let (mut firmware, state) = session(1398, 1398, vec![]);
    let result =
        firmware.set_preset_eq_enabled(true, |_| Ok(()), |_| anyhow::bail!("scheduler stopped"));
    assert_eq!(
        result.failure.unwrap().kind,
        BlackSharkFirmwareFailureKind::Scheduling
    );
    assert!(state.borrow().outputs.is_empty());
    let (mut firmware, state) = session(1398, 1398, vec![vec![2; 65]]);
    let result = firmware.set_preset_eq_enabled(true, |_| Ok(()), waits(state.clone()));
    assert_eq!(
        result.failure.unwrap().kind,
        BlackSharkFirmwareFailureKind::Transport
    );
    assert_eq!(state.borrow().outputs.len(), 1);
}

#[test]
fn defaults_and_metadata_match_every_current_owner_receipt() {
    let evidence: Value = serde_json::from_str(include_str!(
        "../../testdata/re/evidence/thx-blackshark-source-current.json"
    ))
    .unwrap();
    for product in evidence["products"].as_array().unwrap() {
        let owner = product["product_id"].as_u64().unwrap() as u32;
        let presets = default_blackshark_local_eq_presets(owner).unwrap();
        assert_eq!(
            serde_json::to_value(presets).unwrap(),
            product["wire_protocol"]["local_preset_schema"]["defaults"]
        );
        assert_eq!(
            json!(BLACKSHARK_BAND_FREQUENCIES_HZ),
            product["wire_protocol"]["local_preset_schema"]["band_frequencies_hz"]
        );
        assert!(product["wire_protocol"]["route"]["source_factory_receipt"].is_object());
        assert_eq!(
            product["wire_protocol"]["identity"]["unsupported_wired"],
            json!(owner == 4126)
        );
    }
    assert!(default_blackshark_local_eq_presets(1390).is_err());
}

#[test]
fn local_restore_uses_first_matching_preset_then_order_and_batch_stop() {
    let (mut firmware, state) = session(
        1398,
        1398,
        vec![
            reply(0xe1, 2, &[0]),
            reply(0x95, 2, &[0]),
            reply(0xe0, 2, &[0]),
            reply(0xe1, 2, &[0]),
        ],
    );
    let mut presets = values(1398);
    let mut duplicate = presets[3].clone();
    duplicate["bandValues"] = json!(vec![0; 10]);
    presets.push(duplicate);
    let result =
        firmware.restore_local_eq(&presets, Some(&json!(3)), |_| Ok(()), waits(state.clone()));
    assert!(result.succeeded());
    assert!(result.source_bool());
    assert!(result.batch_start_acknowledged);
    assert!(result.batch_stop_acknowledged);
    assert_eq!(result.supplied_indices, vec![3]);
    assert_eq!(
        state
            .borrow()
            .outputs
            .iter()
            .map(|bytes| bytes[10])
            .collect::<Vec<_>>(),
        vec![0xe1, 0x95, 0xe0, 0xe1]
    );
    assert_eq!(state.borrow().outputs[1][17], 129);
    assert_eq!(state.borrow().outputs[3][13], 2);
}

#[test]
fn full_restore_preserves_input_order_and_all_nine_local_defaults() {
    let mut packets = vec![reply(0xe1, 2, &[0])];
    for _ in 0..9 {
        packets.push(reply(0x95, 2, &[0]));
        packets.push(reply(0xe0, 2, &[0]));
    }
    packets.push(reply(0xe1, 2, &[0]));
    let (mut firmware, state) = session(1398, 1398, packets);
    let result = firmware.restore_local_eq(
        &values(1398),
        Some(&json!(-1)),
        |_| Ok(()),
        waits(state.clone()),
    );
    assert!(result.succeeded());
    assert_eq!(result.supplied_indices, (0..=8).collect::<Vec<_>>());
    assert_eq!(result.commands.len(), 20);
    for index in 0..9 {
        assert_eq!(state.borrow().outputs[1 + index * 2][13], index as u8);
        assert_eq!(state.borrow().outputs[2 + index * 2][13], index as u8);
    }
}

#[test]
fn partial_restore_retains_acknowledged_bands_without_automatic_stop_or_rollback() {
    let (mut firmware, state) = session(
        1398,
        1398,
        vec![
            reply(0xe1, 2, &[0]),
            reply(0x95, 2, &[0]),
            reply(0xe0, 3, &[0]),
            reply(0xe0, 3, &[0]),
            reply(0xe0, 3, &[0]),
        ],
    );
    let result = firmware.restore_local_eq(
        &values(1398),
        Some(&json!(1)),
        |_| Ok(()),
        waits(state.clone()),
    );
    assert!(!result.succeeded());
    assert!(!result.source_bool());
    assert!(result.batch_start_acknowledged);
    assert!(!result.batch_stop_acknowledged);
    assert_eq!(result.commands.len(), 3);
    assert!(result.commands[1].acknowledged());
    assert!(!result.commands[2].acknowledged());
    assert_eq!(
        result.failure.unwrap().kind,
        BlackSharkFirmwareFailureKind::DeviceRejected
    );
    assert_eq!(
        state
            .borrow()
            .outputs
            .iter()
            .map(|bytes| bytes[10])
            .collect::<Vec<_>>(),
        vec![0xe1, 0x95, 0xe0, 0xe0, 0xe0]
    );
}

#[test]
fn empty_preset_list_still_runs_batch_and_start_stop_failures_are_explicit() {
    let (mut firmware, state) =
        session(1398, 1398, vec![reply(0xe1, 2, &[0]), reply(0xe1, 2, &[0])]);
    assert!(
        firmware
            .restore_local_eq(&[], None, |_| Ok(()), waits(state.clone()))
            .succeeded()
    );
    assert_eq!(
        state
            .borrow()
            .outputs
            .iter()
            .map(|bytes| bytes[13])
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    for fail_start in [true, false] {
        let mut replies = vec![];
        if !fail_start {
            replies.push(reply(0xe1, 2, &[0]));
        }
        replies.extend(vec![reply(0xe1, 3, &[0]); 3]);
        let (mut firmware, state) = session(1398, 1398, replies);
        let result = firmware.restore_local_eq(&[], None, |_| Ok(()), waits(state.clone()));
        assert!(!result.succeeded());
        assert_eq!(result.batch_start_acknowledged, !fail_start);
        assert!(!result.batch_stop_acknowledged);
        assert_eq!(result.commands.len(), if fail_start { 1 } else { 2 });
    }
}

#[test]
fn invalid_presets_are_rejected_before_batch_as_explicit_rust_boundary_policy() {
    let (mut firmware, state) = session(1398, 1398, vec![]);
    let good = values(1398);
    let mut short = good[0].clone();
    short["bandValues"] = json!(vec![0; 9]);
    let mut float = good[0].clone();
    float["bandValues"] = json!([0, 0, 0, 0, 0, 0, 0, 0, 0, 0.5]);
    let mut overflow = good[0].clone();
    overflow["index"] = json!(256);
    let mut wrapped = good[0].clone();
    wrapped["bandValues"] = json!([-128, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    for invalid in [short, float, overflow, wrapped] {
        let result = firmware.restore_local_eq(&[invalid], None, |_| Ok(()), waits(state.clone()));
        assert_eq!(
            result.failure.unwrap().kind,
            BlackSharkFirmwareFailureKind::InvalidRequest
        );
        assert!(result.commands.is_empty());
    }
    for selected in [json!(99), json!("0"), json!(0.5), json!(-2)] {
        let result =
            firmware.restore_local_eq(&good, Some(&selected), |_| Ok(()), waits(state.clone()));
        assert!(!result.succeeded());
        assert!(result.commands.is_empty());
    }
    assert!(state.borrow().outputs.is_empty());
}

#[test]
fn stale_identity_checks_cover_each_command_in_batch_and_retain_prior_ack() {
    let (mut firmware, state) =
        session(1398, 1398, vec![reply(0xe1, 2, &[0]), reply(0x95, 2, &[0])]);
    let validation_state = state.clone();
    let result = firmware.restore_local_eq(
        &values(1398),
        Some(&json!(0)),
        move |current| {
            assert_eq!(current.generation, 7);
            ensure!(
                validation_state.borrow().reads < 2,
                "new generation selected"
            );
            Ok(())
        },
        waits(state.clone()),
    );
    assert!(!result.succeeded());
    assert_eq!(
        result.failure.unwrap().kind,
        BlackSharkFirmwareFailureKind::Validation
    );
    assert_eq!(result.commands.len(), 2);
    assert!(result.commands[1].acknowledged());
    assert!(!result.commands[1].completed);
    assert_eq!(state.borrow().outputs.len(), 2);
}

#[test]
fn guard_runs_again_after_scheduler_and_stops_without_output() {
    let (mut firmware, state) = session(1398, 1398, vec![]);
    let canceled = Cell::new(false);
    let result = firmware.set_preset_eq_enabled(
        true,
        |_| {
            ensure!(!canceled.get(), "canceled during scheduler");
            Ok(())
        },
        |_| {
            canceled.set(true);
            Ok(())
        },
    );
    assert_eq!(
        result.failure.unwrap().kind,
        BlackSharkFirmwareFailureKind::Validation
    );
    assert!(state.borrow().outputs.is_empty());
}
