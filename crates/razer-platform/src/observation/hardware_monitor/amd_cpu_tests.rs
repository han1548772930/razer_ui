use super::*;

#[test]
fn brand_calibration_respects_family_model_and_threadripper_variants() {
    for (family, model, brand, expected) in [
        (23, 1, "AMD Ryzen 7 1800X", 20.0),
        (23, 0, "AMD Ryzen 5 1600X", 20.0),
        (23, 1, "AMD Ryzen Threadripper 1950X", 27.0),
        (23, 1, "AMD Ryzen Threadripper 1950", 10.0),
        (23, 8, "AMD Ryzen Threadripper 2990WX", 27.0),
        (23, 8, "AMD Ryzen Threadripper 2970WX", 0.0),
        (23, 8, "AMD Ryzen 7 2700X", 10.0),
        (23, 1, "AMD EPYC 7601", 0.0),
        (25, 1, "AMD Ryzen 7 1800X", 0.0),
        (23, 8, "AMD Ryzen 7 1800X", 0.0),
    ] { assert_eq!(temperature_offset(family, model, brand), expected, "{brand}"); }
}

#[test]
fn temperature_range_branch_and_first_observation_calibration_are_retained() {
    let raw = 800 << 21;
    let single = TemperatureSensor::new(raw | 0x30000, 20.0, true).unwrap();
    let multiple = TemperatureSensor::new(raw | 0x30000, 20.0, false).unwrap();
    assert_eq!(single.decode(raw).unwrap(), 31.0);
    assert_eq!(multiple.decode(raw).unwrap(), 80.0);
    assert_eq!(TemperatureSensor::new(raw | 0x80000, 0.0, false).unwrap().decode(raw).unwrap(), 51.0);
    assert!(single.decode(0x80000).is_err());
    assert!(TemperatureSensor::new(0, 0.0, true).is_err());
}

fn counters() -> (ClockCounters, ClockCounters) {
    (ClockCounters { aperf: 100, mperf: 100, acpi: 100, qpc: 100 },
     ClockCounters { aperf: 300, mperf: 200, acpi: 3_579_645, qpc: 10_000_100 })
}

#[test]
fn clock_uses_timer_compensation_and_full_width_counters() {
    let (before, mut after) = counters();
    assert_eq!(clock_mhz(before, after, 2400.0, 10_000_000, None).unwrap(), 4800.0);
    after.aperf += 1_u64 << 32;
    after.mperf += 1_u64 << 31;
    assert_eq!(clock_mhz(before, after, 2400.0, 10_000_000, None).unwrap(), 4800.0);
    after.qpc = 5_000_100;
    assert_eq!(clock_mhz(before, after, 2400.0, 10_000_000, None).unwrap(), 2400.0);
}

#[test]
fn clock_wrap_and_failed_windows_do_not_publish_defaults() {
    let (mut before, mut after) = counters();
    before.aperf = u64::MAX - 99;
    after.aperf = 100;
    assert_eq!(clock_mhz(before, after, 2400.0, 10_000_000, None).unwrap(), 4800.0);
    after.mperf = before.mperf;
    assert!(clock_mhz(before, after, 2400.0, 10_000_000, None).is_err());
    let (before, mut after) = counters();
    after.acpi = 0;
    assert!(clock_mhz(before, after, 2400.0, 10_000_000, None).is_err());
    let (before, after) = counters();
    for dynamic in [(0.0, 0.05), (f32::NAN, 0.0), (-1.0, -1.0)] {
        assert!(clock_mhz(before, after, 2400.0, 10_000_000, Some(dynamic)).is_err());
    }
    assert!(clock_mhz(before, after, 2400.0, 10_000_000, Some((1.0, 1.01))).is_ok());
    assert!(clock_mhz(before, after, f32::NAN, 10_000_000, None).is_err());
}
