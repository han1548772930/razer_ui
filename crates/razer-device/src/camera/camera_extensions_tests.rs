use super::*;
use anyhow::{Result, bail};
use std::{cell::Cell, collections::VecDeque};

#[derive(Debug, Eq, PartialEq)]
enum Event {
    Begin(u32),
    Set([u8; 32], [u8; 8]),
    Wait(u32),
    Get([u8; 32]),
    End,
}

#[derive(Default)]
struct Mock {
    events: Vec<Event>,
    replies: VecDeque<(CameraKsCallOutcome, [u8; 8])>,
    set_hresult: i32,
    fail_begin: bool,
    fail_transport: bool,
    fail_get: bool,
    fail_wait: bool,
}

impl Mock {
    fn reply(response: [u8; 8]) -> Self {
        let mut mock = Self::default();
        mock.replies.push_back((
            CameraKsCallOutcome {
                hresult: 0,
                bytes_returned: 8,
            },
            response,
        ));
        mock
    }
}

impl CameraExtensionTransport for Mock {
    fn begin_node(&mut self, node_id: u32) -> Result<()> {
        self.events.push(Event::Begin(node_id));
        if self.fail_begin {
            bail!("simulated QI failure");
        }
        Ok(())
    }
    fn ks_property(
        &mut self,
        header: &[u8; 32],
        data: &mut [u8; 8],
    ) -> Result<CameraKsCallOutcome> {
        let flags = u32::from_le_bytes(header[20..24].try_into().unwrap());
        if flags == CAMERA_EXTENSION_SET_FLAGS {
            self.events.push(Event::Set(*header, *data));
            if self.fail_transport {
                bail!("simulated transport failure");
            }
            Ok(CameraKsCallOutcome {
                hresult: self.set_hresult,
                bytes_returned: 0,
            })
        } else {
            assert_eq!(flags, CAMERA_EXTENSION_GET_FLAGS);
            assert_eq!(data[0], 0xff, "GET starts with failure status");
            self.events.push(Event::Get(*header));
            if self.fail_get {
                bail!("simulated GET transport failure");
            }
            let (outcome, response) = self.replies.pop_front().unwrap_or((
                CameraKsCallOutcome {
                    hresult: 0,
                    bytes_returned: 8,
                },
                [0; 8],
            ));
            *data = response;
            Ok(outcome)
        }
    }
    fn end_node(&mut self) {
        self.events.push(Event::End);
    }
    fn wait_ms(&mut self, milliseconds: u32) -> Result<()> {
        self.events.push(Event::Wait(milliseconds));
        if self.fail_wait {
            bail!("simulated wait failure");
        }
        Ok(())
    }
}

fn run(mock: &mut Mock, request: CameraExtensionRequest) -> CameraExtensionProgress {
    execute_camera_extension(mock, KIYO_V2_PRO_PRODUCT_ID, request, &|| Ok(()))
}

fn assert_command(request: CameraExtensionRequest, prefixes: &[&[u8]], mutation: bool) {
    let compiled = request.commands();
    assert_eq!(compiled.len(), prefixes.len(), "{}", request.action());
    for (command, prefix) in compiled.iter().zip(prefixes) {
        let mut expected = [0; 8];
        expected[..prefix.len()].copy_from_slice(prefix);
        assert_eq!(command.bytes, expected, "{}", request.action());
        assert_eq!(command.mutates_state, mutation, "{}", request.action());
        assert!(command.native_rva != 0);
    }
}

#[test]
fn current_factory_and_supplemental_native_exports_compile_audited_packets() {
    use CameraExtensionRequest::*;
    // These constants are from IDA packet assignments, independently of the
    // implementation's enum matching; every current factory action is covered.
    let cases: Vec<(CameraExtensionRequest, Vec<&[u8]>, bool)> = vec![
        (SetHdr { enabled: true }, vec![&[0xa0, 1, 1]], true),
        (GetUvcExtCmdStatus, vec![&[0xc0, 3, 0x11]], false),
        (SetLdc { enabled: true }, vec![&[0xff, 1, 1, 3, 0]], true),
        (SetAutoFocusMode { mode: 5 }, vec![&[0xff, 6, 5]], true),
        (
            SetHdrBrightness { brightness: 6 },
            vec![&[0xa0, 0x15, 1, 6]],
            true,
        ),
        (SetHdrMode { mode: 2 }, vec![&[0xa0, 0x15, 1, 2]], true),
        (
            SetMicNoiseSuppression { enabled: true },
            vec![&[0xff, 4, 1]],
            true,
        ),
        (SaveUvcExtCmdToFlash, vec![&[0xc0, 3, 0xa8]], true),
        (GetWatermark, vec![&[0xc0, 8]], false),
        (
            SetWatermark {
                state: 1,
                position: 3,
            },
            vec![&[0xc0, 8, 1, 1, 3]],
            true,
        ),
        (GetIsoSensitivity, vec![&[0xa0, 0x13]], false),
        (
            SetIsoSensitivity { code: 7 },
            vec![&[0xa0, 0x13, 1, 7]],
            true,
        ),
        (GetFdafMode, vec![&[0xc0, 0x0a]], false),
        (
            SetFdafMode {
                mode: 2,
                light_condition: 9,
            },
            vec![&[0xc0, 0x0a, 1, 2, 0, 0, 0, 9]],
            true,
        ),
        (
            SetAutoFramingSetting {
                command: 7,
                position: 8,
            },
            vec![&[0xff, 0x14, 7, 8]],
            true,
        ),
        (
            GetDynamicNoiseReduction { channel: 99 },
            vec![&[0xa0, 6, 2]],
            false,
        ),
        (
            SetDynamicNoiseReduction {
                channel: 99,
                enabled: true,
            },
            vec![&[0xa0, 6, 1]],
            true,
        ),
        (GetMirrorVideo, vec![&[0xa0, 7, 2]], false),
        (SetMirrorVideo { mode: 1 }, vec![&[0xa0, 7, 1]], true),
        (GetAutoExposureMetering, vec![&[0xa0, 8, 0x0f]], false),
        (
            SetAutoExposureMetering { mode: 2 },
            vec![&[0xa0, 8, 2]],
            true,
        ),
        (GetShutterSpeed, vec![&[0xa0, 9]], false),
        (
            SetShutterSpeed { value: 0x01020304 },
            vec![&[0xa0, 9, 1, 1, 2, 3, 4]],
            true,
        ),
        (GetShutterLimitControl, vec![&[0xa0, 0x1c]], false),
        (
            SetShutterLimitControl { mode: 1 },
            vec![&[0xa0, 0x1c, 1, 1]],
            true,
        ),
        (GetEvCompensation, vec![&[0xa0, 0x14]], false),
        (
            SetEvCompensation { value: 30 },
            vec![&[0xa0, 0x14, 1, 9]],
            true,
        ),
        (
            SetAutoFocusLowSensitivity { value: 2 },
            vec![&[0xa0, 0x18, 2]],
            true,
        ),
        (RestoreDefaults, vec![&[0xa0, 0x0a, 2]], true),
        (RebootDevice, vec![&[0xc0, 0x0e, 7, 2]], true),
        (RestoreDefaultAndReboot, vec![&[0xc0, 0x0e, 7, 1]], true),
        (
            GetAdaptiveMjpegQuality { query: 2 },
            vec![&[0xc0, 0x0e, 0x0a, 2]],
            false,
        ),
        (
            SetAdaptiveMjpegQuality {
                enabled: true,
                quality: 85,
            },
            vec![&[0xc0, 0x0e, 9, 1, 0, 0, 85]],
            true,
        ),
        (GetFixedMjpegQuality, vec![&[0xa0, 0x0b]], false),
        (
            SetFixedMjpegQuality { quality: 85 },
            vec![&[0xa0, 0x0b, 1, 85]],
            true,
        ),
        (GetStreamingStatus, vec![&[0xc0, 9, 0x0a]], false),
        (SetAutoframing { mode: 1 }, vec![&[0xa0, 0x0c, 1]], true),
        (
            SetAudioFunction {
                adaptive_noise_reduction: 1,
                beamforming: 2,
            },
            vec![
                &[0xc0, 0x0e, 0xa0, 1, 2],
                &[0xc0, 3, 0xa8],
                &[0xc0, 0x0e, 7, 2],
            ],
            true,
        ),
        (GetAudioFunction, vec![&[0xc0, 0x0e, 0xa1]], false),
        (
            SetAutoframingSpeedAndSize { speed: 1, size: 2 },
            vec![&[0xa0, 0x17, 1], &[0xa0, 0x16, 2]],
            true,
        ),
        (
            GetAutoframingSpeedAndSize,
            vec![&[0xa0, 0x17, 3], &[0xa0, 0x16, 3]],
            false,
        ),
    ];
    assert_eq!(cases.len(), 41);
    let mut actions = std::collections::HashSet::new();
    for (request, prefixes, mutation) in cases {
        assert!(actions.insert(request.action()));
        assert_command(request, &prefixes, mutation);
        let mut mock = Mock::default();
        let result = run(&mut mock, request);
        assert!(
            result.completed,
            "{}: {:?}",
            request.action(),
            result.failure
        );
        assert_eq!(result.exchanges.len(), prefixes.len());
        assert!(
            result
                .exchanges
                .iter()
                .all(|exchange| exchange.acknowledged)
        );
    }
}

#[test]
fn actual_ks_headers_lengths_and_round_trip_lifetime() {
    let request = CameraExtensionRequest::GetUvcExtCmdStatus;
    let mut mock = Mock::reply([0, 90, 1, 1, 2, 3, 4, 99]);
    let result = run(&mut mock, request);
    let mut set = [0; 32];
    set[..16].copy_from_slice(b"RAZER_USBCAM_EXT");
    set[16..20].copy_from_slice(&1u32.to_le_bytes());
    set[20..24].copy_from_slice(&0x1000_0002u32.to_le_bytes());
    set[24..28].copy_from_slice(&3u32.to_le_bytes());
    let mut get = set;
    get[20..24].copy_from_slice(&0x1000_0001u32.to_le_bytes());
    assert_eq!(
        mock.events,
        vec![
            Event::Begin(3),
            Event::Set(set, [0xc0, 3, 0x11, 0, 0, 0, 0, 0]),
            Event::Wait(10),
            Event::Get(get),
            Event::End,
        ]
    );
    assert_eq!(
        result.value,
        Some(CameraExtensionValue::UvcStatus(CameraUvcStatus {
            ldc: 1,
            hdr: 1,
            hdr_mode: 2,
            autofocus_mode: 3,
            adaptive_noise_reduction: 4,
        }))
    );
    assert_eq!(result.exchanges[0].get_outcome.unwrap().bytes_returned, 8);
    assert_eq!(
        result.exchanges[0].response,
        Some([0, 90, 1, 1, 2, 3, 4, 99])
    );
}

#[test]
fn supplemental_pro_exports_keep_native_class_dispatch_and_low_byte_arguments() {
    for (mode, byte) in [(-1, 0xff), (258, 2), (i32::MIN, 0), (i32::MAX, 0xff)] {
        let mode = CameraExtensionRequest::SetHdrMode { mode };
        assert_command(mode, &[&[0xa0, 0x15, 1, byte]], true);
        assert_eq!(mode.commands()[0].native_rva, 0x52290);
    }
    for (request, value) in [
        (CameraExtensionRequest::RebootDevice, 2),
        (CameraExtensionRequest::RestoreDefaultAndReboot, 1),
    ] {
        let mut mock = Mock::default();
        let progress = run(&mut mock, request);
        assert!(progress.completed);
        assert_eq!(progress.value, Some(CameraExtensionValue::Acknowledged));
        assert_eq!(progress.exchanges.len(), 1);
        assert_eq!(progress.exchanges[0].command.native_rva, 0x4e700);
        assert_eq!(
            mock.events,
            vec![
                Event::Begin(3),
                Event::Set(
                    camera_extension_header(true),
                    [0xc0, 0x0e, 7, value, 0, 0, 0, 0]
                ),
                Event::Wait(10),
                Event::Get(camera_extension_header(false)),
                Event::End,
            ]
        );
    }
}

#[test]
fn reboot_reply_loss_retains_attempted_mutation_and_releases_control_without_ack() {
    let mut mock = Mock {
        fail_get: true,
        ..Default::default()
    };
    let progress = run(&mut mock, CameraExtensionRequest::RebootDevice);
    assert!(!progress.completed);
    assert!(progress.value.is_none());
    assert_eq!(
        progress.failure.as_ref().unwrap().phase,
        CameraExtensionPhase::Get
    );
    assert!(progress.exchanges[0].set_attempted && progress.exchanges[0].get_attempted);
    assert!(progress.exchanges[0].command.mutates_state);
    assert!(!progress.exchanges[0].acknowledged);
    assert_eq!(mock.events.last(), Some(&Event::End));
    // Detachment is not proof that a reboot or factory restore succeeded.
    assert_eq!(
        mock.events
            .iter()
            .filter(|e| matches!(e, Event::Set(..)))
            .count(),
        1
    );
}

#[test]
fn signed_shutter_long_uses_all_four_big_endian_bytes() {
    for value in [-1, i32::MIN, i32::MAX, -10] {
        let request = CameraExtensionRequest::SetShutterSpeed { value };
        let command = request.commands()[0].bytes;
        assert_eq!(&command[3..7], &value.to_be_bytes());
        assert_eq!(command[7], 0);
        let mut response = [0; 8];
        response[3..7].copy_from_slice(&value.to_be_bytes());
        let observed = run(
            &mut Mock::reply(response),
            CameraExtensionRequest::GetShutterSpeed,
        );
        assert_eq!(
            observed.value,
            Some(CameraExtensionValue::ShutterSpeed(value))
        );
    }
}

#[test]
fn native_integer_narrowing_and_ignored_channel_are_preserved() {
    assert_command(
        CameraExtensionRequest::SetIsoSensitivity { code: 0x103 },
        &[&[0xa0, 0x13, 1, 3]],
        true,
    );
    assert_command(
        CameraExtensionRequest::SetAutoFocusMode { mode: -1 },
        &[&[0xff, 6, 0xff]],
        true,
    );
    assert_eq!(
        CameraExtensionRequest::GetDynamicNoiseReduction { channel: -1 }.commands(),
        CameraExtensionRequest::GetDynamicNoiseReduction { channel: 500 }.commands(),
    );
    assert_eq!(
        CameraExtensionRequest::SetDynamicNoiseReduction {
            channel: -1,
            enabled: false
        }
        .commands(),
        CameraExtensionRequest::SetDynamicNoiseReduction {
            channel: 500,
            enabled: false
        }
        .commands(),
    );
}

#[test]
fn ev_table_round_trips_nearest_values_ties_and_narrowing() {
    for (code, value) in EV_COMPENSATION_VALUES.iter().enumerate() {
        let command = CameraExtensionRequest::SetEvCompensation {
            value: (*value).into(),
        }
        .commands()[0];
        assert_eq!(command.bytes[3], code as u8);
        let mut response = [0; 8];
        response[3] = code as u8;
        assert_eq!(
            run(
                &mut Mock::reply(response),
                CameraExtensionRequest::GetEvCompensation
            )
            .value,
            Some(CameraExtensionValue::EvCompensation {
                raw_code: code as u8,
                value: Some(*value)
            })
        );
    }
    // 55 is equidistant from 57 (code1) and 53 (code2); native strict < keeps1.
    assert_eq!(
        CameraExtensionRequest::SetEvCompensation { value: 55 }.commands()[0].bytes[3],
        1
    );
    assert_eq!(
        CameraExtensionRequest::SetEvCompensation { value: 55 + 256 }.commands()[0].bytes[3],
        1
    );
    assert_eq!(
        CameraExtensionRequest::SetEvCompensation { value: -1 }.commands()[0].bytes[3],
        0
    );
}

#[test]
fn unknown_iso_ev_and_boolean_selectors_remain_observed() {
    for code in [0, 8, 0xff] {
        let mut response = [0; 8];
        response[3] = code;
        assert_eq!(
            run(
                &mut Mock::reply(response),
                CameraExtensionRequest::GetIsoSensitivity
            )
            .value,
            Some(CameraExtensionValue::IsoSensitivity {
                raw_code: code,
                iso: None
            })
        );
    }
    for code in [19, 0xff] {
        let mut response = [0; 8];
        response[3] = code;
        assert_eq!(
            run(
                &mut Mock::reply(response),
                CameraExtensionRequest::GetEvCompensation
            )
            .value,
            Some(CameraExtensionValue::EvCompensation {
                raw_code: code,
                value: None
            })
        );
    }
    for code in [0, 1, 2, 0xff] {
        let mut response = [0; 8];
        response[3] = code;
        assert_eq!(
            run(
                &mut Mock::reply(response),
                CameraExtensionRequest::GetStreamingStatus
            )
            .value,
            Some(CameraExtensionValue::StreamingStatus {
                raw_code: code,
                is_streaming: code == 1
            })
        );
        let status = CameraUvcStatus {
            ldc: 0,
            hdr: code,
            hdr_mode: 0,
            autofocus_mode: 0,
            adaptive_noise_reduction: 0,
        };
        assert_eq!(status.hdr_enabled(), code == 1);
    }
}

#[test]
fn iso_known_codes_decode_without_reusing_set_parameter_meaning() {
    for (code, iso) in [100, 200, 400, 800, 1600, 3200, 6400]
        .into_iter()
        .enumerate()
    {
        let raw_code = code as u8 + 1;
        let mut response = [0; 8];
        response[3] = raw_code;
        assert_eq!(
            run(
                &mut Mock::reply(response),
                CameraExtensionRequest::GetIsoSensitivity
            )
            .value,
            Some(CameraExtensionValue::IsoSensitivity {
                raw_code,
                iso: Some(iso)
            })
        );
    }
}

#[test]
fn paired_and_primitive_reads_use_native_response_offsets() {
    let response = [0, 101, 2, 3, 4, 5, 6, 7];
    let cases = [
        (
            CameraExtensionRequest::GetWatermark,
            CameraExtensionValue::Watermark {
                state: 2,
                position: 3,
            },
        ),
        (
            CameraExtensionRequest::GetFdafMode,
            CameraExtensionValue::FdafMode {
                mode: 2,
                light_condition: 7,
            },
        ),
        (
            CameraExtensionRequest::GetAudioFunction,
            CameraExtensionValue::AudioFunction {
                adaptive_noise_reduction: 3,
                beamforming: 4,
            },
        ),
        (
            CameraExtensionRequest::GetMirrorVideo,
            CameraExtensionValue::Byte(3),
        ),
        (
            CameraExtensionRequest::GetAutoExposureMetering,
            CameraExtensionValue::Byte(3),
        ),
        (
            CameraExtensionRequest::GetDynamicNoiseReduction { channel: 1 },
            CameraExtensionValue::Byte(3),
        ),
        (
            CameraExtensionRequest::GetShutterLimitControl,
            CameraExtensionValue::Byte(3),
        ),
        (
            CameraExtensionRequest::GetAdaptiveMjpegQuality { query: 1 },
            CameraExtensionValue::Byte(3),
        ),
        (
            CameraExtensionRequest::GetFixedMjpegQuality,
            CameraExtensionValue::Byte(3),
        ),
    ];
    for (request, value) in cases {
        assert_eq!(run(&mut Mock::reply(response), request).value, Some(value));
    }
}

#[test]
fn getter_speed_then_size_requires_two_independent_acknowledged_commands() {
    let mut mock = Mock::reply([0, 0, 0, 9, 0, 0, 0, 0]);
    mock.replies.push_back((
        CameraKsCallOutcome {
            hresult: 0,
            bytes_returned: 8,
        },
        [0, 0, 0, 7, 0, 0, 0, 0],
    ));
    let result = run(
        &mut mock,
        CameraExtensionRequest::GetAutoframingSpeedAndSize,
    );
    assert_eq!(
        result.value,
        Some(CameraExtensionValue::AutoframingSpeedAndSize { speed: 9, size: 7 })
    );
    assert_eq!(
        mock.events
            .iter()
            .filter(|event| matches!(event, Event::Begin(_)))
            .count(),
        2
    );
    assert_eq!(
        mock.events
            .iter()
            .filter(|event| matches!(event, Event::End))
            .count(),
        2
    );
}

#[test]
fn setter_ack_is_not_an_independent_state_observation() {
    let result = run(
        &mut Mock::reply([0, 0, 0, 9, 0, 0, 0, 0]),
        CameraExtensionRequest::SetHdr { enabled: true },
    );
    assert_eq!(result.value, Some(CameraExtensionValue::Acknowledged));
    assert_eq!(result.exchanges[0].response.unwrap()[3], 9);
}

#[test]
fn failed_set_hresult_stops_before_stale_get_and_releases_node() {
    let mut mock = Mock {
        set_hresult: 0x8000_4005u32 as i32,
        ..Default::default()
    };
    let result = run(&mut mock, CameraExtensionRequest::SetHdr { enabled: true });
    assert_eq!(result.failure.unwrap().phase, CameraExtensionPhase::Set);
    assert!(!result.completed);
    assert!(result.exchanges[0].set_attempted);
    assert!(!result.exchanges[0].get_attempted);
    assert!(!result.exchanges[0].acknowledged);
    assert_eq!(mock.events.last(), Some(&Event::End));
    assert!(
        !mock
            .events
            .iter()
            .any(|event| matches!(event, Event::Get(_)))
    );
}

#[test]
fn get_requires_exact_s_ok_full_response_and_zero_device_status() {
    for (hresult, bytes_returned, status, phase) in [
        (1, 8, 0, CameraExtensionPhase::Get),
        (0x8000_4005u32 as i32, 8, 0, CameraExtensionPhase::Get),
        (0, 0, 0, CameraExtensionPhase::Get),
        (0, 7, 0, CameraExtensionPhase::Get),
        (0, 9, 0, CameraExtensionPhase::Get),
        (0, 8, 1, CameraExtensionPhase::Decode),
        (0, 8, 2, CameraExtensionPhase::Decode),
        (0, 8, 255, CameraExtensionPhase::Decode),
    ] {
        let mut response = [0; 8];
        response[0] = status;
        let mut mock = Mock::default();
        mock.replies.push_back((
            CameraKsCallOutcome {
                hresult,
                bytes_returned,
            },
            response,
        ));
        let result = run(&mut mock, CameraExtensionRequest::GetStreamingStatus);
        assert_eq!(result.failure.unwrap().phase, phase);
        assert_eq!(result.exchanges[0].response, Some(response));
        assert!(!result.exchanges[0].acknowledged);
        assert!(result.value.is_none());
        assert_eq!(mock.events.last(), Some(&Event::End));
    }
}

#[test]
fn open_transport_and_wait_failures_release_partial_control() {
    for (mut mock, phase) in [
        (
            Mock {
                fail_begin: true,
                ..Default::default()
            },
            CameraExtensionPhase::OpenControl,
        ),
        (
            Mock {
                fail_transport: true,
                ..Default::default()
            },
            CameraExtensionPhase::Set,
        ),
        (
            Mock {
                fail_wait: true,
                ..Default::default()
            },
            CameraExtensionPhase::Wait,
        ),
    ] {
        let result = run(&mut mock, CameraExtensionRequest::SaveUvcExtCmdToFlash);
        assert_eq!(result.failure.unwrap().phase, phase);
        assert_eq!(mock.events.last(), Some(&Event::End));
        assert!(result.value.is_none());
    }
}

#[test]
fn unsupported_products_and_pre_cancel_do_not_open_or_write() {
    for product in [3595, 3596, 0] {
        let mut mock = Mock::default();
        let result = execute_camera_extension(
            &mut mock,
            product,
            CameraExtensionRequest::GetStreamingStatus,
            &|| Ok(()),
        );
        assert_eq!(result.failure.unwrap().phase, CameraExtensionPhase::Target);
        assert!(mock.events.is_empty());
    }
    let mut mock = Mock::default();
    let result = execute_camera_extension(
        &mut mock,
        3594,
        CameraExtensionRequest::SetHdr { enabled: true },
        &|| bail!("cancelled"),
    );
    assert_eq!(
        result.failure.unwrap().phase,
        CameraExtensionPhase::Validation
    );
    assert!(mock.events.is_empty());
}

#[test]
fn cancellation_after_set_preserves_unconfirmed_attempt_and_releases() {
    let validations = Cell::new(0);
    let mut mock = Mock::default();
    let result = execute_camera_extension(
        &mut mock,
        3594,
        CameraExtensionRequest::SetHdr { enabled: true },
        &|| {
            validations.set(validations.get() + 1);
            if validations.get() == 3 {
                bail!("camera detached");
            }
            Ok(())
        },
    );
    assert_eq!(
        result.failure.unwrap().phase,
        CameraExtensionPhase::Validation
    );
    assert!(result.exchanges[0].set_attempted);
    assert!(result.exchanges[0].set_outcome.is_some());
    assert!(!result.exchanges[0].get_attempted);
    assert_eq!(mock.events.last(), Some(&Event::End));
}

#[test]
fn audio_set_persist_and_reboot_keep_native_delays_and_lifetimes() {
    let mut mock = Mock::default();
    let result = run(
        &mut mock,
        CameraExtensionRequest::SetAudioFunction {
            adaptive_noise_reduction: 1,
            beamforming: 0,
        },
    );
    assert!(result.completed);
    assert_eq!(result.exchanges.len(), 3);
    let commands: Vec<_> = mock
        .events
        .iter()
        .filter_map(|event| {
            if let Event::Set(_, data) = event {
                Some(*data)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        commands,
        vec![
            [0xc0, 0x0e, 0xa0, 1, 0, 0, 0, 0],
            [0xc0, 3, 0xa8, 0, 0, 0, 0, 0],
            [0xc0, 0x0e, 7, 2, 0, 0, 0, 0],
        ]
    );
    assert_eq!(
        mock.events
            .iter()
            .filter(|event| **event == Event::Wait(10))
            .count(),
        5
    );
    assert_eq!(
        mock.events
            .iter()
            .filter(|event| **event == Event::End)
            .count(),
        3
    );
    assert!(matches!(mock.events[4], Event::End));
    assert!(matches!(mock.events[5], Event::Wait(10)));
    assert!(matches!(mock.events[6], Event::Begin(3)));
}

#[test]
fn audio_failure_on_flash_or_reboot_retains_prior_acks_without_rollback() {
    for failing in [1, 2] {
        let mut mock = Mock::default();
        for index in 0..=failing {
            let mut response = [0; 8];
            if index == failing {
                response[0] = 2;
            }
            mock.replies.push_back((
                CameraKsCallOutcome {
                    hresult: 0,
                    bytes_returned: 8,
                },
                response,
            ));
        }
        let result = run(
            &mut mock,
            CameraExtensionRequest::SetAudioFunction {
                adaptive_noise_reduction: 1,
                beamforming: 1,
            },
        );
        assert!(!result.completed);
        let failure = result.failure.unwrap();
        assert_eq!(failure.command_index, failing);
        assert_eq!(failure.phase, CameraExtensionPhase::Decode);
        assert_eq!(result.exchanges.len(), failing + 1);
        assert!(
            result.exchanges[..failing]
                .iter()
                .all(|exchange| exchange.acknowledged)
        );
        assert!(!result.exchanges[failing].acknowledged);
        assert!(result.value.is_none());
        assert_eq!(mock.events.last(), Some(&Event::End));
    }
}

#[test]
fn cancellation_after_ack_retains_success_but_stops_compound_submission() {
    let validations = Cell::new(0);
    let mut mock = Mock::default();
    let request = CameraExtensionRequest::SetAudioFunction {
        adaptive_noise_reduction: 1,
        beamforming: 1,
    };
    let result = execute_camera_extension(&mut mock, 3594, request, &|| {
        validations.set(validations.get() + 1);
        if validations.get() == 5 {
            bail!("cancel after ACK");
        }
        Ok(())
    });
    assert!(!result.completed);
    assert_eq!(result.exchanges.len(), 1);
    assert!(result.exchanges[0].acknowledged);
    assert_eq!(
        result.failure.unwrap().phase,
        CameraExtensionPhase::Validation
    );
}

#[test]
fn failed_get_transport_retains_set_and_failure_seed_without_inventing_state() {
    let mut mock = Mock {
        fail_get: true,
        ..Default::default()
    };
    let result = run(&mut mock, CameraExtensionRequest::GetUvcExtCmdStatus);
    let exchange = &result.exchanges[0];
    assert!(exchange.set_outcome.is_some());
    assert!(exchange.get_attempted);
    assert!(exchange.get_outcome.is_none());
    assert_eq!(exchange.response, Some([255, 0, 0, 0, 0, 0, 0, 0]));
    assert!(result.value.is_none());
    assert_eq!(result.failure.unwrap().phase, CameraExtensionPhase::Get);
    assert_eq!(mock.events.last(), Some(&Event::End));
}

#[test]
fn cancellation_after_wait_releases_without_submitting_get() {
    let validations = Cell::new(0);
    let mut mock = Mock::default();
    let result = execute_camera_extension(
        &mut mock,
        3594,
        CameraExtensionRequest::SetLdc { enabled: true },
        &|| {
            validations.set(validations.get() + 1);
            if validations.get() == 4 {
                bail!("cancelled after delay");
            }
            Ok(())
        },
    );
    assert!(!result.completed);
    assert!(result.exchanges[0].set_attempted);
    assert!(!result.exchanges[0].get_attempted);
    assert!(mock.events.contains(&Event::Wait(10)));
    assert_eq!(mock.events.last(), Some(&Event::End));
}

#[test]
fn failed_size_read_retains_acknowledged_speed_without_combined_observation() {
    let mut mock = Mock::reply([0, 0, 0, 9, 0, 0, 0, 0]);
    mock.replies.push_back((
        CameraKsCallOutcome {
            hresult: 0,
            bytes_returned: 8,
        },
        [2, 0, 0, 7, 0, 0, 0, 0],
    ));
    let result = run(
        &mut mock,
        CameraExtensionRequest::GetAutoframingSpeedAndSize,
    );
    assert_eq!(result.failure.unwrap().command_index, 1);
    assert_eq!(result.exchanges.len(), 2);
    assert!(result.exchanges[0].acknowledged);
    assert!(!result.exchanges[1].acknowledged);
    assert!(result.value.is_none());
    assert!(
        result
            .exchanges
            .iter()
            .all(|exchange| !exchange.command.mutates_state)
    );
    assert_eq!(result.exchanges[0].response.unwrap()[3], 9);
}

#[test]
fn nonnegative_set_hresult_and_zero_set_byte_count_follow_native_direction() {
    let mut mock = Mock {
        set_hresult: 1,
        ..Default::default()
    };
    let result = run(&mut mock, CameraExtensionRequest::SetHdr { enabled: false });
    assert!(result.completed);
    assert_eq!(
        result.exchanges[0].set_outcome,
        Some(CameraKsCallOutcome {
            hresult: 1,
            bytes_returned: 0
        })
    );
    assert_eq!(result.value, Some(CameraExtensionValue::Acknowledged));
}
