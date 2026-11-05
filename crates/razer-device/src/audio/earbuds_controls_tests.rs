use super::*;
use crate::backend::FeatureTransport;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::Duration;

struct NoSleep;
impl Sleeper for NoSleep {
    fn sleep(&self, _: Duration) {}
}

#[derive(Default)]
struct Mock {
    sends: RefCell<Vec<Vec<u8>>>,
    replies: RefCell<VecDeque<Vec<u8>>>,
}

impl FeatureTransport for Mock {
    fn send_feature(&self, report: &[u8]) -> Result<()> {
        self.sends.borrow_mut().push(report.to_vec());
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> Result<usize> {
        let Some(body) = self.replies.borrow_mut().pop_front() else {
            anyhow::bail!("device removed")
        };
        report[0] = 2;
        report[1..1 + body.len()].copy_from_slice(&body);
        Ok(64)
    }
    fn metadata(&self) -> Value {
        Value::Null
    }
}

fn reply(status: u8, tx: u8, class: u8, id: u8, data: &[u8]) -> Vec<u8> {
    let mut body = vec![0u8; 63];
    body[0] = status;
    body[1] = tx;
    body[5] = data.len() as u8;
    body[6] = class;
    body[7] = id;
    body[8..8 + data.len()].copy_from_slice(data);
    body
}

fn ok() -> Result<()> {
    Ok(())
}

fn ambient_reduced() -> AncSetting {
    AncSetting {
        enabled: true,
        mode: AncMode::Ambient,
        level: AncLevel::Reduced,
    }
}

#[test]
fn restore_2657_writes_anc_then_reads_auto_pause_with_device_differing_from_local() {
    // Local profile: ANC ambient/reduced (differs from runtime default) and
    // auto-pause off; device reports auto-pause on.
    let mock = Mock::default();
    mock.replies.borrow_mut().extend([
        reply(2, 0x61, 24, 6, &[1, 80, 1]),
        reply(2, 0x62, 24, 158, &[1, 1]),
        reply(2, 0x63, 24, 30, &[1, 0]),
    ]);
    let mut controls = EarbudsControls::new(2657).unwrap();
    let profile = EarbudsProfileView {
        auto_pause: AutoPauseProfileField {
            is_enabled: Some(false),
        },
        anc: Some(ambient_reduced()),
    };
    controls.load_profile(profile);
    let generation = controls.connect();
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    let result = restore(
        &mut session,
        2657,
        generation,
        controls.anc_runtime_for_task(),
        profile,
        &ok,
    )
    .unwrap();
    let sends = mock.sends.borrow();
    assert_eq!(sends.len(), 3);
    // ANC [3,24,6] first, then auto-pause read [1,24,158], then write [2,24,30].
    assert_eq!(&sends[0][6..12], &[3, 24, 6, 1, 80, 1]);
    assert_eq!(&sends[1][6..10], &[1, 24, 158, 1]);
    assert_eq!(&sends[2][6..11], &[2, 24, 30, 1, 0]);
    drop(sends);
    controls.apply_restore(result).unwrap();
    assert_eq!(
        controls.auto_pause_device,
        AutoPauseObservation::Observed(true)
    );
    assert!(!controls.draft.auto_pause);
    assert_eq!(controls.auto_pause_persisted, Some(false));
    assert_eq!(controls.draft.anc, Some(ambient_reduced()));
    assert_eq!(
        controls.anc_runtime,
        Some(AncRuntimeState(ambient_reduced()))
    );
}

#[test]
fn restore_2660_has_no_anc_traffic() {
    let mock = Mock::default();
    mock.replies
        .borrow_mut()
        .push_back(reply(2, 0x61, 24, 158, &[1, 1]));
    let mut controls = EarbudsControls::new(2660).unwrap();
    assert!(controls.draft.anc.is_none());
    let generation = controls.connect();
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    let result = restore(
        &mut session,
        2660,
        generation,
        AncRuntimeState::default(),
        EarbudsProfileView::default(),
        &ok,
    )
    .unwrap();
    assert!(result.anc.is_none());
    assert_eq!(mock.sends.borrow().len(), 1);
    controls.apply_restore(result).unwrap();
    assert!(controls.draft.auto_pause);
}

#[test]
fn restore_default_anc_skips_io() {
    let mock = Mock::default();
    mock.replies
        .borrow_mut()
        .push_back(reply(2, 0x61, 24, 158, &[1, 1]));
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    let result = restore(
        &mut session,
        2657,
        1,
        AncRuntimeState::default(),
        EarbudsProfileView::default(),
        &ok,
    )
    .unwrap();
    assert_eq!(result.anc.unwrap().0, AncWrite::Skipped);
    assert_eq!(mock.sends.borrow().len(), 1);
}

#[test]
fn disconnect_mid_restore_marks_read_failed_not_off() {
    let mock = Mock::default();
    let mut controls = EarbudsControls::new(2664).unwrap();
    let generation = controls.connect();
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    let profile = EarbudsProfileView {
        auto_pause: AutoPauseProfileField {
            is_enabled: Some(true),
        },
        anc: None,
    };
    let result = restore(
        &mut session,
        2664,
        generation,
        AncRuntimeState::default(),
        profile,
        &ok,
    )
    .unwrap();
    controls.apply_restore(result).unwrap();
    assert!(matches!(
        controls.auto_pause_device,
        AutoPauseObservation::ReadFailed(_)
    ));
    assert!(matches!(
        controls.last_auto_pause_write,
        Some(WriteOutcome::Failed(_))
    ));
    assert!(controls.draft.auto_pause);
}

#[test]
fn reconnect_rejects_old_generation_results() {
    let mut controls = EarbudsControls::new(2668).unwrap();
    let first = controls.connect();
    let task = controls.toggle_auto_pause().unwrap();
    let stale_restore = EarbudsRestore {
        generation: first,
        anc: None,
        auto_pause: None,
    };
    controls.disconnect();
    assert!(controls.toggle_auto_pause().is_none());
    let second = controls.connect();
    assert_ne!(first, second);
    assert!(controls.apply_restore(stale_restore).is_err());
    assert!(
        controls
            .complete(task, EarbudsTaskResult::AutoPause(WriteOutcome::Confirmed))
            .is_err()
    );
    assert_eq!(controls.auto_pause_device, AutoPauseObservation::Unobserved);
}

#[test]
fn canceled_validation_aborts_restore_without_result() {
    let mock = Mock::default();
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    let canceled = || -> Result<()> { anyhow::bail!("canceled") };
    assert!(
        restore(
            &mut session,
            2689,
            1,
            AncRuntimeState::default(),
            EarbudsProfileView::default(),
            &canceled
        )
        .is_err()
    );
    assert!(mock.sends.borrow().is_empty());
}

#[test]
fn toggle_is_optimistic_and_failure_does_not_roll_back() {
    let mock = Mock::default();
    mock.replies
        .borrow_mut()
        .push_back(reply(5, 0x61, 24, 30, &[]));
    let mut controls = EarbudsControls::new(2657).unwrap();
    controls.connect();
    let task = controls.toggle_auto_pause().unwrap();
    assert!(controls.draft.auto_pause);
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    let result = run_task(&mut session, task, controls.anc_runtime_for_task(), &ok).unwrap();
    controls.complete(task, result).unwrap();
    assert!(controls.draft.auto_pause);
    assert!(matches!(
        controls.last_auto_pause_write,
        Some(WriteOutcome::Failed(_))
    ));
    assert_eq!(controls.auto_pause_device, AutoPauseObservation::Unobserved);
}

#[test]
fn disconnected_earbud_disables_actions() {
    let mut controls = EarbudsControls::new(2657).unwrap();
    controls.connect();
    controls.earbud_disconnected = true;
    assert!(controls.toggle_auto_pause().is_none());
    assert!(
        controls
            .change_anc(AncChange {
                enabled: Some(true),
                ..AncChange::default()
            })
            .is_none()
    );
}

#[test]
fn anc_page_gates_follow_source() {
    let mut controls = EarbudsControls::new(2657).unwrap();
    controls.connect();
    // UI initial is disabled: tab and reduce are ignored.
    assert!(
        controls
            .change_anc(AncChange {
                mode: Some(AncMode::Ambient),
                ..AncChange::default()
            })
            .is_none()
    );
    assert!(
        controls
            .change_anc(AncChange {
                level: Some(AncLevel::Reduced),
                ..AncChange::default()
            })
            .is_none()
    );
    let task = controls
        .change_anc(AncChange {
            enabled: Some(true),
            ..AncChange::default()
        })
        .unwrap();
    assert_eq!(
        task,
        EarbudsTask::SetAnc {
            generation: 1,
            setting: proto::ANC_PROFILE_DEFAULT
        }
    );
    controls
        .change_anc(AncChange {
            mode: Some(AncMode::Ambient),
            ..AncChange::default()
        })
        .unwrap();
    // Reduce is inert in Ambient mode.
    assert!(
        controls
            .change_anc(AncChange {
                level: Some(AncLevel::Reduced),
                ..AncChange::default()
            })
            .is_none()
    );
    controls
        .change_anc(AncChange {
            mode: Some(AncMode::Anc),
            ..AncChange::default()
        })
        .unwrap();
    let reduce = controls
        .change_anc(AncChange {
            level: Some(AncLevel::Reduced),
            ..AncChange::default()
        })
        .unwrap();
    assert_eq!(
        reduce,
        EarbudsTask::SetAnc {
            generation: 1,
            setting: AncSetting {
                enabled: true,
                mode: AncMode::Anc,
                level: AncLevel::Reduced
            }
        }
    );
    // Reduce visibility follows the firmware gate.
    controls.firmware_version = Some("1.4.6.1".into());
    assert!(!controls.reduce_supported());
    controls.firmware_version = Some("1.4.7.0".into());
    assert!(controls.reduce_supported());
}

#[test]
fn anc_notification_updates_profile_not_runtime() {
    let mut controls = EarbudsControls::new(2657).unwrap();
    let generation = controls.connect();
    let updated = controls.anc_notification(generation, &[5, 33, 0]).unwrap();
    assert!(!updated.enabled);
    assert_eq!(controls.anc_runtime, Some(AncRuntimeState::default()));
    assert!(
        controls
            .anc_notification(generation + 1, &[5, 33, 1])
            .is_none()
    );
}

#[test]
fn unsupported_products_are_rejected() {
    assert!(EarbudsControls::new(1427).is_err());
}
