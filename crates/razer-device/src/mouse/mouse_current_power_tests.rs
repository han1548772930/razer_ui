use super::*;
use std::{
    collections::VecDeque,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

struct Reply {
    status: u8,
    data: Vec<u8>,
}
struct Simulated {
    sent: Mutex<Vec<Vec<u8>>>,
    replies: Mutex<VecDeque<Reply>>,
    canceled: AtomicBool,
    cancel_after_reply: bool,
    set_report_id: u8,
    get_report_id: u8,
    returned_report_id: u8,
}
impl Simulated {
    fn new(data: Vec<Vec<u8>>) -> Self {
        Self {
            sent: Mutex::new(Vec::new()),
            replies: Mutex::new(
                data.into_iter()
                    .map(|data| Reply { status: 2, data })
                    .collect(),
            ),
            canceled: AtomicBool::new(false),
            cancel_after_reply: false,
            set_report_id: 0,
            get_report_id: 0,
            returned_report_id: 0,
        }
    }
    fn framing(mut self, set: u8, get: u8, returned: u8) -> Self {
        self.set_report_id = set;
        self.get_report_id = get;
        self.returned_report_id = returned;
        self
    }
    fn sent(&self) -> Vec<Vec<u8>> {
        self.sent.lock().unwrap().clone()
    }
}
impl FeatureTransport for Simulated {
    fn send_feature(&self, _report: &[u8]) -> anyhow::Result<()> {
        panic!("Power must use the source host collection-sized SET path")
    }
    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        ensure!(report[0] == self.set_report_id, "Wrong power SET Report ID");
        self.sent.lock().unwrap().push(report.to_vec());
        Ok(())
    }
    fn get_feature(&self, _report: &mut [u8]) -> anyhow::Result<usize> {
        panic!("Power must use the source host collection-sized GET path")
    }
    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        ensure!(report[0] == self.get_report_id, "Wrong power GET Report ID");
        let reply = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .context("Missing simulated power reply")?;
        let sent = self.sent.lock().unwrap();
        let sent = sent.last().unwrap();
        report.fill(0);
        report[0] = self.returned_report_id;
        report[1] = reply.status;
        report[2] = sent[2];
        report[6] = reply.data.len() as u8;
        report[7..9].copy_from_slice(&sent[7..9]);
        report[9..9 + reply.data.len()].copy_from_slice(&reply.data);
        if self.cancel_after_reply {
            self.canceled.store(true, Ordering::Release);
        }
        Ok(report.len())
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({"simulated":true})
    }
}
fn transport(product_id: u32) -> DeviceReadCapability {
    let mut cap = capability(product_id)
        .unwrap()
        .transport(product_id, false)
        .unwrap();
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap.max_retry_in = 2;
    cap.max_retry_out = 2;
    cap
}
fn next(prefix: u8) -> impl FnMut() -> anyhow::Result<u8> {
    let mut value = 0;
    move || {
        let result = prefix | value;
        value = (value + 1) % 31;
        Ok(result)
    }
}

#[test]
fn always_read_initialization_only_gets_actual_seconds_without_a_setter() {
    let device = Simulated::new(vec![vec![0, 75]]);
    let report = execute(
        117,
        &device,
        &transport(117),
        &Change::ReadPowerSaving,
        next(0),
        || Ok(()),
    )
    .unwrap();
    assert!(!report.setter_sent);
    assert_eq!(report.time_to_sleep, Some(75));
    assert_eq!(report.acknowledgements.len(), 1);
    let sent = device.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(&sent[0][6..11], &[2, 7, 131, 0, 0]);
}

#[test]
fn truncated_initialization_response_cannot_report_a_device_value() {
    let device = Simulated::new(vec![vec![75]]);
    let error = execute(
        117,
        &device,
        &transport(117),
        &Change::ReadPowerSaving,
        next(0),
        || Ok(()),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("initialization response is truncated")
    );
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn power_saving_minutes_write_big_endian_seconds_then_confirm_readback() {
    let device = Simulated::new(vec![vec![], vec![1, 164]]);
    let cap = transport(117);
    let report = execute(117, &device, &cap, &Change::PowerSaving(7), next(0), || {
        Ok(())
    })
    .unwrap();
    assert!(report.setter_sent);
    assert_eq!(report.time_to_sleep, Some(420));
    let sent = device.sent();
    assert_eq!(sent.len(), 2);
    assert_eq!(&sent[0][6..11], &[2, 7, 3, 1, 164]);
    assert_eq!(&sent[1][6..11], &[2, 7, 131, 0, 0]);
    assert_eq!(sent[0][2], 0);
    assert_eq!(sent[1][2], 1);
    assert_eq!(
        sent[0][89],
        sent[0][3..89].iter().fold(0, |sum, byte| sum ^ byte)
    );
}
#[test]
fn sleep_ack_with_wrong_readback_never_confirms_a_write() {
    let device = Simulated::new(vec![vec![], vec![1, 163]]);
    let error = execute(
        117,
        &device,
        &transport(117),
        &Change::PowerSaving(7),
        next(0),
        || Ok(()),
    )
    .unwrap_err();
    assert!(error.to_string().contains("readback does not match"));
    assert_eq!(device.sent().len(), 2);
}
#[test]
fn single_profile_caller_seconds_are_not_multiplied_again() {
    for product in [199, 208] {
        let device = Simulated::new(vec![vec![], vec![0, 120]]);
        let report = execute(
            product,
            &device,
            &transport(product),
            &Change::PowerSaving(120),
            next(0),
            || Ok(()),
        )
        .unwrap();
        assert_eq!(report.time_to_sleep, Some(120));
        assert_eq!(&device.sent()[0][9..11], &[0, 120]);
    }
}
#[test]
fn critical_percent_uses_source_ceil_and_floor_for_every_step() {
    for percent in (5..=100u8).step_by(5) {
        let raw = (u16::from(percent) * 255).div_ceil(100) as u8;
        let device = Simulated::new(vec![vec![], vec![raw]]);
        let report = execute(
            117,
            &device,
            &transport(117),
            &Change::LowPower(percent),
            next(0),
            || Ok(()),
        )
        .unwrap();
        assert_eq!(report.critical_percent, Some(percent));
        assert_eq!(&device.sent()[0][6..10], &[1, 7, 1, raw]);
    }
}
#[test]
fn unmounted_low_battery_field_cannot_submit_a_current_product_write() {
    let device = Simulated::new(vec![]);
    assert!(
        execute(
            117,
            &device,
            &transport(117),
            &Change::LowBattery(5),
            next(0),
            || Ok(())
        )
        .is_err()
    );
    assert!(device.sent().is_empty());
}
#[test]
fn pid98_low_battery_effects_uses_device_info_bounds() {
    // Current PID 98 DeviceInfo: minLBE 3, maxLBE 10, StepLBE 1.
    for value in 3..=10 {
        assert!(validate_change(98, &Change::LowBattery(value)).is_ok());
    }
    for value in [0, 2, 11, 15, 25] {
        assert!(validate_change(98, &Change::LowBattery(value)).is_err());
    }
}
#[test]
fn unchanged_dim_value_only_reads_and_changed_dim_uses_profile_zero_and_brightness_twenty() {
    let change = Change::DimLighting {
        enabled: true,
        minutes: 2,
    };
    let device = Simulated::new(vec![vec![0, 1, 0, 120, 20]]);
    let report = execute(117, &device, &transport(117), &change, next(0), || Ok(())).unwrap();
    assert!(!report.setter_sent);
    assert_eq!(device.sent().len(), 1);
    let device = Simulated::new(vec![vec![0, 0, 0, 60, 40], vec![]]);
    let report = execute(117, &device, &transport(117), &change, next(0), || Ok(())).unwrap();
    assert!(report.setter_sent);
    assert_eq!(device.sent().len(), 2);
    assert_eq!(&device.sent()[1][6..14], &[5, 7, 11, 0, 1, 0, 120, 20]);
}
#[test]
fn dim_getter_for_another_profile_cannot_trigger_a_setter() {
    let device = Simulated::new(vec![vec![1, 1, 0, 120, 20]]);
    assert!(
        execute(
            117,
            &device,
            &transport(117),
            &Change::DimLighting {
                enabled: true,
                minutes: 2
            },
            next(0),
            || Ok(())
        )
        .is_err()
    );
    assert_eq!(device.sent().len(), 1);
}
#[test]
fn battery_curve_source_waits_for_ack_without_inventing_getter() {
    for value in 0..=2 {
        let device = Simulated::new(vec![vec![]]);
        let cap = transport(184);
        let report = execute(
            184,
            &device,
            &cap,
            &Change::BatteryType(value),
            next(0),
            || Ok(()),
        )
        .unwrap();
        assert!(report.setter_sent);
        assert_eq!(report.acknowledgements.len(), 1);
        assert_eq!(device.sent().len(), 1);
        assert_eq!(device.sent()[0][2], 0);
        assert_eq!(&device.sent()[0][6..10], &[1, 7, 20, value]);
    }
}
#[test]
fn canceled_response_prevents_following_getter_and_does_not_confirm_setter() {
    let mut device = Simulated::new(vec![vec![]]);
    device.cancel_after_reply = true;
    let error = execute(
        117,
        &device,
        &transport(117),
        &Change::PowerSaving(7),
        next(0),
        || {
            ensure!(
                !device.canceled.load(Ordering::Acquire),
                "Canceled simulated request"
            );
            Ok(())
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("Canceled"));
    assert_eq!(device.sent().len(), 1);
}
#[test]
fn invalid_field_ranges_and_terminal_rejection_do_not_fabricate_success() {
    for change in [
        Change::BatteryType(0),
        Change::PowerSaving(0),
        Change::PowerSaving(16),
        Change::LowPower(7),
    ] {
        let device = Simulated::new(vec![]);
        assert!(execute(117, &device, &transport(117), &change, next(0), || Ok(())).is_err());
        assert!(device.sent().is_empty());
    }
    let device = Simulated::new(vec![]);
    device.replies.lock().unwrap().push_back(Reply {
        status: 5,
        data: vec![],
    });
    assert!(
        execute(
            184,
            &device,
            &transport(184),
            &Change::BatteryType(2),
            next(0),
            || Ok(())
        )
        .is_err()
    );
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn all_current_power_factories_send_and_get_with_their_physical_source_framing() {
    // Current mounted DeviceInfo + rzDeviceType registrations. The original
    // ordinary sender includes reportId:0; DualLinkMouse omits it. The host's
    // SET nullish fallback and GET truthy fallback differ on interface3.
    let products = [
        (89, 90, 0, false),
        (98, 98, 0, false),
        (112, 111, 0, false),
        (115, 114, 0, false),
        (117, 2308, 0, false),
        (122, 123, 0, false),
        (124, 125, 0, true),
        (128, 119, 0, false),
        (131, 131, 0, false),
        (134, 136, 0, false),
        (143, 144, 0, true),
        (148, 148, 0, true),
        (154, 154, 0, true),
        (156, 156, 0, true),
        (158, 159, 0, false),
        (165, 166, 0, true),
        (167, 168, 0, true),
        (170, 171, 0, true),
        (175, 176, 0, true),
        (180, 180, 0, true),
        (184, 184, 0, true),
        (185, 185, 0, true),
        (190, 191, 0, false),
        (192, 193, 0, true),
        (194, 195, 0, false),
        (196, 197, 0, true),
        (199, 200, 0, false),
        (204, 205, 0, true),
        (208, 209, 0, false),
        (211, 212, 0, true),
        (214, 215, 0, true),
        (218, 219, 0, true),
        (221, 221, 3, false),
        (222, 223, 0, true),
        (224, 224, 0, false),
        (226, 227, 3, true),
        (229, 230, 3, false),
        (231, 232, 3, true),
        (235, 234, 3, true),
        (239, 240, 3, false),
        (2594, 2593, 0, false),
        (45066, 45067, 0, true),
    ];
    for (product, dongle, interface, dual) in products {
        let source = capability(product).unwrap();
        for physical in std::collections::BTreeSet::from([product, dongle]) {
            let mut cap = source.transport(physical, false).unwrap();
            let radio = dual && physical == dongle;
            assert_eq!(
                cap.source_class,
                if radio {
                    "rzDevice25DualLinkMouse"
                } else {
                    "rzDevice25"
                }
            );
            assert_eq!(
                cap.transaction_prefix, 0,
                "product {product}, physical {physical}"
            );
            assert_eq!(cap.claim_interface, interface);
            cap.sleep_between_out_ms = 0;
            cap.sleep_between_out_in_ms = 0;
            cap.sleep_between_in_ms = 0;
            let set = if radio { interface } else { 0 };
            let device = Simulated::new(vec![vec![], vec![0, 120]]).framing(set, interface, 73);
            let change = Change::PowerSaving(if [199, 208].contains(&product) { 120 } else { 2 });
            let result = execute(product, &device, &cap, &change, next(0), || Ok(())).unwrap();
            assert!(result.setter_sent);
            assert_eq!(result.time_to_sleep, Some(120));
            let sent = device.sent();
            assert_eq!(sent.len(), 2);
            assert_eq!(
                (sent[0][0], sent[0][2], sent[1][0], sent[1][2]),
                (set, 0, set, 1)
            );
            assert_eq!(&sent[0][6..11], &[2, 7, 3, 0, 120]);
            assert_eq!(&sent[1][6..11], &[2, 7, 131, 0, 0]);
        }
    }
}

#[test]
fn ordinary_mika_and_duallink_preserve_exact_factory_retry_overrides() {
    let ordinary = capability(224).unwrap().transport(224, false).unwrap();
    assert_eq!(
        (
            ordinary.sleep_between_out_ms,
            ordinary.sleep_between_out_in_ms,
            ordinary.sleep_between_in_ms
        ),
        (10, 25, 10)
    );
    assert_eq!((ordinary.max_retry_in, ordinary.max_retry_out), (10, 20));
    for (product, dongle) in [(117, 2308), (2594, 2593)] {
        let source = capability(product).unwrap();
        let wired = source.transport(product, false).unwrap();
        let radio = source.transport(dongle, false).unwrap();
        assert_eq!(
            (
                wired.max_retry_in,
                wired.max_retry_out,
                wired.sleep_between_out_ms
            ),
            (10, 20, 5)
        );
        assert_eq!((radio.max_retry_in, radio.max_retry_out), (20, 20));
        assert_eq!(
            (
                radio.sleep_between_out_ms,
                radio.sleep_between_out_in_ms,
                radio.sleep_between_in_ms
            ),
            (25, 25, 25)
        );
        assert_eq!(radio.source_class, "rzDevice25");
        assert!(!source.accepts(662, Some(product)));
        assert!(source.transport(662, true).is_err());
    }
    let dual = capability(226).unwrap().transport(227, false).unwrap();
    assert_eq!((dual.max_retry_in, dual.max_retry_out), (10, 20));
    assert_eq!(
        (
            dual.sleep_between_out_ms,
            dual.sleep_between_out_in_ms,
            dual.sleep_between_in_ms
        ),
        (25, 25, 25)
    );
}

#[test]
fn dual_mouse_on_a_keyboard_master_keeps_mouse_counter_and_uses_master_interface() {
    let source = capability(226).unwrap();
    assert!(source.accepts(662, Some(226)));
    let mut cap = source.transport(662, true).unwrap();
    assert_eq!(cap.source_class, "rzDevice25DualLinkMouse");
    assert_eq!(cap.transaction_prefix, 0);
    assert_eq!((cap.report_id, cap.get_report_id()), (2, 2));
    assert_eq!(cap.direct_pids, [662]);
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    let device = Simulated::new(vec![vec![0, 75]]).framing(2, 2, 2);
    let result = execute(
        226,
        &device,
        &cap,
        &Change::ReadPowerSaving,
        next(0),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(result.time_to_sleep, Some(75));
    assert_eq!(device.sent()[0][2], 0);
    assert!(source.transport(9999, false).is_err());
    assert!(source.transport(228, false).is_err());
}
