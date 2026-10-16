use super::*;
use crate::mapping_native_output::{DriverOperation, SubmissionId};
use std::collections::VecDeque;

#[test]
fn keyboard_packet_matches_type_21_layout() {
    let report = encode_keyboard(KeyboardInput {
        unit_id: 7,
        scancode: 48,
        flag: 2,
        reserved: 0,
        modifiers: 0x770,
    });
    let mut expected = [0; 32];
    expected[4..8].copy_from_slice(&1u32.to_le_bytes());
    expected[8..10].copy_from_slice(&7u16.to_le_bytes());
    expected[10..12].copy_from_slice(&48u16.to_le_bytes());
    expected[12..14].copy_from_slice(&2u16.to_le_bytes());
    expected[16..20].copy_from_slice(&native_modifier_bits(0x770).to_le_bytes());
    assert_eq!(report.bytes(), &expected);
    assert_eq!(report.ioctl(), DRIVER_SEND_INPUT_IOCTL);
}

#[test]
fn modifier_transform_preserves_recovered_sparse_bit_mapping() {
    let cases = [
        (0x10, 0x100),
        (0x20, 0x80),
        (0x40, 0x40),
        (0x100, 4),
        (0x200, 2),
        (0x400, 1),
        (0x10 | 0x20 | 0x40 | 0x100 | 0x200 | 0x400, 0x1c7),
    ];
    for (source, expected) in cases {
        assert_eq!(native_modifier_bits(source), expected, "source {source:#x}");
    }
}

#[test]
fn mouse_button_and_wheel_packets_keep_native_word_positions() {
    let button = encode_mouse(MouseInput {
        unit_id: 2,
        id: 1,
        data: 0,
        modifiers: 0,
        ..Default::default()
    });
    assert_eq!(&button.bytes()[4..8], &2u32.to_le_bytes());
    assert_eq!(&button.bytes()[8..16], &[2, 0, 0, 0, 1, 0, 0, 0]);
    assert_eq!(&button.bytes()[24..32], &[0; 8]);

    let wheel = encode_mouse(MouseInput {
        unit_id: 0,
        id: 1024,
        data: MouseInput::signed_data(-120),
        modifiers: 0x40,
        ..Default::default()
    });
    assert_eq!(&wheel.bytes()[12..16], &[0, 4, 0x88, 0xff]);
    assert_eq!(&wheel.bytes()[28..32], &[0x40, 0, 0, 0]);
}

#[test]
fn display_opcode_accepts_only_the_three_native_source_ids() {
    assert_eq!(display_from_source_id(3).unwrap().payload_word(), 112);
    assert_eq!(display_from_source_id(4).unwrap().payload_word(), 111);
    assert_eq!(display_from_source_id(5).unwrap().payload_word(), 0);
    assert_eq!(
        display_from_source_id(6),
        Err(NativeInputError::UnsupportedDisplaySource(6))
    );
    assert_eq!(
        &encode_display(DisplayInput::SourceId4).bytes()[4..10],
        &[10, 0, 0, 0, 111, 0]
    );
}

#[test]
fn e1_keyboard_adds_native_scan_45_continuation_after_primary() {
    let reports = encode_keyboard_sequence(KeyboardInput {
        unit_id: 9,
        scancode: 29,
        flag: 4,
        reserved: 0,
        modifiers: 0x40,
    });
    assert_eq!(reports.len(), 2);
    assert_eq!(&reports[0].bytes()[8..14], &[9, 0, 29, 0, 4, 0]);
    assert_eq!(&reports[1].bytes()[8..14], &[9, 0, 69, 0, 0, 0]);
    assert_eq!(
        &reports[1].bytes()[16..20],
        &native_modifier_bits(native_modifier_bits(0x40)).to_le_bytes()
    );
}

#[test]
fn ordinary_keyboard_does_not_add_e1_continuation() {
    assert_eq!(
        encode_keyboard_sequence(KeyboardInput {
            unit_id: 0,
            scancode: 29,
            flag: 2,
            modifiers: 0,
            ..Default::default()
        })
        .len(),
        1
    );
    assert_eq!(
        encode_keyboard_sequence(KeyboardInput {
            unit_id: 0,
            scancode: 30,
            flag: 4,
            modifiers: 0,
            ..Default::default()
        })
        .len(),
        1
    );
}

#[test]
fn ignored_modifier_bits_and_e1_double_transform_match_the_native_integer_formula() {
    assert_eq!(native_modifier_bits(!0x770), 0);
    assert_eq!(native_modifier_bits(u32::MAX), 0x1c7);
    let reports = encode_keyboard_sequence(KeyboardInput {
        unit_id: 0x1234,
        scancode: 29,
        flag: 5,
        reserved: 0x7777,
        modifiers: u32::MAX,
    });
    assert_eq!(&reports[0].bytes()[14..20], &[0x77, 0x77, 0xc7, 1, 0, 0]);
    assert_eq!(&reports[1].bytes()[12..20], &[1, 0, 0, 0, 0x44, 0, 0, 0]);
}

#[test]
fn keyboard_press_release_changes_only_low_flag_bit() {
    let key = KeyboardInput {
        unit_id: 3,
        scancode: 69,
        flag: 0xfff4,
        reserved: 19,
        modifiers: 0x770,
    };
    assert!(!key.is_released());
    assert_eq!(key.released().flag, 0xfff5);
    assert!(key.released().is_released());
    assert_eq!(key.released().pressed(), key);
}

#[test]
fn mouse_seven_button_pairs_keep_other_fields_and_wheels_are_not_button_states() {
    for (press, release) in [
        (1, 2),
        (4, 8),
        (16, 32),
        (64, 128),
        (256, 512),
        (0x10000, 0x20000),
        (0x40000, 0x80000),
    ] {
        let input = MouseInput {
            unit_id: 9,
            flags: 3,
            id: press as u16,
            data: (press >> 16) as u16,
            raw_buttons: 0xfeedbeef,
            movement: [-123, 456],
            modifiers: 0x770,
        };
        assert!(input.mapping_id_valid());
        assert!(input.is_pressed());
        assert!(!input.is_released());
        let released = input.released();
        assert_eq!(released.packed_buttons(), release);
        assert!(released.mapping_id_valid());
        assert!(!released.is_pressed());
        assert!(released.is_released());
        assert_eq!(released.pressed(), input);
        assert_eq!(input.pressed(), input);
        assert_eq!(released.released(), released);
    }
    for id in [1024, 2048] {
        let wheel = MouseInput {
            id,
            data: 0xabcd,
            ..Default::default()
        };
        assert!(wheel.mapping_id_valid());
        assert!(!wheel.is_pressed());
        assert!(!wheel.is_released());
        assert_eq!(wheel.pressed(), wheel);
        assert_eq!(wheel.released(), wheel);
    }
    assert!(
        !MouseInput {
            id: 3,
            ..Default::default()
        }
        .mapping_id_valid()
    );
    assert!(
        !MouseInput {
            id: 1,
            data: 1,
            ..Default::default()
        }
        .mapping_id_valid()
    );
}

#[test]
fn mouse_full_native_layout_retains_signed_movements_and_flags() {
    let input = MouseInput {
        unit_id: 0x1234,
        flags: 0xabcd,
        id: 2048,
        data: MouseInput::signed_data(-1),
        raw_buttons: 0x76543210,
        movement: [i32::MIN, i32::MAX],
        modifiers: 0x770,
    };
    assert_eq!(input.signed_wheel_delta(), -1);
    let packet = encode_mouse(input);
    assert_eq!(
        &packet.bytes()[8..32],
        &[
            0x34, 0x12, 0xcd, 0xab, 0, 8, 0xff, 0xff, 0x10, 0x32, 0x54, 0x76, 0, 0, 0, 0x80, 0xff,
            0xff, 0xff, 0x7f, 0xc7, 1, 0, 0,
        ]
    );
    assert_eq!(
        decode_native_input(packet.bytes()).unwrap(),
        DecodedNativeInput::Mouse {
            unit_id: input.unit_id,
            flags: input.flags,
            id: input.id,
            data: input.data,
            raw_buttons: input.raw_buttons,
            movement: input.movement,
            driver_modifiers: 0x1c7,
        }
    );
}

#[test]
fn keyboard_and_display_decoder_return_exact_fields() {
    let packet = encode_keyboard(KeyboardInput {
        unit_id: 4,
        scancode: 57,
        flag: 5,
        reserved: 0x9876,
        modifiers: 0x700,
    });
    assert_eq!(
        decode_native_input(packet.bytes()).unwrap(),
        DecodedNativeInput::Keyboard {
            unit_id: 4,
            scancode: 57,
            flag: 5,
            reserved: 0x9876,
            driver_modifiers: 7,
        }
    );
    for input in [
        DisplayInput::SourceId3,
        DisplayInput::SourceId4,
        DisplayInput::SourceId5,
    ] {
        assert_eq!(
            decode_native_input(encode_display(input).bytes()).unwrap(),
            DecodedNativeInput::Display(input)
        );
    }
}

#[test]
fn decoder_rejects_bad_lengths_reserved_bytes_opcodes_and_driver_modifier_bits() {
    assert_eq!(
        decode_native_input(&[0; 31]),
        Err(NativeInputError::InvalidPacketLength(31))
    );
    assert_eq!(
        decode_native_input(&[0; 33]),
        Err(NativeInputError::InvalidPacketLength(33))
    );
    let mut packet = *encode_keyboard(KeyboardInput::default()).bytes();
    packet[0] = 1;
    assert_eq!(
        decode_native_input(&packet),
        Err(NativeInputError::NonzeroReserved { offset: 0 })
    );
    packet[0] = 0;
    packet[20] = 1;
    assert_eq!(
        decode_native_input(&packet),
        Err(NativeInputError::NonzeroReserved { offset: 20 })
    );
    packet[20] = 0;
    packet[16] = 8;
    assert_eq!(
        decode_native_input(&packet),
        Err(NativeInputError::InvalidDriverModifiers(8))
    );
    packet[16] = 0;
    packet[4] = 99;
    assert_eq!(
        decode_native_input(&packet),
        Err(NativeInputError::UnsupportedOpcode(99))
    );
    let mut display = *encode_display(DisplayInput::SourceId3).bytes();
    display[8] = 113;
    assert_eq!(
        decode_native_input(&display),
        Err(NativeInputError::UnsupportedDisplayPayload(113))
    );
    display[8] = 112;
    display[31] = 1;
    assert_eq!(
        decode_native_input(&display),
        Err(NativeInputError::NonzeroReserved { offset: 31 })
    );
    let mut mouse = *encode_mouse(MouseInput::default()).bytes();
    mouse[28] = 0x20;
    assert_eq!(
        decode_native_input(&mouse),
        Err(NativeInputError::InvalidDriverModifiers(0x20))
    );
}

#[derive(Debug, PartialEq, Eq)]
enum DriverEvent {
    Submit,
    Poll(SubmissionId),
    Cancel(SubmissionId),
}

#[derive(Default)]
struct MockDriver {
    submissions: Vec<NativeInputReport>,
    events: Vec<DriverEvent>,
    submit_results: VecDeque<Result<SubmissionId, DriverOutputError>>,
    poll_results: VecDeque<Result<CompletionPoll, DriverOutputError>>,
    cancel_results: VecDeque<Result<(), DriverOutputError>>,
    waits: Vec<Duration>,
    first_poll_delay: Duration,
}

impl MappingOutputTransport for MockDriver {
    fn submit(&mut self, report: NativeInputReport) -> Result<SubmissionId, DriverOutputError> {
        self.events.push(DriverEvent::Submit);
        self.submissions.push(report);
        self.submit_results
            .pop_front()
            .unwrap_or(Ok(SubmissionId(self.submissions.len() as u64)))
    }
    fn poll_completion(
        &mut self,
        id: SubmissionId,
        wait: Duration,
    ) -> Result<CompletionPoll, DriverOutputError> {
        self.events.push(DriverEvent::Poll(id));
        self.waits.push(wait);
        if self.waits.len() == 1 {
            std::thread::sleep(self.first_poll_delay);
        }
        self.poll_results
            .pop_front()
            .unwrap_or(Ok(CompletionPoll::Pending))
    }
    fn cancel(&mut self, id: SubmissionId) -> Result<(), DriverOutputError> {
        self.events.push(DriverEvent::Cancel(id));
        self.cancel_results.pop_front().unwrap_or(Ok(()))
    }
    fn close(&mut self) -> Result<(), DriverOutputError> {
        Ok(())
    }
}

#[derive(Default)]
struct MockOs {
    inputs: Vec<OsInput>,
    results: VecDeque<Result<(), OsInputError>>,
}

impl OsInputTransport for MockOs {
    fn send_one(&mut self, input: OsInput) -> Result<(), OsInputError> {
        self.inputs.push(input);
        self.results.pop_front().unwrap_or(Ok(()))
    }
}

fn e1_key() -> NativeInput {
    NativeInput::Keyboard(KeyboardInput {
        scancode: 29,
        flag: 5,
        modifiers: 0x770,
        ..Default::default()
    })
}

fn ordinary_key() -> NativeInput {
    NativeInput::Keyboard(KeyboardInput {
        scancode: 30,
        ..Default::default()
    })
}

fn driver_error(operation: DriverOperation) -> DriverOutputError {
    DriverOutputError::Win32 { operation, code: 5 }
}

fn os_error() -> OsInputError {
    OsInputError::SendInput {
        inserted: 0,
        win32_code: 5,
    }
}

#[test]
fn driver_e1_continuation_is_queued_before_primary_completion_and_caller_order_is_preserved() {
    let mut driver = MockDriver::default();
    let execution =
        NativeInputExecution::submit(&mut driver, &[e1_key(), ordinary_key()], Duration::ZERO);
    assert_eq!(
        driver.events,
        vec![
            DriverEvent::Submit,
            DriverEvent::Submit,
            DriverEvent::Submit,
            DriverEvent::Poll(SubmissionId(1)),
            DriverEvent::Poll(SubmissionId(2)),
            DriverEvent::Poll(SubmissionId(3)),
        ]
    );
    assert_eq!(
        execution.reports[1].origin,
        InputReportOrigin::E1Continuation { primary_index: 0 }
    );
    assert_eq!(
        execution.reports[2].origin,
        InputReportOrigin::Primary { index: 1 }
    );
    assert!(execution.is_pending());
    assert!(!execution.driver_confirmed());
    assert!(!execution.primary_requests_confirmed());
}

#[test]
fn rejected_driver_primary_never_queues_a_driver_continuation() {
    let mut driver = MockDriver {
        submit_results: [Err(driver_error(DriverOperation::Submit))].into(),
        ..Default::default()
    };
    let execution = NativeInputExecution::submit(&mut driver, &[e1_key()], Duration::ZERO);
    assert_eq!(driver.events, vec![DriverEvent::Submit]);
    assert_eq!(execution.reports.len(), 1);
    assert_eq!(
        execution.reports[0].delivery,
        ReportDelivery::Failed(driver_error(DriverOperation::Submit))
    );
    assert!(!execution.is_pending());
}

#[test]
fn failed_driver_continuation_does_not_change_primary_confirmation_or_invoke_os_fallback() {
    let mut driver = MockDriver {
        submit_results: [
            Ok(SubmissionId(1)),
            Err(driver_error(DriverOperation::Submit)),
        ]
        .into(),
        poll_results: [Ok(CompletionPoll::Completed(Ok(())))].into(),
        ..Default::default()
    };
    let mut os = MockOs::default();
    let execution = NativeInputExecution::submit_with_os_fallback(
        &mut driver,
        &mut os,
        &[e1_key()],
        Duration::ZERO,
    );
    assert!(execution.primary_requests_confirmed());
    assert!(!execution.driver_confirmed());
    assert_eq!(
        execution.reports[1].delivery,
        ReportDelivery::Failed(driver_error(DriverOperation::Submit))
    );
    assert!(os.inputs.is_empty());
    assert!(execution.reports[1].os_fallback.is_none());
}

#[test]
fn driver_acceptance_remains_pending_until_final_completion() {
    let mut driver = MockDriver::default();
    let mut execution =
        NativeInputExecution::submit(&mut driver, &[ordinary_key()], Duration::ZERO);
    assert!(execution.is_pending());
    assert!(!execution.driver_confirmed());
    driver
        .poll_results
        .push_back(Ok(CompletionPoll::Completed(Ok(()))));
    execution.poll(&mut driver, Duration::ZERO);
    assert!(execution.driver_confirmed());
    assert!(execution.primary_requests_confirmed());
    assert!(!execution.is_pending());
}

#[test]
fn polling_budget_is_shared_across_all_driver_reports() {
    let budget = Duration::from_millis(2);
    let mut driver = MockDriver {
        first_poll_delay: budget,
        ..Default::default()
    };
    let _execution = NativeInputExecution::submit(&mut driver, &[e1_key()], budget);
    assert!(driver.waits[0] <= budget);
    assert_eq!(driver.waits[1], Duration::ZERO);
}

#[test]
fn poll_and_cancel_observation_errors_keep_pending_requests_and_do_not_trigger_fallback() {
    let mut driver = MockDriver {
        poll_results: [
            Err(driver_error(DriverOperation::Complete)),
            Ok(CompletionPoll::Pending),
        ]
        .into(),
        cancel_results: [Err(driver_error(DriverOperation::Cancel))].into(),
        ..Default::default()
    };
    let mut os = MockOs::default();
    let mut execution = NativeInputExecution::submit_with_os_fallback(
        &mut driver,
        &mut os,
        &[ordinary_key()],
        Duration::ZERO,
    );
    execution.cancel_pending(&mut driver, Duration::ZERO);
    execution.fallback_failed_primaries(&mut os);
    assert!(execution.is_pending());
    assert_eq!(
        execution.reports[0].observation_errors,
        vec![
            driver_error(DriverOperation::Complete),
            driver_error(DriverOperation::Cancel)
        ]
    );
    assert!(os.inputs.is_empty());
    driver
        .poll_results
        .push_back(Ok(CompletionPoll::Completed(Ok(()))));
    execution.poll_with_os_fallback(&mut driver, &mut os, Duration::ZERO);
    assert!(execution.driver_confirmed());
    assert!(os.inputs.is_empty());
}

#[test]
fn cancellation_is_drained_to_a_final_failure_without_claiming_success() {
    let mut driver = MockDriver::default();
    let mut execution =
        NativeInputExecution::submit(&mut driver, &[ordinary_key()], Duration::ZERO);
    driver
        .poll_results
        .push_back(Ok(CompletionPoll::Completed(Err(driver_error(
            DriverOperation::Complete,
        )))));
    execution.cancel_pending(&mut driver, Duration::ZERO);
    assert_eq!(
        &driver.events[2..],
        &[
            DriverEvent::Cancel(SubmissionId(1)),
            DriverEvent::Poll(SubmissionId(1))
        ]
    );
    assert!(!execution.is_pending());
    assert!(!execution.driver_confirmed());
    assert_eq!(
        execution.reports[0].delivery,
        ReportDelivery::Failed(driver_error(DriverOperation::Complete))
    );
}

#[test]
fn empty_input_execution_is_not_a_driver_acknowledgement() {
    let mut driver = MockDriver::default();
    let execution = NativeInputExecution::submit(&mut driver, &[], Duration::ZERO);
    assert!(driver.events.is_empty());
    assert!(!execution.driver_confirmed());
    assert!(!execution.primary_requests_confirmed());
}

#[test]
fn os_keyboard_flags_match_scancode_extended_and_release_bits_only() {
    for flag in [0, 1, 2, 3, 4, 5, 6, 7, 0xfff8, 0xffff] {
        let plan = plan_os_input(NativeInput::Keyboard(KeyboardInput {
            unit_id: 42,
            scancode: 30,
            flag,
            reserved: u16::MAX,
            modifiers: u32::MAX,
        }))
        .unwrap();
        assert_eq!(
            plan.primary,
            OsInput::Keyboard {
                scancode: 30,
                flags: 8 | u32::from(flag & 6 != 0) | (2 * u32::from(flag & 1 != 0))
            }
        );
        assert!(plan.continuation.is_none());
    }
    let plan = plan_os_input(e1_key()).unwrap();
    assert_eq!(
        plan.primary,
        OsInput::Keyboard {
            scancode: 29,
            flags: 11
        }
    );
    assert_eq!(
        plan.continuation,
        Some(OsInput::Keyboard {
            scancode: 69,
            flags: 10
        })
    );
}

#[test]
fn os_mouse_buttons_xbuttons_and_wheels_match_native_fallback_and_drop_movement() {
    for (id, flags, data) in [
        (1, 2, 0),
        (2, 4, 0),
        (4, 8, 0),
        (8, 16, 0),
        (16, 32, 0),
        (32, 64, 0),
        (64, 128, 1),
        (128, 256, 1),
        (256, 128, 2),
        (512, 256, 2),
    ] {
        let plan = plan_os_input(NativeInput::Mouse(MouseInput {
            unit_id: 9,
            flags: u16::MAX,
            id,
            data: 999,
            raw_buttons: u32::MAX,
            movement: [i32::MAX, i32::MIN],
            modifiers: u32::MAX,
        }))
        .unwrap();
        assert_eq!(plan.primary, OsInput::Mouse { flags, data });
        assert!(plan.continuation.is_none());
    }
    for (id, flags) in [(1024, 2048), (2048, 4096)] {
        for source in [0, 119, 120, 121, 240, u16::MAX, (-120i16) as u16] {
            let plan = plan_os_input(NativeInput::Mouse(MouseInput {
                id,
                data: source,
                ..Default::default()
            }))
            .unwrap();
            assert_eq!(
                plan.primary,
                OsInput::Mouse {
                    flags,
                    data: if source == 120 { 120 } else { (-120i32) as u32 }
                }
            );
        }
    }
}

#[test]
fn os_fallback_rejects_display_and_unsupported_mouse_ids_before_sending() {
    let mut os = MockOs::default();
    for id in [0, 3, 7, 0x3000, u16::MAX] {
        let result = OsInputExecution::submit(
            &mut os,
            NativeInput::Mouse(MouseInput {
                id,
                ..Default::default()
            }),
        );
        assert_eq!(result, Err(OsInputError::UnsupportedMouseId(id)));
    }
    assert_eq!(
        OsInputExecution::submit(&mut os, NativeInput::Display(DisplayInput::SourceId5)),
        Err(OsInputError::UnsupportedDisplayInput)
    );
    assert!(os.inputs.is_empty());
}

#[test]
fn failed_os_e1_primary_never_sends_continuation() {
    let mut os = MockOs {
        results: [Err(os_error())].into(),
        ..Default::default()
    };
    let execution = OsInputExecution::submit(&mut os, e1_key()).unwrap();
    assert_eq!(
        os.inputs,
        vec![OsInput::Keyboard {
            scancode: 29,
            flags: 11
        }]
    );
    assert_eq!(execution.reports[0].result, Err(os_error()));
    assert!(!execution.os_accepted());
}

#[test]
fn os_e1_continuation_failure_is_part_of_the_fallback_result() {
    let mut os = MockOs {
        results: [Ok(()), Err(os_error())].into(),
        ..Default::default()
    };
    let execution = OsInputExecution::submit(&mut os, e1_key()).unwrap();
    assert_eq!(execution.reports.len(), 2);
    assert!(!execution.os_accepted());
    assert_eq!(execution.reports[0].result, Ok(()));
    assert_eq!(execution.reports[1].result, Err(os_error()));
}

#[test]
fn failed_driver_primary_runs_os_fallback_once_without_fabricating_driver_confirmation() {
    let mut driver = MockDriver {
        submit_results: [Err(driver_error(DriverOperation::Submit))].into(),
        ..Default::default()
    };
    let mut os = MockOs::default();
    let mut execution = NativeInputExecution::submit_with_os_fallback(
        &mut driver,
        &mut os,
        &[e1_key()],
        Duration::ZERO,
    );
    assert_eq!(driver.submissions.len(), 1);
    assert_eq!(os.inputs.len(), 2);
    assert!(
        execution.reports[0]
            .os_fallback
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .os_accepted()
    );
    assert!(!execution.driver_confirmed());
    assert!(!execution.primary_requests_confirmed());
    execution.fallback_failed_primaries(&mut os);
    execution.poll_with_os_fallback(&mut driver, &mut os, Duration::ZERO);
    assert_eq!(os.inputs.len(), 2);
}

#[test]
fn os_fallback_waits_for_a_final_driver_failure_and_preserves_both_errors() {
    let mut driver = MockDriver::default();
    let mut os = MockOs {
        results: [Err(os_error())].into(),
        ..Default::default()
    };
    let mut execution = NativeInputExecution::submit_with_os_fallback(
        &mut driver,
        &mut os,
        &[ordinary_key()],
        Duration::ZERO,
    );
    assert!(os.inputs.is_empty());
    driver
        .poll_results
        .push_back(Ok(CompletionPoll::Completed(Err(driver_error(
            DriverOperation::Complete,
        )))));
    execution.poll_with_os_fallback(&mut driver, &mut os, Duration::ZERO);
    assert_eq!(
        execution.reports[0].delivery,
        ReportDelivery::Failed(driver_error(DriverOperation::Complete))
    );
    assert_eq!(
        execution.reports[0]
            .os_fallback
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .reports[0]
            .result,
        Err(os_error())
    );
    assert!(!execution.driver_confirmed());
    assert_eq!(os.inputs.len(), 1);
}

#[test]
fn successful_driver_completion_does_not_send_an_os_fallback() {
    let mut driver = MockDriver {
        poll_results: [Ok(CompletionPoll::Completed(Ok(())))].into(),
        ..Default::default()
    };
    let mut os = MockOs::default();
    let execution = NativeInputExecution::submit_with_os_fallback(
        &mut driver,
        &mut os,
        &[ordinary_key()],
        Duration::ZERO,
    );
    assert!(execution.driver_confirmed());
    assert!(os.inputs.is_empty());
}

#[test]
fn unsupported_os_platform_is_recorded_separately_from_driver_failure() {
    let mut driver = MockDriver {
        submit_results: [Err(DriverOutputError::UnsupportedPlatform)].into(),
        ..Default::default()
    };
    let mut os = UnsupportedOsInputTransport;
    let execution = NativeInputExecution::submit_with_os_fallback(
        &mut driver,
        &mut os,
        &[ordinary_key()],
        Duration::ZERO,
    );
    assert_eq!(
        execution.reports[0].delivery,
        ReportDelivery::Failed(DriverOutputError::UnsupportedPlatform)
    );
    assert_eq!(
        execution.reports[0]
            .os_fallback
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .reports[0]
            .result,
        Err(OsInputError::UnsupportedPlatform)
    );
    assert!(!execution.driver_confirmed());
}
