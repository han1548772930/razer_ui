use super::*;

fn drain(machine: &mut Machine, now: u64) -> Vec<Effect> {
    let mut effects = Vec::new();
    while let Some(effect) = machine.next_effect() {
        effects.push(effect);
        let payload = if effect == Effect::Device(DeviceCommand::ReadCalibration) {
            let mut bytes = vec![0, 5];
            for value in 1_u32..=12 {
                bytes.extend(value.to_be_bytes());
            }
            Some(bytes)
        } else {
            None
        };
        machine.complete(effect, Ok(payload), now).unwrap();
    }
    effects
}
fn collecting(kind: Kind) -> Machine {
    let mut machine = Machine::new(kind);
    drain(&mut machine, 0);
    machine.observe(Input::LeftDown, 10);
    drain(&mut machine, 10);
    assert_eq!(machine.phase(), Phase::AwaitingStart);
    machine.observe(Input::Calibration(CalibrationEvent::Start), 20);
    drain(&mut machine, 20);
    assert_eq!(machine.phase(), Phase::Collecting);
    machine
}
fn measured_progress() -> (Progress, crate::device_reads::DeviceReadTarget) {
    let mut machine = collecting(Kind::Manual);
    machine.observe(Input::Sample { z: 2, q: 4 }, 21);
    machine.observe(Input::Sample { z: 4, q: 136 }, 22);
    machine.observe(Input::LeftDown, 30);
    drain(&mut machine, 30);
    machine.observe(Input::Calibration(CalibrationEvent::End), 31);
    let mut getter = vec![0, 5];
    for value in [0x01020304_u32, u32::MAX, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12] {
        getter.extend(value.to_be_bytes());
    }
    let mut commands = Vec::new();
    while let Some(effect) = machine.next_effect() {
        let payload = match effect {
            Effect::Device(DeviceCommand::ReadCalibration) => {
                commands.push(CommandReport {
                    command: DeviceCommand::ReadCalibration,
                    acknowledged: Some(getter.clone()),
                    command_may_have_reached_device: true,
                    error: None,
                });
                Some(getter.clone())
            }
            _ => None,
        };
        machine.complete(effect, Ok(payload), 32).unwrap();
    }
    let target = crate::device_reads::DeviceReadTarget {
        product_id: 70,
        physical_product_id: 70,
        peer_product_id: None,
        device_container_id: "simulated-container".into(),
        path: "simulated-hid".into(),
    };
    let mut progress = machine.progress("measurement-owner", &target, commands);
    progress.driver_ready = true;
    (progress, target)
}
#[test]
fn mouse_philips_measurement_progress_requires_driver_end_and_acknowledged_getter() {
    let (progress, target) = measured_progress();
    progress
        .validate("measurement-owner", &target, Kind::Manual)
        .unwrap();
    for case in 0..7 {
        let mut invalid = progress.clone();
        match case {
            0 => invalid.driver_ready = false,
            1 => invalid.notifications.clear(),
            2 => invalid.commands.clear(),
            3 => invalid.commands[0].acknowledged = None,
            4 => {
                invalid.commands[0].acknowledged.as_mut().unwrap().pop();
            }
            5 => invalid.commands[0].command_may_have_reached_device = false,
            6 => invalid.commands[0].error = Some("simulated getter rejected".into()),
            _ => unreachable!(),
        }
        assert!(
            invalid
                .validate("measurement-owner", &target, Kind::Manual)
                .is_err(),
            "Invalid observation case {case} was accepted"
        );
    }
    for word in 0..12 {
        let mut invalid = progress.clone();
        invalid.commands[0].acknowledged.as_mut().unwrap()[2 + word * 4] ^= 1;
        assert!(
            invalid
                .validate("measurement-owner", &target, Kind::Manual)
                .is_err(),
            "Mismatched calibration word {word} was accepted"
        );
    }
}
#[test]
fn mouse_philips_measurement_progress_requires_full_owner_and_consistent_release() {
    let (progress, target) = measured_progress();
    assert!(
        progress
            .validate("replacement-owner", &target, Kind::Manual)
            .is_err()
    );
    assert!(
        progress
            .validate("measurement-owner", &target, Kind::Mini)
            .is_err()
    );
    for case in 0..5 {
        let mut replaced = target.clone();
        match case {
            0 => replaced.product_id = 71,
            1 => replaced.physical_product_id = 71,
            2 => replaced.peer_product_id = Some(71),
            3 => replaced.device_container_id = "replacement-container".into(),
            4 => replaced.path = "replacement-hid".into(),
            _ => unreachable!(),
        }
        assert!(
            progress
                .validate("measurement-owner", &replaced, Kind::Manual)
                .is_err()
        );
    }
    let mut invalid = progress.clone();
    invalid.phase = Phase::Releasing;
    assert!(
        invalid
            .validate("measurement-owner", &target, Kind::Manual)
            .is_err()
    );
    invalid.terminal = false;
    assert!(
        invalid
            .validate("measurement-owner", &target, Kind::Manual)
            .is_err()
    );
    invalid.cleanup_confirmed = false;
    invalid
        .validate("measurement-owner", &target, Kind::Manual)
        .unwrap();
}
#[test]
fn mouse_philips_measurement_commands_have_current_source_layout() {
    assert_eq!(
        DeviceCommand::Configure.packet(),
        ([8, 11, 1], vec![0, 5, 0, 10, 0, 0, 0, 0])
    );
    assert_eq!(
        DeviceCommand::Control {
            calibration_type: ControlType::Preset,
            stage: ControlStage::Start
        }
        .packet(),
        ([4, 11, 9], vec![0, 5, 1, 0])
    );
    let (header, payload) = DeviceCommand::ReadCalibration.packet();
    assert_eq!(header, [50, 11, 136]);
    assert_eq!(&payload[..2], &[0, 5]);
    assert_eq!(payload.len(), 50);
}
#[test]
fn mouse_philips_measurement_fixture_validates_timeouts_controls_and_notifications() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../testdata/re/mouse-philips-current-source.json"
    ))
    .unwrap();
    let measured = &fixture["measurement"];
    assert_eq!(
        measured["timeouts_ms"],
        serde_json::json!({"initiating":INITIATING_TIMEOUT_MS,"general":GENERAL_TIMEOUT_MS,"mini_calibrating":MINI_COLLECTION_MS})
    );
    assert_eq!(measured["sample_rate"], 10);
    assert_eq!(measured["sample_count"], 0);
    assert_eq!(measured["measured_result"], 0);
    assert_eq!(
        fixture["commands"]["setProximitySensorCalibration"],
        serde_json::json!(DeviceCommand::Configure.packet().0)
    );
    assert_eq!(
        fixture["commands"]["getProximitySensorCalibrationData"],
        serde_json::json!(DeviceCommand::ReadCalibration.packet().0)
    );
    assert_eq!(
        measured["control_types"]["Manual"],
        u8::from(Kind::Manual.control())
    );
    assert_eq!(
        measured["control_types"]["Preset"],
        u8::from(Kind::Mini.control())
    );
    assert_eq!(
        measured["control_stages"]["Start"],
        u8::from(ControlStage::Start)
    );
    assert_eq!(
        measured["control_stages"]["Stop"],
        u8::from(ControlStage::Stop)
    );
    for (key, event) in [
        ("Start", CalibrationEvent::Start),
        ("End", CalibrationEvent::End),
        ("Halt", CalibrationEvent::Halt),
        ("Reset", CalibrationEvent::Reset),
        ("Normal_Speed", CalibrationEvent::NormalSpeed),
        ("Too_Fast", CalibrationEvent::TooFast),
        ("Cancelled", CalibrationEvent::Cancelled),
    ] {
        assert_eq!(measured["events"][key], u8::from(event));
    }
    assert_eq!(measured["save"]["manual_height_mean_additions"], 1);
    assert_eq!(measured["save"]["mini_apply_writes_threshold"], false);
    assert_eq!(
        measured["save"]["recalibrate_persisted_profile_fields"],
        serde_json::json!(["calibrationDate"])
    );
}
#[test]
fn mouse_philips_measurement_decoder_checks_report_and_sensor() {
    for (bytes, expected) in [
        (
            vec![5, 10, 0],
            Some(Input::Calibration(CalibrationEvent::Start)),
        ),
        (
            vec![9, 10, 5],
            Some(Input::Calibration(CalibrationEvent::TooFast)),
        ),
        (vec![77, 0, 1], Some(Input::LeftDown)),
        (vec![77, 0, 0], None),
        (vec![90, 255, 128], Some(Input::Sample { z: 255, q: 128 })),
        (vec![4], None),
    ] {
        assert_eq!(parse_report(&bytes).unwrap(), expected);
    }
    for bytes in [vec![], vec![5, 10], vec![5, 10, 7], vec![90, 1]] {
        assert!(parse_report(&bytes).is_err());
    }
    let mut data = vec![0, 5];
    for value in [0x01020304, u32::MAX, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12] {
        data.extend(value.to_be_bytes());
    }
    let calibration = read_calibration(&data).unwrap();
    assert_eq!(calibration.x_offset, 0x01020304);
    assert_eq!(calibration.x_swing, u32::MAX);
    assert_eq!(calibration.y_reference_height_b, 12);
    assert!(read_calibration(&data[..49]).is_err());
    data[1] = 1;
    assert!(read_calibration(&data).is_err());
}
#[test]
fn mouse_philips_manual_ack_waits_for_start_and_end_getter() {
    let mut machine = Machine::new(Kind::Manual);
    assert_eq!(
        drain(&mut machine, 0),
        [
            Effect::Device(DeviceCommand::SensorState(true)),
            Effect::Device(DeviceCommand::Configure),
            Effect::Escape(true),
        ]
    );
    machine.observe(Input::LeftDown, 1);
    assert_eq!(
        drain(&mut machine, 1),
        [Effect::Device(DeviceCommand::Control {
            calibration_type: ControlType::Manual,
            stage: ControlStage::Start
        })]
    );
    assert_eq!(machine.phase(), Phase::AwaitingStart);
    assert!(machine.notifications.is_empty());
    machine.observe(Input::Calibration(CalibrationEvent::Start), 2);
    assert_eq!(drain(&mut machine, 2), [Effect::Redirect(true)]);
    machine.observe(Input::Sample { z: 2, q: 4 }, 3);
    machine.observe(Input::Sample { z: 4, q: 136 }, 4);
    machine.observe(Input::LeftDown, 5);
    assert_eq!(
        drain(&mut machine, 5),
        [
            Effect::Redirect(false),
            Effect::Escape(false),
            Effect::Device(DeviceCommand::Control {
                calibration_type: ControlType::Manual,
                stage: ControlStage::Stop
            })
        ]
    );
    assert!(machine.measured.is_none());
    machine.observe(Input::Calibration(CalibrationEvent::End), 6);
    assert_eq!(
        drain(&mut machine, 6),
        [
            Effect::Device(DeviceCommand::ReadCalibration),
            Effect::Redirect(false),
            Effect::Escape(false),
            Effect::Release
        ]
    );
    assert!(machine.measured.is_some() && machine.cleanup_confirmed);
    assert_eq!(machine.phase(), Phase::Finished);
}
#[test]
fn mouse_philips_mini_ignores_click_and_uses_preset_stop_after_five_seconds() {
    let mut machine = collecting(Kind::Mini);
    machine.observe(Input::LeftDown, 100);
    assert!(machine.next_effect().is_none());
    machine.tick(5_019);
    assert_eq!(machine.phase(), Phase::Collecting);
    machine.tick(5_020);
    assert!(
        drain(&mut machine, 5_020).contains(&Effect::Device(DeviceCommand::Control {
            calibration_type: ControlType::Preset,
            stage: ControlStage::Stop
        }))
    );
}
#[test]
fn mouse_philips_manual_collection_timeout_stops_instead_of_fabricating_end() {
    let mut machine = collecting(Kind::Manual);
    machine.tick(60_019);
    assert_eq!(machine.phase(), Phase::Collecting);
    machine.tick(60_020);
    drain(&mut machine, 60_020);
    assert_eq!(machine.phase(), Phase::AwaitingEnd);
    assert!(machine.measured.is_none());
    machine.tick(120_020);
    drain(&mut machine, 120_020);
    assert_eq!(machine.phase(), Phase::Finished);
    assert!(machine.measured.is_none() && !machine.errors.is_empty());
}
#[test]
fn mouse_philips_mini_cancel_uses_manual_stop_and_waits_for_release() {
    let mut machine = collecting(Kind::Mini);
    machine.cancel();
    assert_eq!(machine.phase(), Phase::Releasing);
    let effects = drain(&mut machine, 50);
    assert_eq!(
        effects,
        [
            Effect::Redirect(false),
            Effect::Escape(false),
            Effect::Device(DeviceCommand::Control {
                calibration_type: ControlType::Manual,
                stage: ControlStage::Stop
            }),
            Effect::Device(DeviceCommand::SensorState(false)),
            Effect::Release
        ]
    );
    assert!(machine.canceled && machine.cleanup_confirmed && machine.measured.is_none());
}
#[test]
fn mouse_philips_initiation_timeout_and_callback_overflow_cleanup() {
    let mut machine = Machine::new(Kind::Manual);
    drain(&mut machine, 0);
    machine.tick(34_999);
    assert_eq!(machine.phase(), Phase::AwaitingClick);
    machine.tick(35_000);
    let effects = drain(&mut machine, 35_000);
    assert!(
        !effects
            .iter()
            .any(|effect| matches!(effect, Effect::Device(DeviceCommand::Control { .. })))
    );
    assert!(effects.contains(&Effect::Device(DeviceCommand::SensorState(false))));
    let mut machine = collecting(Kind::Manual);
    machine.abort("callback queue overflowed".into());
    drain(&mut machine, 100);
    assert!(machine.errors[0].contains("overflowed"));
    assert!(machine.measured.is_none());
    assert!(
        !machine
            .notifications
            .iter()
            .any(|event| event.event == CalibrationEvent::Halt)
    );
}
#[test]
fn mouse_philips_failed_getter_never_produces_measured_data() {
    let mut machine = collecting(Kind::Manual);
    machine.observe(Input::LeftDown, 30);
    drain(&mut machine, 30);
    machine.observe(Input::Calibration(CalibrationEvent::End), 31);
    let effect = machine.next_effect().unwrap();
    machine
        .complete(effect, Err(anyhow::anyhow!("HID getter rejected")), 32)
        .unwrap();
    drain(&mut machine, 33);
    assert!(machine.measured.is_none());
    assert!(
        machine
            .notifications
            .iter()
            .any(|event| event.event == CalibrationEvent::Halt && !event.succeeded)
    );
}
#[test]
fn mouse_philips_cleanup_failure_is_explicit_and_late_events_are_ignored() {
    let mut machine = collecting(Kind::Manual);
    machine.cancel();
    while let Some(effect) = machine.next_effect() {
        let result = if effect == Effect::Release {
            Err(anyhow::anyhow!("removeUsbDevice failed"))
        } else {
            Ok(None)
        };
        machine.complete(effect, result, 100).unwrap();
    }
    assert_eq!(machine.phase(), Phase::Finished);
    assert!(!machine.cleanup_confirmed);
    let count = machine.notifications.len();
    machine.observe(Input::Calibration(CalibrationEvent::End), 101);
    machine.observe(Input::LeftDown, 102);
    assert_eq!(machine.notifications.len(), count);
    assert!(machine.measured.is_none());
    machine.retry_cleanup();
    assert_eq!(machine.phase(), Phase::Releasing);
    assert_eq!(drain(&mut machine, 103), [Effect::Release]);
    assert!(machine.cleanup_confirmed);
    assert!(
        machine
            .errors
            .iter()
            .any(|error| error.contains("removeUsbDevice"))
    );
}
#[test]
fn mouse_philips_getter_without_payload_still_releases_and_never_saves() {
    let mut machine = collecting(Kind::Manual);
    machine.observe(Input::LeftDown, 30);
    drain(&mut machine, 30);
    machine.observe(Input::Calibration(CalibrationEvent::End), 31);
    let effect = machine.next_effect().unwrap();
    machine.complete(effect, Ok(None), 32).unwrap();
    drain(&mut machine, 33);
    assert_eq!(machine.phase(), Phase::Finished);
    assert!(machine.measured.is_none() && machine.cleanup_confirmed);
}
#[test]
fn mouse_philips_unawaited_redirect_failure_does_not_fabricate_halt_or_reset_deadline() {
    let mut machine = Machine::new(Kind::Mini);
    drain(&mut machine, 0);
    machine.observe(Input::LeftDown, 10);
    drain(&mut machine, 10);
    machine.observe(Input::Calibration(CalibrationEvent::Start), 20);
    let effect = machine.next_effect().unwrap();
    machine
        .complete(effect, Err(anyhow::anyhow!("native redirect rejected")), 40)
        .unwrap();
    assert_eq!(machine.phase(), Phase::Collecting);
    assert!(
        !machine
            .notifications
            .iter()
            .any(|event| event.event == CalibrationEvent::Halt)
    );
    machine.tick(5_019);
    assert_eq!(machine.phase(), Phase::Collecting);
    machine.tick(5_020);
    drain(&mut machine, 5_020);
    assert_eq!(machine.phase(), Phase::AwaitingEnd);
}
#[test]
fn mouse_philips_streaming_statistics_keep_source_order_and_zero_sample_definition() {
    let mut samples = Samples::default();
    assert_eq!(
        samples.statistics(),
        Statistics {
            sample_count: 0,
            mean: 0.0,
            standard_deviation: 0.0,
            v_power: 0.0
        }
    );
    samples.push(2, 4);
    samples.push(4, 136);
    let result = samples.statistics();
    assert_eq!(result.mean, 96.0);
    assert_eq!(result.standard_deviation, 2048_f64.sqrt());
    assert_eq!(result.v_power, 2.0);
    for _ in 2..31_744 {
        samples.push(3, 0);
    }
    assert_ne!(samples.statistics().v_power, 10_000.0);
    samples.push(3, 0);
    assert_eq!(samples.statistics().v_power, 10_000.0);
}
