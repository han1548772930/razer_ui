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
        Ok(report.len())
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({"simulated":true})
    }
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
    assert!(validate_change_for(190, &request).is_err());
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
    let table = pack(draft).unwrap();
    let mut bytes = vec![1, table.active_stage, table.number_of_active_stages];
    for row in table.records {
        bytes.push(row.index + 1);
        bytes.extend_from_slice(&row.x.to_be_bytes());
        bytes.extend_from_slice(&row.y.to_be_bytes());
        bytes.extend_from_slice(&[0, 0]);
    }
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
        stages: Some(table),
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
fn hidden_row_unlink_retains_source_active_zero_in_both_staging_modes() {
    let mut table = draft();
    table.stages[2].visible = false;
    let packed = pack(&table).unwrap();
    assert_eq!(packed.active_stage, 0);
    assert_eq!(packed.records.len(), 4);
    table.enabled = false;
    let packed = pack(&table).unwrap();
    assert_eq!(
        (packed.active_stage, packed.number_of_active_stages),
        (0, 1)
    );
    assert!(packed.records.is_empty());
    assert_eq!(stage_payload(&packed), vec![1, 0, 1, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn setter_ack_without_matching_readback_never_confirms_the_local_profile() {
    let wanted = draft();
    let mut different = wanted.clone();
    different.stages[2].x += 1;
    let device = Mock::new(vec![reply(vec![]), reply(getter(&different))]);
    assert!(
        execute(
            &device,
            Some(&DpiChange {
                profile_guid: None,
                initialize: false,
                stages: Some(wanted),
                active: None
            }),
            next(),
            || Ok(())
        )
        .is_err()
    );
    assert_eq!(device.sent().len(), 2);
}

#[test]
fn rejected_stage_task_does_not_discard_the_separately_requested_active_task() {
    let change = DpiChange {
        profile_guid: None,
        initialize: false,
        stages: Some(draft()),
        active: Some(DpiPair { x: 1600, y: 1600 }),
    };
    let device = Mock::new(vec![
        Reply {
            status: 5,
            data: vec![],
        },
        reply(active(1600, 1600)),
    ]);
    assert!(execute(&device, Some(&change), next(), || Ok(())).is_err());
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
fn cancellation_stops_readback_and_all_later_requested_device_tasks() {
    let mut device = Mock::new(vec![]);
    device.cancel_on_send = true;
    let change = DpiChange {
        profile_guid: None,
        initialize: false,
        stages: Some(draft()),
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
    assert!(
        execute(
            &device,
            Some(&DpiChange {
                profile_guid: None,
                initialize: false,
                stages: Some(draft()),
                active: None
            }),
            || Ok(31),
            || Ok(())
        )
        .is_err()
    );
    assert!(device.sent().is_empty());
}

#[test]
fn pro_click_basic_mode_keeps_the_visible_cycle_table_and_its_selected_stage() {
    let mut table = draft();
    table.enabled = false;
    table.stages[1].visible = false;
    let ordinary = pack_for(190, &table).unwrap();
    let cycle = pack_for(199, &table).unwrap();
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
            stages: Some(table),
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
    assert!(pack_for(239, &table).is_ok());
    assert!(pack_for(190, &table).is_err());
    assert!(pack_for(199, &table).is_err());
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
        stages: Some(draft()),
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
                stages: Some(table),
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
