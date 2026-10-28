//! Simulated transport/storage only; never opens a device or loads vendor code.
use super::{Backend, Controller, Response};
use razer_device::dpi_matcher::{Calibration, Phase};
use serde_json::{Value, json};

struct SimulatedBackend {
    product_id: u32,
    data: Value,
    calls: Vec<&'static str>,
    fail: Option<&'static str>,
    identity: Option<(String, Value)>,
    expected_dpi: Value,
}
impl Default for SimulatedBackend {
    fn default() -> Self {
        Self {
            product_id: 190,
            data: json!({
                "version":1,"activeProfile":"p",
                "profiles":[{"guid":"p","dpiStages":{"active":1,"enable":true,"stages":[
                    {"x":800,"y":800,"independent":false,"visible":true},
                    {"x":1600,"y":1600,"independent":false,"visible":true},
                    {"x":2400,"y":2400,"independent":false,"visible":true},
                    {"x":3200,"y":3200,"independent":false,"visible":true},
                    {"x":4000,"y":4000,"independent":false,"visible":true}
                ]}}],
                "sensitivityMatcher":{"profiles":[{"guid":"match","name":"Observed","dpi":{"x":1200,"y":1200,"independent":false,"visible":true}}]}
            }),
            calls: vec![],
            fail: None,
            identity: None,
            expected_dpi: json!(1200),
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
        let draft = super::worker::stage_draft(stages)?;
        let packed = razer_device::mouse_current_dpi::pack_for(self.product_id, &draft)?;
        assert_eq!(packed.active_stage, 1);
        assert_eq!(packed.number_of_active_stages, 5);
        assert_eq!(packed.records.len(), 5);
        assert_eq!(json!(packed.records[0].x), self.expected_dpi);
        assert_eq!(packed.records[1].x, 1600);
        assert_eq!(packed.records[4].x, 4000);
        assert_eq!(stages["active"], 1);
        assert_eq!(stages["stages"][0]["x"], self.expected_dpi);
        Ok(())
    }
    fn persist_source_data(&mut self, data: &Value) -> anyhow::Result<()> {
        self.call("persist")?;
        self.data = data.clone();
        Ok(())
    }
    fn confirmation_identity(
        &mut self,
        container: &str,
        product: Option<u32>,
    ) -> anyhow::Result<(String, Value)> {
        self.call("identity")?;
        assert_eq!(container, "source");
        assert_eq!(product, Some(123));
        self.identity
            .clone()
            .ok_or_else(|| anyhow::anyhow!("test has not supplied a measured device identity"))
    }
}

fn calibration(response: &Response) -> Calibration {
    let Response::Calibration(value) = response else {
        panic!("expected a calibration observation");
    };
    *value
}

fn profiles(response: &Response) -> &razer_device::dpi_matcher_storage::ProfileUpdate {
    let Response::Profiles(value) = response else {
        panic!("expected a persisted profile update");
    };
    value
}

#[test]
fn dpi_matcher_get_state_does_not_probe_or_write_a_device() {
    let mut controller = Controller::default();
    let mut backend = SimulatedBackend::default();
    let reply = controller.command("GET_STATE", None, &mut backend).unwrap();
    assert_eq!(calibration(&reply[0]).phase(), Phase::Ready);
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
    assert_eq!(profiles(&reply[0]).selected(), Some("match"));
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
fn dpi_matcher_lock_failure_stops_monitoring_and_returns_to_source_idle() {
    let mut controller = Controller::default();
    let mut backend = SimulatedBackend {
        fail: Some("lock"),
        ..Default::default()
    };
    assert!(controller.command("START", None, &mut backend).is_err());
    assert_eq!(backend.calls, ["read", "start", "lock", "stop"]);
    // Current 41374 requestingLock.onError targets idle; a transport diagnostic
    // does not manufacture the source's calibration-error state.
    assert_eq!(controller.response().phase(), Phase::Ready);
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
        assert_eq!(calibration(&replies[0]).phase(), Phase::Ready);
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

fn input(
    container: &str,
    product_id: u32,
    rows: &[(f64, f64)],
) -> razer_device::dpi_matcher::InputBatch {
    use razer_device::dpi_matcher::{InputBatch, Movement};
    InputBatch {
        product_id,
        container_id: container.into(),
        movement: rows
            .iter()
            .map(|&(time_tick, x)| Movement { x, time_tick })
            .collect(),
    }
}

fn await_stop(controller: &mut Controller, backend: &mut SimulatedBackend, product_id: u32) {
    controller.command("START", None, backend).unwrap();
    for (batch, main) in [
        (input("source", 123, &[(20., 1000.)]), false),
        (
            input(
                "main",
                product_id,
                &[(10., 1000.), (20., 2000.), (30., 2500.)],
            ),
            true,
        ),
        (input("source", 123, &[(30., 1250.)]), false),
    ] {
        let response = controller.ingest(batch, main, 0, backend).unwrap();
        assert_eq!(response.phase(), Phase::Calibrating);
    }
    backend.calls.clear();
}

#[test]
fn dpi_matcher_progress_snapshots_advance_without_extra_callbacks_or_early_completion() {
    for product_id in [190, 192, 222, 226, 229, 239] {
        let mut controller =
            Controller::new(razer_device::dpi_matcher::Product::new(product_id).unwrap());
        let mut backend = SimulatedBackend::default();
        controller.command("START", None, &mut backend).unwrap();
        controller
            .ingest(input("source", 123, &[(1., 100.)]), false, 0, &mut backend)
            .unwrap();
        let halfway = controller
            .ingest(
                input("main", product_id, &[(1., 2000.)]),
                true,
                1,
                &mut backend,
            )
            .unwrap();
        assert_eq!(halfway.process(), 50.);
        let state = controller.command("GET_STATE", None, &mut backend).unwrap();
        assert_eq!(state, [Response::Calibration(halfway)]);
        let threshold = controller
            .ingest(
                input("main", product_id, &[(2., 2500.)]),
                true,
                2,
                &mut backend,
            )
            .unwrap();
        assert_eq!(threshold.process(), 99.);
        assert_eq!(threshold.phase(), Phase::Calibrating);
        assert!(controller.session.new_dpi().is_none());
        assert_eq!(backend.calls, ["read", "start", "lock"]);
        assert!(controller.monitoring && controller.locked);
        let canceled = controller.command("CANCEL", None, &mut backend).unwrap();
        assert_eq!(backend.calls, ["read", "start", "lock", "stop", "unlock"]);
        assert_eq!(calibration(&canceled[0]).process(), 0.);
        assert_eq!(calibration(&canceled[0]).phase(), Phase::Ready);
        assert!(!controller.monitoring && !controller.locked);
    }
}

#[test]
fn dpi_matcher_completed_requires_successful_stop_then_releases_the_capture_lock() {
    for product_id in [190, 192, 222, 226, 229, 239] {
        let mut controller =
            Controller::new(razer_device::dpi_matcher::Product::new(product_id).unwrap());
        let mut backend = SimulatedBackend::default();
        await_stop(&mut controller, &mut backend, product_id);
        assert!(controller.session.new_dpi().is_none());
        let response = controller
            .ingest(
                input("main", product_id, &[(40., -100.)]),
                true,
                1,
                &mut backend,
            )
            .unwrap();
        assert_eq!(backend.calls, ["stop", "unlock"]);
        assert_eq!(response.phase(), Phase::Completed);
        assert_eq!(response.new_dpi(), 400.);
        assert!(!controller.monitoring && !controller.locked);
        assert!(controller.tick(50_000, &mut backend).unwrap().is_none());
        assert_eq!(backend.calls, ["stop", "unlock"]);
    }
}

#[test]
fn dpi_matcher_confirm_names_and_write_failures_preserve_the_measured_session() {
    for product_id in [190, 192, 222, 226, 229, 239] {
        for failure in [None, Some("identity"), Some("submit"), Some("persist")] {
            let mut controller =
                Controller::new(razer_device::dpi_matcher::Product::new(product_id).unwrap());
            let name = if product_id == 226 {
                json!("Observed")
            } else {
                json!({"en":"Observed","zh-cn":"Localized"})
            };
            let mut backend = SimulatedBackend {
                product_id,
                identity: Some(("measured".into(), name.clone())),
                expected_dpi: json!(400),
                ..Default::default()
            };
            await_stop(&mut controller, &mut backend, product_id);
            controller
                .ingest(
                    input("main", product_id, &[(40., -100.)]),
                    true,
                    1,
                    &mut backend,
                )
                .unwrap();
            backend.calls.clear();
            backend.fail = failure;
            let before = backend.data.clone();
            let result = controller.command("CONFIRM", None, &mut backend);
            if let Some(failure) = failure {
                assert!(result.is_err());
                assert_eq!(backend.data, before);
                assert_eq!(controller.session.new_dpi(), Some(400.));
                let expected = match failure {
                    "identity" => vec!["read", "identity"],
                    "submit" => vec!["read", "identity", "submit"],
                    _ => vec!["read", "identity", "submit", "persist"],
                };
                assert_eq!(backend.calls, expected);
            } else {
                let result = result.unwrap();
                assert_eq!(backend.calls, ["read", "identity", "submit", "persist"]);
                assert_eq!(
                    backend.data["sensitivityMatcher"]["profiles"][1]["name"],
                    if product_id == 226 {
                        json!("Observed 1")
                    } else {
                        name
                    }
                );
                assert_eq!(
                    backend.data["profiles"][0]["dpiStages"]["stages"][0]["x"],
                    400
                );
                assert_eq!(profiles(&result[0]).selected(), Some("measured"));
                assert!(controller.session.new_dpi().is_none());
            }
            assert!(!controller.monitoring && !controller.locked);
        }
    }
}

#[test]
fn dpi_matcher_failed_stop_cannot_publish_or_confirm_a_premature_result() {
    for product_id in [190, 192, 222, 226, 229, 239] {
        let mut controller =
            Controller::new(razer_device::dpi_matcher::Product::new(product_id).unwrap());
        let mut backend = SimulatedBackend::default();
        await_stop(&mut controller, &mut backend, product_id);
        backend.fail = Some("stop");
        assert!(
            controller
                .ingest(
                    input("main", product_id, &[(40., -100.)]),
                    true,
                    1,
                    &mut backend
                )
                .is_err()
        );
        assert_eq!(backend.calls, ["stop"]);
        assert!(controller.monitoring && controller.locked);
        assert_eq!(controller.session.new_dpi(), None);
        assert_eq!(controller.response().phase(), Phase::Calibrating);
        assert_eq!(controller.response().new_dpi(), 0.);
        assert!(controller.command("CONFIRM", None, &mut backend).is_err());
        assert_eq!(backend.calls, ["stop"]);
        assert!(controller.tick(50_000, &mut backend).unwrap().is_none());
        backend.fail = None;
        let replies = controller.command("CANCEL", None, &mut backend).unwrap();
        assert_eq!(backend.calls, ["stop", "stop", "unlock"]);
        assert_eq!(calibration(&replies[0]).phase(), Phase::Ready);
        assert_eq!(controller.session.new_dpi(), None);
        assert!(!controller.monitoring && !controller.locked);
    }
}

#[test]
fn dpi_matcher_failed_post_stop_calculation_still_releases_the_capture_lock() {
    let mut controller = Controller::default();
    let mut backend = SimulatedBackend::default();
    controller.command("START", None, &mut backend).unwrap();
    assert!(
        controller
            .ingest(input("source", 123, &[(5., 0.)]), false, 0, &mut backend)
            .unwrap()
            .phase()
            == Phase::Calibrating
    );
    assert!(
        controller
            .ingest(input("main", 190, &[(10., 5000.)]), true, 1, &mut backend)
            .unwrap()
            .phase()
            == Phase::Calibrating
    );
    assert!(
        controller
            .ingest(input("source", 123, &[(20., 0.)]), false, 2, &mut backend)
            .unwrap()
            .phase()
            == Phase::Calibrating
    );
    let result = controller.ingest(input("main", 190, &[(21., 0.)]), true, 3, &mut backend);
    assert!(result.is_err());
    assert_eq!(backend.calls, ["read", "start", "lock", "stop", "unlock"]);
    assert!(!controller.monitoring && !controller.locked);
    assert_eq!(controller.session.new_dpi(), None);
    assert_eq!(controller.response().phase(), Phase::Calibrating);
}
