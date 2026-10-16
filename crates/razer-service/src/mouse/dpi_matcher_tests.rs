//! Simulated transport/storage only; never opens a device or loads vendor code.
use super::{Backend, Controller};
use serde_json::{Value, json};

struct SimulatedBackend {
    data: Value,
    calls: Vec<&'static str>,
    fail: Option<&'static str>,
}
impl Default for SimulatedBackend {
    fn default() -> Self {
        Self {
            data: json!({
                "version":1,"activeProfile":"p",
                "profiles":[{"guid":"p","dpiStages":{"active":1,"enable":true,"stages":[{"x":800,"y":800,"independent":false,"visible":true}]}}],
                "sensitivityMatcher":{"profiles":[{"guid":"match","name":"Observed","dpi":{"x":1200,"y":1200,"independent":false,"visible":true}}]}
            }),
            calls: vec![],
            fail: None,
        }
    }
}
impl SimulatedBackend {
    fn call(&mut self, name: &'static str) -> anyhow::Result<()> {
        self.calls.push(name);
        anyhow::ensure!(self.fail != Some(name), "simulated {name} failure");
        Ok(())
    }
}
impl Backend for SimulatedBackend {
    fn read_source_data(&mut self) -> anyhow::Result<Value> {
        self.call("read")?;
        Ok(self.data.clone())
    }
    fn start_input_monitoring(&mut self) -> anyhow::Result<()> {
        self.call("start")
    }
    fn stop_input_monitoring(&mut self) -> anyhow::Result<()> {
        self.call("stop")
    }
    fn acquire_matcher_locks(&mut self) -> anyhow::Result<()> {
        self.call("lock")
    }
    fn release_matcher_locks(&mut self) -> anyhow::Result<()> {
        self.call("unlock")
    }
    fn submit_dpi_stages(&mut self, stages: &Value) -> anyhow::Result<()> {
        self.call("submit")?;
        assert_eq!(stages["active"], 1);
        assert_eq!(stages["stages"][0]["x"], 1200);
        Ok(())
    }
    fn persist_source_data(&mut self, data: &Value) -> anyhow::Result<()> {
        self.call("persist")?;
        self.data = data.clone();
        Ok(())
    }
    fn confirmation_identity(
        &mut self,
        _: &str,
        _: Option<u32>,
    ) -> anyhow::Result<(String, Value)> {
        anyhow::bail!("test has not supplied a measured device identity")
    }
}

#[test]
fn dpi_matcher_get_state_does_not_probe_or_write_a_device() {
    let mut controller = Controller::default();
    let mut backend = SimulatedBackend::default();
    let reply = controller.command("GET_STATE", None, &mut backend).unwrap();
    assert_eq!(reply[0]["type"], "MW_SET_DPI_MATCHER_STATE");
    assert_eq!(reply[0]["payload"]["state"], "ready");
    assert!(backend.calls.is_empty());
    assert!(controller.command("CONFIRM", None, &mut backend).is_err());
    assert!(backend.calls.is_empty());
}

#[test]
fn dpi_matcher_selection_submits_stages_before_durable_storage_and_response() {
    let mut controller = Controller::default();
    let mut backend = SimulatedBackend::default();
    let reply = controller.select("match", &mut backend).unwrap();
    assert_eq!(backend.calls, ["read", "submit", "persist"]);
    assert_eq!(
        backend.data["profiles"][0]["selectedSensitivityMatcher"],
        "match"
    );
    assert_eq!(
        backend.data["profiles"][0]["dpiStages"]["stages"][0]["x"],
        1200
    );
    assert_eq!(reply[0]["type"], "MW_SET_SENSITIVITY_PROFILE_TO_UI");
    assert_eq!(reply[0]["payload"]["selectedSensitivityMatcher"], "match");
}

#[test]
fn dpi_matcher_write_and_storage_failures_do_not_acknowledge() {
    for fail in ["submit", "persist"] {
        let mut controller = Controller::default();
        let mut backend = SimulatedBackend {
            fail: Some(fail),
            ..Default::default()
        };
        let before = backend.data.clone();
        assert!(controller.select("match", &mut backend).is_err());
        assert_eq!(backend.data, before);
        let expected = if fail == "submit" {
            vec!["read", "submit"]
        } else {
            vec!["read", "submit", "persist"]
        };
        assert_eq!(backend.calls, expected);
    }
}

#[test]
fn dpi_matcher_lock_failure_stops_monitoring_and_preserves_error() {
    let mut controller = Controller::default();
    let mut backend = SimulatedBackend {
        fail: Some("lock"),
        ..Default::default()
    };
    assert!(controller.command("START", None, &mut backend).is_err());
    assert_eq!(backend.calls, ["read", "start", "lock", "stop"]);
    assert_eq!(controller.response()["payload"]["state"], "error");
    assert!(!controller.monitoring);
    assert!(!controller.locked);
}

#[test]
fn dpi_matcher_cancel_attempts_both_releases_and_retries_only_failed_one() {
    for fail in ["stop", "unlock"] {
        let mut controller = Controller::default();
        let mut backend = SimulatedBackend::default();
        controller.command("START", None, &mut backend).unwrap();
        backend.calls.clear();
        backend.fail = Some(fail);
        assert!(controller.command("CANCEL", None, &mut backend).is_err());
        assert_eq!(backend.calls, ["stop", "unlock"]);
        assert_eq!(controller.monitoring, fail == "stop");
        assert_eq!(controller.locked, fail == "unlock");
        backend.calls.clear();
        backend.fail = None;
        let replies = controller.command("CANCEL", None, &mut backend).unwrap();
        assert_eq!(backend.calls, [fail]);
        assert_eq!(replies[0]["payload"]["state"], "ready");
        assert!(!controller.monitoring && !controller.locked);
    }
}

#[test]
fn dpi_matcher_delete_all_preserves_dpi_and_never_submits_a_stage() {
    let mut controller = Controller::default();
    let mut backend = SimulatedBackend::default();
    let dpi = backend.data["profiles"][0]["dpiStages"].clone();
    controller
        .command("DELETE_ALL_PROFILES", None, &mut backend)
        .unwrap();
    assert_eq!(backend.calls, ["read", "persist"]);
    assert_eq!(backend.data["profiles"][0]["dpiStages"], dpi);
    assert_eq!(backend.data["sensitivityMatcher"]["profiles"], json!([]));
}
