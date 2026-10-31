use super::*;
use crate::mouse_dpi_stages::DpiStage;
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
struct Mock {
    sent: Mutex<Vec<Vec<u8>>>,
    get_selectors: Mutex<Vec<u8>>,
    replies: Mutex<VecDeque<Reply>>,
    response_prefix: u8,
    canceled: AtomicBool,
    cancel_on_send: bool,
    cancel_after_reply: Option<(u8, u8)>,
}
impl Mock {
    fn new(replies: Vec<Reply>) -> Self {
        Self {
            sent: Mutex::new(vec![]),
            get_selectors: Mutex::new(vec![]),
            replies: Mutex::new(replies.into()),
            response_prefix: 0,
            canceled: AtomicBool::new(false),
            cancel_on_send: false,
            cancel_after_reply: None,
        }
    }
    fn sent(&self) -> Vec<Vec<u8>> {
        self.sent.lock().unwrap().clone()
    }
}
impl FeatureTransport for Mock {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        self.sent.lock().unwrap().push(report.to_vec());
        if self.cancel_on_send {
            self.canceled.store(true, Ordering::Release);
        }
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        assert_eq!(report.len(), 91);
        assert!(report[1..].iter().all(|byte| *byte == 0));
        self.get_selectors.lock().unwrap().push(report[0]);
        let reply = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .context("Missing simulated reply")?;
        let sent = self.sent.lock().unwrap();
        let sent = sent.last().unwrap();
        report.fill(0);
        report[0] = self.response_prefix;
        report[1] = reply.status;
        report[2] = sent[2];
        report[7..9].copy_from_slice(&sent[7..9]);
        report[6] = reply.data.len() as u8;
        report[9..9 + reply.data.len()].copy_from_slice(&reply.data);
        if self.cancel_after_reply == Some((sent[8], sent[9])) {
            self.canceled.store(true, Ordering::Release);
        }
        Ok(report.len())
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({"simulated":true})
    }
}

#[test]
fn one_shot_admission_failure_stops_all_current_dpi_routes() {
    use crate::feature_exchange::tests::ExchangeProbe;
    use crate::keyboard_control_test_transport::Mock as Script;
    use std::cell::Cell;

    for source in make_mouse_current_dpi_table() {
        let canonical = capability(source.product_id).unwrap();
        for physical in &canonical.transport.direct_pids {
            let mut cap = canonical.clone();
            cap.transport = canonical.transport.for_physical_product(*physical).unwrap();
            cap.transport.sleep_between_out_ms = 0;
            cap.transport.sleep_between_out_in_ms = 0;
            cap.transport.sleep_between_in_ms = 0;
            let device = Script::new([]);
            let admissions = Cell::new(0);
            let admit = || {
                let previous = admissions.replace(admissions.get() + 1);
                ensure!(previous != 0, "one-shot DPI admission failure");
                Ok(())
            };
            let probe = ExchangeProbe::with_admission_check(&device, &admit);
            let error = execute_with_capability(
                &cap,
                &probe,
                None,
                || Ok(cap.transport.transaction_prefix),
                || Ok(()),
            )
            .unwrap_err();
            assert!(
                crate::backend::exchange_interrupted(&error),
                "PID{} / {physical}",
                source.product_id
            );
            assert_eq!(admissions.get(), 1, "PID{} / {physical}", source.product_id);
            assert!(device.sent().is_empty());
        }
    }
}

#[test]
fn native_interruption_stops_dpi_retries_and_retains_actual_read_bytes() {
    use crate::backend::{NativeIoOutcome, native_io_outcome};
    use crate::keyboard_control_test_transport::{Mock as Script, reply as packet};

    for source in make_mouse_current_dpi_table() {
        let canonical = capability(source.product_id).unwrap();
        for physical in &canonical.transport.direct_pids {
            let mut cap = canonical.clone();
            cap.transport = canonical.transport.for_physical_product(*physical).unwrap();
            cap.transport.sleep_between_out_ms = 0;
            cap.transport.sleep_between_out_in_ms = 0;
            cap.transport.sleep_between_in_ms = 0;
            for outcome in [
                NativeIoOutcome::NotCalled,
                NativeIoOutcome::Returned { byte_count: 90 },
                NativeIoOutcome::Returned { byte_count: 91 },
            ] {
                let device = Script::new([]).fail_next_send(outcome);
                let error = execute_with_capability(
                    &cap,
                    &device,
                    None,
                    || Ok(cap.transport.transaction_prefix),
                    || Ok(()),
                )
                .unwrap_err();
                assert_eq!(native_io_outcome(&error), Some(outcome));
                assert_eq!(device.sent().len(), 1);
                assert!(crate::feature_exchange::acknowledged_error(&error).is_none());
            }
            let device = Script::new([packet(&[7, 8])]).interrupt_next_read();
            let error = execute_with_capability(
                &cap,
                &device,
                None,
                || Ok(cap.transport.transaction_prefix),
                || Ok(()),
            )
            .unwrap_err();
            assert_eq!(
                native_io_outcome(&error),
                Some(NativeIoOutcome::Returned { byte_count: 91 })
            );
            assert_eq!(
                crate::feature_exchange::acknowledged_error(&error),
                Some(&[7, 8][..])
            );
            assert_eq!(device.sent().len(), 1);
            let progress = &error.downcast_ref::<WriteFailure>().unwrap().progress;
            assert!(progress.stages.is_none() && progress.active.is_none());
            assert!(!progress.verified);
        }
    }
}

#[test]
fn finish_failure_keeps_the_decoded_dpi_ack_and_stops_independent_following_tasks() {
    let device = Mock::new(vec![reply(vec![7, 8])]);
    let leased = crate::feature_exchange::tests::ExchangeProbe::failing_finish(&device);
    let mut cap = required(PRODUCT_ID).unwrap().clone();
    cap.transport.sleep_between_out_ms = 0;
    cap.transport.sleep_between_out_in_ms = 0;
    cap.transport.sleep_between_in_ms = 0;
    let change = DpiChange {
        stages: Some(draft().into()),
        active: Some(DpiPair { x: 800, y: 800 }),
        profile_guid: None,
        initialize: false,
    };
    let error =
        execute_with_capability(&cap, &leased, Some(&change), next(), || Ok(())).unwrap_err();
    let progress = &error.downcast_ref::<WriteFailure>().unwrap().progress;
    assert_eq!(progress.acknowledgements.len(), 1);
    assert_eq!(progress.acknowledgements[0].data, [7, 8]);
    assert!(progress.stages.is_none() && progress.active.is_none());
    assert!(!progress.verified);
    assert_eq!(device.sent().len(), 1);
    assert!(crate::backend::exchange_finalization_failed(&error));
}

#[test]
fn profile_identity_read_finishes_before_mode_or_any_profile_task() {
    let request = DpiChange {
        stages: None,
        active: None,
        profile_guid: None,
        initialize: true,
    };
    assert!(validate_change_for(182, &request).is_ok());
    assert!(validate_change_for(190, &request).is_ok());
    assert!(validate_change_for(80, &request).is_err());
    let current190 = Mock::new(vec![reply(b"DEVICE-190\0".to_vec())]);
    let current190_report = execute_with_capability(
        &obm_capability(),
        &current190,
        Some(&request),
        next(),
        || Ok(()),
    )
    .unwrap();
    assert!(current190_report.verified);
    assert_eq!(
        current190_report.serial_number.as_deref(),
        Some("DEVICE-190")
    );
    assert_eq!(current190.sent().len(), 1);
    assert_eq!(&current190.sent()[0][6..9], &[22, 0, 130]);
    let device = Mock::new(vec![reply(b"  DEVICE-182 \0\xff".to_vec())]);
    let report = execute_for(182, &device, Some(&request), next(), || Ok(())).unwrap();
    assert!(report.verified);
    assert_eq!(report.serial_number.as_deref(), Some("  DEVICE-182 "));
    assert_eq!(report.requested.as_ref(), Some(&request));
    assert!(report.stages.is_none() && report.active.is_none());
    assert!(report.acknowledgements.is_empty() && report.profile_slot_writes.is_empty());
    let sent = device.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(&sent[0][6..9], &[22, 0, 130]);

    let empty = Mock::new(vec![reply(vec![0; 22])]);
    assert!(execute_for(182, &empty, Some(&request), next(), || Ok(())).is_err());
    assert_eq!(empty.sent().len(), 1);
    let mut canceled = Mock::new(vec![]);
    canceled.cancel_on_send = true;
    assert!(
        execute_for(182, &canceled, Some(&request), next(), || {
            ensure!(
                !canceled.canceled.load(Ordering::Acquire),
                "Canceled identity query"
            );
            Ok(())
        })
        .is_err()
    );
    assert_eq!(canceled.sent().len(), 1);
    assert!(canceled.get_selectors.lock().unwrap().is_empty());
}

#[test]
fn interface_three_dpi_uses_distinct_get_selector_and_discards_returned_prefix() {
    for product_id in [150, 153, 203, 221, 226, 229, 239] {
        let mut device = Mock::new(vec![reply(vec![]), reply(active(1600, 800))]);
        // UsbRzDeviceAction slices the returned prefix; it is not an ACK field.
        device.response_prefix = 9;
        let mut source = required(product_id).unwrap().clone();
        source.transport.sleep_between_out_ms = 0;
        source.transport.sleep_between_out_in_ms = 0;
        source.transport.sleep_between_in_ms = 0;
        exchange(
            &device,
            &source,
            [7, 4, 5],
            &[0, 6, 64, 3, 32, 0, 0],
            0,
            &|| Ok(()),
        )
        .unwrap();
        let observed = exchange(&device, &source, [7, 4, 133], &[0], 1, &|| Ok(())).unwrap();
        assert_eq!(observed.data, active(1600, 800));
        assert_eq!(*device.get_selectors.lock().unwrap(), [3, 3]);
        for sent in device.sent() {
            assert_eq!(sent[0], 0);
            assert_eq!(sent[89], sent[3..89].iter().fold(0, |sum, byte| sum ^ byte));
        }
    }
}
fn reply(data: Vec<u8>) -> Reply {
    Reply { status: 2, data }
}
fn next() -> impl FnMut() -> anyhow::Result<u8> {
    let mut value = 0;
    move || {
        let transaction = value;
        value = (value + 1) % 31;
        Ok(transaction)
    }
}
fn draft() -> DpiStagesDraft {
    DpiStagesDraft {
        enabled: true,
        active_stage: 3,
        stages: [400, 800, 1600, 3200, 6400]
            .into_iter()
            .map(|dpi| DpiStage {
                x: dpi,
                y: dpi,
                independent: false,
                visible: true,
            })
            .collect(),
    }
}
fn getter(draft: &DpiStagesDraft) -> Vec<u8> {
    let table = pack_editor_for(PRODUCT_ID, draft).unwrap();
    let mut bytes = vec![1, table.active_stage, table.number_of_active_stages];
    for row in table.records {
        bytes.push(row.index + 1);
        bytes.extend_from_slice(&row.x.unwrap().word().to_be_bytes());
        bytes.extend_from_slice(&row.y.unwrap().word().to_be_bytes());
        bytes.extend_from_slice(&[0, 0]);
    }
    bytes.extend_from_slice(&[0; 7]);
    bytes
}
fn active(x: u16, y: u16) -> Vec<u8> {
    let mut value = vec![0];
    value.extend_from_slice(&x.to_be_bytes());
    value.extend_from_slice(&y.to_be_bytes());
    value.extend_from_slice(&[0, 0]);
    value
}

#[test]
fn source_numbers_keep_ipc_identity_and_project_only_at_the_file_boundary() {
    for value in [
        0.,
        -0.,
        -1.75,
        65_536.,
        4_294_967_297.,
        f64::MAX,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::from_bits(0x7ff8_0000_0000_0042),
    ] {
        let source = SourceNumber::new(value);
        let wire = serde_json::to_vec(&source).unwrap();
        let restored: SourceNumber = serde_json::from_slice(&wire).unwrap();
        assert_eq!(restored, source);
        assert_eq!(restored.value().to_bits(), value.to_bits());
        assert_eq!(source.file_value().is_null(), !value.is_finite());
    }
    assert_ne!(SourceNumber::new(0.), SourceNumber::new(-0.));
    assert!(SourceNumber::new(-0.).strict_equals_u16(0));
    assert!(!SourceNumber::new(f64::NAN).strict_equals_u16(0));
    assert_eq!(SourceNumber::new(-0.).file_value(), serde_json::json!(0));
    assert_eq!(SourceNumber::new(400.).file_value(), serde_json::json!(400));
    for invalid in [
        serde_json::json!(null),
        serde_json::json!({"nonfinite_bits":"3ff0000000000000"}),
        serde_json::json!({"nonfinite_bits":"7ff0000000000000","extra":1}),
        serde_json::json!({"nonfinite_bits":"7ff"}),
    ] {
        assert!(serde_json::from_value::<SourceNumber>(invalid).is_err());
    }
}

#[test]
fn source_stage_file_projection_retains_explicit_null_axes_and_unknown_fields() {
    let source = serde_json::json!({
        "enable":true,"active":1,"vendor_flags":[7,9],
        "stages":[{"x":null,"y":400,"visible":true,"vendor_axis":8},null,{}]
    });
    let stages = SourceStages::from_source_file(&source).unwrap();
    assert_eq!(stages.source_file_value(), source);
    assert!(stages.stages()[0].x().is_none());
    let changed = stages
        .with_replaced_stage(
            0,
            SourceStage::new(
                Some(SourceNumber::new(f64::INFINITY)),
                None,
                Some(true),
                Some(false),
            ),
        )
        .unwrap();
    assert_eq!(
        changed.source_file_value()["stages"][0]["x"],
        serde_json::Value::Null
    );
    assert_eq!(
        changed.source_file_value()["vendor_flags"],
        serde_json::json!([7, 9])
    );
    assert!(
        stages
            .with_replaced_stage(3, stages.stages()[0].clone())
            .is_err()
    );
}

#[test]
fn source_stage_bytes_use_bitwise_wrapping_without_changing_strict_getter_expectations() {
    let stages = SourceStages::new(
        true,
        1,
        [
            (-0., -1.),
            (65_536., 65_537.),
            (4_294_967_297., -65_537.),
            (f64::NAN, f64::INFINITY),
            (f64::NEG_INFINITY, 300.9),
        ]
        .into_iter()
        .map(|(x, y)| {
            SourceStage::new(
                Some(SourceNumber::new(x)),
                Some(SourceNumber::new(y)),
                Some(true),
                Some(false),
            )
        })
        .collect(),
    );
    let packed = pack_for(190, &stages).unwrap();
    assert_eq!(
        stage_payload(&packed, 1),
        [
            1, 1, 5, 0, 0, 0, 255, 255, 0, 0, 1, 0, 0, 0, 1, 0, 0, 2, 0, 1, 255, 255, 0, 0, 3, 0,
            0, 0, 0, 0, 0, 4, 0, 0, 1, 44, 0, 0,
        ]
    );
    assert!(packed.records[3].x.unwrap().value().is_nan());
    let getter = stage_reading(
        &Acknowledgement {
            command: [80, 4, 134],
            transaction: 1,
            profile: None,
            response_bytes: 45,
            data: vec![
                1, 1, 5, 1, 0, 0, 255, 255, 0, 0, 2, 0, 0, 0, 1, 0, 0, 3, 0, 1, 255, 255, 0, 0, 4,
                0, 0, 0, 0, 0, 0, 5, 0, 0, 1, 44, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
        },
        1,
    )
    .unwrap();
    assert!(!same_stages(&packed, &getter));
}

#[test]
fn six_source_stage_tasks_keep_raw_requests_and_actual_readback_after_numeric_coercion() {
    for product_id in [190, 192, 222, 226, 229, 239] {
        let mut cap = required(product_id).unwrap().clone();
        cap.transport = cap.transport.for_physical_product(cap.wired_pid).unwrap();
        cap.transport.sleep_between_out_ms = 0;
        cap.transport.sleep_between_out_in_ms = 0;
        cap.transport.sleep_between_in_ms = 0;
        for (raw, integer, matches) in [
            (-0., 0_u16, true),
            (0., 0, true),
            (f64::NAN, 0, false),
            (f64::INFINITY, 0, false),
            (f64::NEG_INFINITY, 0, false),
            (-1., 65535, false),
            (65_536., 0, false),
            (65_537., 1, false),
            (4_294_967_297., 1, false),
            (300.9, 300, false),
        ] {
            let change = DpiChange {
                stages: Some(SourceStages::new(
                    true,
                    1,
                    vec![SourceStage::new(
                        Some(SourceNumber::new(raw)),
                        Some(SourceNumber::new(raw)),
                        Some(true),
                        Some(false),
                    )],
                )),
                active: None,
                profile_guid: None,
                initialize: false,
            };
            let device = Mock::new(vec![
                reply(vec![]),
                reply(source_stage_data(1, 1, 1, &[(integer, integer)])),
            ]);
            let report =
                execute_with_capability(&cap, &device, Some(&change), next(), || Ok(())).unwrap();
            assert_eq!(report.requested.as_ref(), Some(&change));
            assert_eq!(report.verified, matches, "PID{product_id} raw {raw}");
            assert_eq!(report.write_errors.stages().is_none(), matches);
            let observed = report.stages.as_ref().unwrap();
            assert_eq!(
                (observed.records[0].x, observed.records[0].y),
                (integer, integer)
            );
            assert_eq!(report.acknowledgements.len(), 1);
            let reports = device.sent();
            assert_eq!(reports.len(), 2);
            assert_eq!(&reports[0][6..13], &[10, 4, 6, 1, 1, 1, 0]);
            let [hi, lo] = integer.to_be_bytes();
            assert_eq!(&reports[0][13..19], &[hi, lo, hi, lo, 0, 0]);
            assert_eq!(&reports[1][6..10], &[80, 4, 134, 1]);
        }
    }
}

#[test]
fn six_source_runners_compare_raw_record_prefixes_with_the_current_length_boundary() {
    let stages = SourceStages::new(
        true,
        1,
        vec![
            SourceStage::new(
                Some(400u16.into()),
                Some(500u16.into()),
                Some(true),
                Some(false),
            ),
            SourceStage::new(
                Some(800u16.into()),
                Some(900u16.into()),
                Some(true),
                Some(false),
            ),
        ],
    );
    for product in [190, 192, 222, 226, 229, 239] {
        let packed = pack_for(product, &stages).unwrap();
        let mut data = vec![1, 9, 7, 1, 1, 144, 1, 244, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let ack = |data: Vec<u8>| Acknowledgement {
            command: [80, 4, 134],
            transaction: 1,
            profile: None,
            response_bytes: data.len(),
            data,
        };
        let prefix = stage_reading(&ack(data.clone()), 1).unwrap();
        assert!(same_stages(&packed, &prefix), "source prefix PID{product}");
        data.truncate(10);
        assert!(same_stages(&packed, &stage_reading(&ack(data), 1).unwrap()));
        assert!(same_stages(
            &packed,
            &stage_reading(&ack(vec![1, 1, 2]), 1).unwrap()
        ));
        let mut mismatch = prefix;
        mismatch.records[0].x = 401;
        assert!(!same_stages(&packed, &mismatch));
        let zero = SourceStages::new(
            true,
            1,
            vec![SourceStage::new(
                Some(SourceNumber::new(-0.)),
                Some(0u16.into()),
                Some(true),
                None,
            )],
        );
        let observed = stage_reading(
            &ack(vec![1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            1,
        )
        .unwrap();
        assert!(same_stages(&pack_for(product, &zero).unwrap(), &observed));
        let missing = SourceStages::new(
            true,
            1,
            vec![SourceStage::new(None, None, Some(true), None)],
        );
        assert!(!same_stages(
            &pack_for(product, &missing).unwrap(),
            &observed
        ));
    }
}

#[test]
fn source_visibility_filters_missing_fields_and_null_rows_for_all_six_products() {
    let null_row = SourceStages::from_source_file(&serde_json::json!({
        "enable":true,"active":1,"stages":[null,{}]
    }))
    .unwrap();
    for product in [190, 192, 222, 226, 229, 239] {
        let packed = pack_for(product, &null_row).unwrap();
        assert!(packed.records.is_empty());
        assert_eq!(packed.active_stage, 0);
        assert_eq!(packed.number_of_active_stages, 0);
        let absent_selection = pack_for(product, &SourceStages::new(false, 20, vec![])).unwrap();
        assert!(absent_selection.records.is_empty());
        assert_eq!(absent_selection.active_stage, 0);
        assert_eq!(absent_selection.number_of_active_stages, 1);
    }
    let absent_visibility = SourceStages::new(
        true,
        1,
        vec![SourceStage::new(
            Some(400u16.into()),
            Some(400u16.into()),
            None,
            None,
        )],
    );
    for product in [190, 192, 222, 226, 229, 239] {
        let packed = pack_for(product, &absent_visibility).unwrap();
        assert_eq!(
            (packed.active_stage, packed.number_of_active_stages),
            (0, 0)
        );
    }
}

const OBM_GUID: &str = "12345678-9abc-def0-1234-56789abcdef0";
const OTHER_OBM_GUID: &str = "22345678-9abc-def0-1234-56789abcdef0";

fn obm_capability() -> Capability {
    let mut cap = required(190).unwrap().clone();
    cap.transport = cap.transport.for_physical_product(190).unwrap();
    cap.transport.sleep_between_out_ms = 0;
    cap.transport.sleep_between_out_in_ms = 0;
    cap.transport.sleep_between_in_ms = 0;
    cap.transport.max_retry_in = 1;
    cap.transport.max_retry_out = 1;
    cap
}

fn source_stage_data(profile: u8, active: u8, declared: u8, xy: &[(u16, u16)]) -> Vec<u8> {
    let mut bytes = vec![profile, active, declared];
    for (index, (x, y)) in xy.iter().enumerate() {
        bytes.push(index as u8 + 1);
        bytes.extend_from_slice(&x.to_be_bytes());
        bytes.extend_from_slice(&y.to_be_bytes());
        bytes.extend_from_slice(&[0, 0]);
    }
    bytes.extend_from_slice(&[0; 7]);
    bytes
}

fn source_default_stage_data(profile: u8, active: u8) -> Vec<u8> {
    // Current190's five local defaults; independent of the packer under test.
    source_stage_data(
        profile,
        active,
        5,
        &[
            (400, 400),
            (800, 800),
            (1600, 1600),
            (3200, 3200),
            (6400, 6400),
        ],
    )
}

fn obm_inventory_replies(slots: &[(u8, bool)]) -> Vec<Reply> {
    let count = slots.len() as u8 + 1;
    let mut ids = vec![count, 1];
    ids.extend(slots.iter().map(|(id, _)| *id));
    let mut replies = vec![reply(vec![count]), reply(vec![count]), reply(ids)];
    for (slot, matching) in slots {
        let mut metadata = vec![0; 250];
        metadata[..16].copy_from_slice(&[
            0x78, 0x56, 0x34, 0x12, 0xbc, 0x9a, 0xf0, 0xde, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc,
            0xde, 0xf0,
        ]);
        if !matching {
            metadata[3] = 0x22;
        }
        for offset in (0..250).step_by(64) {
            let count = (250 - offset).min(64);
            let mut block = vec![*slot];
            block.extend_from_slice(&(offset as u16).to_be_bytes());
            block.extend_from_slice(&(count as u16).to_be_bytes());
            block.extend_from_slice(&metadata[offset..offset + count]);
            replies.push(reply(block));
        }
    }
    replies
}

fn obm_change() -> DpiChange {
    DpiChange {
        stages: Some(draft().into()),
        active: None,
        profile_guid: Some(OBM_GUID.into()),
        initialize: false,
    }
}

#[test]
fn ordinary_obm_current190_reads_actual_associations_and_updates_all_unequal_matching_slots() {
    let mut replies = vec![reply(vec![]), reply(source_default_stage_data(1, 3))];
    replies.extend(obm_inventory_replies(&[(4, true), (2, false), (3, true)]));
    replies.extend([
        reply(source_default_stage_data(4, 3)),
        reply(source_default_stage_data(3, 1)),
        reply(vec![0x5a]),
    ]);
    let device = Mock::new(replies);
    let cap = obm_capability();
    assert!(crate::mouse_onboard::MouseOnboard::new(&device, cap.transport()).is_err());
    let report =
        execute_with_capability(&cap, &device, Some(&obm_change()), next(), || Ok(())).unwrap();
    assert!(report.verified);
    assert_eq!(
        report
            .profile_slots
            .iter()
            .map(|slot| (slot.slot_id, slot.guid.as_str()))
            .collect::<Vec<_>>(),
        [(4, OBM_GUID), (2, OTHER_OBM_GUID), (3, OBM_GUID)]
    );
    assert_eq!(report.profile_slot_writes.len(), 2);
    let equal = &report.profile_slot_writes[0];
    assert_eq!(equal.slot_id, 4);
    assert!(equal.was_equal && equal.before.is_some() && equal.acknowledged.is_none());
    let changed = &report.profile_slot_writes[1];
    assert_eq!(changed.slot_id, 3);
    assert!(!changed.was_equal && changed.error.is_none());
    assert_eq!(changed.before.as_ref().unwrap().active_stage, 1);
    assert_eq!(changed.acknowledged.as_ref().unwrap().raw_data, [0x5a]);
    let sent = device.sent();
    assert_eq!(&sent[2][6..9], &[1, 5, 138]);
    assert_eq!(&sent[3][6..9], &[1, 5, 128]);
    assert_eq!(&sent[4][6..9], &[80, 5, 129]);
    for (index, slot) in [4, 2, 3].into_iter().enumerate() {
        for (block, (offset, size)) in [(0u16, 69), (64, 69), (128, 69), (192, 63)]
            .into_iter()
            .enumerate()
        {
            let frame = &sent[5 + index * 4 + block];
            assert_eq!(&frame[6..9], &[size, 5, 136]);
            assert_eq!(frame[9], slot);
            assert_eq!(&frame[10..12], &offset.to_be_bytes());
            assert_eq!(&frame[12..14], &[0, 0]);
        }
    }
    assert_eq!(
        sent[17..]
            .iter()
            .map(|frame| (frame[8], frame[9]))
            .collect::<Vec<_>>(),
        [(134, 4), (134, 3), (6, 3)]
    );
    assert_eq!(&sent.last().unwrap()[6..12], &[38, 4, 6, 3, 3, 5]);
    assert!(device.replies.lock().unwrap().is_empty());
}

#[test]
fn ordinary_obm_current190_missing_guid_match_keeps_the_observed_inventory_without_writes() {
    let mut replies = vec![reply(vec![]), reply(source_default_stage_data(1, 3))];
    replies.extend(obm_inventory_replies(&[(2, false)]));
    let device = Mock::new(replies);
    let report = execute_with_capability(
        &obm_capability(),
        &device,
        Some(&obm_change()),
        next(),
        || Ok(()),
    )
    .unwrap();
    assert!(report.verified && report.profile_slot_writes.is_empty());
    assert_eq!(report.profile_slots[0].guid, OTHER_OBM_GUID);
    assert_eq!(device.sent().len(), 9);
    assert!(device.replies.lock().unwrap().is_empty());
}

#[test]
fn ordinary_obm_current190_collects_multiple_slot_errors_and_finishes_other_slots_and_active_task()
{
    let mut replies = vec![reply(vec![]), reply(source_default_stage_data(1, 3))];
    replies.extend(obm_inventory_replies(&[(2, true), (3, true), (4, true)]));
    replies.extend([
        reply(vec![2, 1, 1, 1]),
        reply(source_default_stage_data(3, 1)),
        Reply {
            status: 5,
            data: vec![],
        },
        reply(source_default_stage_data(4, 1)),
        reply(vec![]),
        reply(active(1600, 1600)),
    ]);
    let device = Mock::new(replies);
    let mut change = obm_change();
    change.active = Some(DpiPair { x: 1600, y: 1600 });
    let report =
        execute_with_capability(&obm_capability(), &device, Some(&change), next(), || Ok(()))
            .unwrap();
    assert!(!report.verified);
    assert_eq!(report.active, change.active);
    assert_eq!(report.profile_slot_writes.len(), 3);
    let error = report.write_errors.stages().unwrap();
    assert!(error.contains("PID190 OBM stage synchronization failed"));
    assert!(error.contains("truncated or belongs to another profile"));
    assert!(error.contains("device status 5"));
    let sent = device.sent();
    assert_eq!(
        sent[17..]
            .iter()
            .map(|frame| (frame[8], frame[9]))
            .collect::<Vec<_>>(),
        [(134, 2), (134, 3), (6, 3), (134, 4), (6, 4), (133, 0)]
    );
    assert!(device.replies.lock().unwrap().is_empty());
}

#[test]
fn ordinary_obm_current190_cancellation_after_slot_read_or_set_ack_stops_later_tasks() {
    for cancel_command in [134, 6] {
        let mut replies = vec![reply(vec![]), reply(source_default_stage_data(1, 3))];
        replies.extend(obm_inventory_replies(&[(2, true), (3, true)]));
        replies.push(reply(source_default_stage_data(2, 1)));
        if cancel_command == 6 {
            replies.push(reply(vec![]));
        }
        let mut device = Mock::new(replies);
        device.cancel_after_reply = Some((cancel_command, 2));
        let mut change = obm_change();
        change.active = Some(DpiPair { x: 1600, y: 1600 });
        let error =
            execute_with_capability(&obm_capability(), &device, Some(&change), next(), || {
                ensure!(
                    !device.canceled.load(Ordering::Acquire),
                    "Canceled slot task"
                );
                Ok(())
            })
            .unwrap_err();
        assert!(format!("{error:#}").contains("Canceled slot task"));
        let sent = device.sent();
        assert_eq!(
            (sent.last().unwrap()[8], sent.last().unwrap()[9]),
            (cancel_command, 2)
        );
        assert_eq!(sent.len(), 14 + usize::from(cancel_command == 6));
        assert!(device.replies.lock().unwrap().is_empty());
    }
}

#[test]
fn ordinary_obm_current190_preserves_hidden_selection_and_disabled_declared_count() {
    let slots = [crate::mouse_dpi_stages::ObservedProfileSlot {
        slot_id: 2,
        guid: OBM_GUID.into(),
    }];
    for enabled in [true, false] {
        let mut draft = draft();
        draft.enabled = enabled;
        draft.stages[2].visible = false;
        let table = pack_editor_for(190, &draft).unwrap();
        let device = Mock::new(vec![reply(source_default_stage_data(2, 3)), reply(vec![])]);
        let result = synchronize_ordinary_profile_slots(
            &device,
            &obm_capability(),
            OBM_GUID,
            &slots,
            &table,
            &mut next(),
            &|| Ok(()),
        )
        .unwrap();
        assert!(!result[0].was_equal && result[0].acknowledged.is_some());
        assert!(result[0].error.is_none());
        let sent = device.sent();
        assert_eq!(sent.len(), 2);
        assert_eq!(&sent[0][6..10], &[80, 4, 134, 2]);
        if enabled {
            assert_eq!(&sent[1][6..12], &[31, 4, 6, 2, 0, 4]);
            assert_eq!(&sent[1][27..31], &[12, 128, 12, 128]);
        } else {
            assert_eq!(&sent[1][6..19], &[10, 4, 6, 2, 0, 1, 0, 0, 0, 0, 0, 0, 0]);
        }
    }
}

#[test]
fn ordinary_obm_current190_disabled_stage_matches_the_selected_visible_row_only() {
    let slots = [crate::mouse_dpi_stages::ObservedProfileSlot {
        slot_id: 2,
        guid: OBM_GUID.into(),
    }];
    let mut draft = draft();
    draft.enabled = false;
    let device = Mock::new(vec![reply(source_stage_data(2, 1, 1, &[(1600, 1600)]))]);
    let result = synchronize_ordinary_profile_slots(
        &device,
        &obm_capability(),
        OBM_GUID,
        &slots,
        &pack_editor_for(190, &draft).unwrap(),
        &mut next(),
        &|| Ok(()),
    )
    .unwrap();
    assert!(result[0].was_equal && result[0].acknowledged.is_none());
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn stage_table_uses_profile_one_and_source_transactions_checksum_and_big_endian_records() {
    let mut table = draft();
    table.stages[1].visible = false;
    table.stages[4].visible = false;
    table.stages[2].x = 45000;
    table.stages[2].y = 44999;
    let device = Mock::new(vec![reply(vec![]), reply(getter(&table))]);
    let change = DpiChange {
        profile_guid: None,
        initialize: false,
        stages: Some(table.into()),
        active: None,
    };
    let observed = execute(&device, Some(&change), next(), || Ok(())).unwrap();
    assert!(observed.verified && observed.active.is_none());
    let sent = device.sent();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0][2], 0);
    assert_eq!(sent[1][2], 1);
    assert_eq!(&sent[0][6..12], &[24, 4, 6, 1, 2, 3]);
    assert_eq!(&sent[0][12..19], &[0, 1, 144, 1, 144, 0, 0]);
    assert_eq!(&sent[0][19..26], &[1, 175, 200, 175, 199, 0, 0]);
    assert_eq!(
        sent[0][89],
        sent[0][3..89].iter().fold(0, |sum, byte| sum ^ byte)
    );
    assert_eq!(&sent[1][6..10], &[80, 4, 134, 1]);
}

#[test]
fn active_dpi_reads_profile_zero_skips_unchanged_value_and_confirms_each_changed_axis() {
    let change = DpiChange {
        profile_guid: None,
        initialize: false,
        stages: None,
        active: Some(DpiPair { x: 45000, y: 100 }),
    };
    let device = Mock::new(vec![reply(active(45000, 100))]);
    execute(&device, Some(&change), next(), || Ok(())).unwrap();
    assert_eq!(device.sent().len(), 1);
    let device = Mock::new(vec![
        reply(active(800, 800)),
        reply(vec![]),
        reply(active(45000, 100)),
    ]);
    let result = execute(&device, Some(&change), next(), || Ok(())).unwrap();
    assert_eq!(result.active, change.active);
    let sent = device.sent();
    assert_eq!(sent.len(), 3);
    assert_eq!(&sent[1][6..16], &[7, 4, 5, 0, 175, 200, 0, 100, 0, 0]);
}

#[test]
fn read_keeps_the_actual_2000_pair_when_the_independent_stage_getter_fails() {
    let device = Mock::new(vec![
        Reply {
            status: 5,
            data: vec![],
        },
        reply(active(2000, 2100)),
    ]);
    let report = execute(&device, None, next(), || Ok(())).unwrap();
    assert_eq!(report.active, Some(DpiPair { x: 2000, y: 2100 }));
    assert!(report.stages.is_none());
    assert!(report.read_errors.stages().is_some());
    assert!(report.read_errors.active().is_none());
    assert!(!report.verified);
    assert!(report.acknowledgements.is_empty());
    assert_eq!(
        device
            .sent()
            .iter()
            .map(|packet| packet[8])
            .collect::<Vec<_>>(),
        [134, 133]
    );
}

#[test]
fn read_keeps_the_actual_stage_table_when_the_independent_active_getter_fails() {
    let mut observed = draft();
    observed.stages[2].x = 2000;
    observed.stages[2].y = 2100;
    let device = Mock::new(vec![
        reply(getter(&observed)),
        Reply {
            status: 5,
            data: vec![],
        },
    ]);
    let report = execute(&device, None, next(), || Ok(())).unwrap();
    let stages = report.stages.unwrap();
    assert_eq!((stages.records[2].x, stages.records[2].y), (2000, 2100));
    assert!(report.active.is_none());
    assert!(report.read_errors.stages().is_none());
    assert!(report.read_errors.active().is_some());
    assert!(!report.verified);
    assert!(report.acknowledgements.is_empty());
}

#[test]
fn failed_getters_report_each_failure_without_any_cached_or_default_value() {
    let device = Mock::new(vec![
        Reply {
            status: 5,
            data: vec![],
        },
        Reply {
            status: 5,
            data: vec![],
        },
    ]);
    let report = execute(&device, None, next(), || Ok(())).unwrap();
    assert!(report.active.is_none() && report.stages.is_none() && report.stage_level.is_none());
    assert!(report.read_errors.stages().is_some() && report.read_errors.active().is_some());
    assert!(!report.verified);
    assert!(report.acknowledgements.is_empty());
}

#[test]
fn hidden_row_unlink_retains_source_active_zero_in_both_staging_modes() {
    let mut table = draft();
    table.stages[2].visible = false;
    let packed = pack_editor_for(PRODUCT_ID, &table).unwrap();
    assert_eq!(packed.active_stage, 0);
    assert_eq!(packed.records.len(), 4);
    table.enabled = false;
    let packed = pack_editor_for(PRODUCT_ID, &table).unwrap();
    assert_eq!(
        (packed.active_stage, packed.number_of_active_stages),
        (0, 1)
    );
    assert!(packed.records.is_empty());
    assert_eq!(
        stage_payload(&packed, 1),
        vec![1, 0, 1, 0, 0, 0, 0, 0, 0, 0]
    );
}

#[test]
fn setter_ack_without_matching_readback_never_confirms_the_local_profile() {
    let wanted = draft();
    let mut different = wanted.clone();
    different.stages[2].x += 1;
    let device = Mock::new(vec![reply(vec![]), reply(getter(&different))]);
    let report = execute(
        &device,
        Some(&DpiChange {
            profile_guid: None,
            initialize: false,
            stages: Some(wanted.into()),
            active: None,
        }),
        next(),
        || Ok(()),
    )
    .unwrap();
    assert!(!report.verified);
    assert!(
        report
            .write_errors
            .stages()
            .unwrap()
            .contains("readback differs")
    );
    assert_eq!(
        report.stages.as_ref().unwrap().records[2].x,
        different.stages[2].x
    );
    assert_eq!(device.sent().len(), 2);
}

#[test]
fn rejected_stage_task_does_not_discard_the_separately_requested_active_task() {
    let change = DpiChange {
        profile_guid: None,
        initialize: false,
        stages: Some(draft().into()),
        active: Some(DpiPair { x: 1600, y: 1600 }),
    };
    let device = Mock::new(vec![
        Reply {
            status: 5,
            data: vec![],
        },
        reply(active(1600, 1600)),
    ]);
    let report = execute(&device, Some(&change), next(), || Ok(())).unwrap();
    assert!(!report.verified);
    assert!(report.write_errors.stages().is_some());
    assert!(report.write_errors.active().is_none());
    assert_eq!(report.active, change.active);
    assert_eq!(
        device
            .sent()
            .iter()
            .map(|report| report[8])
            .collect::<Vec<_>>(),
        vec![6, 133]
    );
}

#[test]
fn active_write_mismatch_retains_the_actual_readback_and_rejection() {
    let device = Mock::new(vec![
        reply(active(1600, 1600)),
        reply(vec![]),
        reply(active(2300, 2400)),
    ]);
    let report = execute(
        &device,
        Some(&DpiChange {
            profile_guid: None,
            initialize: false,
            stages: None,
            active: Some(DpiPair { x: 2200, y: 2200 }),
        }),
        next(),
        || Ok(()),
    )
    .unwrap();
    assert!(!report.verified);
    assert_eq!(report.active, Some(DpiPair { x: 2300, y: 2400 }));
    assert!(
        report
            .write_errors
            .active()
            .unwrap()
            .contains("readback differs")
    );
    assert_eq!(report.acknowledgements.len(), 1);
    assert_eq!(device.sent().len(), 3);
}

#[test]
fn cancellation_stops_readback_and_all_later_requested_device_tasks() {
    let mut device = Mock::new(vec![]);
    device.cancel_on_send = true;
    let change = DpiChange {
        profile_guid: None,
        initialize: false,
        stages: Some(draft().into()),
        active: Some(DpiPair { x: 1600, y: 1600 }),
    };
    assert!(
        execute(&device, Some(&change), next(), || {
            ensure!(!device.canceled.load(Ordering::Acquire), "canceled");
            Ok(())
        })
        .is_err()
    );
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn offline_dongle_and_out_of_range_transaction_do_not_report_success() {
    let device = Mock::new(vec![reply(vec![0])]);
    assert!(verify_wireless_online(&device, 0, || Ok(())).is_err());
    assert_eq!(&device.sent()[0][6..10], &[1, 0, 184, 0]);
    let device = Mock::new(vec![]);
    let report = execute(
        &device,
        Some(&DpiChange {
            profile_guid: None,
            initialize: false,
            stages: Some(draft().into()),
            active: None,
        }),
        || Ok(31),
        || Ok(()),
    )
    .unwrap();
    assert!(!report.verified);
    assert!(report.write_errors.stages().is_some());
    assert!(device.sent().is_empty());
}

#[test]
fn pro_click_basic_mode_keeps_the_visible_cycle_table_and_its_selected_stage() {
    let mut table = draft();
    table.enabled = false;
    table.stages[1].visible = false;
    let ordinary = pack_editor_for(190, &table).unwrap();
    let cycle = pack_editor_for(199, &table).unwrap();
    assert_eq!(
        (ordinary.active_stage, ordinary.number_of_active_stages),
        (1, 1)
    );
    assert_eq!((cycle.active_stage, cycle.number_of_active_stages), (2, 4));
    let mut readback = vec![1, 2, 4];
    for (index, dpi) in [400_u16, 1600, 3200, 6400].into_iter().enumerate() {
        readback.push(index as u8 + 1);
        readback.extend_from_slice(&dpi.to_be_bytes());
        readback.extend_from_slice(&dpi.to_be_bytes());
        readback.extend_from_slice(&[0, 0]);
    }
    let device = Mock::new(vec![reply(vec![]), reply(readback)]);
    let report = execute_for(
        199,
        &device,
        Some(&DpiChange {
            profile_guid: None,
            initialize: false,
            stages: Some(table.into()),
            active: None,
        }),
        next(),
        || Ok(()),
    )
    .unwrap();
    assert!(report.verified);
    let sent = device.sent();
    assert_eq!(&sent[0][6..12], &[31, 4, 6, 1, 2, 4]);
    assert_eq!(&sent[0][19..26], &[1, 6, 64, 6, 64, 0, 0]);
}

#[test]
fn current_product_limits_interfaces_and_receiver_factories_are_kept_separate() {
    let mut table = draft();
    table.stages[2].x = 50000;
    table.stages[2].y = 49999;
    assert!(pack_editor_for(239, &table).is_ok());
    assert!(pack_editor_for(190, &table).is_err());
    assert!(pack_editor_for(199, &table).is_err());
    let carbon = capability(239).unwrap();
    assert_eq!(carbon.transport().claim_interface_for(239).unwrap(), 3);
    assert_eq!(carbon.transport().claim_interface_for(240).unwrap(), 3);
    assert!(carbon.is_physical_branch(240, true));
    let pro = capability(208).unwrap();
    assert!(pro.is_physical_branch(208, false));
    assert!(pro.is_physical_branch(209, true));
    assert!(!pro.is_physical_branch(210, false));
    let dual = capability(143).unwrap();
    assert!(dual.is_physical_branch(143, false));
    assert!(!dual.is_physical_branch(144, true));
    assert_eq!(capability(104).unwrap().transport().max_retry_out, 20);
}

#[test]
fn level_only_stage_task_sets_profile_one_without_inventing_a_stage_table_getter() {
    let device = Mock::new(vec![reply(vec![]), reply(active(1600, 1600))]);
    let change = DpiChange {
        profile_guid: None,
        initialize: false,
        stages: Some(draft().into()),
        active: Some(DpiPair { x: 1600, y: 1600 }),
    };
    let report = execute_for(92, &device, Some(&change), next(), || Ok(())).unwrap();
    assert!(report.verified);
    assert!(report.stages.is_none() && report.stage_level.is_none());
    let sent = device.sent();
    assert_eq!(sent.len(), 2);
    assert_eq!(&sent[0][6..16], &[7, 4, 5, 1, 6, 64, 6, 64, 0, 0]);
    assert_eq!(&sent[1][6..10], &[7, 4, 133, 0]);
}

#[test]
fn naga_legacy_inherits_dpi_packets_and_basilisk_keeps_interface_three_wired_only() {
    for product_id in [80, 83] {
        let source = capability(product_id).unwrap();
        assert_eq!(
            source.transport().source_class,
            "rzDevice25LegacyNoBrightness"
        );
        assert!(source.is_stage_level_only());
        let mut table = draft();
        table.enabled = false;
        table.stages[2].x = 16000;
        table.stages[2].y = 15999;
        let device = Mock::new(vec![reply(vec![1, 62, 128, 62, 127, 0, 0])]);
        let report = execute_for(
            product_id,
            &device,
            Some(&DpiChange {
                profile_guid: None,
                initialize: false,
                stages: Some(table.into()),
                active: None,
            }),
            next(),
            || Ok(()),
        )
        .unwrap();
        assert_eq!(report.stage_level, Some(DpiPair { x: 16000, y: 15999 }));
        assert!(report.stages.is_none() && report.active.is_none());
        assert_eq!(device.sent().len(), 1);
        assert_eq!(
            &device.sent()[0][6..16],
            &[7, 4, 5, 1, 62, 128, 62, 127, 0, 0]
        );
    }
    let source = capability(226).unwrap();
    assert_eq!((source.minimum(), source.maximum()), (100, 50000));
    assert_eq!(source.transport().claim_interface_for(226).unwrap(), 3);
    assert!(source.is_physical_branch(226, false));
    assert!(!source.is_physical_branch(227, true));
    assert!(!source.is_physical_branch(228, false));
    assert!(capability(162).is_none());
}
