use super::*;
use crate::{
    mapping_native_analog::{AnalogResolution, JoystickBank, JoystickCurve, JoystickPoint},
    mapping_native_digital::JoystickGroup,
    mapping_native_session::{
        NativeDeviceKind, NativeDriverPath, NativeOperation, NativeProductIdentity, NativeRoute,
    },
};
use std::{cell::RefCell, rc::Rc};

// Reconstructed from IDA offsets, NOT device captures. Opaque prefix/reserved
// fields deliberately contain nonzero bytes to catch unsupported zero checks.
const KEYBOARD_FIXTURE: [u8; 32] = [
    0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x11, 0x22, 2, 0, 0, 0, 0x91, 0x92, 0x93, 0x94, 1, 0, 0, 0,
    0x34, 0x12, 0x1d, 0, 5, 0, 0xef, 0xbe, 0xef, 0xbe, 0xad, 0xde,
];
const MOUSE_FIXTURE: [u8; 44] = [
    0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x11, 0x22, 2, 0, 0, 0, 0x91, 0x92, 0x93, 0x94, 2, 0, 0, 0,
    0x34, 0x12, 2, 0, 1, 0, 0x10, 0xff, 0x68, 0x24, 0x57, 0x13, 17, 0, 0, 0, 0xf7, 0xff, 0xff,
    0xff, 0xdd, 0xcc, 0xbb, 0xaa,
];

type Trace = Rc<RefCell<Vec<&'static str>>>;

#[derive(Default)]
struct SimTransport {
    next_id: u64,
    operations: BTreeMap<NativeSubmissionId, NativeOperation>,
    completions: BTreeMap<NativeSubmissionId, NativeCompletion>,
    read_submit_error: Option<NativeTransportError>,
    output_submit_error: Option<NativeTransportError>,
    poll_error: Option<NativeTransportError>,
    trace: Trace,
    closed: bool,
}

impl SimTransport {
    fn complete(&mut self, id: NativeSubmissionId, result: Result<Vec<u8>, NativeTransportError>) {
        self.completions
            .insert(id, NativeCompletion::Completed(result));
    }
}

impl NativeSessionTransport for SimTransport {
    fn open(&mut self, _: &NativeDriverPath) -> Result<(), NativeTransportError> {
        Ok(())
    }

    fn submit(
        &mut self,
        operation: NativeOperation,
    ) -> Result<NativeSubmissionId, NativeTransportError> {
        let error = if matches!(operation, NativeOperation::ReadInput { .. }) {
            self.trace.borrow_mut().push("read");
            self.read_submit_error.take()
        } else {
            self.trace.borrow_mut().push("write");
            self.output_submit_error.take()
        };
        if let Some(error) = error {
            return Err(error);
        }
        self.next_id += 1;
        let id = NativeSubmissionId(self.next_id);
        self.operations.insert(id, operation);
        Ok(id)
    }

    fn poll(
        &mut self,
        id: NativeSubmissionId,
        _: Duration,
    ) -> Result<NativeCompletion, NativeTransportError> {
        if let Some(error) = self.poll_error.take() {
            return Err(error);
        }
        assert!(self.operations.contains_key(&id));
        let completion = self
            .completions
            .remove(&id)
            .unwrap_or(NativeCompletion::Pending);
        if matches!(completion, NativeCompletion::Completed(_)) {
            self.operations.remove(&id);
        }
        Ok(completion)
    }

    fn cancel(&mut self, id: NativeSubmissionId) -> Result<(), NativeTransportError> {
        self.complete(id, Err(NativeTransportError::Poll { code: 995 }));
        Ok(())
    }

    fn close(&mut self) -> Result<(), NativeTransportError> {
        assert!(
            self.operations.is_empty(),
            "accepted buffers must be drained first"
        );
        self.closed = true;
        Ok(())
    }
}

struct Handler {
    resolution: NativeHookResolution,
    hooks: Vec<NativeInput>,
    observations: Vec<NativeObservedEvent>,
    trace: Trace,
}

impl Handler {
    fn new(resolution: NativeHookResolution) -> Self {
        Self {
            resolution,
            hooks: Vec::new(),
            observations: Vec::new(),
            trace: Trace::default(),
        }
    }
}

impl NativeStreamHandler for Handler {
    fn resolve_hook(&mut self, input: NativeInput) -> NativeHookResolution {
        self.trace.borrow_mut().push("hook");
        self.hooks.push(input);
        self.resolution.clone()
    }

    fn observe(&mut self, event: NativeObservedEvent) {
        self.trace.borrow_mut().push("observe");
        self.observations.push(event);
    }
}

fn all_capabilities() -> NativeCapabilitySet {
    NativeCapabilitySet::empty()
        .with(NativeCapability::KeyboardInput)
        .with(NativeCapability::MouseInput)
        .with(NativeCapability::DisplayInput)
        .with(NativeCapability::JoystickOutput)
        .with(NativeCapability::GamepadOutput)
        .with(NativeCapability::AnalogOutput)
}

fn session_with(capabilities: NativeCapabilitySet) -> NativeSession<SimTransport> {
    let product = NativeProductIdentity::razer(1234, NativeDeviceKind::Keyboard);
    let mut session = NativeSession::new(
        NativeRoute::windows(
            NativeDriverPath::new("\\\\?\\simulated#mapping#1").unwrap(),
            product,
            capabilities,
        ),
        product,
        NativeCapabilitySet::empty(),
        SimTransport::default(),
    )
    .unwrap();
    session.open().unwrap();
    session
}

fn connected() -> NativeSession<SimTransport> {
    session_with(all_capabilities())
}

fn deliver(session: &mut NativeSession<SimTransport>, packet: Vec<u8>) {
    let id = session.input_stream_submission().unwrap();
    session.transport_mut().complete(id, Ok(packet));
}

fn dispatch(
    runtime: &mut NativeEventRuntime,
    session: &mut NativeSession<SimTransport>,
    handler: &mut Handler,
    bytes: &[u8],
) -> NativeEventDispatch {
    runtime.dispatch(
        session,
        handler,
        decode_native_stream_packet(bytes).unwrap(),
        true,
        NativeModifierSnapshot::default(),
    )
}

fn mouse(
    kind: u32,
    flags: u16,
    button_flags: u16,
    data: i16,
    movement: [i32; 2],
    extra: u32,
) -> Vec<u8> {
    let mut packet = MOUSE_FIXTURE.to_vec();
    packet[8..12].copy_from_slice(&kind.to_le_bytes());
    packet[22..24].copy_from_slice(&flags.to_le_bytes());
    packet[24..26].copy_from_slice(&button_flags.to_le_bytes());
    packet[26..28].copy_from_slice(&data.to_le_bytes());
    packet[32..36].copy_from_slice(&movement[0].to_le_bytes());
    packet[36..40].copy_from_slice(&movement[1].to_le_bytes());
    packet[40..44].copy_from_slice(&extra.to_le_bytes());
    packet
}

fn receipt_id(receipt: &NativeEventReportReceipt) -> NativeSubmissionId {
    receipt.submission.as_ref().unwrap().id()
}

#[test]
fn keyboard_union_offsets_and_opaque_bytes_are_preserved() {
    let packet = decode_native_stream_packet(&KEYBOARD_FIXTURE).unwrap();
    assert_eq!(packet.kind, NativeStreamKind::Hook);
    assert_eq!(
        packet.input,
        NativeStreamInput::Keyboard(NativeKeyboardEvent {
            unit_id: 0x1234,
            scancode: 29,
            flags: 5,
            reserved: 0xbeef,
            extra_information: 0xdeadbeef,
        })
    );
    assert_eq!(packet.raw(), KEYBOARD_FIXTURE);
}

#[test]
fn mouse_union_uses_signed_data_movement_and_original_extra_info() {
    let packet = decode_native_stream_packet(&MOUSE_FIXTURE).unwrap();
    assert_eq!(packet.kind, NativeStreamKind::Hook);
    assert_eq!(
        packet.input,
        NativeStreamInput::Mouse(NativeMouseEvent {
            unit_id: 0x1234,
            flags: 2,
            button_flags: 1,
            button_data: -240,
            raw_buttons: 0x13572468,
            movement: [17, -9],
            extra_information: 0xaabbccdd,
        })
    );
}

#[test]
fn every_truncation_of_known_input_branches_is_an_error() {
    for bytes in [KEYBOARD_FIXTURE.as_slice(), MOUSE_FIXTURE.as_slice()] {
        for length in 0..bytes.len() {
            assert!(
                matches!(
                    decode_native_stream_packet(&bytes[..length]),
                    Err(NativeStreamDecodeError::Truncated { .. })
                ),
                "length {length}"
            );
        }
    }
}

#[test]
fn capacity_is_not_an_exact_return_length_or_a_zero_padding_contract() {
    let mut bytes = KEYBOARD_FIXTURE.to_vec();
    bytes.resize(304, 0x9b);
    assert_eq!(decode_native_stream_packet(&bytes).unwrap().raw(), bytes);
    bytes.push(0);
    assert_eq!(
        decode_native_stream_packet(&bytes),
        Err(NativeStreamDecodeError::Oversized {
            actual: 305,
            capacity: 304
        })
    );
}

#[test]
fn unknown_kinds_need_only_kind_and_are_not_fabricated_analog_batches() {
    for kind in [0u32, 3, 5, u32::MAX] {
        let mut bytes = vec![0xc3; 12];
        bytes[8..12].copy_from_slice(&kind.to_le_bytes());
        let packet = decode_native_stream_packet(&bytes).unwrap();
        assert_eq!(packet.kind, NativeStreamKind::Unknown(kind));
        assert_eq!(
            packet.input,
            NativeStreamInput::Unknown { input_type: None }
        );
        bytes.resize(20, 0);
        bytes[16..20].copy_from_slice(&77u32.to_le_bytes());
        assert_eq!(
            decode_native_stream_packet(&bytes).unwrap().input,
            NativeStreamInput::Unknown {
                input_type: Some(77)
            }
        );
    }
}

#[test]
fn known_kind_with_unknown_type_preserves_the_header_without_union_assumptions() {
    let mut bytes = KEYBOARD_FIXTURE[..20].to_vec();
    bytes[16..20].copy_from_slice(&99u32.to_le_bytes());
    assert_eq!(
        decode_native_stream_packet(&bytes).unwrap().input,
        NativeStreamInput::Unknown {
            input_type: Some(99)
        }
    );
    bytes[8..12].copy_from_slice(&4u32.to_le_bytes());
    bytes[16..20].copy_from_slice(&4u32.to_le_bytes());
    assert_eq!(
        decode_native_stream_packet(&bytes).unwrap().input,
        NativeStreamInput::Unknown {
            input_type: Some(4)
        }
    );
}

#[test]
fn redirect_state_requires_offsets_41_and_42_and_preserves_non_boolean_bytes() {
    let mut bytes = vec![0x7a; 43];
    bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
    bytes[16..20].copy_from_slice(&4u32.to_le_bytes());
    bytes[41] = 0xa5;
    bytes[42] = 0x6b;
    assert_eq!(
        decode_native_stream_packet(&bytes).unwrap().input,
        NativeStreamInput::RedirectState {
            first: 0xa5,
            second: 0x6b
        }
    );
    assert!(matches!(
        decode_native_stream_packet(&bytes[..42]),
        Err(NativeStreamDecodeError::Truncated { required: 43, .. })
    ));
}

#[test]
fn modifier_snapshot_matches_eight_independent_get_key_state_bits() {
    let snapshots = [
        (
            NativeModifierSnapshot {
                left_alt: true,
                ..Default::default()
            },
            0x100,
        ),
        (
            NativeModifierSnapshot {
                left_control: true,
                ..Default::default()
            },
            0x200,
        ),
        (
            NativeModifierSnapshot {
                left_shift: true,
                ..Default::default()
            },
            0x400,
        ),
        (
            NativeModifierSnapshot {
                left_windows: true,
                ..Default::default()
            },
            0x800,
        ),
        (
            NativeModifierSnapshot {
                right_alt: true,
                ..Default::default()
            },
            0x10,
        ),
        (
            NativeModifierSnapshot {
                right_control: true,
                ..Default::default()
            },
            0x20,
        ),
        (
            NativeModifierSnapshot {
                right_shift: true,
                ..Default::default()
            },
            0x40,
        ),
        (
            NativeModifierSnapshot {
                right_windows: true,
                ..Default::default()
            },
            0x80,
        ),
    ];
    for (snapshot, expected) in snapshots {
        assert_eq!(snapshot.source_bits(), expected);
    }
}

#[test]
fn keyboard_normalization_does_not_copy_reserved_or_extra_info() {
    let input = decode_native_stream_packet(&KEYBOARD_FIXTURE)
        .unwrap()
        .input;
    let modifiers = NativeModifierSnapshot {
        left_alt: true,
        right_control: true,
        ..Default::default()
    };
    for preserve in [false, true] {
        assert_eq!(
            input.normalized(preserve, modifiers),
            Some(NativeInput::Keyboard(KeyboardInput {
                unit_id: if preserve { 0x1234 } else { 0 },
                scancode: 29,
                flag: 5,
                modifiers: 0x120,
                reserved: 0,
            }))
        );
    }
}

#[test]
fn mouse_normalization_clamps_signed_word_and_separates_physical_motion() {
    for value in [i16::MIN, -121, -120, -119, 0, 119, 120, 121, i16::MAX] {
        let input = decode_native_stream_packet(&mouse(1, 2, 1024, value, [123, -123], 0xdeadbeef))
            .unwrap()
            .input;
        let NativeInput::Mouse(normalized) = input
            .normalized(false, NativeModifierSnapshot::default())
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(normalized.signed_wheel_delta(), value.clamp(-120, 120));
        assert_eq!(normalized.unit_id, 0);
        assert_eq!(normalized.flags, 0);
        assert_eq!(normalized.raw_buttons, 0);
        assert_eq!(normalized.movement, [0, 0]);
    }
}

#[test]
fn rearm_precedes_hook_resolution_and_output_and_acceptance_is_not_completion() {
    let mut session = connected();
    let mut runtime = NativeEventRuntime::default();
    let mut handler = Handler::new(NativeHookResolution::PassThrough);
    let trace = session.transport_mut().trace.clone();
    trace.borrow_mut().clear();
    handler.trace = trace.clone();
    deliver(&mut session, KEYBOARD_FIXTURE.to_vec());
    let NativeEventPoll::Completed {
        rearmed,
        dispatch: Ok(dispatch),
    } = runtime
        .poll(
            &mut session,
            &mut handler,
            Duration::ZERO,
            true,
            Default::default(),
        )
        .unwrap()
    else {
        panic!()
    };
    assert!(rearmed.is_ok());
    assert_eq!(*trace.borrow(), ["read", "hook", "write", "write"]);
    assert_eq!(dispatch.reports.len(), 2); // E1 keyboard continuation.
    assert_eq!(runtime.pending_output_count(), 2);
    let output = runtime.poll_outputs(&mut session, Duration::ZERO);
    assert!(
        output
            .observations
            .iter()
            .all(|item| item.completion == Ok(NativeCompletion::Pending))
    );
    let id = receipt_id(&dispatch.reports[0]);
    session
        .transport_mut()
        .complete(id, Err(NativeTransportError::Poll { code: 31 }));
    let output = runtime.poll_outputs(&mut session, Duration::ZERO);
    assert_eq!(
        output.observations[0].completion,
        Ok(NativeCompletion::Completed(Err(
            NativeTransportError::Poll { code: 31 }
        )))
    );
    assert_eq!(runtime.pending_output_count(), 1);
}

#[test]
fn notify_and_redirect_are_observations_without_hook_resolution_or_echo() {
    let mut session = connected();
    let mut runtime = NativeEventRuntime::default();
    let mut handler = Handler::new(NativeHookResolution::PassThrough);
    for kind in [4u32, 1] {
        let mut bytes = KEYBOARD_FIXTURE;
        bytes[8..12].copy_from_slice(&kind.to_le_bytes());
        let result = dispatch(&mut runtime, &mut session, &mut handler, &bytes);
        assert!(result.reports.is_empty());
    }
    assert!(handler.hooks.is_empty());
    assert!(matches!(
        handler.observations[0],
        NativeObservedEvent::Notify(_)
    ));
    assert!(matches!(
        handler.observations[1],
        NativeObservedEvent::Redirect(_)
    ));
}

#[test]
fn state_dispatch_does_not_resolve_a_keyboard_mapping() {
    let mut bytes = vec![0; 43];
    bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
    bytes[16..20].copy_from_slice(&4u32.to_le_bytes());
    bytes[41] = 9;
    bytes[42] = 27;
    let mut handler = Handler::new(NativeHookResolution::PassThrough);
    let result = dispatch(
        &mut NativeEventRuntime::default(),
        &mut connected(),
        &mut handler,
        &bytes,
    );
    assert_eq!(result.disposition, NativeEventDisposition::RedirectState);
    assert_eq!(
        handler.observations,
        [NativeObservedEvent::RedirectState {
            first: 9,
            second: 27
        }]
    );
    assert!(handler.hooks.is_empty());
    assert!(result.reports.is_empty());
}

#[test]
fn mouse_redirect_suppression_checks_the_full_button_dword() {
    let mut session = connected();
    let mut runtime = NativeEventRuntime::default();
    let mut handler = Handler::new(NativeHookResolution::Suppress);
    let empty = dispatch(
        &mut runtime,
        &mut session,
        &mut handler,
        &mouse(1, 0, 0, 0, [12, 5], 0),
    );
    assert_eq!(
        empty.disposition,
        NativeEventDisposition::EmptyMouseRedirect
    );
    assert!(handler.hooks.is_empty());
    let data_only = dispatch(
        &mut runtime,
        &mut session,
        &mut handler,
        &mouse(1, 0, 0, 1, [12, 5], 0),
    );
    assert_eq!(data_only.disposition, NativeEventDisposition::Redirected);
    assert!(handler.hooks.is_empty());
    assert_eq!(handler.observations.len(), 1);
}

#[test]
fn hook_movement_condition_uses_button_flags_word_and_relative_motion() {
    let mut session = connected();
    let mut runtime = NativeEventRuntime::default();
    let mut handler = Handler::new(NativeHookResolution::Suppress);
    let data_only = dispatch(
        &mut runtime,
        &mut session,
        &mut handler,
        &mouse(2, 0, 0, 1, [1, 0], 0),
    );
    assert!(data_only.reports.is_empty());
    let absolute = dispatch(
        &mut runtime,
        &mut session,
        &mut handler,
        &mouse(2, 1, 1, 0, [1, 0], 0),
    );
    assert_eq!(absolute.issues, [NativeEventIssue::AbsoluteMouseMovement]);
    assert!(absolute.reports.is_empty());
    assert_eq!(absolute.disposition, NativeEventDisposition::Suppressed);
    assert_eq!(handler.hooks.len(), 2);
}

#[test]
fn movement_packet_preserves_physical_extra_info_and_sends_before_hook() {
    let mut session = connected();
    let trace = session.transport_mut().trace.clone();
    trace.borrow_mut().clear();
    let mut handler = Handler::new(NativeHookResolution::Suppress);
    handler.trace = trace.clone();
    let result = dispatch(
        &mut NativeEventRuntime::default(),
        &mut session,
        &mut handler,
        &MOUSE_FIXTURE,
    );
    assert_eq!(*trace.borrow(), ["write", "hook"]);
    assert_eq!(result.reports.len(), 1);
    let bytes = result.reports[0].report.bytes();
    assert_eq!(&bytes[4..8], &2u32.to_le_bytes());
    assert_eq!(&bytes[8..10], &[0, 0]);
    assert_eq!(&bytes[10..12], &2u16.to_le_bytes());
    assert_eq!(&bytes[12..20], &[0; 8]);
    assert_eq!(&bytes[20..24], &17i32.to_le_bytes());
    assert_eq!(&bytes[24..28], &(-9i32).to_le_bytes());
    assert_eq!(&bytes[28..32], &0xaabbccddu32.to_le_bytes());
}

#[test]
fn pending_movement_coalesces_wrapping_axes_and_last_metadata_until_completion() {
    let mut session = connected();
    let mut runtime = NativeEventRuntime::default();
    let mut handler = Handler::new(NativeHookResolution::Suppress);
    let first = dispatch(
        &mut runtime,
        &mut session,
        &mut handler,
        &mouse(2, 0, 1, 0, [3, 4], 1),
    );
    let id = receipt_id(&first.reports[0]);
    for (movement, flags, extra) in [([i32::MAX, i32::MIN], 2, 2), ([1, -1], 4, 3)] {
        let result = dispatch(
            &mut runtime,
            &mut session,
            &mut handler,
            &mouse(2, flags, 1, 0, movement, extra),
        );
        assert!(result.reports.is_empty());
    }
    assert!(
        runtime
            .poll_outputs(&mut session, Duration::ZERO)
            .followup_reports
            .is_empty()
    );
    session.transport_mut().complete(id, Ok(Vec::new()));
    let followup = runtime.poll_outputs(&mut session, Duration::ZERO);
    assert_eq!(followup.followup_reports.len(), 1);
    let bytes = followup.followup_reports[0].report.bytes();
    assert_eq!(&bytes[20..24], &i32::MIN.to_le_bytes());
    assert_eq!(&bytes[24..28], &i32::MAX.to_le_bytes());
    assert_eq!(&bytes[10..12], &4u16.to_le_bytes());
    assert_eq!(&bytes[28..32], &3u32.to_le_bytes());
    assert_eq!(runtime.pending_output_count(), 1);
}

#[test]
fn failed_movement_completion_still_flushes_dirty_queue_with_failure_retained() {
    let mut session = connected();
    let mut runtime = NativeEventRuntime::default();
    let mut handler = Handler::new(NativeHookResolution::Suppress);
    let first = dispatch(&mut runtime, &mut session, &mut handler, &MOUSE_FIXTURE);
    dispatch(&mut runtime, &mut session, &mut handler, &MOUSE_FIXTURE);
    session.transport_mut().complete(
        receipt_id(&first.reports[0]),
        Err(NativeTransportError::Poll { code: 31 }),
    );
    let result = runtime.poll_outputs(&mut session, Duration::ZERO);
    assert_eq!(
        result.observations[0].completion,
        Ok(NativeCompletion::Completed(Err(
            NativeTransportError::Poll { code: 31 }
        )))
    );
    assert_eq!(result.followup_reports.len(), 1);
}

#[test]
fn movement_submit_failure_does_not_replay_discarded_accumulator() {
    let mut session = connected();
    session.transport_mut().output_submit_error = Some(NativeTransportError::Submit { code: 31 });
    let mut runtime = NativeEventRuntime::default();
    let mut handler = Handler::new(NativeHookResolution::Suppress);
    let first = dispatch(&mut runtime, &mut session, &mut handler, &MOUSE_FIXTURE);
    assert!(first.reports[0].submission.is_err());
    assert_eq!(runtime.pending_output_count(), 0);
    let second = dispatch(
        &mut runtime,
        &mut session,
        &mut handler,
        &mouse(2, 0, 1, 0, [1, 2], 0),
    );
    assert_eq!(
        &second.reports[0].report.bytes()[20..24],
        &1i32.to_le_bytes()
    );
}

#[test]
fn output_poll_observation_error_keeps_report_and_movement_inflight() {
    let mut session = connected();
    let mut runtime = NativeEventRuntime::default();
    let mut handler = Handler::new(NativeHookResolution::Suppress);
    dispatch(&mut runtime, &mut session, &mut handler, &MOUSE_FIXTURE);
    session.transport_mut().poll_error = Some(NativeTransportError::Poll { code: 21 });
    let result = runtime.poll_outputs(&mut session, Duration::ZERO);
    assert!(result.observations[0].completion.is_err());
    assert_eq!(runtime.pending_output_count(), 1);
    let next = dispatch(&mut runtime, &mut session, &mut handler, &MOUSE_FIXTURE);
    assert!(next.reports.is_empty());
}

#[test]
fn malformed_read_is_reported_after_rearm_without_handler_calls() {
    let mut session = connected();
    let mut runtime = NativeEventRuntime::default();
    let mut handler = Handler::new(NativeHookResolution::PassThrough);
    deliver(&mut session, KEYBOARD_FIXTURE[..31].to_vec());
    let NativeEventPoll::Completed { rearmed, dispatch } = runtime
        .poll(
            &mut session,
            &mut handler,
            Duration::ZERO,
            true,
            Default::default(),
        )
        .unwrap()
    else {
        panic!()
    };
    assert!(rearmed.is_ok());
    assert!(dispatch.is_err());
    assert!(handler.hooks.is_empty());
    assert_eq!(session.state(), NativeSessionState::Connected);
}

#[test]
fn pending_and_terminal_read_failure_do_not_fabricate_events() {
    let mut session = connected();
    let mut runtime = NativeEventRuntime::default();
    let mut handler = Handler::new(NativeHookResolution::PassThrough);
    assert_eq!(
        runtime
            .poll(
                &mut session,
                &mut handler,
                Duration::ZERO,
                true,
                Default::default()
            )
            .unwrap(),
        NativeEventPoll::Pending
    );
    let id = session.input_stream_submission().unwrap();
    session
        .transport_mut()
        .complete(id, Err(NativeTransportError::Poll { code: 995 }));
    assert_eq!(
        runtime
            .poll(
                &mut session,
                &mut handler,
                Duration::ZERO,
                true,
                Default::default()
            )
            .unwrap(),
        NativeEventPoll::ReadFailed(NativeTransportError::Poll { code: 995 })
    );
    assert!(handler.hooks.is_empty());
    assert_eq!(runtime.pending_output_count(), 0);
    assert_eq!(session.state(), NativeSessionState::Failed);
}

#[test]
fn rearm_failure_preserves_completed_event_and_rejects_its_output() {
    let mut session = connected();
    let mut runtime = NativeEventRuntime::default();
    let mut handler = Handler::new(NativeHookResolution::PassThrough);
    deliver(&mut session, KEYBOARD_FIXTURE.to_vec());
    session.transport_mut().read_submit_error = Some(NativeTransportError::Submit { code: 31 });
    let NativeEventPoll::Completed {
        rearmed,
        dispatch: Ok(result),
    } = runtime
        .poll(
            &mut session,
            &mut handler,
            Duration::ZERO,
            true,
            Default::default(),
        )
        .unwrap()
    else {
        panic!()
    };
    assert!(rearmed.is_err());
    assert_eq!(handler.hooks.len(), 1);
    assert_eq!(result.packet.raw(), KEYBOARD_FIXTURE);
    assert_eq!(result.reports.len(), 1); // Primary rejected, no E1 continuation.
    assert!(matches!(
        result.reports[0].submission,
        Err(NativeEventSubmitError::Session(
            NativeSessionError::InvalidState {
                state: NativeSessionState::Failed,
                ..
            }
        ))
    ));
}

#[test]
fn cancellation_drains_read_and_outputs_without_dispatch_rearm_or_movement_flush() {
    let mut session = connected();
    let mut runtime = NativeEventRuntime::default();
    let mut handler = Handler::new(NativeHookResolution::Suppress);
    dispatch(&mut runtime, &mut session, &mut handler, &MOUSE_FIXTURE);
    dispatch(&mut runtime, &mut session, &mut handler, &MOUSE_FIXTURE);
    let count = session.transport_mut().next_id;
    runtime.begin_disconnect(&mut session).unwrap();
    let output = runtime.poll_outputs(&mut session, Duration::ZERO);
    assert_eq!(
        output.observations[0].completion,
        Ok(NativeCompletion::Completed(Err(
            NativeTransportError::Poll { code: 995 }
        )))
    );
    assert!(output.followup_reports.is_empty());
    assert_eq!(runtime.pending_output_count(), 0);
    assert_eq!(
        session.poll_disconnect(Duration::ZERO).unwrap(),
        NativeCompletion::Completed(Ok(Vec::new()))
    );
    assert_eq!(session.transport_mut().next_id, count);
    assert!(session.transport_mut().closed);
}

#[test]
fn typed_digital_hook_actions_submit_actual_reports_and_pair_button_counts() {
    let mut session = connected();
    let mut runtime = NativeEventRuntime::default();
    let mut handler = Handler::new(NativeHookResolution::Suppress);
    for flag in [0u8, 1] {
        handler.resolution = NativeHookResolution::Mapped(vec![NativeEventOutput::Digital {
            input_id: "key:a".into(),
            output: NativeDigitalOutput::JoystickButton { button: 2, flag },
        }]);
        let result = dispatch(&mut runtime, &mut session, &mut handler, &KEYBOARD_FIXTURE);
        assert_eq!(result.disposition, NativeEventDisposition::Mapped);
        assert_eq!(result.reports.len(), 1);
        assert_eq!(result.reports[0].kind, NativeEventReportKind::Joystick);
        assert_eq!(&result.reports[0].report.bytes()[4..8], &7u32.to_le_bytes());
        assert_eq!(&result.reports[0].report.bytes()[8..10], &[flag, 2]);
        assert_eq!(
            runtime.desired_state().joystick_button_count(2),
            Some(u8::from(flag == 0))
        );
    }
}

#[test]
fn unsupported_digital_capability_does_not_mutate_desired_mapping_state() {
    let mut session = session_with(NativeCapability::KeyboardInput.into());
    let mut runtime = NativeEventRuntime::default();
    let mut handler = Handler::new(NativeHookResolution::Mapped(vec![
        NativeEventOutput::Digital {
            input_id: "a".into(),
            output: NativeDigitalOutput::JoystickButton { button: 1, flag: 0 },
        },
    ]));
    let result = dispatch(&mut runtime, &mut session, &mut handler, &KEYBOARD_FIXTURE);
    assert_eq!(
        result.issues,
        [NativeEventIssue::MissingCapabilities(
            NativeCapability::JoystickOutput.into()
        )]
    );
    assert!(result.reports.is_empty());
    assert_eq!(runtime.desired_state().joystick_button_count(1), Some(0));
}

#[test]
fn analog_and_digital_use_shared_contributions_but_distinct_event_ingress() {
    let mut session = connected();
    let mut runtime = NativeEventRuntime::default();
    let mut handler = Handler::new(NativeHookResolution::Mapped(vec![
        NativeEventOutput::Digital {
            input_id: "digital".into(),
            output: NativeDigitalOutput::JoystickAxes {
                group: JoystickGroup::Axes,
                value: [100, 0, 0],
            },
        },
    ]));
    dispatch(&mut runtime, &mut session, &mut handler, &KEYBOARD_FIXTURE);
    let curve = NativeAnalogOutput::Joystick(
        JoystickCurve::compile(
            AnalogResolution::V1,
            JoystickBank::Axes,
            &[
                JoystickPoint {
                    actuation: 0,
                    axes: [0; 3],
                },
                JoystickPoint {
                    actuation: 255,
                    axes: [-50, 0, 0],
                },
            ],
        )
        .unwrap(),
    );
    let event = runtime
        .apply_analog(&mut session, "analog", &curve, 255)
        .unwrap();
    assert_eq!(event.delta.desired.joystick.axes, [50, 0, 0]);
    assert_eq!(
        runtime
            .desired_state()
            .joystick_contribution_count(JoystickGroup::Axes),
        2
    );
    assert_eq!(event.reports.len(), 1);
    assert_eq!(&event.reports[0].report.bytes()[4..8], &5u32.to_le_bytes());
    assert_eq!(&event.reports[0].report.bytes()[8..11], &[178, 128, 128]);
    let release = runtime
        .apply_analog(&mut session, "analog", &curve, 0)
        .unwrap();
    assert_eq!(release.delta.desired.joystick.axes, [100, 0, 0]);
}

#[test]
fn analog_ingress_requires_both_verified_analog_and_target_capabilities() {
    let mut session = session_with(NativeCapability::JoystickOutput.into());
    let mut runtime = NativeEventRuntime::default();
    let curve = NativeAnalogOutput::Joystick(
        JoystickCurve::compile(
            AnalogResolution::V1,
            JoystickBank::Axes,
            &[JoystickPoint {
                actuation: 255,
                axes: [127, 0, 0],
            }],
        )
        .unwrap(),
    );
    assert_eq!(
        runtime.apply_analog(&mut session, "a", &curve, 255),
        Err(NativeEventIssue::MissingCapabilities(
            NativeCapability::AnalogOutput.into()
        ))
    );
    assert_eq!(
        runtime
            .desired_state()
            .joystick_contribution_count(JoystickGroup::Axes),
        0
    );
}

#[test]
fn unresolved_mapping_is_explicit_and_does_not_fall_through_to_success() {
    let mut session = connected();
    let mut runtime = NativeEventRuntime::default();
    let mut handler = Handler::new(NativeHookResolution::Unresolved(
        "macro timer branch unavailable".into(),
    ));
    let result = dispatch(&mut runtime, &mut session, &mut handler, &KEYBOARD_FIXTURE);
    assert_eq!(
        result.disposition,
        NativeEventDisposition::Unresolved("macro timer branch unavailable".into())
    );
    assert!(result.reports.is_empty());
    assert_eq!(runtime.pending_output_count(), 0);
}

#[test]
fn primary_e1_rejection_does_not_queue_continuation_or_claim_completion() {
    let mut session = connected();
    session.transport_mut().output_submit_error = Some(NativeTransportError::Submit { code: 5 });
    let mut runtime = NativeEventRuntime::default();
    let mut handler = Handler::new(NativeHookResolution::PassThrough);
    let result = dispatch(&mut runtime, &mut session, &mut handler, &KEYBOARD_FIXTURE);
    assert_eq!(result.reports.len(), 1);
    assert!(result.reports[0].submission.is_err());
    assert_eq!(runtime.pending_output_count(), 0);
}

#[test]
fn unknown_input_and_unknown_kind_do_not_call_hooks_or_write() {
    let mut session = connected();
    let mut runtime = NativeEventRuntime::default();
    let mut handler = Handler::new(NativeHookResolution::PassThrough);
    let mut bytes = KEYBOARD_FIXTURE;
    bytes[16..20].copy_from_slice(&99u32.to_le_bytes());
    let result = dispatch(&mut runtime, &mut session, &mut handler, &bytes);
    assert_eq!(result.disposition, NativeEventDisposition::UnknownInput);
    bytes[8..12].copy_from_slice(&3u32.to_le_bytes());
    let result = dispatch(&mut runtime, &mut session, &mut handler, &bytes);
    assert_eq!(result.disposition, NativeEventDisposition::UnknownEvent);
    assert!(handler.hooks.is_empty());
    assert!(handler.observations.is_empty());
    assert_eq!(runtime.pending_output_count(), 0);
}
