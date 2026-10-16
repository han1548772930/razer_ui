use super::*;
use crate::mapping_native_analog::{
    AnalogResolution, GamepadAxis, GamepadCurve, GamepadPoint, JoystickBank, JoystickCurve,
    JoystickPoint,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Action {
    Submit([u8; 32]),
    Poll(SubmissionId),
    Cancel(SubmissionId),
    Close,
}

#[derive(Default)]
struct SimulatedDriver {
    log: Vec<Action>,
    next_id: u64,
    submit_count: usize,
    fail_submit: Option<usize>,
    fail_completion: Option<usize>,
    poll_error: bool,
    close_error: bool,
    pending_polls: usize,
    pending: BTreeMap<SubmissionId, (usize, Result<(), DriverOutputError>)>,
}

fn os_error(operation: DriverOperation, code: u32) -> DriverOutputError {
    DriverOutputError::Win32 { operation, code }
}

impl MappingOutputTransport for SimulatedDriver {
    fn submit(&mut self, report: DriverInputReport) -> Result<SubmissionId, DriverOutputError> {
        self.log.push(Action::Submit(*report.bytes()));
        self.submit_count += 1;
        if self.fail_submit == Some(self.submit_count) {
            return Err(os_error(DriverOperation::Submit, 6));
        }
        self.next_id += 1;
        let id = SubmissionId(self.next_id);
        let result = if self.fail_completion == Some(self.submit_count) {
            Err(os_error(DriverOperation::Complete, 31))
        } else {
            Ok(())
        };
        self.pending.insert(id, (self.pending_polls, result));
        Ok(id)
    }

    fn poll_completion(
        &mut self,
        id: SubmissionId,
        _: Duration,
    ) -> Result<CompletionPoll, DriverOutputError> {
        self.log.push(Action::Poll(id));
        if self.poll_error {
            return Err(os_error(DriverOperation::Complete, 5));
        }
        let operation = self
            .pending
            .get_mut(&id)
            .ok_or(DriverOutputError::UnknownSubmission(id))?;
        if operation.0 > 0 {
            operation.0 -= 1;
            return Ok(CompletionPoll::Pending);
        }
        Ok(CompletionPoll::Completed(
            self.pending.remove(&id).unwrap().1,
        ))
    }

    fn cancel(&mut self, id: SubmissionId) -> Result<(), DriverOutputError> {
        self.log.push(Action::Cancel(id));
        let operation = self
            .pending
            .get_mut(&id)
            .ok_or(DriverOutputError::UnknownSubmission(id))?;
        operation.1 = Err(os_error(DriverOperation::Complete, 995));
        Ok(())
    }

    fn close(&mut self) -> Result<(), DriverOutputError> {
        self.log.push(Action::Close);
        if !self.pending.is_empty() {
            return Err(DriverOutputError::ExecutionPending);
        }
        if self.close_error {
            return Err(os_error(DriverOperation::Close, 6));
        }
        Ok(())
    }
}

fn joystick(value: [i32; 3]) -> NativeAnalogOutput {
    NativeAnalogOutput::Joystick(
        JoystickCurve::compile(
            AnalogResolution::V2,
            JoystickBank::Axes,
            &[
                JoystickPoint {
                    actuation: 0,
                    axes: [0; 3],
                },
                JoystickPoint {
                    actuation: 65535,
                    axes: value,
                },
            ],
        )
        .unwrap(),
    )
}

fn gamepad(axis: GamepadAxis, value: i32) -> NativeAnalogOutput {
    NativeAnalogOutput::Gamepad(
        GamepadCurve::compile(
            AnalogResolution::V2,
            axis,
            &[
                GamepadPoint {
                    actuation: 0,
                    value: 0,
                },
                GamepadPoint {
                    actuation: 65535,
                    value,
                },
            ],
        )
        .unwrap(),
    )
}

#[test]
fn source_axes_packet_uses_unsigned_centers_and_distinct_opcodes() {
    let xyz = encode_driver_input(DriverInput::JoystickAxes {
        group: JoystickAxisGroup::Axes,
        axes: [-128, 0, 127],
    });
    let mut golden = [0; 32];
    golden[4] = 5;
    golden[8..11].copy_from_slice(&[0, 128, 255]);
    assert_eq!(xyz.bytes(), &golden);
    assert_eq!(xyz.ioctl(), 0x8888_3020);
    golden[4] = 6;
    assert_eq!(
        encode_driver_input(DriverInput::JoystickAxes {
            group: JoystickAxisGroup::RotationAxes,
            axes: [-128, 0, 127],
        })
        .bytes(),
        &golden
    );
}

#[test]
fn source_button_bytes_use_one_based_number_and_zero_for_press() {
    assert!(JoystickButton::new(0).is_err());
    assert!(JoystickButton::new(25).is_err());
    for (pressed, native_state) in [(true, 0), (false, 1)] {
        let report = encode_driver_input(DriverInput::JoystickButton {
            button: JoystickButton::new(24).unwrap(),
            pressed,
        });
        let mut golden = [0; 32];
        golden[4] = 7;
        golden[8] = native_state;
        golden[9] = 24;
        assert_eq!(report.bytes(), &golden);
    }
}

#[test]
fn source_pov_uses_all_eight_sign_pairs_and_ignores_z() {
    for (index, value) in [
        [0, -128, 0],
        [1, -1, 0],
        [127, 0, 0],
        [1, 1, 0],
        [0, 127, 0],
        [-1, 1, 0],
        [-128, 0, 0],
        [-1, -1, 0],
        [0, 0, 127],
    ]
    .into_iter()
    .enumerate()
    {
        let report = encode_driver_input(DriverInput::JoystickPov(value));
        let mut golden = [0; 32];
        golden[4] = 9;
        golden[8] = index as u8;
        assert_eq!(report.bytes(), &golden);
    }
}

#[test]
fn source_gamepad_packet_preserves_reserved_gap_and_signed_word_order() {
    let report = encode_driver_input(DriverInput::Gamepad(GamepadOutputState {
        buttons: 0xa501,
        left_trigger: 9,
        right_trigger: 255,
        left_thumb: [i16::MIN, 1],
        right_thumb: [i16::MAX, -2],
    }));
    assert_eq!(
        report.bytes(),
        &[
            0, 0, 0, 0, 12, 0, 0, 0, 0, 0, 1, 0xa5, 9, 255, 0, 0x80, 1, 0, 0xff, 0x7f, 0xfe, 0xff,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]
    );
}

#[test]
fn delta_order_matches_native_and_keeps_pov_z_in_group_comparison() {
    let mut current = MappingOutputState::default();
    current.joystick.buttons[23] = true;
    current.joystick.buttons[0] = true;
    current.joystick.axes = [1, 0, 0];
    current.joystick.rotation_axes = [0, 2, 0];
    current.joystick.pov = [0, 0, 1];
    current.gamepad.left_trigger = 3;
    let changes = changed_driver_inputs(MappingOutputState::default(), current);
    assert_eq!(changes.len(), 6);
    assert!(
        matches!(changes[0], DriverInput::JoystickButton { button, .. } if button.number() == 1)
    );
    assert!(
        matches!(changes[1], DriverInput::JoystickButton { button, .. } if button.number() == 24)
    );
    assert_eq!(
        changes
            .iter()
            .map(|v| encode_driver_input(*v).bytes()[4])
            .collect::<Vec<_>>(),
        [7, 7, 5, 6, 9, 12]
    );
    assert_eq!(encode_driver_input(changes[4]).bytes()[8], 8);
    assert!(changed_driver_inputs(current, current).is_empty());
}

#[test]
fn analog_batch_submits_only_the_final_opposing_aggregate() {
    let positive = joystick([127, 0, 0]);
    let negative = joystick([-128, 0, 0]);
    let thumb = gamepad(GamepadAxis::LeftThumbY, -12000);
    let mut session = MappingOutputSession::new(SimulatedDriver::default());
    let execution = session
        .analog_batch(
            &[
                AnalogEvent {
                    input_id: "A",
                    output: &positive,
                    raw_actuation: u16::MAX,
                },
                AnalogEvent {
                    input_id: "B",
                    output: &negative,
                    raw_actuation: u16::MAX,
                },
                AnalogEvent {
                    input_id: "C",
                    output: &thumb,
                    raw_actuation: u16::MAX,
                },
            ],
            Duration::ZERO,
        )
        .unwrap();
    assert_eq!(session.local_output().joystick.axes, [0; 3]);
    assert_eq!(execution.reports.len(), 1);
    assert_eq!(execution.reports[0].report.bytes()[4], 12);
    assert!(execution.driver_confirmed());
    let release = session.remove_analog_input("B", Duration::ZERO).unwrap();
    assert_eq!(release.reports.len(), 1);
    assert_eq!(release.reports[0].report.bytes()[8], 255);
    assert!(release.driver_confirmed());
}

#[test]
fn pending_completion_blocks_later_mutations_and_never_claims_success() {
    let curve = joystick([127, 0, 0]);
    let driver = SimulatedDriver {
        pending_polls: 1,
        ..Default::default()
    };
    let mut session = MappingOutputSession::new(driver);
    let queued = session
        .analog_batch(
            &[AnalogEvent {
                input_id: "A",
                output: &curve,
                raw_actuation: u16::MAX,
            }],
            Duration::ZERO,
        )
        .unwrap();
    assert!(queued.is_pending());
    assert!(!queued.driver_confirmed());
    assert_eq!(
        session.clear(Duration::ZERO).unwrap_err(),
        DriverOutputError::ExecutionPending
    );
    assert_eq!(session.local_output().joystick.axes, [127, 0, 0]);
    let done = session.poll(Duration::ZERO).unwrap();
    assert!(!done.is_pending());
    assert!(done.driver_confirmed());
}

#[test]
fn no_change_is_local_noop_and_does_not_invent_a_driver_ack() {
    let mut session = MappingOutputSession::new(SimulatedDriver::default());
    let no_change = session.analog_batch(&[], Duration::ZERO).unwrap();
    assert!(no_change.reports.is_empty());
    assert!(!no_change.driver_confirmed());
    assert!(session.transport.log.is_empty());
}

#[test]
fn batch_retains_each_failure_and_continues_submitting_later_groups() {
    let mut output = MappingOutputState::default();
    output.joystick.axes = [127, 0, 0];
    output.joystick.rotation_axes = [0, 127, 0];
    output.gamepad.right_trigger = 255;
    let driver = SimulatedDriver {
        fail_submit: Some(1),
        fail_completion: Some(2),
        ..Default::default()
    };
    let mut session = MappingOutputSession::new(driver);
    let execution = session.output_state(output, Duration::ZERO).unwrap();
    assert_eq!(execution.reports.len(), 3);
    assert_eq!(
        execution.reports[0].delivery,
        ReportDelivery::Failed(os_error(DriverOperation::Submit, 6))
    );
    assert_eq!(
        execution.reports[1].delivery,
        ReportDelivery::Failed(os_error(DriverOperation::Complete, 31))
    );
    assert_eq!(execution.reports[2].delivery, ReportDelivery::Completed);
    assert!(!execution.driver_confirmed());
    assert_eq!(session.transport.submit_count, 3);
    // Native handlers do not silently retry failed writes on an unchanged state.
    assert!(
        session
            .output_state(output, Duration::ZERO)
            .unwrap()
            .reports
            .is_empty()
    );
}

#[test]
fn poll_error_keeps_request_alive_until_a_final_completion_exists() {
    let mut output = MappingOutputState::default();
    output.joystick.axes = [1, 0, 0];
    let driver = SimulatedDriver {
        poll_error: true,
        ..Default::default()
    };
    let mut session = MappingOutputSession::new(driver);
    let first = session.output_state(output, Duration::ZERO).unwrap();
    assert!(first.is_pending());
    assert_eq!(
        first.reports[0].observation_errors,
        [os_error(DriverOperation::Complete, 5)]
    );
    assert_eq!(session.transport.pending.len(), 1);
    session.transport.poll_error = false;
    assert!(session.poll(Duration::ZERO).unwrap().driver_confirmed());
    assert!(session.transport.pending.is_empty());
}

#[test]
fn cancellation_drains_final_aborted_result_and_cannot_report_success() {
    let mut output = MappingOutputState::default();
    output.gamepad.left_trigger = 100;
    let driver = SimulatedDriver {
        pending_polls: 2,
        ..Default::default()
    };
    let mut session = MappingOutputSession::new(driver);
    session.output_state(output, Duration::ZERO).unwrap();
    let cancelling = session.cancel_pending(Duration::ZERO).unwrap();
    assert!(cancelling.is_pending());
    assert_eq!(session.transport.pending.len(), 1);
    let done = session.poll(Duration::ZERO).unwrap();
    assert_eq!(
        done.reports[0].delivery,
        ReportDelivery::Failed(os_error(DriverOperation::Complete, 995))
    );
    assert!(!done.driver_confirmed());
    assert!(session.transport.pending.is_empty());
}

#[test]
fn clear_flushes_neutral_joystick_and_gamepad_and_removes_contributions() {
    let xyz = joystick([127, 0, 0]);
    let trigger = gamepad(GamepadAxis::LeftTrigger, 255);
    let mut session = MappingOutputSession::new(SimulatedDriver::default());
    session
        .analog_batch(
            &[
                AnalogEvent {
                    input_id: "A",
                    output: &xyz,
                    raw_actuation: u16::MAX,
                },
                AnalogEvent {
                    input_id: "B",
                    output: &trigger,
                    raw_actuation: u16::MAX,
                },
            ],
            Duration::ZERO,
        )
        .unwrap();
    let cleared = session.clear(Duration::ZERO).unwrap();
    assert!(cleared.driver_confirmed());
    assert_eq!(cleared.reports.len(), 2);
    assert_eq!(&cleared.reports[0].report.bytes()[8..11], &[128; 3]);
    assert_eq!(&cleared.reports[1].report.bytes()[8..], &[0; 24]);
    assert_eq!(session.local_output(), MappingOutputState::default());
    assert!(
        session
            .remove_analog_input("A", Duration::ZERO)
            .unwrap()
            .reports
            .is_empty()
    );
}

#[test]
fn disconnect_drains_current_then_release_and_only_then_closes() {
    let mut output = MappingOutputState::default();
    output.joystick.axes = [127, 0, 0];
    let driver = SimulatedDriver {
        pending_polls: 2,
        ..Default::default()
    };
    let mut session = MappingOutputSession::new(driver);
    session.output_state(output, Duration::ZERO).unwrap();
    assert_eq!(
        session.disconnect(Duration::ZERO).stage,
        DisconnectStage::DrainingCurrentOutput
    );
    assert_eq!(
        session.disconnect(Duration::ZERO).stage,
        DisconnectStage::ReleasingOutput
    );
    assert!(!session.transport.log.contains(&Action::Close));
    assert_eq!(
        session.clear(Duration::ZERO).unwrap_err(),
        DriverOutputError::Disconnecting
    );
    assert_eq!(
        session.disconnect(Duration::ZERO).stage,
        DisconnectStage::ReleasingOutput
    );
    let closed = session.disconnect(Duration::ZERO);
    assert_eq!(closed.stage, DisconnectStage::Closed);
    assert!(closed.prior_output.unwrap().driver_confirmed());
    assert!(closed.release_output.unwrap().driver_confirmed());
    assert_eq!(session.transport.log.last(), Some(&Action::Close));
    assert_eq!(session.transport.submit_count, 2);
    assert_eq!(
        session.clear(Duration::ZERO).unwrap_err(),
        DriverOutputError::Closed
    );
    assert_eq!(
        session.disconnect(Duration::ZERO).stage,
        DisconnectStage::Closed
    );
    assert_eq!(
        session
            .transport
            .log
            .iter()
            .filter(|a| **a == Action::Close)
            .count(),
        1
    );
}

#[test]
fn disconnect_preserves_release_failure_and_retries_only_failed_close() {
    let mut output = MappingOutputState::default();
    output.gamepad.left_thumb = [1000, 0];
    let driver = SimulatedDriver {
        fail_completion: Some(2),
        close_error: true,
        ..Default::default()
    };
    let mut session = MappingOutputSession::new(driver);
    session.output_state(output, Duration::ZERO).unwrap();
    let waiting = session.disconnect(Duration::ZERO);
    assert_eq!(waiting.stage, DisconnectStage::ClosingDriver);
    assert_eq!(
        waiting.close_error,
        Some(os_error(DriverOperation::Close, 6))
    );
    assert!(!waiting.release_output.unwrap().driver_confirmed());
    session.transport.close_error = false;
    let closed = session.disconnect(Duration::ZERO);
    assert_eq!(closed.stage, DisconnectStage::Closed);
    assert!(!closed.release_output.unwrap().driver_confirmed());
    assert_eq!(session.transport.submit_count, 2);
}

#[test]
fn v1_analog_events_reach_the_same_driver_state_without_v2_rescaling() {
    let curve = NativeAnalogOutput::Joystick(
        JoystickCurve::compile(
            AnalogResolution::V1,
            JoystickBank::RotationAxes,
            &[
                JoystickPoint {
                    actuation: 0,
                    axes: [0; 3],
                },
                JoystickPoint {
                    actuation: 255,
                    axes: [0, 127, -128],
                },
            ],
        )
        .unwrap(),
    );
    let mut session = MappingOutputSession::new(SimulatedDriver::default());
    let execution = session
        .analog_batch(
            &[AnalogEvent {
                input_id: "A",
                output: &curve,
                raw_actuation: 255,
            }],
            Duration::ZERO,
        )
        .unwrap();
    assert_eq!(execution.reports.len(), 1);
    assert_eq!(execution.reports[0].report.bytes()[4], 6);
    assert_eq!(&execution.reports[0].report.bytes()[8..11], &[128, 255, 0]);
    assert!(execution.driver_confirmed());
}

#[test]
fn disconnect_replays_native_raw_zero_instead_of_forcing_axes_to_zero() {
    let axes = NativeAnalogOutput::Joystick(
        JoystickCurve::compile(
            AnalogResolution::V2,
            JoystickBank::Axes,
            &[
                JoystickPoint {
                    actuation: 0,
                    axes: [64, 0, 0],
                },
                JoystickPoint {
                    actuation: 65535,
                    axes: [127, 0, 0],
                },
            ],
        )
        .unwrap(),
    );
    let thumb = NativeAnalogOutput::Gamepad(
        GamepadCurve::compile(
            AnalogResolution::V2,
            GamepadAxis::LeftThumbX,
            &[
                GamepadPoint {
                    actuation: 0,
                    value: 1000,
                },
                GamepadPoint {
                    actuation: 65535,
                    value: 20000,
                },
            ],
        )
        .unwrap(),
    );
    let mut session = MappingOutputSession::new(SimulatedDriver::default());
    session
        .analog_batch(
            &[
                AnalogEvent {
                    input_id: "A",
                    output: &axes,
                    raw_actuation: u16::MAX,
                },
                AnalogEvent {
                    input_id: "B",
                    output: &thumb,
                    raw_actuation: u16::MAX,
                },
            ],
            Duration::ZERO,
        )
        .unwrap();
    let closed = session.disconnect(Duration::ZERO);
    assert_eq!(closed.stage, DisconnectStage::Closed);
    let release = closed.release_output.unwrap();
    assert_eq!(release.reports.len(), 2);
    assert_eq!(&release.reports[0].report.bytes()[8..11], &[192, 128, 128]);
    assert_eq!(
        &release.reports[1].report.bytes()[14..16],
        &1000i16.to_le_bytes()
    );
    assert_eq!(session.local_output().joystick.axes, [64, 0, 0]);
    assert_eq!(session.local_output().gamepad.left_thumb, [1000, 0]);
    assert!(release.driver_confirmed());
}

#[test]
fn explicit_raw_zero_is_not_reinterpreted_as_contribution_removal() {
    let curve = NativeAnalogOutput::Joystick(
        JoystickCurve::compile(
            AnalogResolution::V2,
            JoystickBank::Axes,
            &[
                JoystickPoint {
                    actuation: 0,
                    axes: [64, 0, 0],
                },
                JoystickPoint {
                    actuation: 65535,
                    axes: [127, 0, 0],
                },
            ],
        )
        .unwrap(),
    );
    let mut session = MappingOutputSession::new(SimulatedDriver::default());
    for raw_actuation in [u16::MAX, 0] {
        session
            .analog_batch(
                &[AnalogEvent {
                    input_id: "A",
                    output: &curve,
                    raw_actuation,
                }],
                Duration::ZERO,
            )
            .unwrap();
    }
    assert!(session.active_analog.is_empty());
    assert_eq!(session.local_output().joystick.axes, [64, 0, 0]);
    let closed = session.disconnect(Duration::ZERO);
    assert_eq!(closed.stage, DisconnectStage::Closed);
    let release = closed.release_output.unwrap();
    assert!(release.reports.is_empty());
    assert!(!release.driver_confirmed());
    assert_eq!(session.local_output().joystick.axes, [64, 0, 0]);
}

#[test]
fn unsupported_platform_is_explicit_and_never_becomes_confirmed() {
    let curve = joystick([127, 0, 0]);
    let mut session = MappingOutputSession::new(UnsupportedMappingOutputTransport);
    let execution = session
        .analog_batch(
            &[AnalogEvent {
                input_id: "A",
                output: &curve,
                raw_actuation: u16::MAX,
            }],
            Duration::ZERO,
        )
        .unwrap();
    assert_eq!(
        execution.reports[0].delivery,
        ReportDelivery::Failed(DriverOutputError::UnsupportedPlatform)
    );
    assert!(!execution.driver_confirmed());
    let disconnect = session.disconnect(Duration::ZERO);
    assert_eq!(disconnect.stage, DisconnectStage::ClosingDriver);
    assert_eq!(
        disconnect.close_error,
        Some(DriverOutputError::UnsupportedPlatform)
    );
}
