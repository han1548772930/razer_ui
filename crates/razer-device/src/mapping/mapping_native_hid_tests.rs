use super::*;
use crate::{
    mapping_native_analog::{
        GamepadAxis, GamepadCurve, GamepadPoint, JoystickBank, JoystickCurve, JoystickPoint,
    },
    mapping_native_digital::JoystickGroup,
    mapping_native_events::{NativeAnalogBatchError, NativeEventReportKind},
    mapping_native_session::{
        NativeCapability, NativeCapabilitySet, NativeCompletion, NativeDeviceKind,
        NativeDriverPath, NativeOperation, NativeProductIdentity, NativeRoute, NativeSessionState,
        NativeSubmissionId, NativeTransportError,
    },
};
use std::time::Duration;

// Reconstructed from static IDA byte offsets/strides, never device captures.
const V1_SPARSE_FIXTURE: [u8; 9] = [0x91, 8, 0x20, 2, 0xff, 8, 0x7f, 0, 0xa5];
const V2_SPARSE_FIXTURE: [u8; 12] = [
    0x92, 7, 0x12, 0x34, 251, 0x80, 0x00, 252, 0x7f, 0xff, 0, 0xa5,
];
const V1_DENSE_FIXTURE: [u8; 6] = [0x93, 0, 0x21, 0, 0xff, 0];

#[derive(Default)]
struct SimTransport {
    next: u64,
    operations: BTreeMap<NativeSubmissionId, NativeOperation>,
    completions: BTreeMap<NativeSubmissionId, NativeCompletion>,
    next_write_error: Option<NativeTransportError>,
}

impl NativeSessionTransport for SimTransport {
    fn open(&mut self, _: &NativeDriverPath) -> Result<(), NativeTransportError> {
        Ok(())
    }

    fn submit(
        &mut self,
        operation: NativeOperation,
    ) -> Result<NativeSubmissionId, NativeTransportError> {
        if !matches!(operation, NativeOperation::ReadInput { .. }) {
            if let Some(error) = self.next_write_error.take() {
                return Err(error);
            }
        }
        self.next += 1;
        let id = NativeSubmissionId(self.next);
        self.operations.insert(id, operation);
        Ok(id)
    }

    fn poll(
        &mut self,
        id: NativeSubmissionId,
        _: Duration,
    ) -> Result<NativeCompletion, NativeTransportError> {
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
        self.completions.insert(
            id,
            NativeCompletion::Completed(Err(NativeTransportError::Poll { code: 995 })),
        );
        Ok(())
    }

    fn close(&mut self) -> Result<(), NativeTransportError> {
        assert!(self.operations.is_empty());
        Ok(())
    }
}

fn connected(capabilities: NativeCapabilitySet) -> NativeSession<SimTransport> {
    let product = NativeProductIdentity::razer(1234, NativeDeviceKind::Keyboard);
    let mut session = NativeSession::new(
        NativeRoute::windows(
            NativeDriverPath::new("\\\\?\\simulated#hid#1").unwrap(),
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

fn all_capabilities() -> NativeCapabilitySet {
    NativeCapabilitySet::empty()
        .with(NativeCapability::AnalogOutput)
        .with(NativeCapability::JoystickOutput)
        .with(NativeCapability::GamepadOutput)
}

fn descriptor() -> NativeHidDescriptor {
    NativeHidDescriptor::new(
        false,
        [
            (0x91, NativeHidReportKind::AnalogKeyboard),
            (0x92, NativeHidReportKind::AnalogKeyboardV2),
            (0x93, NativeHidReportKind::AnalogKeypad),
        ],
    )
}

fn packet(bytes: &[u8]) -> NativeHidPacket {
    decode_native_hid_packet(NativeHidFraming::Usb, Some(&descriptor()), bytes).unwrap()
}

fn analog(packet: &NativeHidPacket) -> &NativeHidAnalogReport {
    let NativeHidPayload::Analog(report) = &packet.payload else {
        panic!("expected analog fixture");
    };
    report
}

fn curve(
    resolution: AnalogResolution,
    bank: JoystickBank,
    at_zero: i32,
    at_max: i32,
) -> NativeAnalogOutput {
    NativeAnalogOutput::Joystick(
        JoystickCurve::compile(
            resolution,
            bank,
            &[
                JoystickPoint {
                    actuation: 0,
                    axes: [at_zero, 0, 0],
                },
                JoystickPoint {
                    actuation: if resolution == AnalogResolution::V1 {
                        255
                    } else {
                        65535
                    },
                    axes: [at_max, 0, 0],
                },
            ],
        )
        .unwrap(),
    )
}

fn trigger(resolution: AnalogResolution) -> NativeAnalogOutput {
    NativeAnalogOutput::Gamepad(
        GamepadCurve::compile(
            resolution,
            GamepadAxis::LeftTrigger,
            &[
                GamepadPoint {
                    actuation: 0,
                    value: 0,
                },
                GamepadPoint {
                    actuation: if resolution == AnalogResolution::V1 {
                        255
                    } else {
                        65535
                    },
                    value: 255,
                },
            ],
        )
        .unwrap(),
    )
}

#[derive(Default)]
struct Handler {
    mappings: BTreeMap<u8, Vec<NativeHidAnalogMapping>>,
    events: Vec<NativeHidAnalogEvent>,
    unresolved: Option<u8>,
}

impl Handler {
    fn map(&mut self, key: u8, id: &str, output: NativeAnalogOutput) {
        self.mappings
            .entry(key)
            .or_default()
            .push(NativeHidAnalogMapping {
                input_id: id.into(),
                output,
            });
    }
}

impl NativeHidHandler for Handler {
    fn resolve_analog(&mut self, event: NativeHidAnalogEvent) -> NativeHidKeyResolution {
        self.events.push(event);
        if self.unresolved == Some(event.key_id) {
            return NativeHidKeyResolution::Unresolved("dynamic-key caller missing".into());
        }
        match self.mappings.get(&event.key_id) {
            Some(mappings) => NativeHidKeyResolution::Mapped(mappings.clone()),
            None => NativeHidKeyResolution::Observed,
        }
    }
}

fn enabled() -> NativeHidProcessingContext {
    NativeHidProcessingContext {
        analog_enabled: true,
        ..Default::default()
    }
}

fn deliver(
    hid: &mut NativeHidRuntime,
    output: &mut NativeEventRuntime,
    session: &mut NativeSession<SimTransport>,
    handler: &mut Handler,
    bytes: &[u8],
    context: &NativeHidProcessingContext,
) -> Result<NativeHidFrameDispatch, NativeHidRuntimeError> {
    hid.dispatch(
        output,
        session,
        handler,
        packet(bytes),
        context,
        NativeModifierSnapshot::default(),
    )
}

#[test]
fn descriptor_names_follow_the_nine_native_selectors() {
    for (selector, name) in [
        "analogKeyboardReportID",
        "analogKeyboardV2ReportID",
        "analogKeypadReportID",
        "controlPodReportID",
        "coolingPadReportID",
        "razerEventReportID",
        "razerKeyReportID",
        "streamControllerReportID",
        "hardwareEventReportID",
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(
            NativeHidReportKind::from_native_name(name).selector(),
            selector as u8
        );
    }
    assert_eq!(
        NativeHidReportKind::from_native_name("newUnsupportedReportID"),
        NativeHidReportKind::HardwareEvent
    );
}

#[test]
fn native_descriptor_narrows_uint32_ids_in_lexical_assignment_order() {
    let descriptor = NativeHidDescriptor::from_native_json(&serde_json::json!({
        "bluetoothLE": true,
        "reportIDs": {"+255": "analogKeyboardReportID", "255": "analogKeyboardV2ReportID", "+1": "analogKeyboardReportID", "257": "analogKeypadReportID", "4294967295": "analogKeyboardV2ReportID"}
    })).unwrap();
    assert!(descriptor.bluetooth_le);
    assert_eq!(
        descriptor.report_kind(255),
        Some(NativeHidReportKind::AnalogKeyboardV2)
    );
    assert_eq!(
        descriptor.report_kind(1),
        Some(NativeHidReportKind::AnalogKeypad)
    );
    assert_eq!(descriptor.report_kind(3), None);
}

#[test]
fn descriptor_rejects_invalid_numeric_keys_and_values_before_use() {
    for key in ["", " 1", "1 ", "1x", "4294967296", "-1", "++1"] {
        let value = serde_json::json!({"reportIDs": {key: "analogKeyboardReportID"}});
        assert!(matches!(
            NativeHidDescriptor::from_native_json(&value),
            Err(NativeHidDescriptorError::InvalidReportKey(_))
        ));
    }
    assert!(
        NativeHidDescriptor::from_native_json(&serde_json::json!({"reportIDs": {"1": 0}})).is_err()
    );
    assert!(NativeHidDescriptor::from_native_json(&serde_json::json!({"bluetoothLE": 1})).is_err());
    assert!(NativeHidDescriptor::from_native_json(&serde_json::json!([])).is_err());
}

#[test]
fn sparse_v1_deduplicates_and_orders_keys_but_preserves_raw_tail() {
    let packet = packet(&V1_SPARSE_FIXTURE);
    assert_eq!(packet.raw(), V1_SPARSE_FIXTURE);
    let report = analog(&packet);
    assert_eq!(
        report.levels,
        BTreeMap::from([
            (2, NativeHidLevel::Actuation(255)),
            (8, NativeHidLevel::Actuation(127))
        ])
    );
    assert_eq!(report.terminator_offset, Some(7));
    assert_eq!(report.ignored_tail, [0, 0xa5]);
}

#[test]
fn sparse_v2_uses_big_endian_u16_and_distinct_signed_keys() {
    let packet = packet(&V2_SPARSE_FIXTURE);
    let report = analog(&packet);
    assert_eq!(
        report.levels,
        BTreeMap::from([
            (7, NativeHidLevel::Actuation(0x1234)),
            (251, NativeHidLevel::SignedStick(-32768)),
            (252, NativeHidLevel::SignedStick(32767))
        ])
    );
    // A zero key in an incomplete final triple isn't a native terminator.
    assert_eq!(report.terminator_offset, None);
    assert_eq!(report.ignored_tail, [0, 0xa5]);
}

#[test]
fn dense_v1_uses_byte_index_as_key_and_skips_zero_levels() {
    let packet = packet(&V1_DENSE_FIXTURE);
    assert_eq!(
        analog(&packet).levels,
        BTreeMap::from([
            (2, NativeHidLevel::Actuation(0x21)),
            (4, NativeHidLevel::Actuation(255))
        ])
    );
    assert!(analog(&packet).ignored_tail.is_empty());
}

#[test]
fn usb_bluetooth_flag_and_ble_strip_only_the_outer_byte() {
    let descriptor = NativeHidDescriptor::new(true, [(0x91, NativeHidReportKind::AnalogKeyboard)]);
    let raw = [0xa7, 0x91, 2, 255];
    for framing in [NativeHidFraming::Usb, NativeHidFraming::Ble] {
        let packet = decode_native_hid_packet(framing, Some(&descriptor), &raw).unwrap();
        assert_eq!(packet.report_id, 0x91);
        assert_eq!(packet.payload_offset, 1);
        assert_eq!(packet.raw(), raw);
        assert_eq!(analog(&packet).levels[&2], NativeHidLevel::Actuation(255));
    }
}

#[test]
fn ble_framing_ignores_descriptor_bluetooth_flag() {
    let raw = [0x11, 0x91, 2, 255];
    let packet =
        decode_native_hid_packet(NativeHidFraming::Ble, Some(&descriptor()), &raw).unwrap();
    assert_eq!(packet.payload_offset, 1);
    assert!(packet.descriptor_binding);
}

#[test]
fn unavailable_report_map_still_preserves_the_bluetooth_report_id_offset() {
    let descriptor = NativeHidDescriptor::new(true, [(0x91, NativeHidReportKind::AnalogKeyboard)])
        .with_report_types_available(false);
    let raw = [0xa7, 0x91, 2, 255];
    let packet = decode_native_hid_packet(NativeHidFraming::Usb, Some(&descriptor), &raw).unwrap();
    assert_eq!(packet.report_id, 0x91);
    assert_eq!(packet.kind, NativeHidReportKind::HardwareEvent);
    assert!(!packet.descriptor_binding);
    assert_eq!(packet.raw(), raw);
}

#[test]
fn numeric_report_id_alone_never_implies_analog_support() {
    for descriptor in [None, Some(NativeHidDescriptor::default())] {
        let packet = decode_native_hid_packet(
            NativeHidFraming::Usb,
            descriptor.as_ref(),
            &V1_SPARSE_FIXTURE,
        )
        .unwrap();
        assert_eq!(packet.kind, NativeHidReportKind::HardwareEvent);
        assert!(!packet.descriptor_binding);
        assert_eq!(packet.raw(), V1_SPARSE_FIXTURE);
    }
}

#[test]
fn all_nonanalog_selectors_are_explicit_unsupported_packets() {
    for kind in [
        NativeHidReportKind::ControlPod,
        NativeHidReportKind::CoolingPad,
        NativeHidReportKind::RazerEvent,
        NativeHidReportKind::RazerKey,
        NativeHidReportKind::StreamController,
        NativeHidReportKind::HardwareEvent,
    ] {
        let descriptor = NativeHidDescriptor::new(false, [(0x75, kind)]);
        let packet =
            decode_native_hid_packet(NativeHidFraming::Usb, Some(&descriptor), &[0x75, 1, 2, 3])
                .unwrap();
        assert_eq!(packet.payload, NativeHidPayload::Unsupported { kind });
        assert_eq!(packet.payload_offset, 0);
        assert!(packet.descriptor_binding);
    }
}

#[test]
fn unknown_packets_do_not_touch_held_keys_or_outputs() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x91, 2, 255],
        &enabled(),
    )
    .unwrap();
    let packet = decode_native_hid_packet(NativeHidFraming::Usb, None, &[0x91, 0, 0]).unwrap();
    let result = hid
        .dispatch(
            &mut output,
            &mut session,
            &mut handler,
            packet,
            &enabled(),
            NativeModifierSnapshot::default(),
        )
        .unwrap();
    assert_eq!(hid.tracked_level(2), Some(255));
    assert!(result.analog_events.is_empty());
    assert!(result.output.is_none());
}

#[test]
fn each_framing_and_analog_minimum_is_checked() {
    assert!(decode_native_hid_packet(NativeHidFraming::Usb, None, &[]).is_err());
    assert!(decode_native_hid_packet(NativeHidFraming::Ble, None, &[0]).is_err());
    for (id, minimum) in [(0x91, 3), (0x92, 4), (0x93, 2)] {
        for length in 1..minimum {
            let mut bytes = vec![1; length];
            bytes[0] = id;
            assert!(matches!(
                decode_native_hid_packet(NativeHidFraming::Usb, Some(&descriptor()), &bytes),
                Err(NativeHidDecodeError::Truncated { .. })
            ));
        }
    }
}

#[test]
fn hardware_capacity_is_100_and_not_driver_capacity_304() {
    let bytes = vec![0x71; NATIVE_HARDWARE_REPORT_CAPACITY];
    assert!(decode_native_hid_packet(NativeHidFraming::Usb, None, &bytes).is_ok());
    assert_eq!(
        decode_native_hid_packet(NativeHidFraming::Usb, None, &[0; 101]),
        Err(NativeHidDecodeError::Oversized {
            actual: 101,
            capacity: 100
        })
    );
}

#[test]
fn terminator_stops_before_trailing_nonzero_entries() {
    let packet = packet(&[0x92, 7, 0, 1, 0, 0xaa, 0xbb, 8, 0xff, 0xff]);
    assert_eq!(analog(&packet).levels.len(), 1);
    assert_eq!(analog(&packet).terminator_offset, Some(4));
    assert_eq!(analog(&packet).ignored_tail, [0, 0xaa, 0xbb, 8, 0xff, 0xff]);
}

#[test]
fn incomplete_tuple_tail_is_ignored_without_losing_complete_keys() {
    let packet = packet(&[0x92, 1, 0xab, 0xcd, 2, 0xef]);
    assert_eq!(
        analog(&packet).levels,
        BTreeMap::from([(1, NativeHidLevel::Actuation(0xabcd))])
    );
    assert_eq!(analog(&packet).ignored_tail, [2, 0xef]);
}

#[test]
fn sparse_zero_levels_are_present_and_v1_special_ids_are_unsigned() {
    let packet = packet(&[0x91, 251, 255, 252, 0]);
    assert_eq!(analog(&packet).levels[&251], NativeHidLevel::Actuation(255));
    assert_eq!(analog(&packet).levels[&252], NativeHidLevel::Actuation(0));
}

#[test]
fn frame_updates_are_sorted_and_duplicate_keys_do_not_duplicate_events() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    let result = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &V1_SPARSE_FIXTURE,
        &enabled(),
    )
    .unwrap();
    assert_eq!(
        result
            .analog_events
            .iter()
            .map(|event| event.key_id)
            .collect::<Vec<_>>(),
        [2, 8]
    );
    assert!(
        result
            .analog_events
            .iter()
            .all(|event| event.phase == NativeHidAnalogPhase::Actuation { first: true })
    );
    assert_eq!(hid.tracked_level(8), Some(127));
}

#[test]
fn repeated_identical_levels_do_not_dispatch_and_modifiers_are_sampled_once() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    let modifiers = NativeModifierSnapshot {
        left_control: true,
        right_shift: true,
        ..Default::default()
    };
    let result = hid
        .dispatch(
            &mut output,
            &mut session,
            &mut handler,
            packet(&[0x91, 2, 123]),
            &enabled(),
            modifiers,
        )
        .unwrap();
    assert_eq!(result.analog_events[0].modifiers, 0x240);
    let result = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x91, 2, 123],
        &enabled(),
    )
    .unwrap();
    assert!(result.analog_events.is_empty());
    assert_eq!(handler.events.len(), 1);
}

#[test]
fn missing_sparse_key_releases_but_explicit_zero_stays_present() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x91, 2, 255],
        &enabled(),
    )
    .unwrap();
    let result = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x91, 2, 0],
        &enabled(),
    )
    .unwrap();
    assert_eq!(
        result.analog_events[0].phase,
        NativeHidAnalogPhase::Actuation { first: false }
    );
    assert_eq!(result.analog_events[0].previous_level, 255);
    assert_eq!(hid.tracked_level(2), Some(0));
    let result = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x91, 0, 0],
        &enabled(),
    )
    .unwrap();
    assert_eq!(
        result.analog_events[0].phase,
        NativeHidAnalogPhase::Released
    );
    assert_eq!(hid.tracked_level(2), None);
}

#[test]
fn dense_zero_and_missing_index_both_release_prior_keys() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &V1_DENSE_FIXTURE,
        &enabled(),
    )
    .unwrap();
    let result = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x93, 0, 0],
        &enabled(),
    )
    .unwrap();
    assert_eq!(
        result
            .analog_events
            .iter()
            .map(|event| event.key_id)
            .collect::<Vec<_>>(),
        [2, 4]
    );
    assert!(
        result
            .analog_events
            .iter()
            .all(|event| event.phase == NativeHidAnalogPhase::Released)
    );
}

#[test]
fn disabled_ingress_keeps_present_keys_but_still_runs_missing_key_cleanup() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x91, 2, 200],
        &enabled(),
    )
    .unwrap();
    let disabled = NativeHidProcessingContext::default();
    let present = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x91, 2, 100, 3, 80],
        &disabled,
    )
    .unwrap();
    assert!(present.analog_events.is_empty());
    assert_eq!(hid.tracked_level(2), Some(200));
    assert_eq!(hid.tracked_level(3), None);
    let absent = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x91, 0, 0],
        &disabled,
    )
    .unwrap();
    assert_eq!(
        absent.analog_events[0].phase,
        NativeHidAnalogPhase::Released
    );
}

#[test]
fn blocked_key_updates_and_release_lookup_are_suppressed_explicitly() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x91, 2, 200],
        &enabled(),
    )
    .unwrap();
    let context = NativeHidProcessingContext {
        blocked_keys: BTreeSet::from([2, 3]),
        ..enabled()
    };
    let result = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x91, 3, 200],
        &context,
    )
    .unwrap();
    assert_eq!(result.suppressed_keys, [3, 2]);
    assert!(result.analog_events.is_empty());
    assert_eq!(hid.tracked_level(2), None);
}

#[test]
fn current_actuation_ceiling_applies_before_change_comparison() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    let context = NativeHidProcessingContext {
        actuation_ceiling: Some(128),
        ..enabled()
    };
    let result = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x91, 2, 255],
        &context,
    )
    .unwrap();
    assert_eq!(
        (
            result.analog_events[0].raw_level,
            result.analog_events[0].level
        ),
        (255, 128)
    );
    let result = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x91, 2, 200],
        &context,
    )
    .unwrap();
    assert!(result.analog_events.is_empty());
}

#[test]
fn v2_frame_uses_high_byte_curve_and_shared_output_runtime() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    handler.map(
        2,
        "v2",
        curve(AnalogResolution::V2, JoystickBank::Axes, 0, 127),
    );
    let result = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x92, 2, 0xff, 0x01],
        &enabled(),
    )
    .unwrap();
    assert_eq!(
        result.output.unwrap().delta.desired.joystick.axes,
        [127, 0, 0]
    );
    assert_eq!(
        output
            .desired_state()
            .joystick_contribution_count(JoystickGroup::Axes),
        1
    );
}

#[test]
fn many_keys_in_one_frame_publish_only_final_opposing_extrema() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    handler.map(
        1,
        "positive",
        curve(AnalogResolution::V1, JoystickBank::Axes, 0, 100),
    );
    handler.map(
        2,
        "negative",
        curve(AnalogResolution::V1, JoystickBank::Axes, 0, -50),
    );
    let result = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x91, 2, 255, 1, 255],
        &enabled(),
    )
    .unwrap()
    .output
    .unwrap();
    assert_eq!(result.delta.desired.joystick.axes, [50, 0, 0]);
    assert_eq!(result.reports.len(), 1);
    assert_eq!(&result.reports[0].report.bytes()[8..11], &[178, 128, 128]);
}

#[test]
fn frame_report_order_is_axes_then_rotation_then_gamepad() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    handler.map(
        1,
        "rotation",
        curve(AnalogResolution::V1, JoystickBank::RotationAxes, 0, 80),
    );
    handler.map(2, "gamepad", trigger(AnalogResolution::V1));
    handler.map(
        3,
        "axes",
        curve(AnalogResolution::V1, JoystickBank::Axes, 0, 100),
    );
    let result = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x91, 3, 255, 2, 255, 1, 255],
        &enabled(),
    )
    .unwrap()
    .output
    .unwrap();
    assert_eq!(
        result
            .reports
            .iter()
            .map(|report| report.kind)
            .collect::<Vec<_>>(),
        [
            NativeEventReportKind::Joystick,
            NativeEventReportKind::Joystick,
            NativeEventReportKind::Gamepad
        ]
    );
    let opcodes: Vec<_> = result
        .reports
        .iter()
        .map(|report| u32::from_le_bytes(report.report.bytes()[4..8].try_into().unwrap()))
        .collect();
    assert_eq!(opcodes, [5, 6, 12]);
}

#[test]
fn missing_key_release_evaluates_the_curve_at_zero_instead_of_removing_it() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    handler.map(
        2,
        "nonzero-base",
        curve(AnalogResolution::V1, JoystickBank::Axes, 30, 100),
    );
    deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x91, 2, 255],
        &enabled(),
    )
    .unwrap();
    let result = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x91, 0, 0],
        &enabled(),
    )
    .unwrap()
    .output
    .unwrap();
    assert_eq!(result.delta.desired.joystick.axes, [30, 0, 0]);
    assert_eq!(
        output
            .desired_state()
            .joystick_contribution_count(JoystickGroup::Axes),
        1
    );
}

#[test]
fn unresolved_second_key_rolls_back_whole_frame_state_and_reports() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    handler.map(
        1,
        "first",
        curve(AnalogResolution::V1, JoystickBank::Axes, 0, 100),
    );
    handler.unresolved = Some(2);
    assert!(matches!(
        deliver(
            &mut hid,
            &mut output,
            &mut session,
            &mut handler,
            &[0x91, 1, 255, 2, 255],
            &enabled()
        ),
        Err(NativeHidRuntimeError::Unresolved { key_id: 2, .. })
    ));
    assert_eq!(hid.tracked_level(1), None);
    assert_eq!(
        output
            .desired_state()
            .joystick_contribution_count(JoystickGroup::Axes),
        0
    );
    assert_eq!(output.pending_output_count(), 0);
}

#[test]
fn curve_resolution_mismatch_rejects_frame_before_any_mutation() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    handler.map(
        1,
        "first",
        curve(AnalogResolution::V1, JoystickBank::Axes, 0, 100),
    );
    handler.map(2, "wrong", trigger(AnalogResolution::V2));
    assert!(matches!(
        deliver(
            &mut hid,
            &mut output,
            &mut session,
            &mut handler,
            &[0x91, 1, 255, 2, 255],
            &enabled()
        ),
        Err(NativeHidRuntimeError::ResolutionMismatch { key_id: 2, .. })
    ));
    assert_eq!(hid.tracked_level(1), None);
    assert_eq!(
        output
            .desired_state()
            .joystick_contribution_count(JoystickGroup::Axes),
        0
    );
}

#[test]
fn mixed_output_batch_checks_all_capabilities_before_mutating_first_key() {
    let capabilities = NativeCapabilitySet::from(NativeCapability::AnalogOutput)
        .with(NativeCapability::JoystickOutput);
    let mut session = connected(capabilities);
    let prior_operations = session.transport_mut().operations.clone();
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    handler.map(
        1,
        "first",
        curve(AnalogResolution::V1, JoystickBank::Axes, 0, 100),
    );
    handler.map(2, "gamepad", trigger(AnalogResolution::V1));
    assert_eq!(
        deliver(
            &mut hid,
            &mut output,
            &mut session,
            &mut handler,
            &[0x91, 1, 255, 2, 255],
            &enabled()
        ),
        Err(NativeHidRuntimeError::Output(
            NativeAnalogBatchError::MissingCapabilities(NativeCapability::GamepadOutput.into())
        ))
    );
    assert_eq!(hid.tracked_level(1), None);
    assert_eq!(
        output
            .desired_state()
            .joystick_contribution_count(JoystickGroup::Axes),
        0
    );
    assert_eq!(session.transport_mut().operations, prior_operations);
}

#[test]
fn disconnect_cancellation_rejects_batch_and_keeps_ingress_uncommitted() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    handler.map(
        1,
        "first",
        curve(AnalogResolution::V1, JoystickBank::Axes, 0, 100),
    );
    session.begin_disconnect().unwrap();
    assert_eq!(session.state(), NativeSessionState::Disconnecting);
    assert_eq!(
        deliver(
            &mut hid,
            &mut output,
            &mut session,
            &mut handler,
            &[0x91, 1, 255],
            &enabled()
        ),
        Err(NativeHidRuntimeError::Output(
            NativeAnalogBatchError::SessionState(NativeSessionState::Disconnecting)
        ))
    );
    assert_eq!(hid.tracked_level(1), None);
    assert_eq!(
        output
            .desired_state()
            .joystick_contribution_count(JoystickGroup::Axes),
        0
    );
}

#[test]
fn observation_and_signed_only_frames_are_rejected_after_cancellation() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    session.begin_disconnect().unwrap();
    let context = NativeHidProcessingContext {
        stick_available: true,
        ..enabled()
    };
    assert!(
        deliver(
            &mut hid,
            &mut output,
            &mut session,
            &mut handler,
            &[0x92, 1, 0, 1, 251, 0x80, 0],
            &context
        )
        .is_err()
    );
    assert!(handler.events.is_empty());
    assert_eq!(hid.tracked_level(1), None);
    assert!(!hid.stick_pressed(251));
}

#[test]
fn stopped_shared_runtime_rejects_late_hid_frames() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    output.begin_disconnect(&mut session).unwrap();
    assert_eq!(
        deliver(
            &mut hid,
            &mut output,
            &mut session,
            &mut handler,
            &[0x91, 1, 255],
            &enabled()
        ),
        Err(NativeHidRuntimeError::Output(
            NativeAnalogBatchError::Stopping
        ))
    );
    assert!(handler.events.is_empty());
    assert_eq!(hid.tracked_level(1), None);
}

#[test]
fn rejected_report_is_a_receipt_error_and_valid_desired_state_is_explicit() {
    let mut session = connected(all_capabilities());
    session.transport_mut().next_write_error = Some(NativeTransportError::Submit { code: 31 });
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    handler.map(
        2,
        "mapped",
        curve(AnalogResolution::V1, JoystickBank::Axes, 0, 100),
    );
    let result = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x91, 2, 255],
        &enabled(),
    )
    .unwrap()
    .output
    .unwrap();
    assert!(result.reports[0].submission.is_err());
    assert_eq!(result.delta.desired.joystick.axes, [100, 0, 0]);
    assert_eq!(hid.tracked_level(2), Some(255));
    assert_eq!(output.pending_output_count(), 0);
}

#[test]
fn accepted_hid_output_uses_existing_completion_ownership() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    handler.map(
        2,
        "mapped",
        curve(AnalogResolution::V1, JoystickBank::Axes, 0, 100),
    );
    let result = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x91, 2, 255],
        &enabled(),
    )
    .unwrap()
    .output
    .unwrap();
    let id = result.reports[0].submission.as_ref().unwrap().id();
    assert_eq!(output.pending_output_count(), 1);
    assert_eq!(
        output
            .poll_outputs(&mut session, Duration::ZERO)
            .observations[0]
            .completion,
        Ok(NativeCompletion::Pending)
    );
    session.transport_mut().completions.insert(
        id,
        NativeCompletion::Completed(Err(NativeTransportError::Poll { code: 31 })),
    );
    let result = output.poll_outputs(&mut session, Duration::ZERO);
    assert_eq!(
        result.observations[0].completion,
        Ok(NativeCompletion::Completed(Err(
            NativeTransportError::Poll { code: 31 }
        )))
    );
    assert_eq!(output.pending_output_count(), 0);
}

#[test]
fn signed_threshold_rejects_native_invalid_values() {
    for value in [0, 32767, 32768, 65535] {
        assert!(NativeHidStickThreshold::new(value).is_none());
    }
    assert_eq!(NativeHidStickThreshold::default().value(), 20000);
    assert_eq!(NativeHidStickThreshold::new(1).unwrap().value(), 1);
    assert_eq!(NativeHidStickThreshold::new(32766).unwrap().value(), 32766);
}

#[test]
fn signed_stick_requires_actual_input_availability_and_does_not_enter_curves() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    let result = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x92, 251, 0x80, 0],
        &enabled(),
    )
    .unwrap();
    assert!(result.signed_transitions.is_empty());
    assert!(!hid.stick_pressed(251));
    assert!(handler.events.is_empty());
    assert!(result.output.is_none());
}

#[test]
fn signed_extremes_threshold_equality_and_direction_bytes_match_native() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    let context = NativeHidProcessingContext {
        stick_available: true,
        stick_threshold: NativeHidStickThreshold::new(32766).unwrap(),
        ..enabled()
    };
    let result = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x92, 251, 0x80, 0, 252, 0x7f, 0xfe],
        &context,
    )
    .unwrap();
    assert_eq!(
        result.signed_transitions,
        [
            NativeHidSignedTransition {
                key_id: 251,
                value: -32768,
                direction: 0,
                released: false
            },
            NativeHidSignedTransition {
                key_id: 252,
                value: 32766,
                direction: 2,
                released: false
            }
        ]
    );
    let result = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x92, 251, 0, 1, 252, 0xff, 0xff],
        &context,
    )
    .unwrap();
    assert_eq!(
        result
            .signed_transitions
            .iter()
            .map(|transition| (transition.direction, transition.released))
            .collect::<Vec<_>>(),
        [(1, true), (3, true)]
    );
}

#[test]
fn signed_sign_reversal_above_threshold_does_not_create_another_press() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    let context = NativeHidProcessingContext {
        stick_available: true,
        ..enabled()
    };
    deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x92, 251, 0x7f, 0xff],
        &context,
    )
    .unwrap();
    let result = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x92, 251, 0x80, 0],
        &context,
    )
    .unwrap();
    assert!(result.signed_transitions.is_empty());
    let result = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x92, 251, 0, 0],
        &context,
    )
    .unwrap();
    assert_eq!(result.signed_transitions[0].direction, 0);
    assert!(result.signed_transitions[0].released);
}

#[test]
fn absent_signed_key_does_not_invent_analog_pressed_table_membership() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    let context = NativeHidProcessingContext {
        stick_available: true,
        ..enabled()
    };
    deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x92, 251, 0x7f, 0xff],
        &context,
    )
    .unwrap();
    let result = deliver(
        &mut hid,
        &mut output,
        &mut session,
        &mut handler,
        &[0x92, 0, 0, 0],
        &context,
    )
    .unwrap();
    assert!(result.signed_transitions.is_empty());
    assert!(hid.stick_pressed(251));
    assert_eq!(hid.tracked_level(251), None);
    hid.reset_after_disconnect();
    assert!(!hid.stick_pressed(251));
}

#[test]
fn invalid_frame_does_not_commit_staged_signed_transitions_either() {
    let mut session = connected(all_capabilities());
    let (mut hid, mut output, mut handler) = (
        NativeHidRuntime::default(),
        NativeEventRuntime::default(),
        Handler::default(),
    );
    handler.unresolved = Some(255);
    let context = NativeHidProcessingContext {
        stick_available: true,
        ..enabled()
    };
    assert!(
        deliver(
            &mut hid,
            &mut output,
            &mut session,
            &mut handler,
            &[0x92, 251, 0x80, 0, 255, 0xff, 0xff],
            &context
        )
        .is_err()
    );
    assert!(!hid.stick_pressed(251));
    assert_eq!(hid.tracked_level(255), None);
}
