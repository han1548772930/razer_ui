//! Callback queue and session rejection tests only; no DLL is loaded.
use super::*;
use serde_json::json;

fn callback(temperature: f64) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "event":"physical-device", "dllName":"RzSystemCommon",
        "dllFunc":"HardwareMonitorCallback", "dataType":0,"status":"SUCCESS",
        "CPUTemperature":temperature,"CPUClock":4200.0,
        "GPUTemperature":null,"GPUClock":null
    }))
    .unwrap()
}

#[test]
fn system_monitor_queue_preserves_each_temperature_change_and_poll_boundary() {
    let mut queue = CallbackQueue {
        container_id: "physical-device".into(),
        ..Default::default()
    };
    for temperature in [35.0, 36.0, 35.0] {
        queue.accept(&callback(temperature));
    }
    let first = queue.drain("current-session", 2);
    first.validate_for("current-session").unwrap();
    assert_eq!(
        first
            .samples
            .iter()
            .map(|sample| sample.cpu_temperature)
            .collect::<Vec<_>>(),
        [Some(35.0), Some(36.0)]
    );
    let last = queue.drain("current-session", 2);
    assert_eq!(last.samples.len(), 1);
    assert_eq!(last.samples[0].cpu_temperature, Some(35.0));
    assert!(queue.drain("current-session", 2).samples.is_empty());
}

#[test]
fn system_monitor_queue_reports_malformed_callbacks_without_losing_valid_samples() {
    let mut queue = CallbackQueue {
        container_id: "physical-device".into(),
        ..Default::default()
    };
    queue.accept(&callback(35.0));
    queue.accept(br#"{"event":"old-device"}"#);
    queue.accept(&callback(36.0));
    let observation = queue.drain("current-session", usize::MAX);
    assert_eq!(observation.samples.len(), 2);
    assert_eq!(observation.diagnostics.len(), 1);
    assert!(queue.drain("current-session", 1).diagnostics.is_empty());
}

#[test]
fn system_monitor_rejects_unknown_stop_and_invalid_start_before_any_native_load() {
    let mut state = State::default();
    assert!(state.poll("previous-owner").is_err());
    assert!(state.stop("previous-owner").is_err());
    assert!(state.prepare("owner".into(), "device".into(), 123).is_err());
    assert!(state.prepare("".into(), "device".into(), 3893).is_err());
    assert!(state.prepare("owner".into(), "".into(), 3893).is_err());
    assert!(state.active.is_none());
    assert!(state.shutdown().unwrap().is_none());
}

#[derive(Default)]
struct SimulatedCleanup {
    steps: Vec<&'static str>,
    fail_once: Option<&'static str>,
}

impl SimulatedCleanup {
    fn step(&mut self, name: &'static str) -> anyhow::Result<()> {
        self.steps.push(name);
        if self.fail_once == Some(name) {
            self.fail_once = None;
            bail!("simulated {name} failure");
        }
        Ok(())
    }
}

impl Cleanup for SimulatedCleanup {
    fn unregister(&mut self) -> anyhow::Result<()> {
        self.step("unregister")
    }
    fn detach_callback(&mut self) -> anyhow::Result<()> {
        self.step("detach_callback")
    }
    fn device_terminate(&mut self) -> anyhow::Result<()> {
        self.step("device_terminate")
    }
    fn retain_window(&mut self) -> anyhow::Result<()> {
        self.step("retain_window")
    }
    fn terminate(&mut self) -> anyhow::Result<()> {
        self.step("terminate")
    }
    fn join_and_release(&mut self) -> anyhow::Result<()> {
        self.step("join_and_release")
    }
}

fn started() -> Lifecycle {
    Lifecycle {
        initialize_attempted: true,
        device_initialized: true,
        callback_attached: true,
        registration_attempted: true,
        ..Default::default()
    }
}

#[test]
fn system_monitor_cleanup_unregisters_then_terminates_then_joins_before_release() {
    let mut lifecycle = started();
    let mut operations = SimulatedCleanup::default();
    cleanup(&mut lifecycle, &mut operations).unwrap();
    assert_eq!(
        operations.steps,
        [
            "unregister",
            "detach_callback",
            "device_terminate",
            "retain_window",
            "terminate",
            "join_and_release"
        ]
    );
    assert!(lifecycle.released);
    let completed = operations.steps.clone();
    cleanup(&mut lifecycle, &mut operations).unwrap();
    assert_eq!(operations.steps, completed);
}

#[test]
fn system_monitor_cleanup_does_not_skip_termination_after_unregister_failure() {
    let mut lifecycle = started();
    let mut operations = SimulatedCleanup {
        fail_once: Some("unregister"),
        ..Default::default()
    };
    assert!(cleanup(&mut lifecycle, &mut operations).is_err());
    assert!(operations.steps.contains(&"terminate"));
    assert!(operations.steps.contains(&"join_and_release"));
    assert!(lifecycle.released);
}

#[test]
fn system_monitor_cleanup_keeps_failed_termination_owned_for_retry() {
    let mut lifecycle = started();
    let mut operations = SimulatedCleanup {
        fail_once: Some("terminate"),
        ..Default::default()
    };
    assert!(cleanup(&mut lifecycle, &mut operations).is_err());
    assert!(!lifecycle.terminated);
    assert!(!lifecycle.released);
    assert!(!operations.steps.contains(&"join_and_release"));
    operations.steps.clear();
    cleanup(&mut lifecycle, &mut operations).unwrap();
    assert_eq!(
        operations.steps,
        ["retain_window", "terminate", "join_and_release"]
    );
}

#[test]
fn system_monitor_cleanup_retries_window_join_without_repeating_terminate() {
    let mut lifecycle = started();
    let mut operations = SimulatedCleanup {
        fail_once: Some("join_and_release"),
        ..Default::default()
    };
    assert!(cleanup(&mut lifecycle, &mut operations).is_err());
    assert!(lifecycle.terminated);
    assert!(!lifecycle.released);
    operations.steps.clear();
    cleanup(&mut lifecycle, &mut operations).unwrap();
    assert_eq!(operations.steps, ["join_and_release"]);
}

#[test]
fn system_monitor_partial_initialize_failure_still_terminates_and_joins() {
    let mut lifecycle = Lifecycle {
        initialize_attempted: true,
        ..Default::default()
    };
    let mut operations = SimulatedCleanup {
        fail_once: Some("retain_window"),
        ..Default::default()
    };
    assert!(cleanup(&mut lifecycle, &mut operations).is_err());
    assert_eq!(
        operations.steps,
        ["retain_window", "terminate", "join_and_release"]
    );
}
