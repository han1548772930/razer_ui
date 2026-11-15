use super::*;
use crate::{
    keyboard_control_test_transport::{Mock, assert_report_integrity, reply, sequence},
    keyboard_macro_test_support::*,
};
use std::cell::Cell;

#[test]
fn four_products_rebuild_observed_slots_modes_and_macro_bytes_in_source_order() {
    for product in [579, 600, 602, 697] {
        let cap = cap(product);
        let reading = observation(product, true);
        let device = Mock::new(reconstruction_replies(&reading, &[2]));
        let waits = Cell::new(0);
        let report = write_macro_with_compaction(
            &device,
            &cap,
            &reading,
            8,
            &request(),
            &mut sequence(),
            &|| Ok(()),
            &mut |duration| {
                assert_eq!(duration, Duration::from_millis(100));
                waits.set(waits.get() + 1);
                Ok(())
            },
        )
        .unwrap();
        assert!(report.confirmed());
        assert_eq!(waits.get(), 1);
        assert_eq!(
            report
                .reconstruction
                .as_ref()
                .unwrap()
                .profiles
                .iter()
                .map(|p| p.slot)
                .collect::<Vec<_>>(),
            [5, 2]
        );
        let sent = device.sent();
        assert_report_integrity(&sent);
        let removals = sent
            .iter()
            .filter(|r| r[7] == 5 && r[8] == if product == 579 { 11 } else { 3 })
            .map(|r| r[9])
            .collect::<Vec<_>>();
        assert_eq!(
            removals,
            if product == 579 { vec![5, 1, 2] } else { vec![1, 2] }
        );
        let names = sent
            .iter()
            .filter(|r| r[7..9] == [5, 8])
            .collect::<Vec<_>>();
        assert_eq!(names.len(), 8);
        assert_eq!(
            names
                .iter()
                .take(4)
                .map(|r| (r[10], r[11], r[6]))
                .collect::<Vec<_>>(),
            [(0, 0, 69), (0, 64, 69), (0, 128, 69), (0, 192, 63)]
        );
        assert_eq!(&names[3][14 + 52..14 + 58], &[0; 6]);
        assert!(
            !sent.iter().any(|r| r[7..9] == [3, 3]),
            "Reconstruction does not write brightness"
        );
        let assignments = report
            .steps
            .iter()
            .filter_map(|step| match step.operation {
                Operation::Assignment { slot, key, mode } => Some((slot, key, mode)),
                _ => None,
            })
            .collect::<Vec<_>>();
        for slot in [5, 2] {
            let rows = assignments
                .iter()
                .filter(|row| row.0 == slot)
                .collect::<Vec<_>>();
            assert!(
                rows.windows(2)
                    .all(|pair| (pair[0].2, pair[0].1) < (pair[1].2, pair[1].1))
            );
        }
        let restored = &report.reconstruction.as_ref().unwrap().macros[0];
        assert_eq!(
            restored.rebuilt.request.data,
            [1, 49, 2, 49, 17, 0, 21, 0, 0, 0, 0]
        );
        assert_eq!(restored.rebuilt.request.num_items, 4);
        assert!(
            restored.rebuilt.request.timestamp.is_empty()
                && restored.rebuilt.request.hash.is_empty()
        );
        assert_eq!(restored.reading.metadata.timestamp, "old");
        assert!(
            report.macro_restoration[0]
                .report
                .metadata_readback
                .is_none()
        );
        assert_eq!(cap.max_retry_out, 1);
        assert_eq!(cap.sleep_between_out_in_ms, 0);
    }
}

#[test]
fn capacity_admission_never_formats_on_failure_or_when_contiguous_space_suffices() {
    let reading = observation(600, false);
    for (available, recyclable, success) in [(0, 256, false), (1024, 0, true)] {
        let mut replies = vec![storage(available, recyclable)];
        if success {
            replies.extend((0..6).map(|_| reply(&[])));
            replies.push(storage(512, 0));
        }
        let device = Mock::new(replies);
        let result = write_macro_with_compaction(
            &device,
            &cap(600),
            &reading,
            8,
            &request(),
            &mut sequence(),
            &|| Ok(()),
            &mut |_| panic!("Unexpected format wait"),
        );
        assert_eq!(result.is_ok(), success);
        assert!(!device.sent().iter().any(|r| r[7..9] == [6, 10]));
        if let Ok(report) = result {
            assert!(report.confirmed());
            assert!(report.reconstruction.is_none());
        }
    }
    let mut failed = reply(&[]);
    failed.status = 3;
    let device = Mock::new([failed]);
    let error = write_macro_with_compaction(
        &device,
        &cap(600),
        &reading,
        8,
        &request(),
        &mut sequence(),
        &|| Ok(()),
        &mut |_| panic!(),
    )
    .unwrap_err();
    assert!(!error.progress.device_state_may_have_changed());
}

#[test]
fn cancellation_after_format_retains_mutation_and_stops_reconstruction() {
    let reading = observation(579, false);
    let device = Mock::new([
        storage(0, 4096),
        reply(&[]),
        reply(&[]),
        reply(&[]),
        reply(&[]),
    ]);
    let error = write_macro_with_compaction(
        &device,
        &cap(579),
        &reading,
        8,
        &request(),
        &mut sequence(),
        &|| Ok(()),
        &mut |_| anyhow::bail!("cancelled during format wait"),
    )
    .unwrap_err();
    assert!(error.progress.device_state_may_have_changed());
    assert!(!error.progress.source_rebuild_entered);
    assert!(error.progress.upload.is_none());
    assert_eq!(device.sent().len(), 5);
}

#[test]
fn final_successful_poll_enters_source_branch_without_claiming_format_completion() {
    let reading = observation(600, false);
    let device = Mock::new(reconstruction_replies(&reading, &[1, 1, 1]));
    let report = write_macro_with_compaction(
        &device,
        &cap(600),
        &reading,
        8,
        &request(),
        &mut sequence(),
        &|| Ok(()),
        &mut |_| Ok(()),
    )
    .unwrap();
    assert!(report.source_rebuild_entered);
    assert_eq!(report.format_statuses, [1, 1, 1]);
    assert!(report.upload.is_some());
    assert!(!report.format_confirmed());
    assert!(!report.confirmed());
}

#[test]
fn last_profile_result_controls_format_and_earlier_rejections_remain_visible() {
    let reading = observation(600, false);
    let mut replies = reconstruction_replies(&reading, &[2]);
    replies[1].status = 3;
    let device = Mock::new(replies);
    let report = write_macro_with_compaction(
        &device,
        &cap(600),
        &reading,
        8,
        &request(),
        &mut sequence(),
        &|| Ok(()),
        &mut |_| Ok(()),
    )
    .unwrap();
    assert!(report.source_rebuild_entered);
    assert_eq!(report.steps[0].returned_status, Some(3));
    assert!(!report.confirmed());

    let mut failed = reply(&[]);
    failed.status = 3;
    let device = Mock::new([storage(0, 4096), reply(&[]), failed, storage(0, 4096)]);
    let error = write_macro_with_compaction(
        &device,
        &cap(600),
        &reading,
        8,
        &request(),
        &mut sequence(),
        &|| Ok(()),
        &mut |_| panic!(),
    )
    .unwrap_err();
    assert!(!error.progress.source_rebuild_entered);
    assert_eq!(error.progress.steps.len(), 2);
    assert!(error.progress.storage_after_rebuild.is_some());
}

#[test]
fn unread_profile_metadata_blocks_destructive_reconstruction() {
    let mut reading = observation(600, false);
    reading.snapshot.inventory.slots[0].metadata = ProfileMetadataObservation::Failed {
        message: "real read failed".into(),
    };
    let device = Mock::new([storage(0, 4096)]);
    let error = write_macro_with_compaction(
        &device,
        &cap(600),
        &reading,
        8,
        &request(),
        &mut sequence(),
        &|| Ok(()),
        &mut |_| panic!(),
    )
    .unwrap_err();
    assert!(!error.progress.device_state_may_have_changed());
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn returned_macro_failures_finish_source_blocks_without_claiming_success() {
    let reading = observation(600, false);
    let mut request = request();
    request.data = vec![4; 145];
    let mut replies = vec![storage(2048, 0)];
    for operation in 0..8 {
        if matches!(operation, 1 | 5) {
            let mut rejected = reply(&[]);
            rejected.status = 3;
            replies.extend(std::iter::repeat_n(rejected, 40));
        } else {
            replies.push(reply(&[]));
        }
    }
    replies.push(storage(1536, 0));
    let device = Mock::new(replies);
    let report = write_macro_with_compaction(
        &device,
        &cap(600),
        &reading,
        8,
        &request,
        &mut sequence(),
        &|| Ok(()),
        &mut |_| panic!("Contiguous storage needs no format"),
    )
    .unwrap();
    assert!(!report.confirmed());
    assert!(report.storage_after.is_some());
    let upload = report.upload.unwrap();
    assert!(upload.error.is_some());
    assert_eq!(
        upload
            .report
            .steps
            .iter()
            .filter(|step| step.error.is_some())
            .count(),
        2
    );
    let sent = device.sent();
    assert_eq!(sent.iter().filter(|row| row[7..9] == [6, 12]).count(), 43);
    let blocks = sent
        .iter()
        .filter(|row| row[7..9] == [6, 9])
        .collect::<Vec<_>>();
    assert_eq!(blocks.len(), 42);
    let mut block_layouts = blocks
        .iter()
        .map(|row| (row[14], row[15], row[6]))
        .collect::<Vec<_>>();
    block_layouts.dedup();
    assert_eq!(block_layouts, [(0, 72, 79), (72, 72, 79), (144, 1, 8)]);
    assert!(upload.report.metadata_readback.is_none());
    assert!(upload.report.data_readback.is_none());
}

#[test]
fn thrown_name_failure_is_retained_while_source_continues_with_macro_data() {
    let reading = observation(600, false);
    let mut malformed = reply(&[]);
    malformed.count = 90;
    let device = Mock::new([
        storage(2048, 0),
        reply(&[]),
        malformed,
        reply(&[]),
        storage(1536, 0),
    ]);
    let report = write_macro_with_compaction(
        &device,
        &cap(600),
        &reading,
        8,
        &request(),
        &mut sequence(),
        &|| Ok(()),
        &mut |_| panic!(),
    )
    .unwrap();
    assert!(!report.confirmed());
    let upload = report.upload.unwrap();
    assert!(upload.error.is_some());
    assert_eq!(upload.report.steps.len(), 3);
    assert!(upload.report.steps[1].error.is_some());
    assert!(upload.report.steps[2].error.is_none());
    assert_eq!(device.sent()[3][7..9], [6, 9]);
}

#[test]
fn stale_connection_after_allocation_retains_partial_mutation_and_stops() {
    let reading = observation(600, false);
    let device = Mock::new([storage(2048, 0), reply(&[])]);
    let guard = || {
        ensure!(
            device.sent().len() < 2,
            "physical connection generation changed"
        );
        Ok(())
    };
    let error = write_macro_with_compaction(
        &device,
        &cap(600),
        &reading,
        8,
        &request(),
        &mut sequence(),
        &guard,
        &mut |_| panic!(),
    )
    .unwrap_err();
    assert!(error.progress.device_state_may_have_changed());
    assert!(error.progress.storage_after.is_none());
    assert!(error.progress.upload.unwrap().error.is_some());
    assert_eq!(device.sent().len(), 2);
}

#[test]
fn absent_macro_details_are_read_before_formatting_and_read_failure_prevents_mutation() {
    let mut reading = observation(600, true);
    reading.import_details.macros.clear();
    let mut replies = reconstruction_replies(&reading, &[2]);
    let mut memory = vec![0, 7, 0, 0, 0, 0, 11];
    memory.extend_from_slice(&[1, 50, 2, 50, 17, 0, 21, 0, 0, 0, 0]);
    replies.splice(1..1, [reply(&[0, 7, 0, 0, 0, 11]), reply(&memory)]);
    let device = Mock::new(replies);
    let report = write_macro_with_compaction(
        &device,
        &cap(600),
        &reading,
        8,
        &request(),
        &mut sequence(),
        &|| Ok(()),
        &mut |_| Ok(()),
    )
    .unwrap();
    assert!(report.confirmed());
    let sent = device.sent();
    assert_eq!(sent[1][7..9], [6, 136]);
    assert_eq!(sent[2][7..9], [6, 137]);
    assert_eq!(sent[3][7..9], [5, 3]);
    assert_eq!(
        report.reconstruction.unwrap().macros[0].reading.data,
        observed_macro().data
    );

    let mut failure = reply(&[]);
    failure.status = 3;
    let device = Mock::new([storage(0, 4096), failure]);
    let error = write_macro_with_compaction(
        &device,
        &cap(600),
        &reading,
        8,
        &request(),
        &mut sequence(),
        &|| Ok(()),
        &mut |_| panic!(),
    )
    .unwrap_err();
    assert!(!error.progress.device_state_may_have_changed());
    assert_eq!(device.sent().len(), 2);
}
