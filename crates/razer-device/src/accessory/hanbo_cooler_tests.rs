use super::*;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

#[path = "hanbo_startup_tests.rs"]
mod startup;

#[test]
fn cpu_gpu_temperature_preserves_source_string_and_uint8_coercion() {
    for (cpu, gpu, expected) in [
        (42.7, 73.25, [42, 0, 73, 0]),
        (-2.5, 258.9, [254, 0, 2, 0]),
        (1e-7, 1e21, [1, 0, 1, 0]),
    ] {
        let command = CoolerCommand::for_temperatures(Some(cpu), Some(gpu)).unwrap();
        assert_eq!(
            &command.report()[..8],
            &[
                0,
                192,
                1,
                expected[0],
                expected[1],
                expected[2],
                expected[3],
                0
            ]
        );
        assert_eq!(command.reply_header(), [193, 2]);
    }
    assert!(CoolerCommand::for_temperatures(None, Some(42.)).is_err());
    assert!(CoolerCommand::for_temperatures(Some(42.), None).is_err());
    assert!(CoolerCommand::for_temperatures(Some(f64::NAN), Some(42.)).is_err());
}

fn fan_packet() -> Vec<u8> {
    let mut packet = vec![0; 64];
    packet[..10].copy_from_slice(&[33, 2, 3, 1, 2, 3, 0x12, 0x34, 99, 48]);
    packet
}

fn pump_packet() -> Vec<u8> {
    let mut packet = vec![0; 64];
    packet[..11].copy_from_slice(&[19, 2, 0, 2, 1, 36, 7, 0x56, 0x78, 80, 79]);
    packet
}

struct FakeTransport {
    writes: RefCell<Vec<Vec<u8>>>,
    counts: RefCell<VecDeque<std::result::Result<usize, &'static str>>>,
    inputs: RefCell<VecDeque<std::result::Result<Vec<u8>, &'static str>>>,
    reads: Cell<usize>,
    events: RefCell<Vec<String>>,
}

impl FakeTransport {
    fn new(inputs: impl IntoIterator<Item = std::result::Result<Vec<u8>, &'static str>>) -> Self {
        Self {
            writes: RefCell::new(Vec::new()),
            counts: RefCell::new(VecDeque::new()),
            inputs: RefCell::new(inputs.into_iter().collect()),
            reads: Cell::new(0),
            events: RefCell::new(Vec::new()),
        }
    }
}

impl FeatureTransport for FakeTransport {
    fn send_feature(&self, _: &[u8]) -> Result<()> {
        bail!("unexpected Feature send")
    }

    fn get_feature(&self, _: &mut [u8]) -> Result<usize> {
        bail!("unexpected Feature read")
    }

    fn write_output(&self, _: &[u8]) -> Result<()> {
        bail!("uncounted Output send cannot prove the source return count")
    }

    fn write_output_counted(&self, report: &[u8]) -> Result<usize> {
        self.writes.borrow_mut().push(report.to_vec());
        let count = self.counts.borrow_mut().pop_front().unwrap_or(Ok(65));
        self.events.borrow_mut().push(format!("write:{count:?}"));
        count.map_err(anyhow::Error::msg)
    }

    fn read_interrupt(&self, buffer: &mut [u8], timeout_ms: u32) -> Result<usize> {
        assert_eq!(timeout_ms, 50);
        self.reads.set(self.reads.get() + 1);
        self.events.borrow_mut().push("read".into());
        let input = self
            .inputs
            .borrow_mut()
            .pop_front()
            .unwrap_or(Ok(Vec::new()))
            .map_err(anyhow::Error::msg)?;
        buffer[..input.len()].copy_from_slice(&input);
        Ok(input.len())
    }

    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({"transport": "simulated"})
    }
}

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

#[test]
fn getters_send_the_full_source_buffer_and_have_distinct_headers() {
    let mut fan = [0; 66];
    fan[..3].copy_from_slice(&[0, 32, 1]);
    let mut pump = [0; 66];
    pump[..3].copy_from_slice(&[0, 18, 1]);
    assert_eq!(CoolerQuery::FanControllerInfo.report(), fan);
    assert_eq!(CoolerQuery::PumpControllerFunctionalityInfo.report(), pump);
    assert_eq!(EXPECTED_OUTPUT_COUNT, 65);
    assert_eq!(OUTPUT_ATTEMPTS, 20);
    assert_eq!(INPUT_ATTEMPTS, 10);
}

#[test]
fn fan_getter_decodes_channel_status_and_big_endian_rpm() {
    assert_eq!(
        decode_fan_controller_info(&fan_packet()),
        Some(FanControllerInfo {
            channel_number: 3,
            presence: ControllerPresence::Present,
            set_mode: ControllerMode::Normal,
            actual_mode: ControllerMode::Performance,
            rpm: 0x1234,
            set_duty_cycle: 99,
            actual_duty_cycle: 48,
        })
    );
}

#[test]
fn pump_getter_does_not_confuse_disconnected_with_a_rejected_command() {
    let pump = decode_pump_controller_functionality_info(&pump_packet()).unwrap();
    assert_eq!(
        pump,
        PumpControllerFunctionalityInfo {
            presence: ControllerPresence::Absent,
            set_mode: ControllerMode::Normal,
            actual_mode: ControllerMode::Quiet,
            temperature_tenths_celsius: 367,
            rpm: 0x5678,
            set_duty_cycle: 80,
            actual_duty_cycle: 79,
        }
    );
    assert_eq!(pump.temperature_celsius(), 36.7);
}

#[test]
fn telemetry_preserves_unknown_codes_and_the_full_temperature_bytes() {
    let mut fan = fan_packet();
    fan[3] = 0x81;
    fan[4] = 0xfe;
    fan[5] = 0xff;
    fan[8] = 255;
    let fan = decode_fan_controller_info(&fan).unwrap();
    assert_eq!(fan.presence, ControllerPresence::Unknown(0x81));
    assert_eq!(fan.set_mode, ControllerMode::Unknown(0xfe));
    assert_eq!(fan.actual_mode, ControllerMode::Unknown(0xff));
    assert_eq!(fan.set_duty_cycle, 255);
    let mut pump = pump_packet();
    pump[2] = 7;
    pump[5] = 255;
    pump[6] = 255;
    let pump = decode_pump_controller_functionality_info(&pump).unwrap();
    assert_eq!(pump.presence, ControllerPresence::Unknown(7));
    assert_eq!(pump.temperature_tenths_celsius, 2805);
    Telemetry { fan, pump }.validate().unwrap();
}

#[test]
fn getter_decoders_reject_wrong_headers_truncated_and_overlong_packets() {
    assert!(decode_fan_controller_info(&pump_packet()).is_none());
    assert!(decode_pump_controller_functionality_info(&fan_packet()).is_none());
    for packet in [
        Vec::new(),
        vec![33],
        fan_packet()[..63].to_vec(),
        [fan_packet(), vec![0]].concat(),
    ] {
        assert!(decode_fan_controller_info(&packet).is_none());
    }
    for packet in [
        Vec::new(),
        vec![19],
        pump_packet()[..63].to_vec(),
        [pump_packet(), vec![0]].concat(),
    ] {
        assert!(decode_pump_controller_functionality_info(&packet).is_none());
    }
}

#[test]
fn telemetry_reads_fan_then_pump_and_publishes_one_complete_sample() {
    let transport = FakeTransport::new([Ok(fan_packet()), Ok(pump_packet())]);
    let mut waits = Vec::new();
    let telemetry =
        read_telemetry_with_wait(&transport, || Ok(()), |delay| waits.push(delay)).unwrap();
    assert_eq!(telemetry.fan.rpm, 0x1234);
    assert_eq!(telemetry.pump.rpm, 0x5678);
    assert_eq!(telemetry.pump.temperature_tenths_celsius, 367);
    assert_eq!(transport.reads.get(), 2);
    let writes = transport.writes.borrow();
    assert_eq!(writes.len(), 2);
    assert_eq!(&writes[0][..3], &[0, 32, 1]);
    assert_eq!(&writes[1][..3], &[0, 18, 1]);
    assert!(writes.iter().all(|report| report.len() == 66));
    assert_eq!(waits, [std::time::Duration::from_millis(5); 2]);
}

#[test]
fn full_66_byte_native_write_is_not_the_source_65_byte_success() {
    let transport = FakeTransport::new([Ok(fan_packet()), Ok(pump_packet())]);
    transport.counts.borrow_mut().extend([Ok(66); 20]);
    let error = read_telemetry_with_wait(&transport, || Ok(()), |_| {}).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("native write returned 66 bytes; source requires 65")
    );
    assert_eq!(transport.writes.borrow().len(), 20);
    assert_eq!(transport.reads.get(), 0);
}

#[test]
fn native_write_count_is_checked_before_any_input_read() {
    let transport = FakeTransport::new([Ok(fan_packet()), Ok(pump_packet())]);
    transport
        .counts
        .borrow_mut()
        .extend([Ok(66), Ok(65), Ok(65)]);
    read_telemetry_with_wait(&transport, || Ok(()), |_| {}).unwrap();
    assert_eq!(
        *transport.events.borrow(),
        [
            "write:Ok(66)",
            "write:Ok(65)",
            "read",
            "write:Ok(65)",
            "read"
        ]
    );
}

#[test]
fn interrupt_input_requires_exactly_64_bytes_without_a_synthetic_id() {
    let transport = FakeTransport::new([
        Ok([vec![0], fan_packet()].concat()),
        Ok(fan_packet()),
        Ok(pump_packet()),
    ]);
    let mut waits = Vec::new();
    read_telemetry_with_wait(&transport, || Ok(()), |delay| waits.push(delay)).unwrap();
    assert_eq!(transport.writes.borrow().len(), 2);
    assert_eq!(transport.reads.get(), 3);
    assert_eq!(waits, [std::time::Duration::from_millis(5); 3]);
}

#[test]
fn a_stale_other_getter_packet_is_discarded_before_the_next_query() {
    let transport = FakeTransport::new([Ok(pump_packet()), Ok(fan_packet()), Ok(pump_packet())]);
    let telemetry = read_telemetry_with_wait(&transport, || Ok(()), |_| {}).unwrap();
    assert_eq!(transport.writes.borrow().len(), 2);
    assert_eq!(transport.reads.get(), 3);
    assert_eq!(telemetry.fan.channel_number, 3);
}

#[test]
fn invalid_input_exhausts_the_source_retry_budget_without_success() {
    let transport = FakeTransport::new([]);
    let error = read_telemetry_with_wait(&transport, || Ok(()), |_| {}).unwrap_err();
    assert!(error.to_string().contains("invalid input count 0"));
    assert_eq!(transport.writes.borrow().len(), 20);
    assert_eq!(transport.reads.get(), 200);
}

#[test]
fn pump_failure_retains_the_native_error_and_does_not_publish_partial_fan_data() {
    let transport = FakeTransport::new(std::iter::once(Ok(fan_packet())).chain(
        std::iter::repeat_n(Err("simulated pump interrupt failure"), 20),
    ));
    let error = read_telemetry_with_wait(&transport, || Ok(()), |_| {}).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("simulated pump interrupt failure")
    );
    let writes = transport.writes.borrow();
    assert_eq!(writes.len(), 21);
    assert_eq!(&writes[0][1..3], &[32, 1]);
    assert!(writes[1..].iter().all(|report| report[1..3] == [18, 1]));
    assert_eq!(transport.reads.get(), 21);
}

#[test]
fn native_write_error_is_retained_without_reading_or_claiming_success() {
    let transport = FakeTransport::new([Ok(fan_packet())]);
    transport
        .counts
        .borrow_mut()
        .push_back(Err("simulated write failure"));
    let error = read_telemetry_with_wait(&transport, || Ok(()), |_| {}).unwrap_err();
    assert!(format!("{error:#}").contains("simulated write failure"));
    assert_eq!(transport.writes.borrow().len(), 1);
    assert_eq!(transport.reads.get(), 0);
}

#[test]
fn cancellation_before_the_first_query_performs_no_io() {
    let transport = FakeTransport::new([Ok(fan_packet())]);
    let error = read_telemetry_with_wait(&transport, || bail!("canceled"), |_| {}).unwrap_err();
    assert_eq!(error.to_string(), "canceled");
    assert!(transport.writes.borrow().is_empty());
    assert_eq!(transport.reads.get(), 0);
}

#[test]
fn cancellation_after_native_write_prevents_the_input_read() {
    let transport = FakeTransport::new([Ok(fan_packet())]);
    let error = read_telemetry_with_wait(
        &transport,
        || {
            ensure!(
                transport.writes.borrow().is_empty(),
                "replaced after native write"
            );
            Ok(())
        },
        |_| {},
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "replaced after native write");
    assert_eq!(transport.writes.borrow().len(), 1);
    assert_eq!(transport.reads.get(), 0);
}

#[test]
fn cancellation_after_fan_read_prevents_the_pump_query() {
    let transport = FakeTransport::new([Ok(fan_packet()), Ok(pump_packet())]);
    let error = read_telemetry_with_wait(
        &transport,
        || {
            ensure!(transport.reads.get() == 0, "replaced after fan query");
            Ok(())
        },
        |_| {},
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "replaced after fan query");
    assert_eq!(transport.writes.borrow().len(), 1);
    assert_eq!(transport.reads.get(), 1);
}

#[test]
fn cancellation_after_pump_read_rejects_the_late_complete_sample() {
    let transport = FakeTransport::new([Ok(fan_packet()), Ok(pump_packet())]);
    let error = read_telemetry_with_wait(
        &transport,
        || {
            ensure!(transport.reads.get() < 2, "replaced after pump query");
            Ok(())
        },
        |_| {},
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "replaced after pump query");
    assert_eq!(transport.writes.borrow().len(), 2);
    assert_eq!(transport.reads.get(), 2);
}

#[test]
fn setter_keeps_its_status_one_acknowledgement_semantics() {
    let command = CoolerCommand::FanParameter { mode: 3, speed: 80 };
    for (status, accepted) in [(1, true), (0, false), (2, false), (255, false)] {
        let mut input = vec![0; 64];
        input[..3].copy_from_slice(&[35, 2, status]);
        let transport = FakeTransport::new([Ok(input)]);
        let result = exchange_with_wait(&transport, &command, || Ok(()), |_| {});
        assert_eq!(result.is_ok(), accepted);
        if let Err(error) = result {
            assert!(
                error
                    .to_string()
                    .contains(&format!("rejected, status {status}"))
            );
        }
        assert_eq!(transport.writes.borrow().len(), 1);
        assert_eq!(transport.writes.borrow()[0].len(), 66);
        assert_eq!(transport.reads.get(), 1);
    }
}

#[test]
fn only_the_source_vid_pid_and_interface_can_enter_the_hanbo_route() {
    let node = HidNode {
        path: b"hanbo".to_vec(),
        vendor_id: 5426,
        product_id: 3893,
        usage_page: 0xff00,
        usage: 1,
        interface_number: 0,
        release_number: 0x100,
        serial_number: None,
        manufacturer: None,
        product: None,
    };
    validate_node(&node, 3893).unwrap();
    assert!(validate_node(&node, 182).is_err());
    let mut changed = node.clone();
    changed.vendor_id = 0;
    assert!(validate_node(&changed, 3893).is_err());
    changed = node.clone();
    changed.product_id = 0;
    assert!(validate_node(&changed, 3893).is_err());
    changed = node;
    changed.interface_number = -1;
    assert!(validate_node(&changed, 3893).is_err());
}
