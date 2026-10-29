//! Simulated transport/storage only; never opens a device or loads vendor code.
use super::{Backend, Controller, Response};
use razer_device::{
    dpi_matcher::{Calibration, Phase},
    dpi_matcher_storage::{Change, ProfileName, SourceData},
    mouse_current_dpi::SourceStages,
};
use serde_json::json;

struct SimulatedBackend {
    product_id: u32,
    data: SourceData,
    version: u64,
    calls: Vec<&'static str>,
    fail: Option<&'static str>,
    identity: Option<(String, ProfileName)>,
    expected_dpi: f64,
    failures: Vec<String>,
    drafts: Vec<SourceStages>,
    calibrations: Vec<Calibration>,
    persisted: Vec<Change>,
}
impl Default for SimulatedBackend {
    fn default() -> Self {
        Self {
            product_id: 190,
            data: SourceData::from_document(&json!({
                "version":1,"activeProfile":"p",
                "profiles":[{"guid":"p","dpiStages":{"active":1,"enable":true,"stages":[
                    {"x":800,"y":800,"independent":false,"visible":true},
                    {"x":1600,"y":1600,"independent":false,"visible":true},
                    {"x":2400,"y":2400,"independent":false,"visible":true},
                    {"x":3200,"y":3200,"independent":false,"visible":true},
                    {"x":4000,"y":4000,"independent":false,"visible":true}
                ]}}],
                "sensitivityMatcher":{"profiles":[{"guid":"match","name":"Observed","dpi":{"x":1200,"y":1200,"independent":false,"visible":true}}]}
            })).unwrap(),
            version: 1,
            calls: vec![],
            fail: None,
            identity: None,
            expected_dpi: 1200.,
            failures: vec![],
            drafts: vec![],
            calibrations: vec![],
            persisted: vec![],
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
    fn read_source_data(&mut self) -> anyhow::Result<SourceData> {
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
    fn submit_dpi_stages(&mut self, stages: &SourceStages) -> anyhow::Result<()> {
        self.call("submit")?;
        assert_eq!(self.data.stages(), Some(stages));
        let packed = razer_device::mouse_current_dpi::pack_for(self.product_id, stages)?;
        assert_eq!(packed.active_stage, 1);
        assert_eq!(packed.number_of_active_stages, 5);
        assert_eq!(packed.records.len(), 5);
        assert_eq!(packed.records[0].x.unwrap().value(), self.expected_dpi);
        assert_eq!(packed.records[1].x.unwrap().value(), 1600.);
        assert_eq!(packed.records[4].x.unwrap().value(), 4000.);
        assert_eq!(stages.active_stage(), 1);
        assert_eq!(stages.stages()[0].x().unwrap().value(), self.expected_dpi);
        Ok(())
    }
    fn persist_source_data(&mut self, change: &Change) -> anyhow::Result<()> {
        self.call(match change {
            Change::Stages(_) => "persist_stages",
            Change::Profiles(_) => "persist_profiles",
        })?;
        self.data.apply(change);
        self.version += 1;
        self.persisted.push(change.clone());
        Ok(())
    }
    fn report_failure(&mut self, error: String) -> anyhow::Result<()> {
        self.call("failure")?;
        self.failures.push(error);
        Ok(())
    }
    fn publish_stage_draft(&mut self, stages: &SourceStages) -> anyhow::Result<()> {
        self.call("draft")?;
        self.drafts.push(stages.clone());
        Ok(())
    }
    fn publish_calibration(&mut self, calibration: Calibration) -> anyhow::Result<()> {
        self.call("calibration")?;
        self.calibrations.push(calibration);
        Ok(())
    }
    fn confirmation_identity(
        &mut self,
        container: &str,
        product: Option<u32>,
    ) -> anyhow::Result<(String, ProfileName)> {
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
fn dpi_matcher_selection_publishes_draft_then_saves_and_enqueues_before_library_task() {
    let mut controller = Controller::default();
    let mut backend = SimulatedBackend::default();
    let reply = controller.select("match", &mut backend).unwrap();
    assert_eq!(
        backend.calls,
        [
            "read",
            "draft",
            "persist_stages",
            "submit",
            "persist_profiles"
        ]
    );
    assert_eq!(backend.version, 3);
    assert_eq!(backend.data.selected(), Some("match"));
    assert_eq!(
        backend.data.stages().unwrap().stages()[0]
            .x()
            .unwrap()
            .value(),
        1200.
    );
    assert_eq!(backend.drafts.len(), 1);
    assert_eq!(backend.data.stages(), backend.drafts.first());
    assert!(matches!(
        backend.persisted.as_slice(),
        [Change::Stages(_), Change::Profiles(_)]
    ));
    assert!(backend.failures.is_empty());
    assert_eq!(profiles(&reply[0]).selected(), Some("match"));
}

#[test]
fn dpi_matcher_reset_keeps_already_accepted_stage_and_library_tasks() {
    let mut controller = Controller::default();
    let mut backend = SimulatedBackend::default();
    controller.select("match", &mut backend).unwrap();
    let accepted = backend.persisted.clone();
    let stages = backend.data.stages().unwrap().clone();
    let library = backend.data.library().clone();
    let drafts = backend.drafts.clone();
    backend.calls.clear();
    let response = controller.command("RESET", None, &mut backend).unwrap();
    assert_eq!(calibration(&response[0]).phase(), Phase::Ready);
    assert!(backend.calls.is_empty());
    assert_eq!(backend.persisted, accepted);
    assert_eq!(backend.drafts, drafts);
    assert_eq!(backend.data.stages(), Some(&stages));
    assert_eq!(backend.data.library(), &library);
    assert_eq!(backend.data.selected(), Some("match"));
    assert_eq!(backend.version, 3);
    assert!(!controller.monitoring && !controller.locked);
}

#[test]
fn dpi_matcher_failed_stage_tasks_still_save_library_without_rollback() {
    for fail in ["persist_stages", "submit"] {
        let mut controller = Controller::default();
        let mut backend = SimulatedBackend {
            fail: Some(fail),
            ..Default::default()
        };
        let before = backend.data.stages().unwrap().clone();
        let reply = controller.select("match", &mut backend).unwrap();
        let expected = if fail == "persist_stages" {
            assert_eq!(backend.data.stages(), Some(&before));
            assert_eq!(backend.version, 2);
            assert!(matches!(
                backend.persisted.as_slice(),
                [Change::Profiles(_)]
            ));
            vec![
                "read",
                "draft",
                "persist_stages",
                "failure",
                "persist_profiles",
            ]
        } else {
            assert_eq!(
                backend.data.stages().unwrap().stages()[0]
                    .x()
                    .unwrap()
                    .value(),
                1200.
            );
            assert_eq!(backend.version, 3);
            assert!(matches!(
                backend.persisted.as_slice(),
                [Change::Stages(_), Change::Profiles(_)]
            ));
            vec![
                "read",
                "draft",
                "persist_stages",
                "submit",
                "failure",
                "persist_profiles",
            ]
        };
        assert_eq!(backend.calls, expected);
        assert_eq!(profiles(&reply[0]).selected(), Some("match"));
        assert_eq!(backend.data.selected(), Some("match"));
        assert_eq!(backend.failures, [format!("simulated {fail} failure")]);
        assert_eq!(backend.drafts.len(), 1);
    }
}

#[test]
fn dpi_matcher_library_failure_does_not_revert_successfully_saved_stages() {
    let mut controller = Controller::default();
    let mut backend = SimulatedBackend {
        fail: Some("persist_profiles"),
        ..Default::default()
    };
    let library = backend.data.library().clone();
    assert!(controller.select("match", &mut backend).is_err());
    assert_eq!(
        backend.calls,
        [
            "read",
            "draft",
            "persist_stages",
            "submit",
            "persist_profiles"
        ]
    );
    assert_eq!(backend.version, 2);
    assert_eq!(
        backend.data.stages().unwrap().stages()[0]
            .x()
            .unwrap()
            .value(),
        1200.
    );
    assert_eq!(backend.data.library(), &library);
    assert!(matches!(backend.persisted.as_slice(), [Change::Stages(_)]));
    assert!(backend.failures.is_empty());
}

#[test]
fn dpi_matcher_start_baseline_error_does_not_start_monitoring_or_acquire_locks() {
    let mut controller = Controller::default();
    let mut backend = SimulatedBackend {
        data: SourceData::from_document(&json!({
            "activeProfile":"p","profiles":[{"guid":"p","dpiStages":{
                "active":2,"enable":true,"stages":[{"x":800,"y":800,"visible":true}]
            }}]
        }))
        .unwrap(),
        ..Default::default()
    };
    assert!(controller.command("START", None, &mut backend).is_err());
    assert_eq!(backend.calls, ["read"]);
    assert_eq!(controller.response().phase(), Phase::Ready);
    assert!(controller.session.new_dpi().is_none());
    assert!(!controller.monitoring && !controller.locked);
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
    let dpi = backend.data.stages().unwrap().clone();
    controller
        .command("DELETE_ALL_PROFILES", None, &mut backend)
        .unwrap();
    assert_eq!(backend.calls, ["read", "persist_profiles"]);
    assert_eq!(backend.data.stages(), Some(&dpi));
    assert!(backend.data.library().profiles().is_empty());
    assert_eq!(backend.data.selected(), Some(""));
    assert_eq!(backend.version, 2);
    assert!(backend.drafts.is_empty());
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
fn dpi_matcher_confirm_resets_and_publishes_before_independent_save_results() {
    for product_id in [190, 192, 222, 226, 229, 239] {
        for failure in [
            None,
            Some("identity"),
            Some("persist_stages"),
            Some("submit"),
            Some("persist_profiles"),
        ] {
            let mut controller =
                Controller::new(razer_device::dpi_matcher::Product::new(product_id).unwrap());
            let name: ProfileName =
                serde_json::from_value(json!({"en":"Observed","zh-cn":"Localized"})).unwrap();
            let mut backend = SimulatedBackend {
                product_id,
                identity: Some(("measured".into(), name.clone())),
                expected_dpi: 400.,
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
            let before = backend.data.stages().unwrap().clone();
            let result = controller.command("CONFIRM", None, &mut backend);
            if failure == Some("identity") {
                assert!(result.is_err());
                assert_eq!(backend.data.stages(), Some(&before));
                assert_eq!(controller.session.new_dpi(), Some(400.));
                assert_eq!(backend.calls, ["read", "identity"]);
                assert_eq!(backend.version, 1);
                assert!(backend.calibrations.is_empty());
                assert!(backend.persisted.is_empty());
                assert_eq!(backend.data.selected(), None);
            } else {
                let mut expected =
                    vec!["read", "identity", "calibration", "draft", "persist_stages"];
                if failure != Some("persist_stages") {
                    expected.push("submit");
                }
                if matches!(failure, Some("persist_stages" | "submit")) {
                    expected.push("failure");
                }
                expected.push("persist_profiles");
                assert_eq!(backend.calls, expected);
                assert!(controller.session.new_dpi().is_none());
                assert_eq!(controller.response().phase(), Phase::Ready);
                assert_eq!(backend.calibrations.len(), 1);
                assert_eq!(backend.calibrations[0].phase(), Phase::Ready);
                assert_eq!(backend.calibrations[0].process(), 0.);
                assert_eq!(backend.calibrations[0].new_dpi(), 0.);
                if failure == Some("persist_profiles") {
                    assert!(result.is_err());
                    assert_eq!(backend.version, 2);
                    assert_eq!(backend.data.library().profiles().len(), 1);
                    assert_eq!(backend.data.selected(), Some(""));
                } else {
                    let result = result.unwrap();
                    assert_eq!(profiles(&result[0]).selected(), Some("measured"));
                    assert_eq!(backend.data.selected(), Some("measured"));
                    assert_eq!(backend.data.library().profiles().len(), 2);
                    assert_eq!(
                        backend.data.library().profiles()[1].name(),
                        &serde_json::from_value::<ProfileName>(
                            json!({"en":"Observed 1","zh-cn":"Localized 1"})
                        )
                        .unwrap()
                    );
                    assert_eq!(
                        backend.version,
                        if failure == Some("persist_stages") { 2 } else { 3 }
                    );
                }
                if failure == Some("persist_stages") {
                    assert_eq!(backend.data.stages(), Some(&before));
                } else {
                    assert_eq!(
                        backend.data.stages().unwrap().stages()[0]
                            .x()
                            .unwrap()
                            .value(),
                        400.
                    );
                }
                if matches!(failure, Some("persist_stages" | "submit")) {
                    assert_eq!(
                        backend.failures,
                        [format!("simulated {} failure", failure.unwrap())]
                    );
                } else {
                    assert!(backend.failures.is_empty());
                }
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
fn dpi_matcher_zero_source_distance_completes_zero_after_releasing_capture_resources() {
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
    let result = controller
        .ingest(input("main", 190, &[(21., 0.)]), true, 3, &mut backend)
        .unwrap();
    assert_eq!(backend.calls, ["read", "start", "lock", "stop", "unlock"]);
    assert!(!controller.monitoring && !controller.locked);
    assert_eq!(controller.session.new_dpi(), Some(0.));
    assert_eq!(result.phase(), Phase::Completed);
    assert_eq!(result.new_dpi(), 0.);
}
