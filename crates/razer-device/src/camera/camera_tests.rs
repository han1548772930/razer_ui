use super::*;
use anyhow::{Result, bail};
use std::cell::Cell;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Call {
    Get(bool, u32),
    Set(bool, u32, i32, bool),
}

#[derive(Default)]
struct StandardMock {
    calls: Vec<Call>,
    fail_set: Option<usize>,
    set_count: usize,
    get_hresult: i32,
    flags: i32,
    value: i32,
}
impl StandardMock {
    fn get(&mut self, control: bool, id: u32) -> Result<CameraLongObservation> {
        self.calls.push(Call::Get(control, id));
        CameraLongObservation::from_native(self.value, self.flags, self.get_hresult)
    }
    fn set(&mut self, control: bool, id: u32, value: i32, is_auto: bool) -> Result<CameraWriteAck> {
        self.calls.push(Call::Set(control, id, value, is_auto));
        self.set_count += 1;
        if self.fail_set == Some(self.set_count) {
            bail!("simulated transport failure after entering setter");
        }
        CameraWriteAck::from_hresult(0)
    }
}
impl DirectShowCameraTransport for StandardMock {
    fn get_video_proc_amp(&mut self, id: u32) -> Result<CameraLongObservation> {
        self.get(false, id)
    }
    fn set_video_proc_amp(&mut self, id: u32, value: i32, is_auto: bool) -> Result<CameraWriteAck> {
        self.set(false, id, value, is_auto)
    }
    fn get_camera_control(&mut self, id: u32) -> Result<CameraLongObservation> {
        self.get(true, id)
    }
    fn set_camera_control(&mut self, id: u32, value: i32, is_auto: bool) -> Result<CameraWriteAck> {
        self.set(true, id, value, is_auto)
    }
}
fn target() -> CameraTarget {
    CameraTarget::new(
        3594,
        r"\\?\usb#vid_1532&pid_0e0a&mi_00#instance#{video}\global".into(),
    )
    .unwrap()
}
fn session(mock: StandardMock) -> CameraSession<StandardMock> {
    CameraSession::open(mock, target()).unwrap()
}
fn brightness() -> StandardCameraProperty {
    StandardCameraProperty::VideoProcAmp(VideoProcAmpProperty::Brightness)
}
fn setting(property: StandardCameraProperty) -> StandardCameraSetting {
    StandardCameraSetting {
        property,
        setting: CameraSetting {
            value: -7,
            is_auto: false,
        },
    }
}

#[test]
fn all_twenty_properties_route_to_the_observed_interface_and_id() {
    // Independent IDA expectations, including the two nonstandard IDs.
    let expected = [
        (false, 0),
        (false, 1),
        (false, 2),
        (false, 3),
        (false, 4),
        (false, 5),
        (false, 6),
        (false, 7),
        (false, 8),
        (false, 9),
        (false, 13),
        (true, 0),
        (true, 1),
        (true, 2),
        (true, 3),
        (true, 4),
        (true, 5),
        (true, 6),
        (true, 7),
        (true, 19),
    ];
    let mut s = session(StandardMock::default());
    for (property, (control, id)) in StandardCameraProperty::ALL.into_iter().zip(expected) {
        s.read(property, &|| Ok(())).unwrap();
        s.write(property, -11, true, &|| Ok(())).unwrap();
        assert_eq!(
            s.transport().calls[s.transport().calls.len() - 2..],
            [Call::Get(control, id), Call::Set(control, id, -11, true)]
        );
    }
}
#[test]
fn source_response_preserves_signed_values_flags_and_exact_auto_comparison() {
    for flags in [0, 1, 2, 3, -1] {
        let observed = CameraLongObservation::from_native(-11, flags, 1).unwrap();
        assert_eq!(observed.native_flags, flags);
        assert_eq!(observed.native_hresult, 1);
        assert_eq!(
            observed.source_response(),
            serde_json::json!({"value":-11,"isAuto":flags==1})
        );
    }
}
#[test]
fn failed_get_does_not_publish_initialized_zeroes() {
    let mut s = session(StandardMock {
        get_hresult: i32::MIN + 5,
        ..Default::default()
    });
    assert!(s.read(brightness(), &|| Ok(())).is_err());
    assert_eq!(s.transport().calls, [Call::Get(false, 0)]);
}
#[test]
fn hresult_ack_accepts_nonnegative_values_and_rejects_errors() {
    assert_eq!(CameraWriteAck::from_hresult(1).unwrap().native_hresult, 1);
    assert!(CameraWriteAck::from_hresult(-1).is_err());
    assert!(CameraLongObservation::from_native(0, 0, -1).is_err());
}
#[test]
fn native_set_modes_are_auto_one_and_manual_two() {
    assert_eq!(
        CameraSetting {
            value: -9,
            is_auto: true
        }
        .native_flags(),
        1
    );
    assert_eq!(
        CameraSetting {
            value: -9,
            is_auto: false
        }
        .native_flags(),
        2
    );
}
#[test]
fn target_rejects_other_products_spoofs_and_nul_before_io() {
    for pid in [3595, 3596, 719, 0] {
        assert!(CameraTarget::new(pid, target().device_path.clone()).is_err());
    }
    for path in [
        "camera-name",
        r"\\?\usb#vid_1532&pid_0e0b#instance#{video}",
        r"\\?\usb#vid_1532&pid_0e0a&pid_ffff#instance#{video}",
        r"\\?\usb#vid_1532&pid_0e0aff#instance#{video}",
        r"\\?\usb#vid_ffff&pid_0e0a#instance#{video}",
        "\\\\?\\usb#vid_1532&pid_0e0a#instance\0#{video}",
    ] {
        assert!(CameraTarget::new(3594, path.into()).is_err(), "{path:?}");
    }
    let mut invalid = target();
    invalid.product_id = 3595;
    assert!(CameraSession::open(StandardMock::default(), invalid).is_err());
}
#[test]
fn target_accepts_case_variants_of_full_usb_identity() {
    assert!(CameraTarget::new(3594, target().device_path.to_ascii_uppercase()).is_ok());
}
#[test]
fn standard_camera_family_requires_its_own_exact_pid() {
    for pid in [3594, 3595, 3596] {
        let path = format!(r"\\?\usb#vid_1532&pid_{pid:04x}&mi_00#instance#{{video}}\global");
        let target = CameraTarget::new(pid, path.clone()).unwrap();
        let mut session = CameraSession::open(StandardMock::default(), target).unwrap();
        assert!(
            session
                .write(brightness(), 42, false, &|| Ok(()))
                .unwrap()
                .acknowledgement
                .accepted
        );
        assert!(CameraTarget::new(if pid == 3594 { 3595 } else { 3594 }, path).is_err());
    }
}
#[test]
fn cancellation_before_read_or_write_performs_no_transport_calls() {
    let mut s = session(StandardMock::default());
    let cancel = || -> Result<()> { bail!("cancelled") };
    assert!(s.read(brightness(), &cancel).is_err());
    assert!(s.write(brightness(), 1, false, &cancel).is_err());
    assert!(s.transport().calls.is_empty());
}
#[test]
fn cancellation_after_get_does_not_publish_a_stale_observation() {
    let checks = Cell::new(0);
    let mut s = session(StandardMock::default());
    assert!(
        s.read(brightness(), &|| {
            checks.set(checks.get() + 1);
            if checks.get() == 2 {
                bail!("cancelled")
            }
            Ok(())
        })
        .is_err()
    );
    assert_eq!(s.transport().calls, [Call::Get(false, 0)]);
}
#[test]
fn standard_set_does_not_fabricate_a_readback_or_flash_save() {
    let mut s = session(StandardMock::default());
    let write = s.write(brightness(), 42, false, &|| Ok(())).unwrap();
    assert_eq!(
        write.requested,
        CameraSetting {
            value: 42,
            is_auto: false
        }
    );
    assert!(write.acknowledgement.accepted);
    assert_eq!(s.transport().calls, [Call::Set(false, 0, 42, false)]);
}
#[test]
fn successful_batch_reports_each_ack_and_performs_no_extra_reads() {
    let mut s = session(StandardMock::default());
    let plan: Vec<_> = StandardCameraProperty::ALL
        .into_iter()
        .map(setting)
        .collect();
    let report = s.write_batch(&plan, &|| Ok(()));
    assert!(report.completed && report.error.is_none() && report.unconfirmed_request.is_none());
    assert_eq!(report.acknowledged.len(), 20);
    assert_eq!(s.transport().calls.len(), 20);
}
#[test]
fn partial_failure_retains_previous_ack_and_marks_uncertain_request() {
    let mut s = session(StandardMock {
        fail_set: Some(2),
        ..Default::default()
    });
    let plan = [
        setting(brightness()),
        setting(StandardCameraProperty::CameraControl(
            CameraControlProperty::Exposure,
        )),
        setting(StandardCameraProperty::VideoProcAmp(
            VideoProcAmpProperty::Gain,
        )),
    ];
    let report = s.write_batch(&plan, &|| Ok(()));
    assert!(!report.completed && report.error.is_some());
    assert_eq!(report.acknowledged.len(), 1);
    assert_eq!(report.unconfirmed_request, Some(plan[1]));
    assert_eq!(s.transport().calls.len(), 2);
}
#[test]
fn cancellation_after_ack_retains_it_without_claiming_batch_completion() {
    let mut s = session(StandardMock::default());
    let checks = Cell::new(0);
    let report = s.write_batch(
        &[
            setting(brightness()),
            setting(StandardCameraProperty::VideoProcAmp(
                VideoProcAmpProperty::Gain,
            )),
        ],
        &|| {
            checks.set(checks.get() + 1);
            if checks.get() == 2 {
                bail!("cancelled")
            }
            Ok(())
        },
    );
    assert!(!report.completed && report.error.is_some());
    assert_eq!(report.acknowledged.len(), 1);
    assert!(report.unconfirmed_request.is_none());
    assert_eq!(s.transport().calls.len(), 1);
}
#[test]
fn empty_duplicate_and_oversized_plans_are_rejected_before_io() {
    let mut s = session(StandardMock::default());
    for plan in [
        vec![],
        vec![setting(brightness()); 2],
        vec![setting(brightness()); 21],
    ] {
        let report = s.write_batch(&plan, &|| Ok(()));
        assert!(!report.completed && report.error.is_some() && report.acknowledged.is_empty());
    }
    assert!(s.transport().calls.is_empty());
}
#[test]
fn transport_is_dropped_when_its_owned_session_is_closed() {
    struct Lifetime<'a>(&'a Cell<bool>);
    impl Drop for Lifetime<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let dropped = Cell::new(false);
    let s = CameraSession::open(Lifetime(&dropped), target()).unwrap();
    assert!(!dropped.get());
    drop(s);
    assert!(dropped.get());
}
