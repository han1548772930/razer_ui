use super::*;
use razer_device::hanbo_cooler::{
    ControllerMode, ControllerPresence, FanControllerInfo, PumpControllerFunctionalityInfo,
    Telemetry,
};

fn sample(fan_rpm: u16, pump_rpm: u16, liquid_tenths: u16) -> Telemetry {
    Telemetry {
        fan: FanControllerInfo {
            channel_number: 1,
            presence: ControllerPresence::Present,
            set_mode: ControllerMode::Normal,
            actual_mode: ControllerMode::Normal,
            rpm: fan_rpm,
            set_duty_cycle: 50,
            actual_duty_cycle: 50,
        },
        pump: PumpControllerFunctionalityInfo {
            presence: ControllerPresence::Present,
            set_mode: ControllerMode::Normal,
            actual_mode: ControllerMode::Normal,
            temperature_tenths_celsius: liquid_tenths,
            rpm: pump_rpm,
            set_duty_cycle: 50,
            actual_duty_cycle: 50,
        },
    }
}

#[::core::prelude::v1::test]
fn hanbo_source_initial_values_are_not_observations() {
    let runtime = HanboRuntime::default();
    assert_eq!(runtime.cpu_temp, None);
    assert_eq!(runtime.cpu_speed, None);
    assert_eq!(runtime.gpu_temp, None);
    assert_eq!(runtime.gpu_speed, None);
    assert_eq!(runtime.fan_speed, None);
    assert_eq!(runtime.pump_speed, None);
    assert_eq!(runtime.liquid_temp, None);
    assert!(!runtime.hardware_outdated());
    assert_eq!(temperature(None, true), "0 °C");
    assert_eq!(temperature(None, false), "32.0 °F");
}

#[::core::prelude::v1::test]
fn hanbo_getter_pair_merges_only_its_three_observations() {
    let mut runtime = HanboRuntime {
        cpu_temp: Some(61.),
        cpu_speed: Some(4.2),
        gpu_temp: Some(52.),
        gpu_speed: Some(1.5),
        hardware_outdated: true,
        ..Default::default()
    };
    runtime.observe_telemetry(&sample(1_234, 2_345, 355));
    assert_eq!(runtime.fan_speed, Some(1_234));
    assert_eq!(runtime.pump_speed, Some(2_345));
    assert_eq!(runtime.liquid_temp, Some(355));
    assert_eq!(runtime.cpu_temp, Some(61.));
    assert_eq!(runtime.cpu_speed, Some(4.2));
    assert_eq!(runtime.gpu_temp, Some(52.));
    assert_eq!(runtime.gpu_speed, Some(1.5));
    assert!(runtime.hardware_outdated());
}

#[::core::prelude::v1::test]
fn hanbo_observed_zero_keeps_its_sample_identity() {
    let mut runtime = HanboRuntime::default();
    runtime.observe_telemetry(&sample(0, 0, 0));
    assert_eq!(runtime.fan_speed, Some(0));
    assert_eq!(runtime.pump_speed, Some(0));
    assert_eq!(runtime.liquid_temp, Some(0));
    assert_eq!(runtime.cpu_temp, None);
    assert_eq!(runtime.gpu_temp, None);
    assert_eq!(liquid_temperature(runtime.liquid_temp, true), "0.0 °C");
}

#[::core::prelude::v1::test]
fn hanbo_liquid_temperature_keeps_source_decimal_text_and_unit_conversion() {
    assert_eq!(liquid_temperature(None, true), "0 °C");
    assert_eq!(liquid_temperature(Some(350), true), "35.0 °C");
    assert_eq!(liquid_temperature(Some(355), true), "35.5 °C");
    assert_eq!(liquid_temperature(Some(355), false), "95.9 °F");
    assert_eq!(temperature(Some(35.), true), "35 °C");
    assert_eq!(temperature(Some(35.), false), "95.0 °F");
}

#[::core::prelude::v1::test]
fn hanbo_system_monitor_keeps_unobserved_values_and_source_callback_text() {
    let mut runtime = HanboRuntime::default();
    runtime.observe_system(&razer_device::system_monitor::SystemSample {
        cpu_temperature: Some(42.7),
        cpu_clock_mhz: Some(4200.),
        gpu_temperature: None,
        gpu_clock_mhz: None,
    });
    assert!(runtime.system_observed);
    assert_eq!(runtime.cpu_temp, Some(42.7));
    assert_eq!(runtime.cpu_speed, Some(4.2));
    assert_eq!(runtime.gpu_temp, None);
    assert_eq!(runtime.gpu_speed, None);
    assert_eq!(system_temperature(runtime.cpu_temp, true, true), "42.7 °C");
    assert_eq!(system_temperature(runtime.gpu_temp, true, true), " °C");
    assert_eq!(system_temperature(runtime.gpu_temp, false, true), "NaN °F");
    assert_eq!(system_clock(runtime.gpu_speed, true), "0.0 GHZ");
    assert_eq!(runtime.fan_speed, None);
}
