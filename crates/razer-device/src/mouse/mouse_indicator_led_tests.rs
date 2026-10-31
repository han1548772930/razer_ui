//! Pure transports: no vendor code, application process or actual HID access.
use super::*;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

enum Reply {
    Data(u8, Vec<u8>),
    GetterError,
    Mismatch,
}
struct Simulated {
    sent: RefCell<Vec<Vec<u8>>>,
    replies: RefCell<VecDeque<Reply>>,
    fail_send: bool,
    get_ids: RefCell<Vec<u8>>,
    fail_sends: Cell<usize>,
}
impl Simulated {
    fn new(replies: impl IntoIterator<Item = Reply>) -> Self {
        Self {
            sent: RefCell::new(vec![]),
            replies: RefCell::new(replies.into_iter().collect()),
            fail_send: false,
            get_ids: RefCell::new(vec![]),
            fail_sends: Cell::new(0),
        }
    }
}
impl FeatureTransport for Simulated {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        self.sent.borrow_mut().push(report.to_vec());
        if self.fail_sends.get() > 0 {
            self.fail_sends.set(self.fail_sends.get() - 1);
            return Err(crate::backend::native_io_failure(
                crate::backend::NativeIoOutcome::Rejected,
                anyhow::anyhow!("simulated transient host send failure"),
            ));
        }
        if self.fail_send {
            return Err(crate::backend::native_io_failure(
                crate::backend::NativeIoOutcome::Rejected,
                anyhow::anyhow!("simulated send failure"),
            ));
        }
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        self.get_ids.borrow_mut().push(report[0]);
        let id = report[0];
        let reply = self
            .replies
            .borrow_mut()
            .pop_front()
            .context("No simulated reply")?;
        let sent = self.sent.borrow();
        let sent = sent.last().unwrap();
        report.fill(0);
        report[0] = id;
        report[2] = sent[2];
        report[7..9].copy_from_slice(&sent[7..9]);
        match reply {
            Reply::Data(status, data) => {
                report[1] = status;
                report[6] = data.len() as u8;
                report[9..9 + data.len()].copy_from_slice(&data);
            }
            Reply::GetterError => anyhow::bail!("simulated getter failure"),
            Reply::Mismatch => {
                report[1] = 2;
                report[2] = 31;
            }
        }
        Ok(report.len())
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({"simulated":true})
    }
}
fn cap(pid: u32) -> DeviceReadCapability {
    let source = capability(pid).unwrap();
    let mut cap = source.transport(source.dongle_id()).unwrap();
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap.max_retry_in = 2;
    cap.max_retry_out = 3;
    cap
}
fn next() -> impl FnMut() -> anyhow::Result<u8> {
    let mut value = 0;
    move || {
        let result = value;
        value += 1;
        Ok(result)
    }
}

#[test]
fn interrupted_receiver_led_calls_never_retry_or_zero_the_actual_reply() {
    use crate::backend::{NativeIoOutcome, native_io_outcome};
    use crate::keyboard_control_test_transport::{Mock, reply};
    for pid in [190, 229, 239] {
        let cap = cap(pid);
        for outcome in [
            NativeIoOutcome::NotCalled,
            NativeIoOutcome::Returned { byte_count: 90 },
            NativeIoOutcome::Returned { byte_count: 91 },
        ] {
            let device = Mock::new([]).fail_next_send(outcome);
            let mut progress = (false, false);
            let error = exchange(
                &device,
                &cap,
                [1, 0, 184],
                &[],
                0,
                &|| Ok(()),
                &mut progress,
            )
            .unwrap_err();
            assert_eq!(native_io_outcome(&error), Some(outcome));
            assert_eq!(device.sent().len(), 1);
            assert_eq!(progress.0, outcome != NativeIoOutcome::NotCalled);
            assert_eq!(
                progress.1,
                outcome == NativeIoOutcome::Returned { byte_count: 91 }
            );
        }
        let device = Mock::new([reply(&[1])]).interrupt_next_read();
        let mut progress = (false, false);
        let error = exchange(
            &device,
            &cap,
            [1, 0, 184],
            &[],
            0,
            &|| Ok(()),
            &mut progress,
        )
        .unwrap_err();
        assert_eq!(
            crate::feature_exchange::acknowledged_error(&error),
            Some(&[1][..])
        );
        assert_eq!(device.sent().len(), 1);
        assert_eq!(progress, (true, true));
    }
}

#[test]
fn finish_failure_retains_the_led_ack_without_confirming_a_configuration_or_quality() {
    let device = Simulated::new([Reply::Data(2, vec![7, 8])]);
    let leased = crate::feature_exchange::tests::ExchangeProbe::failing_finish(&device);
    let report = execute(
        239,
        &leased,
        &cap(239),
        &Change::SetRaw(vec![0, 1, 2]),
        next(),
        || Ok(()),
    )
    .unwrap();
    assert!(report.error.as_deref().unwrap().contains("release failed"));
    assert!(report.setter_attempted && report.setter_sent && report.setter_acknowledged);
    assert_eq!(report.acknowledgements.len(), 1);
    assert_eq!(report.acknowledgements[0].data, [7, 8]);
    assert!(report.configuration.is_none() && report.quality.is_none());
    assert!(report.source_quality_update.is_none());
    assert_eq!(device.sent.borrow().len(), 1);
}

#[test]
fn configuration_roundtrip_sorts_source_indexes_and_rejects_invalid_profiles() {
    let configuration: Configuration = serde_json::from_value(serde_json::json!([
        {"index":2,"ledStatus":4},{"index":0,"ledStatus":0},{"index":1,"ledStatus":2}]))
    .unwrap();
    assert_eq!(
        configuration.modes(),
        [Mode::Off, Mode::ConnectionQuality, Mode::DpiStage]
    );
    assert_eq!(
        serde_json::to_value(configuration).unwrap(),
        serde_json::json!([
        {"index":0,"ledStatus":0},{"index":1,"ledStatus":2},{"index":2,"ledStatus":4}])
    );
    for bad in [
        serde_json::json!([]),
        serde_json::json!([
        {"index":0,"ledStatus":1},{"index":0,"ledStatus":1},{"index":2,"ledStatus":3}]),
        serde_json::json!([{"index":0,"ledStatus":5},{"index":1,"ledStatus":1},{"index":2,"ledStatus":3}]),
    ] {
        assert!(serde_json::from_value::<Configuration>(bad).is_err());
    }
}

#[test]
fn source_arrays_keep_shape_unknown_values_and_metadata_before_fixed_packet_copy() {
    use serde_json::json;
    let input = json!([
        {"index":2,"ledStatus":259,"extra":{"kept":true}},
        {"index":0,"ledStatus":99},
        {"index":0,"ledStatus":-1},
        {"index":1,"ledStatus":"0x102"}
    ]);
    let (sorted, bytes) = prepare_source_configuration(&input).unwrap();
    assert_eq!(bytes, [99, 255, 2, 3]);
    assert_eq!(sorted.as_array().unwrap().len(), 4);
    assert_eq!(sorted[0]["ledStatus"], 99);
    assert_eq!(sorted[1]["ledStatus"], -1);
    assert_eq!(sorted[3]["extra"], json!({"kept":true}));
    let device = Simulated::new([Reply::Data(2, vec![])]);
    let report = execute(
        190,
        &device,
        &cap(190),
        &Change::SetRaw(bytes),
        next(),
        || Ok(()),
    )
    .unwrap();
    assert!(report.setter_acknowledged && report.error.is_none());
    assert_eq!(&device.sent.borrow()[0][9..13], &[99, 255, 2, 0]);
    assert_eq!(input[0]["index"], 2); // The caller retains its unsorted UI value.
}

#[test]
fn source_short_arrays_zero_fill_and_js_uint8_coercion_preserves_throw_boundaries() {
    use serde_json::json;
    for (input, expected) in [
        (json!([]), vec![]),
        (json!([{"index":0,"ledStatus":true}]), vec![1]),
        (
            json!([{"index":0,"ledStatus":null},{"index":1}]),
            vec![0, 0],
        ),
        (
            json!([{"index":0,"ledStatus":"\u{feff}257.9\u{feff}"},{"index":1,"ledStatus":-258.9}]),
            vec![1, 254],
        ),
        (
            json!([{"index":0,"ledStatus":["0b11"]},{"index":1,"ledStatus":[true]},{"index":2,"ledStatus":"Infinity"}]),
            vec![3, 0, 0],
        ),
    ] {
        let (sorted, bytes) = prepare_source_configuration(&input).unwrap();
        assert_eq!(sorted, input);
        assert_eq!(bytes, expected);
        let device = Simulated::new([Reply::Data(2, vec![])]);
        let report = execute(
            190,
            &device,
            &cap(190),
            &Change::SetRaw(bytes.clone()),
            next(),
            || Ok(()),
        )
        .unwrap();
        assert!(report.setter_acknowledged && report.error.is_none());
        let mut fixed = [0; 3];
        for (slot, byte) in fixed.iter_mut().zip(bytes) {
            *slot = byte;
        }
        assert_eq!(&device.sent.borrow()[0][9..12], &fixed);
    }
    for invalid in [json!(null), json!({}), json!([null])] {
        assert!(prepare_source_configuration(&invalid).is_err());
    }
}

#[test]
fn source_index_sort_is_stable_and_does_not_require_total_order_from_json() {
    use serde_json::json;
    let input = json!([
        {"index":3,"id":"a"},{"index":1,"id":"b"},{"index":1,"id":"c"},
        {"index":2,"id":"d"},{"index":0,"id":"e"}
    ]);
    let (sorted, _) = prepare_source_configuration(&input).unwrap();
    assert_eq!(
        sorted
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["e", "b", "c", "d", "a"]
    );
    // The JS comparator makes missing/object indices compare equal to every
    // number. Preserve all records without feeding that non-total comparator
    // into slice::sort, whose contract permits a panic for this input.
    let input = serde_json::Value::Array(
        (0..128)
            .map(|id| match id % 3 {
                0 => json!({"index":127-id,"id":id}),
                1 => json!({"id":id}),
                _ => json!({"index":{},"id":id}),
            })
            .collect(),
    );
    let (sorted, bytes) = prepare_source_configuration(&input).unwrap();
    let mut ids = sorted
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_u64().unwrap())
        .collect::<Vec<_>>();
    ids.sort_unstable();
    assert_eq!(ids, (0..128).collect::<Vec<_>>());
    assert_eq!(bytes, vec![0; 128]);
}

#[test]
fn initialization_all_three_receivers_sets_profile_then_gets_quality_without_ui_notification() {
    for product in [190, 229, 239] {
        let device = Simulated::new([Reply::Data(2, vec![4, 4, 4]), Reply::Data(2, vec![1])]);
        let configuration = Configuration::default();
        let report = execute(
            product,
            &device,
            &cap(product),
            &Change::Initialize(configuration),
            next(),
            || Ok(()),
        )
        .unwrap();
        assert!(report.error.is_none());
        assert!(report.setter_acknowledged);
        assert!(report.configuration.is_none());
        assert_eq!(report.quality, Some(Quality::Red));
        assert!(report.source_quality_update.is_none());
        let sent = device.sent.borrow();
        assert_eq!(sent.len(), 2);
        assert_eq!(
            *device.get_ids.borrow(),
            vec![if product == 190 { 0 } else { 3 }; 2]
        );
        assert_eq!(&sent[0][6..12], &[3, 7, 21, 2, 1, 3]);
        assert_eq!(&sent[1][6..10], &[1, 7, 150, 0]);
        assert!(sent[0][12..89].iter().all(|byte| *byte == 0));
        for (index, packet) in sent.iter().enumerate() {
            assert_eq!(packet.len(), 91);
            assert_eq!(packet[0], 0);
            assert_eq!(packet[2], index as u8);
            assert_eq!(
                packet[89],
                packet[3..89].iter().fold(0, |sum, byte| sum ^ byte)
            );
        }
    }
}
#[test]
fn setters_do_not_get_or_treat_echo_as_observed_configuration() {
    let device = Simulated::new([Reply::Data(2, vec![0, 0, 0])]);
    let configuration = Configuration::new([Mode::DpiStage, Mode::Off, Mode::BatteryLevel]);
    let report = execute(
        229,
        &device,
        &cap(229),
        &Change::Set(configuration),
        next(),
        || Ok(()),
    )
    .unwrap();
    assert!(report.error.is_none());
    assert!(report.setter_attempted && report.setter_sent && report.setter_acknowledged);
    assert_eq!(device.sent.borrow().len(), 1);
    assert!(report.configuration.is_none());
    assert!(report.quality.is_none());
    assert_eq!(report.acknowledgements[0].data, vec![0, 0, 0]);
}
#[test]
fn actual_getter_is_separate_and_never_initializes_profile_ui() {
    let device = Simulated::new([Reply::Data(2, vec![4, 3, 0])]);
    let report = execute(
        239,
        &device,
        &cap(239),
        &Change::ReadConfiguration,
        next(),
        || Ok(()),
    )
    .unwrap();
    assert!(!report.setter_attempted);
    assert_eq!(
        report.configuration.unwrap().modes(),
        [Mode::DpiStage, Mode::PollingRate, Mode::Off]
    );
    assert!(report.source_quality_update.is_none());
    assert_eq!(&device.sent.borrow()[0][6..12], &[3, 7, 149, 0, 0, 0]);
}
#[test]
fn initialize_preserves_real_set_ack_when_later_quality_fails() {
    let device = Simulated::new([Reply::Data(2, vec![]), Reply::GetterError]);
    let report = execute(
        190,
        &device,
        &cap(190),
        &Change::Initialize(Configuration::default()),
        next(),
        || Ok(()),
    )
    .unwrap();
    assert!(report.setter_acknowledged);
    assert!(report.error.is_some());
    assert!(report.quality.is_none());
    assert_eq!(report.acknowledgements.len(), 1);
    assert_eq!(device.sent.borrow().len(), 4);
    let mut device = Simulated::new([]);
    device.fail_send = true;
    let report = execute(
        190,
        &device,
        &cap(190),
        &Change::Set(Configuration::default()),
        next(),
        || Ok(()),
    )
    .unwrap();
    assert!(report.setter_attempted && !report.setter_sent && !report.setter_acknowledged);
    assert!(report.error.is_some());
    assert_eq!(device.sent.borrow().len(), 3);
}
#[test]
fn retry_keeps_same_transaction_and_source_busy_getter_semantics() {
    for replies in [
        vec![Reply::Mismatch, Reply::Data(2, vec![])],
        vec![Reply::GetterError, Reply::Data(2, vec![])],
        vec![
            Reply::Data(1, vec![]),
            Reply::GetterError,
            Reply::Data(2, vec![]),
        ],
        vec![Reply::Data(3, vec![]), Reply::Data(2, vec![])],
    ] {
        let device = Simulated::new(replies);
        let report = execute(
            190,
            &device,
            &cap(190),
            &Change::Set(Configuration::default()),
            next(),
            || Ok(()),
        )
        .unwrap();
        assert!(report.error.is_none());
        let sent = device.sent.borrow();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0], sent[1]);
    }
    let device = Simulated::new([Reply::Data(5, vec![])]);
    let report = execute(
        190,
        &device,
        &cap(190),
        &Change::Set(Configuration::default()),
        next(),
        || Ok(()),
    )
    .unwrap();
    assert!(report.error.is_some());
    assert_eq!(device.sent.borrow().len(), 1);
}
#[test]
fn host_send_failure_retries_out_without_get_and_identity_failure_stops_retry() {
    let device = Simulated::new([Reply::Data(2, vec![])]);
    device.fail_sends.set(1);
    let report = execute(
        229,
        &device,
        &cap(229),
        &Change::Set(Configuration::default()),
        next(),
        || Ok(()),
    )
    .unwrap();
    assert!(report.error.is_none() && report.setter_sent && report.setter_acknowledged);
    assert_eq!(device.sent.borrow().len(), 2);
    assert_eq!(*device.get_ids.borrow(), vec![3]);
    let device = Simulated::new([Reply::GetterError, Reply::Data(2, vec![])]);
    let report = execute(
        239,
        &device,
        &cap(239),
        &Change::Set(Configuration::default()),
        next(),
        || {
            ensure!(
                device.get_ids.borrow().is_empty(),
                "physical instance changed after GET"
            );
            Ok(())
        },
    )
    .unwrap();
    assert!(report.error.unwrap().contains("physical instance changed"));
    assert_eq!(device.sent.borrow().len(), 1);
    assert!(!report.setter_acknowledged);
}
#[test]
fn protocol_serial_preserves_printable_spaces_and_distinguishes_empty_from_failure() {
    for (bytes, ascii, runtime) in [
        (
            vec![0, 31, 32, b'R', b'Z', 126, 127, 255, 32],
            " RZ~ ",
            " RZ~ ",
        ),
        (vec![0, 31, 127, 255], "", "NOSERIALNUMBER"),
    ] {
        let device = Simulated::new([Reply::Data(2, bytes)]);
        let report = execute(
            229,
            &device,
            &cap(229),
            &Change::ReadSerialNumber,
            next(),
            || Ok(()),
        )
        .unwrap();
        assert!(report.error.is_none());
        let serial = SerialObservation::from_report(&report).unwrap();
        assert_eq!(serial.ascii.as_deref(), Some(ascii));
        assert_eq!(serial.runtime_serial_number, runtime);
        assert_eq!(&device.sent.borrow()[0][6..9], &GET_SERIAL_NUMBER);
        assert!(
            capability(229)
                .unwrap()
                .initialization_allowed(230, &serial.runtime_serial_number)
                .unwrap()
        );
    }
    let device = Simulated::new([Reply::GetterError]);
    let report = execute(
        190,
        &device,
        &cap(190),
        &Change::ReadSerialNumber,
        next(),
        || Ok(()),
    )
    .unwrap();
    let serial = SerialObservation::from_report(&report).unwrap();
    assert!(
        serial.ascii.is_none() && serial.runtime_serial_number.is_empty() && serial.error.is_some()
    );
    assert!(
        !capability(190)
            .unwrap()
            .initialization_allowed(191, &serial.runtime_serial_number)
            .unwrap()
    );
    assert!(
        capability(190)
            .unwrap()
            .initialization_allowed(190, &serial.runtime_serial_number)
            .unwrap()
    );
}
#[test]
fn protocol_array_boundary_is_independent_of_three_led_ui_validation() {
    for (input, expected) in [
        (vec![], [0, 0, 0]),
        (vec![255], [255, 0, 0]),
        (vec![4, 7, 2, 1], [4, 7, 2]),
    ] {
        let device = Simulated::new([Reply::Data(2, vec![])]);
        let report = execute(
            239,
            &device,
            &cap(239),
            &Change::SetRaw(input),
            next(),
            || Ok(()),
        )
        .unwrap();
        assert!(report.error.is_none() && report.setter_acknowledged);
        assert_eq!(&device.sent.borrow()[0][9..12], &expected);
    }
    for data in [vec![], vec![255], vec![4, 3, 2, 1]] {
        let device = Simulated::new([Reply::Data(2, data.clone())]);
        let report = execute(
            239,
            &device,
            &cap(239),
            &Change::ReadConfiguration,
            next(),
            || Ok(()),
        )
        .unwrap();
        assert!(report.error.is_none() && report.configuration.is_none());
        assert_eq!(report.raw_configuration, Some(data));
    }
}
#[test]
fn battery_commands_keep_the_source_two_byte_payload_and_second_byte_parsers() {
    for (change, command, data, level, status) in [
        (
            Change::ReadBatteryLevel,
            GET_BATTERY_LEVEL,
            vec![0, 128],
            Some(50),
            None,
        ),
        (
            Change::ReadChargingStatus,
            GET_CHARGING_STATUS,
            vec![0, 4],
            None,
            Some("FastCharging"),
        ),
        (
            Change::ReadChargingStatus,
            GET_CHARGING_STATUS,
            vec![0, 255],
            None,
            None,
        ),
    ] {
        let device = Simulated::new([Reply::Data(2, data)]);
        let report = execute(239, &device, &cap(239), &change, next(), || Ok(())).unwrap();
        assert!(report.error.is_none());
        assert_eq!(report.battery_level, level);
        assert_eq!(report.charging_status.as_deref(), status);
        assert_eq!(&device.sent.borrow()[0][6..9], &command);
        assert_eq!(&device.sent.borrow()[0][9..11], &[0, 0]);
    }
}
#[test]
fn battery_events_trigger_actual_queries_and_keep_raw_levels_out_of_ui() {
    for id in [5, 9] {
        for kind in [0, 1, 2, 3, 7, 8, 255] {
            assert_eq!(
                notification(&[id, 49, 255, 4, 0, kind]).unwrap(),
                Some(Notification::BatteryChanged {
                    external_power_removed: true
                })
            );
        }
        assert_eq!(
            notification(&[id, 49, 255, 4, 1, 2]).unwrap(),
            Some(Notification::BatteryChanged {
                external_power_removed: false
            })
        );
        assert_eq!(
            notification(&[id, 12, 1]).unwrap(),
            Some(Notification::ExternalPowerChanged { connected: true })
        );
        assert_eq!(
            notification(&[id, 12, 0]).unwrap(),
            Some(Notification::ExternalPowerChanged { connected: false })
        );
        assert!(notification(&[id, 12, 7]).unwrap().is_none());
        assert_eq!(
            notification(&[id, 9, 3, 1]).unwrap(),
            Some(Notification::WirelessChanged {
                connected: true,
                product_id: None,
            })
        );
        assert_eq!(
            notification(&[id, 9, 2, 1]).unwrap(),
            Some(Notification::WirelessChanged {
                connected: false,
                product_id: None,
            })
        );
        assert_eq!(
            notification(&[id, 53, 3, 1, 0, 191]).unwrap(),
            Some(Notification::WirelessChanged {
                connected: true,
                product_id: Some(191),
            })
        );
        assert!(notification(&[id, 53, 3, 2, 0, 191]).unwrap().is_none());
    }
    assert!(notification(&[5, 49, 255, 1]).is_err());
}
#[test]
fn wireless_notifications_keep_legacy_undefined_and_v2_big_endian_product_identity() {
    for report in [5, 9] {
        assert_eq!(
            notification(&[report, 9, 3, 2, 0x12, 0x34]).unwrap(),
            Some(Notification::WirelessChanged {
                connected: true,
                product_id: None,
            })
        );
        for (bytes, expected) in [
            (vec![report, 53, 3, 1, 0x12, 0x34], 0x1234),
            (vec![report, 53, 3, 1, 0, 0], 0),
            (vec![report, 53, 3, 1], 0),
            (vec![report, 53, 3, 1, 0x12], 0x1200),
        ] {
            assert_eq!(
                notification(&bytes).unwrap(),
                Some(Notification::WirelessChanged {
                    connected: true,
                    product_id: Some(expected),
                })
            );
        }
        // Pure_Chroma_device=8 has a non-MOUSE typeDesc and is rejected;
        // unknown enum values have no typeDesc and pass the original gate.
        assert!(notification(&[report, 53, 3, 8, 0, 191]).unwrap().is_none());
        assert_eq!(
            notification(&[report, 53, 2, 255, 0, 240]).unwrap(),
            Some(Notification::WirelessChanged {
                connected: false,
                product_id: Some(240),
            })
        );
    }
}
#[test]
fn cancellation_and_other_physical_receivers_never_send() {
    let device = Simulated::new([]);
    assert!(
        execute(
            190,
            &device,
            &cap(190),
            &Change::Set(Configuration::default()),
            next(),
            || anyhow::bail!("canceled")
        )
        .is_err()
    );
    assert!(capability(190).unwrap().transport(190).is_ok());
    assert!(capability(190).unwrap().transport(230).is_err());
    assert!(!capability(190).unwrap().accepts(191, Some(191)));
    assert!(device.sent.borrow().is_empty());
}
#[test]
fn quality_notifications_accept_only_current_report_selector_and_enum_and_cleanup_on_cancel() {
    for report in [5, 9] {
        for value in 1..=4 {
            assert_eq!(
                quality_event(&[report, 86, value]).unwrap().unwrap().code(),
                value
            );
        }
    }
    for raw in [&[0, 86, 4][..], &[5, 85, 4], &[5, 86, 0], &[5, 86, 5]] {
        assert!(quality_event(raw).unwrap().is_none());
    }
    assert!(quality_event(&[5, 86]).is_err());
    struct Events<'a> {
        count: usize,
        canceled: &'a Cell<bool>,
        cleaned: bool,
    }
    impl HardwareEvents for Events<'_> {
        fn begin(&mut self) -> anyhow::Result<()> {
            Ok(())
        }
        fn receive(&mut self, _: Duration) -> anyhow::Result<Option<Vec<u8>>> {
            self.count += 1;
            if self.count == 2 {
                self.canceled.set(true);
            }
            Ok(Some(vec![5, 86, self.count as u8]))
        }
        fn finish(&mut self) -> anyhow::Result<()> {
            self.cleaned = true;
            Ok(())
        }
    }
    let canceled = Cell::new(false);
    let mut events = Events {
        count: 0,
        canceled: &canceled,
        cleaned: false,
    };
    let mut published = vec![];
    assert!(
        monitor(
            &mut events,
            &|| {
                ensure!(!canceled.get(), "canceled");
                Ok(())
            },
            &mut |quality| {
                published.push(quality);
                Ok(())
            }
        )
        .is_err()
    );
    assert!(events.cleaned);
    assert_eq!(published, vec![Quality::Red]);
}
