use super::*;

#[test]
fn preset_modes_use_hanbo_mode_settings() {
    let profile = ThermalProfile::default();
    assert_eq!(
        CoolerCommand::for_port(CoolerChannel::Fans, &profile.fans).unwrap(),
        CoolerCommand::FanParameter { mode: 2, speed: 50 }
    );
    let report = CoolerCommand::PumpParameter { mode: 1, speed: 20 }.report();
    assert_eq!(&report[..6], &[0, 20, 1, 1, 20, 0]);
    assert!(report[6..].iter().all(|byte| *byte == 0));
}

#[test]
fn advanced_curves_pad_and_truncate_like_uint8_array() {
    let mut profile = ThermalProfile::default();
    profile.fans.active_thermal_indicator.mode_selected = ThermalMode::Advanced;
    profile.fans.advanced_setting = vec![20.9, 30., 40., 50., 60., 80., 100.];
    let command = CoolerCommand::for_port(CoolerChannel::Fans, &profile.fans).unwrap();
    assert_eq!(
        &command.report()[..15],
        &[0, 200, 1, 0, 0, 20, 30, 40, 50, 60, 80, 100, 100, 100, 0]
    );
    profile.pump.active_thermal_indicator.mode_selected = ThermalMode::Advanced;
    let command = CoolerCommand::for_port(CoolerChannel::Pump, &profile.pump).unwrap();
    assert_eq!(
        &command.report()[..15],
        &[0, 24, 1, 1, 0, 20, 40, 60, 80, 100, 100, 100, 100, 100, 0]
    );
    profile.pump.advanced_setting.pop();
    assert!(CoolerCommand::for_port(CoolerChannel::Pump, &profile.pump).is_err());
}

#[test]
fn replies_require_header_and_status() {
    let command = CoolerCommand::FanParameter { mode: 3, speed: 80 };
    let mut input = [0u8; INPUT_LENGTH];
    assert_eq!(decode_reply(&command, &input), None);
    input[..3].copy_from_slice(&[35, 2, 1]);
    assert_eq!(decode_reply(&command, &input), Some(CoolerReply::Accepted));
    input[2] = 0;
    assert_eq!(
        decode_reply(&command, &input),
        Some(CoolerReply::Rejected { status: 0 })
    );
    assert_eq!(decode_reply(&command, &input[..63]), None);
}

#[test]
fn restore_rejects_malformed_curves() {
    let mut profile = ThermalProfile::default();
    let mut saved = profile.pump.clone();
    saved.advanced_setting = vec![10.; 7];
    assert!(!profile.restore_port(CoolerChannel::Pump, saved));
    assert_eq!(profile, ThermalProfile::default());
}
