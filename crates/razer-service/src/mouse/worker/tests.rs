use super::*;
use razer_device::mouse_rotation::{Reading, Report};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

struct Backend {
    calls: Rc<RefCell<Vec<&'static str>>>,
    fail_shutdown: bool,
    termination_failures: usize,
}
impl Cleanup for Backend {
    fn stop(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("stop");
        Ok(())
    }
    fn shutdown(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("shutdown");
        ensure!(!self.fail_shutdown, "shutdown acknowledgement lost");
        Ok(())
    }
    fn terminate(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("terminate");
        if self.termination_failures > 0 {
            self.termination_failures -= 1;
            anyhow::bail!("pipe workers remain owned");
        }
        Ok(())
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        self.calls.borrow_mut().push("drop");
    }
}

fn reading() -> Report {
    Report::Read {
        reading: Reading {
            raw_state: 1,
            raw_angle: (-17_i8) as u8,
        },
    }
}

#[test]
fn canceled_before_admission_never_creates_a_service_client() {
    let completion = device_with::<Report, Backend>(
        &AtomicBool::new(false),
        || panic!("canceled owner must not create a client"),
        |_| panic!("canceled owner must not access the device"),
        || panic!("no process exists to wait for"),
    );
    assert!(completion.result.unwrap_err().contains("canceled"));
    assert!(completion.cleanup_failure.is_none());
}

#[test]
fn cancellation_after_device_reply_discards_success_but_releases_the_actual_owner() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let active = AtomicBool::new(true);
    let completion = device_with(
        &active,
        || {
            Ok(Backend {
                calls: calls.clone(),
                fail_shutdown: false,
                termination_failures: 0,
            })
        },
        |backend| {
            backend.calls.borrow_mut().push("request");
            active.store(false, Ordering::Release);
            Ok(reading())
        },
        || panic!("termination succeeded immediately"),
    );
    assert!(completion.result.unwrap_err().contains("canceled"));
    assert!(completion.cleanup_failure.is_none());
    assert_eq!(
        *calls.borrow(),
        ["request", "stop", "shutdown", "terminate", "drop"]
    );
}

#[test]
fn valid_device_result_survives_cleanup_failure_and_release_waits_for_termination() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let waits = Cell::new(0);
    let completion = device_with(
        &AtomicBool::new(true),
        || {
            Ok(Backend {
                calls: calls.clone(),
                fail_shutdown: true,
                termination_failures: 1,
            })
        },
        |_| Ok(reading()),
        || {
            assert!(!calls.borrow().contains(&"drop"));
            waits.set(waits.get() + 1);
        },
    );
    assert_eq!(
        completion.result.unwrap(),
        Report::Read {
            reading: Reading {
                raw_state: 1,
                raw_angle: 239
            }
        }
    );
    let cleanup = completion.cleanup_failure.unwrap();
    assert!(cleanup.contains("shutdown acknowledgement lost"));
    assert!(cleanup.contains("pipe workers remain owned"));
    assert_eq!(waits.get(), 1);
    assert_eq!(
        *calls.borrow(),
        ["stop", "shutdown", "terminate", "terminate", "drop"]
    );
}

#[test]
fn transport_failure_or_unwind_still_performs_shutdown_and_process_cleanup() {
    for unwind in [false, true] {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let completion = device_with::<Report, Backend>(
            &AtomicBool::new(true),
            || {
                Ok(Backend {
                    calls: calls.clone(),
                    fail_shutdown: false,
                    termination_failures: 0,
                })
            },
            |_| {
                if unwind {
                    panic!("transport unwind");
                }
                anyhow::bail!("device rejected write")
            },
            || panic!("termination succeeded immediately"),
        );
        assert!(completion.result.unwrap_err().contains(if unwind {
            "panicked"
        } else {
            "device rejected write"
        }));
        assert!(completion.cleanup_failure.is_none());
        assert_eq!(*calls.borrow(), ["stop", "shutdown", "terminate", "drop"]);
    }
}

#[test]
fn mouse_scroll_partial_device_readbacks_survive_cleanup_failure_and_actual_termination() {
    use razer_device::mouse_scroll::{ScrollChange, ScrollValue, Submission, SubmissionReport};
    let requested = Submission::new(
        vec![
            ScrollChange::VirtualAcceleration(true),
            ScrollChange::SmartReel(true),
        ],
        Default::default(),
    );
    let report: SubmissionReport = serde_json::from_value(serde_json::json!({
        "requested":requested,"errors":["first task readback differs"],"reports":[
            {"steps":[{"profile_id":1,"requested":ScrollValue::VirtualAcceleration(true),
                "acknowledgment_received":true,"acknowledged":null,
                "observed":ScrollValue::VirtualAcceleration(false)}]},
            {"steps":[{"profile_id":1,"requested":ScrollValue::SmartReel(true),
                "acknowledgment_received":true,"acknowledged":null,
                "observed":ScrollValue::SmartReel(true)}]}
        ]
    }))
    .unwrap();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let completion = device_with(
        &AtomicBool::new(true),
        || {
            Ok(Backend {
                calls: calls.clone(),
                fail_shutdown: true,
                termination_failures: 1,
            })
        },
        |_| Ok(report),
        || assert!(!calls.borrow().contains(&"drop")),
    );
    let report = completion.result.unwrap();
    assert_eq!(report.requested, requested);
    assert!(report.validate_for(170).is_ok());
    assert!(!report.verified_for(170));
    assert_eq!(
        report.reports[0].steps[0].observed,
        Some(ScrollValue::VirtualAcceleration(false))
    );
    assert!(report.reports[1].verified());
    assert!(
        completion
            .cleanup_failure
            .unwrap()
            .contains("shutdown acknowledgement lost")
    );
    assert_eq!(
        *calls.borrow(),
        ["stop", "shutdown", "terminate", "terminate", "drop"]
    );
}
