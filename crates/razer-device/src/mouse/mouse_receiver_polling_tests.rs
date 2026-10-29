//! Scripted Feature replies: no operating-system, vendor or device calls.
use super::*;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

enum Reply {
    Data(Vec<u8>),
    Busy,
    Stale,
    Unsupported,
    Short,
    DifferentPrefix(Vec<u8>),
}

struct Transport {
    inputs: RefCell<VecDeque<Reply>>,
    outputs: RefCell<Vec<Vec<u8>>>,
    reads: Cell<usize>,
}

impl Transport {
    fn new(replies: impl IntoIterator<Item = Reply>) -> Self {
        Self {
            inputs: RefCell::new(replies.into_iter().collect()),
            outputs: RefCell::new(Vec::new()),
            reads: Cell::new(0),
        }
    }
}

impl FeatureTransport for Transport {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        self.outputs.borrow_mut().push(report.to_vec());
        Ok(())
    }

    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        assert_eq!(
            report[0],
            self.outputs.borrow().last().unwrap()[0],
            "GET must request the source Report ID"
        );
        let next = self
            .inputs
            .borrow_mut()
            .pop_front()
            .expect("unexpected GET");
        report.copy_from_slice(self.outputs.borrow().last().expect("GET must follow SET"));
        report[1] = 2;
        match &next {
            Reply::Data(data) | Reply::DifferentPrefix(data) => {
                report[6] = data.len() as u8;
                report[9..9 + data.len()].copy_from_slice(data);
                if matches!(next, Reply::DifferentPrefix(_)) {
                    report[0] ^= 1;
                }
            }
            Reply::Busy => report[1] = 1,
            Reply::Stale => report[2] ^= 1,
            Reply::Unsupported => report[1] = 5,
            Reply::Short => {}
        }
        self.reads.set(self.reads.get() + 1);
        Ok(report.len() - usize::from(matches!(next, Reply::Short)))
    }

    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

fn route(physical: u32) -> ReceiverRead {
    route_for(physical, DeviceReadKind::Polling)
}

fn route_for(physical: u32, kind: DeviceReadKind) -> ReceiverRead {
    let mut route = ReceiverRead::new(PRODUCT_ID, physical, Some(PEER_PRODUCT_ID), kind).unwrap();
    route.transport.sleep_between_out_ms = 0;
    route.transport.sleep_between_out_in_ms = 0;
    route.transport.sleep_between_in_ms = 0;
    route.receiver.sleep_between_out_ms = 0;
    route.receiver.sleep_between_out_in_ms = 0;
    route.receiver.sleep_between_in_ms = 0;
    route
}

fn online() -> Reply {
    Reply::Data(vec![1, 1, 0, 183])
}

fn transactions() -> impl FnMut() -> anyhow::Result<u8> {
    let mut next = 5;
    move || {
        let result = next;
        next += 1;
        Ok(result)
    }
}

fn stage_data() -> Vec<u8> {
    vec![1, 2, 2, 1, 1, 144, 1, 144, 0, 0, 2, 7, 208, 7, 208, 0, 0]
}

fn stage_draft() -> crate::mouse_dpi_stages::DpiStagesDraft {
    use crate::mouse_dpi_stages::{DpiStage, DpiStagesDraft};
    DpiStagesDraft {
        enabled: true,
        active_stage: 2,
        stages: [400, 2000]
            .into_iter()
            .map(|dpi| DpiStage {
                x: dpi,
                y: dpi,
                visible: true,
                independent: false,
            })
            .collect(),
    }
}

#[test]
fn final_peer_failure_retains_the_idle_ack_and_readback_as_failed_request_diagnostics() {
    let transport = Transport::new([
        online(),
        Reply::Data(vec![7, 8]),
        Reply::Data(vec![0, 120]),
        Reply::Data(vec![1, 0, 0, 183]),
    ]);
    let error = route_for(183, DeviceReadKind::Idle)
        .apply_idle(&transport, 120, transactions(), || Ok(()))
        .err()
        .expect("a changed peer cannot publish the completed task as current");
    let progress = error
        .downcast_ref::<ObservationFailure<crate::device_writes::DeviceWriteResult>>()
        .unwrap();
    assert_eq!(progress.receiver_before, [(183, 1)]);
    assert_eq!(
        progress.receiver_after.as_deref(),
        Some([(183, 0)].as_slice())
    );
    assert_eq!(progress.reading.acknowledged.as_ref().unwrap().data, [7, 8]);
    assert!(matches!(
        progress.reading.observed,
        Some(DeviceReadValue::Idle { raw_time: 120 })
    ));
    assert_eq!(transport.outputs.borrow().len(), 4);
    assert!(transport.inputs.borrow().is_empty());
    let diagnostic = serde_json::to_vec(progress).unwrap();
    let restored: ObservationFailure<crate::device_writes::DeviceWriteResult> =
        serde_json::from_slice(&diagnostic).unwrap();
    assert_eq!(restored.reading.acknowledged, progress.reading.acknowledged);
}

#[test]
fn receiver_dpi_stage_get_uses_profile_one_and_preserves_nonzero_report_ids() {
    for physical in [183, 164, 179, 241] {
        let transport = Transport::new([online(), Reply::Data(stage_data()), online()]);
        let result = route_for(physical, DeviceReadKind::Dpi)
            .read_dpi_stages(&transport, transactions(), || Ok(()))
            .unwrap();
        assert_eq!(
            (result.reading.profile_id, result.reading.active_stage),
            (1, 2)
        );
        assert_eq!(result.reading.records[1].x, 2000);
        let reports = transport.outputs.borrow();
        assert_eq!(reports.len(), 3);
        assert_eq!(&reports[1][6..10], &[80, 4, 134, 1]);
        assert!(
            reports
                .iter()
                .all(|report| report[0] == if physical == 241 { 3 } else { 0 })
        );
        assert!(reports.iter().all(|report| report[2] < 31));
    }
}

#[test]
fn receiver_dpi_stage_write_ack_readback_and_peer_check_share_one_transport() {
    for physical in [183, 179, 241] {
        let transport = Transport::new([
            online(),
            Reply::Data(vec![]),
            Reply::Data(stage_data()),
            online(),
        ]);
        let result = route_for(physical, DeviceReadKind::Dpi)
            .apply_dpi_stages(&transport, &stage_draft(), transactions(), || Ok(()))
            .unwrap();
        assert!(result.reading.verified);
        assert_eq!(result.reading.acknowledged.response_bytes, 0);
        assert_eq!(result.reading.observed.records[1].x, 2000);
        let reports = transport.outputs.borrow();
        assert_eq!(reports.len(), 4);
        assert_eq!(&reports[1][6..13], &[17, 4, 6, 1, 2, 2, 0]);
        assert_eq!(&reports[2][6..10], &[80, 4, 134, 1]);
        assert_eq!(&reports[3][6..9], &[80, 0, 191]);
    }
}

#[test]
fn receiver_dpi_stage_cancel_after_ack_never_confirms_or_starts_readback() {
    let transport = Transport::new([online(), Reply::Data(vec![])]);
    let result = route_for(241, DeviceReadKind::Dpi).apply_dpi_stages(
        &transport,
        &stage_draft(),
        transactions(),
        || {
            ensure!(transport.reads.get() < 2, "scope canceled after setter ACK");
            Ok(())
        },
    );
    assert!(result.is_err());
    assert_eq!(transport.outputs.borrow().len(), 2);
}

fn hz(observation: &Observation) -> u32 {
    match observation.reading {
        DeviceReadValue::Polling { hz } => hz,
        _ => panic!("unexpected observation kind"),
    }
}

#[test]
fn own_receiver_reads_profile_one_between_two_live_peer_observations() {
    let transport = Transport::new([online(), Reply::Data(vec![1, 1]), online()]);
    let route = route(183);
    let result = route.read(&transport, transactions(), || Ok(())).unwrap();
    assert_eq!(hz(&result), 1000);
    assert_eq!(route.method(), "getProfilePollingRate");
    assert_eq!(result.receiver_before, [(183, 1)]);
    assert_eq!(result.receiver_after, [(183, 1)]);
    let reports = transport.outputs.borrow();
    assert_eq!(reports.len(), 3);
    assert_eq!(&reports[0][6..9], &[80, 0, 191]);
    assert_eq!(&reports[1][6..11], &[2, 0, 142, 1, 0]);
    assert_eq!(&reports[2][6..9], &[80, 0, 191]);
    for (index, report) in reports.iter().enumerate() {
        assert_eq!(report.len(), 91);
        assert_eq!(report[0], 0);
        assert_eq!(report[2], 5 + index as u8);
        assert_eq!(report[89], report[3..89].iter().fold(0, |a, b| a ^ b));
    }
}

#[test]
fn uma_and_zia_use_high_speed_codes_in_the_mouse_transaction_namespace() {
    for physical in [164, 179] {
        let transport = Transport::new([online(), Reply::Data(vec![1, 1]), online()]);
        let route = route(physical);
        let result = route.read(&transport, transactions(), || Ok(())).unwrap();
        assert_eq!(hz(&result), 8000);
        assert_eq!(route.method(), "getUSBHighSpeedPollingRate");
        let reports = transport.outputs.borrow();
        assert_eq!(&reports[1][6..11], &[2, 0, 192, 1, 0]);
        assert!(reports.iter().all(|report| report[2] < 31));
    }
}

#[test]
fn another_shared_master_keeps_ordinary_polling_and_its_host_report_id() {
    let transport = Transport::new([online(), Reply::Data(vec![1, 2]), online()]);
    let route = route(241);
    let result = route.read(&transport, transactions(), || Ok(())).unwrap();
    assert_eq!(hz(&result), 500);
    assert_eq!(route.method(), "getProfilePollingRate");
    assert_eq!(route.transport().claim_interface, 3);
    assert!(
        transport
            .outputs
            .borrow()
            .iter()
            .all(|report| report[0] == 3)
    );
}

#[test]
fn polling_write_keeps_the_source_master_cap_setter_and_independent_readback() {
    for (physical, effective_hz, get_command, set_command, old_code, new_code) in [
        (183, 1000, 142, 14, 2, 1),
        (164, 8000, 192, 64, 16, 1),
        (179, 8000, 192, 64, 16, 1),
        (241, 1000, 142, 14, 2, 1),
    ] {
        let transport = Transport::new([
            online(),
            Reply::Data(vec![1, old_code]),
            Reply::Data(vec![]),
            Reply::Data(vec![1, new_code]),
            online(),
        ]);
        let result = route(physical)
            .apply_polling(&transport, 8000, transactions(), || Ok(()))
            .unwrap();

        assert_eq!(result.reading.requested_hz, 8000);
        assert_eq!(result.reading.effective_hz, effective_hz);
        assert_eq!(result.reading.observed_hz, effective_hz);
        assert_eq!(result.reading.previous.hz(), Some(500));
        assert!(result.reading.changed);
        assert_eq!(result.reading.acknowledgement.as_deref(), Some(&[][..]));
        assert_eq!(result.receiver_before, [(183, 1)]);
        assert_eq!(result.receiver_after, [(183, 1)]);
        let reports = transport.outputs.borrow();
        assert_eq!(reports.len(), 5);
        assert_eq!(&reports[1][6..11], &[2, 0, get_command, 1, 0]);
        assert_eq!(&reports[2][6..11], &[2, 0, set_command, 1, new_code]);
        assert_eq!(&reports[3][6..11], &[2, 0, get_command, 1, 0]);
        for (ix, report) in reports.iter().enumerate() {
            assert_eq!(report[0], if physical == 241 { 3 } else { 0 });
            assert_eq!(report[2], 5 + ix as u8);
        }
    }
}

#[test]
fn matching_pre_read_skips_the_setter_but_keeps_the_final_live_peer_check() {
    for (physical, requested_hz, effective_hz, code) in [(183, 8000, 1000, 1), (179, 8000, 8000, 1)]
    {
        let transport = Transport::new([online(), Reply::Data(vec![1, code]), online()]);
        let result = route(physical)
            .apply_polling(&transport, requested_hz, transactions(), || Ok(()))
            .unwrap();

        assert_eq!(result.reading.previous.hz(), Some(effective_hz));
        assert_eq!(result.reading.observed_hz, effective_hz);
        assert!(!result.reading.changed);
        assert!(result.reading.acknowledgement.is_none());
        assert_eq!(transport.outputs.borrow().len(), 3);
        assert_eq!(&transport.outputs.borrow()[2][6..9], &[80, 0, 191]);
    }
}

#[test]
fn failed_pre_read_continues_the_source_setter_without_fabricating_a_previous_rate() {
    for physical in [183, 164, 179, 241] {
        for failed in [
            Reply::Unsupported,
            Reply::Short,
            Reply::Data(vec![2, 1]),
            Reply::Data(vec![1]),
        ] {
            let mut route = route(physical);
            route.transport.max_retry_out = 1;
            route.transport.max_retry_in = 1;
            let high_speed = matches!(physical, 164 | 179);
            let code = if high_speed { 8 } else { 1 };
            let transport = Transport::new([
                online(),
                failed,
                Reply::Data(vec![]),
                Reply::Data(vec![1, code]),
                online(),
            ]);
            let result = route
                .apply_polling(&transport, 1000, transactions(), || Ok(()))
                .unwrap();

            assert!(matches!(
                &result.reading.previous,
                PollingPreRead::Failed { message } if !message.is_empty()
            ));
            assert_eq!(result.reading.previous.hz(), None);
            assert_eq!(result.reading.observed_hz, 1000);
            assert!(result.reading.changed);
            assert!(result.reading.acknowledgement.is_some());
            let reports = transport.outputs.borrow();
            assert_eq!(reports.len(), 5);
            assert_eq!(
                &reports[2][6..11],
                &[2, 0, if high_speed { 64 } else { 14 }, 1, code]
            );
            assert_eq!(&reports[4][6..9], &[80, 0, 191]);
        }
    }
}

#[test]
fn caught_pre_read_failure_cannot_resume_a_canceled_or_replaced_owner() {
    let transport = Transport::new([online(), Reply::Unsupported]);
    let error = route(183)
        .apply_polling(&transport, 1000, transactions(), || {
            ensure!(transport.reads.get() < 2, "owner replaced after pre-read");
            Ok(())
        })
        .err()
        .expect("canceled pre-read must stop the setter");

    assert!(format!("{error:#}").contains("owner replaced after pre-read"));
    assert_eq!(transport.outputs.borrow().len(), 2);
}

#[test]
fn failed_post_read_never_confirms_the_setter_even_after_a_failed_pre_read() {
    for post_read in [
        Reply::Unsupported,
        Reply::Short,
        Reply::Data(vec![2, 1]),
        Reply::Data(vec![1, 2]),
    ] {
        let transport =
            Transport::new([online(), Reply::Unsupported, Reply::Data(vec![]), post_read]);
        let mut route = route(183);
        route.transport.max_retry_out = 1;
        route.transport.max_retry_in = 1;

        assert!(
            route
                .apply_polling(&transport, 1000, transactions(), || Ok(()))
                .is_err()
        );
        let reports = transport.outputs.borrow();
        assert_eq!(reports.len(), 4);
        assert_eq!(&reports[2][6..11], &[2, 0, 14, 1, 1]);
        assert_eq!(&reports[3][6..11], &[2, 0, 142, 1, 0]);
    }
}

#[test]
fn polling_setter_ack_cancellation_or_lost_peer_cannot_become_success() {
    let transport = Transport::new([online(), Reply::Data(vec![1, 2]), Reply::Data(vec![])]);
    assert!(
        route(183)
            .apply_polling(&transport, 1000, transactions(), || {
                ensure!(transport.reads.get() < 3, "canceled after setter ACK");
                Ok(())
            })
            .is_err()
    );
    assert_eq!(transport.outputs.borrow().len(), 3);

    let transport = Transport::new([
        online(),
        Reply::Data(vec![1, 2]),
        Reply::Data(vec![]),
        Reply::Data(vec![1, 1]),
        Reply::Data(vec![1, 0, 0, 183]),
    ]);
    assert!(
        route(183)
            .apply_polling(&transport, 1000, transactions(), || Ok(()))
            .is_err()
    );
    assert_eq!(transport.outputs.borrow().len(), 5);
}

#[test]
fn source_decoders_distinguish_ordinary_and_high_speed_response_values() {
    for (physical, codes) in [
        (
            183,
            vec![
                (1, 1000),
                (2, 500),
                (3, 333),
                (4, 250),
                (5, 200),
                (8, 125),
                (10, 100),
            ],
        ),
        (
            179,
            vec![
                (1, 8000),
                (2, 4000),
                (4, 2000),
                (8, 1000),
                (16, 500),
                (32, 250),
                (64, 125),
            ],
        ),
    ] {
        for (code, expected) in codes {
            let transport = Transport::new([online(), Reply::Data(vec![1, code]), online()]);
            let result = route(physical)
                .read(&transport, transactions(), || Ok(()))
                .unwrap();
            assert_eq!(hz(&result), expected);
        }
    }
}

#[test]
fn unknown_receiver_polling_codes_never_become_observed_source_defaults() {
    // Source j/$ switch defaults are presentation fallbacks. Neither source
    // polling enum assigns these bytes a device-observed rate.
    for physical in [183, 164, 179] {
        for code in [0, 77, 255] {
            let transport = Transport::new([online(), Reply::Data(vec![1, code])]);
            let error = route(physical)
                .read(&transport, transactions(), || Ok(()))
                .err()
                .expect("unknown polling code must not produce an observation");
            assert!(format!("{error:#}").contains(&format!("未知回报率编码：{code}")));
            assert_eq!(transport.outputs.borrow().len(), 2);
        }
    }
}

#[test]
fn unknown_polling_readback_never_confirms_an_acknowledged_receiver_write() {
    for (physical, requested_hz, setter) in [(183, 1000, 14), (164, 8000, 64), (179, 8000, 64)] {
        let transport = Transport::new([
            online(),
            Reply::Data(vec![1, 8]),
            Reply::Data(vec![]),
            Reply::Data(vec![1, 77]),
        ]);
        let error = route(physical)
            .apply_polling(&transport, requested_hz, transactions(), || Ok(()))
            .err()
            .expect("unknown readback must not confirm the requested rate");
        assert!(format!("{error:#}").contains("未知回报率编码：77"));
        let reports = transport.outputs.borrow();
        assert_eq!(reports.len(), 4);
        assert_eq!(&reports[2][6..11], &[2, 0, setter, 1, 1]);
    }
}

#[test]
fn unknown_polling_pre_read_remains_failed_until_the_explicit_write_is_confirmed() {
    for (physical, requested_hz) in [(183, 1000), (164, 8000), (179, 8000)] {
        let transport = Transport::new([
            online(),
            Reply::Data(vec![1, 77]),
            Reply::Data(vec![]),
            Reply::Data(vec![1, 1]),
            online(),
        ]);
        let result = route(physical)
            .apply_polling(&transport, requested_hz, transactions(), || Ok(()))
            .unwrap();
        let PollingPreRead::Failed { message } = &result.reading.previous else {
            panic!("unknown pre-read must remain separate from an observed rate");
        };
        assert!(message.contains("未知回报率编码：77"));
        assert!(result.reading.changed);
        assert_eq!(result.reading.observed_hz, requested_hz);
        assert!(result.reading.acknowledgement.is_some());
        assert_eq!(transport.outputs.borrow().len(), 5);
    }
}

#[test]
fn unaudited_or_mismatched_route_is_rejected_before_communication() {
    for (product, physical, peer) in [
        (180, 183, Some(183)),
        (182, 183, Some(182)),
        (182, 183, None),
        (182, 182, Some(183)),
        (182, 65535, Some(183)),
    ] {
        assert!(ReceiverRead::new(product, physical, peer, DeviceReadKind::Polling).is_err());
    }
}

#[test]
fn missing_offline_ambiguous_or_truncated_peer_never_sends_a_polling_get() {
    for data in [
        vec![0],
        vec![1, 0, 0, 183],
        vec![1, 1, 0, 182],
        vec![2, 1, 0, 183, 1, 0, 183],
        vec![2, 1, 0, 183],
    ] {
        let transport = Transport::new([Reply::Data(data)]);
        assert!(
            route(183)
                .read(&transport, transactions(), || Ok(()))
                .is_err()
        );
        assert_eq!(transport.outputs.borrow().len(), 1);
    }
}

#[test]
fn disappearing_peer_after_polling_discards_the_reading() {
    let transport = Transport::new([
        online(),
        Reply::Data(vec![1, 1]),
        Reply::Data(vec![1, 0, 0, 183]),
    ]);
    assert!(
        route(183)
            .read(&transport, transactions(), || Ok(()))
            .is_err()
    );
    assert_eq!(transport.outputs.borrow().len(), 3);
}

#[test]
fn busy_and_stale_responses_preserve_the_source_retry_transaction() {
    let transport = Transport::new([
        Reply::Busy,
        online(),
        Reply::Stale,
        Reply::Data(vec![1, 2]),
        online(),
    ]);
    let result = route(183)
        .read(&transport, transactions(), || Ok(()))
        .unwrap();
    assert_eq!(hz(&result), 500);
    let reports = transport.outputs.borrow();
    assert_eq!(reports.len(), 4);
    assert_eq!(reports[1], reports[2]);
    assert_eq!(reports[3][2], 7);
}

#[test]
fn failed_truncated_or_other_profile_cannot_be_a_reading() {
    for reply in [
        Reply::Unsupported,
        Reply::Short,
        Reply::Data(vec![2, 1]),
        Reply::Data(vec![1]),
    ] {
        let transport = Transport::new([online(), reply]);
        assert!(
            route(183)
                .read(&transport, transactions(), || Ok(()))
                .is_err()
        );
        assert_eq!(transport.outputs.borrow().len(), 2);
    }
}

#[test]
fn different_returned_prefix_preserves_polling_and_both_peer_checks() {
    let transport = Transport::new([online(), Reply::DifferentPrefix(vec![1, 1]), online()]);
    let observation = route(183)
        .read(&transport, transactions(), || Ok(()))
        .unwrap();
    assert_eq!(hz(&observation), 1000);
    assert_eq!(observation.receiver_before, [(183, 1)]);
    assert_eq!(observation.receiver_after, [(183, 1)]);
    assert_eq!(transport.outputs.borrow().len(), 3);
    assert!(transport.inputs.borrow().is_empty());
}

#[test]
fn canceled_or_replaced_target_stops_before_out_or_after_in() {
    let transport = Transport::new([]);
    assert!(
        route(183)
            .read(&transport, transactions(), || anyhow::bail!("cancelled"))
            .is_err()
    );
    assert!(transport.outputs.borrow().is_empty());

    let transport = Transport::new([online(), Reply::Data(vec![1, 1])]);
    assert!(
        route(183)
            .read(&transport, transactions(), || {
                ensure!(
                    transport.reads.get() < 2,
                    "instance replaced after polling IN"
                );
                Ok(())
            })
            .is_err()
    );
    assert_eq!(transport.outputs.borrow().len(), 2);
}

#[test]
fn every_current_basic_mouse_getter_uses_its_own_fields_on_the_physical_receiver() {
    use serde_json::json;
    let cases = [
        (
            DeviceReadKind::Firmware,
            vec![3, 14, 5, 9],
            [4, 0, 135],
            json!(DeviceReadValue::Firmware {
                version: "3.14.5.9".into()
            }),
        ),
        (
            DeviceReadKind::Battery,
            vec![0, 187],
            [2, 7, 128],
            json!(DeviceReadValue::Battery { percent: 73 }),
        ),
        (
            DeviceReadKind::Charging,
            vec![0, 1],
            [2, 7, 132],
            json!(DeviceReadValue::Charging {
                status: "Charging".into()
            }),
        ),
        (
            DeviceReadKind::Dpi,
            vec![0, 0x12, 0x34, 0xab, 0xcd, 0, 0],
            [7, 4, 133],
            json!(DeviceReadValue::Dpi {
                x: 0x1234,
                y: 0xabcd
            }),
        ),
        (
            DeviceReadKind::Idle,
            vec![0x02, 0x58],
            [2, 7, 131],
            json!(DeviceReadValue::Idle { raw_time: 600 }),
        ),
    ];
    for physical in [183, 164, 179, 241] {
        for (kind, data, command, expected) in &cases {
            let transport = Transport::new([online(), Reply::Data(data.clone()), online()]);
            let result = route_for(physical, *kind)
                .read(&transport, transactions(), || Ok(()))
                .unwrap();
            assert_eq!(json!(result.reading), *expected);
            let reports = transport.outputs.borrow();
            assert_eq!(&reports[1][6..9], command);
            assert_eq!(reports[1][2], 6);
            assert_eq!(reports[1][0], if physical == 241 { 3 } else { 0 });
        }
    }
}

#[test]
fn receiver_idle_runs_set_then_one_get_on_every_master_without_profile_or_pre_read() {
    for physical in [183, 164, 179, 241] {
        for raw_time in [60u16, 300, 900] {
            let transport = Transport::new([
                online(),
                Reply::Data(vec![0xab]),
                Reply::DifferentPrefix(raw_time.to_be_bytes().to_vec()),
                online(),
            ]);
            let result = route_for(physical, DeviceReadKind::Idle)
                .apply_idle(&transport, raw_time, transactions(), || Ok(()))
                .unwrap();
            assert!(result.reading.verified && result.reading.changed);
            assert!(result.reading.previous.is_none());
            assert_eq!(result.reading.acknowledged.unwrap().data, [0xab]);
            assert!(
                matches!(result.reading.observed, Some(DeviceReadValue::Idle { raw_time: actual }) if actual == raw_time)
            );
            let reports = transport.outputs.borrow();
            assert_eq!(reports.len(), 4);
            assert_eq!(&reports[1][6..9], &[2, 7, 3]);
            assert_eq!(&reports[1][9..11], raw_time.to_be_bytes());
            assert_eq!(&reports[2][6..9], &[2, 7, 131]);
            assert_eq!(&reports[2][9..89], [0; 80]);
            for (index, report) in reports.iter().enumerate() {
                assert_eq!(report[0], if physical == 241 { 3 } else { 0 });
                assert_eq!(report[2], 5 + index as u8);
                assert_eq!(
                    report[89],
                    report[3..89].iter().fold(0, |crc, byte| crc ^ byte)
                );
            }
            assert!(transport.inputs.borrow().is_empty());
        }
    }
}

#[test]
fn receiver_idle_rejects_failed_readback_and_cancels_after_ack_before_get() {
    for reply in [
        Reply::Unsupported,
        Reply::Short,
        Reply::Data(vec![0]),
        Reply::Data(vec![0, 60]),
    ] {
        let transport = Transport::new([online(), Reply::Data(vec![]), reply]);
        let mut route = route_for(183, DeviceReadKind::Idle);
        route.transport.max_retry_out = 1;
        route.transport.max_retry_in = 1;
        assert!(
            route
                .apply_idle(&transport, 300, transactions(), || Ok(()))
                .is_err()
        );
        assert_eq!(transport.outputs.borrow().len(), 3);
    }
    let transport = Transport::new([online(), Reply::Data(vec![])]);
    assert!(
        route_for(183, DeviceReadKind::Idle)
            .apply_idle(&transport, 300, transactions(), || {
                ensure!(transport.reads.get() < 2, "canceled after setter ACK");
                Ok(())
            })
            .is_err()
    );
    assert_eq!(transport.outputs.borrow().len(), 2);
    let invalid = Transport::new([]);
    assert!(
        route_for(183, DeviceReadKind::Polling)
            .apply_idle(&invalid, 300, transactions(), || Ok(()))
            .is_err()
    );
    assert!(invalid.outputs.borrow().is_empty());
}
