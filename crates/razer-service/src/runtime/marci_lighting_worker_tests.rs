use super::*;
use razer_device::lighting_engine::{EngineEffect, EngineFrame, LedConfiguration};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

type Calls = Rc<RefCell<Vec<String>>>;

struct Step {
    command: [u8; 3],
    payload: Vec<u8>,
    response: Vec<u8>,
    status: u8,
}
fn step(command: [u8; 3], payload: &[u8], response: &[u8]) -> Step {
    Step {
        command,
        payload: payload.into(),
        response: response.into(),
        status: 2,
    }
}
fn script(normal_rejected: bool) -> VecDeque<Step> {
    // Current Marci source: firmware, GET mode, SET driver, then on exit
    // GET mode, SET normal, then the source-synchronized Static exit target.
    let mut steps = VecDeque::from([
        step([4, 0, 135], &[0, 0, 0, 0], &[1, 0, 0, 0]),
        step([2, 0, 132], &[0, 0], &[0, 0]),
        step([2, 0, 4], &[3, 0], &[]),
        step([2, 0, 132], &[0, 0], &[3, 0]),
        step([2, 0, 4], &[0, 0], &[]),
        step([9, 15, 2], &[1, 0, 1, 0, 0, 1, 42, 42, 42], &[]),
    ]);
    if normal_rejected {
        steps[4].status = 5;
    }
    steps
}
fn node(control: bool) -> HidNode {
    HidNode {
        path: if control {
            b"control".as_slice()
        } else {
            b"output".as_slice()
        }
        .to_vec(),
        vendor_id: 5426,
        product_id: 3942,
        usage_page: if control { 12 } else { 0xff14 },
        usage: 1,
        interface_number: 2,
        release_number: 1,
        serial_number: None,
        manufacturer: None,
        product: None,
    }
}

struct Transport {
    node: HidNode,
    calls: Calls,
    steps: Rc<RefCell<VecDeque<Step>>>,
    pending: RefCell<Option<(Vec<u8>, Step)>>,
    output_count: Cell<usize>,
    short_frame: bool,
    cancel: Arc<AtomicBool>,
}
impl FeatureTransport for Transport {
    fn hid_node(&self) -> Option<&HidNode> {
        Some(&self.node)
    }
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        let expected = self
            .steps
            .borrow_mut()
            .pop_front()
            .expect("extra Feature OUT");
        assert_eq!(&report[6..9], expected.command);
        assert_eq!(&report[9..9 + expected.payload.len()], expected.payload);
        assert_eq!(report.len(), 91);
        self.calls.borrow_mut().push(format!(
            "feature:{}:{}:{}:{:?}",
            report[6], report[7], report[8], expected.payload
        ));
        self.pending.replace(Some((report.to_vec(), expected)));
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        let (request, step) = self.pending.take().expect("GET without OUT");
        report.copy_from_slice(&request);
        report[1] = step.status;
        report[6] = step.response.len() as u8;
        report[9..9 + step.response.len()].copy_from_slice(&step.response);
        Ok(91)
    }
    fn write_output_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        assert_eq!(report.len(), 781);
        assert_eq!(&report[..7], &[23, 0, 132, 0, 1, 0, 87]);
        assert_eq!(&report[535..], &[0; 246]);
        let index = self.output_count.get();
        self.output_count.set(index + 1);
        if index == 0 {
            assert_eq!(&report[7..535], &[0; 528]);
            self.calls.borrow_mut().push("probe".into());
            return Ok(781);
        }
        assert_eq!(&report[7..535], &[42; 528]);
        self.calls.borrow_mut().push("frame".into());
        if self.short_frame {
            Ok(780)
        } else {
            self.cancel.store(true, Ordering::Release);
            Ok(781)
        }
    }
    fn report_lengths(&self) -> anyhow::Result<ReportLengths> {
        Ok(ReportLengths {
            feature: [(16, 91)].into(),
            output: [(23, 781)].into(),
            ..Default::default()
        })
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}
impl Drop for Transport {
    fn drop(&mut self) {
        self.calls.borrow_mut().push(if self.node.usage_page == 12 {
            "close-control".into()
        } else {
            "close-output".into()
        });
    }
}

struct Renderer {
    calls: Calls,
    fail_create_device: bool,
    fail_destroy: bool,
    fail_disable: bool,
}
impl EngineAdapter for Renderer {
    fn create_device(&mut self, config: &LedConfiguration) -> anyhow::Result<u32> {
        assert_eq!(config.led_matrix.len(), 60);
        self.calls.borrow_mut().push("create-device".into());
        ensure!(!self.fail_create_device, "injected device creation failure");
        Ok(7)
    }
    fn create_engine(&mut self) -> anyhow::Result<u32> {
        self.calls.borrow_mut().push("create-engine".into());
        Ok(9)
    }
    fn add_device(&mut self, engine: u32, device: u32) -> anyhow::Result<()> {
        assert_eq!((engine, device), (9, 7));
        self.calls.borrow_mut().push("add-device".into());
        Ok(())
    }
    fn add_effect(&mut self, _: u32, effect: &EngineEffect) -> anyhow::Result<u32> {
        assert_eq!(effect.effect, 6);
        self.calls.borrow_mut().push("add-effect".into());
        Ok(11)
    }
    fn remove_effect(&mut self, _: u32, _: u32) -> anyhow::Result<()> {
        Ok(())
    }
    fn enable(&mut self, _: u32, enable: bool) -> anyhow::Result<()> {
        self.calls.borrow_mut().push(format!("enable:{enable}"));
        ensure!(enable || !self.fail_disable, "injected disable failure");
        Ok(())
    }
    fn input(&mut self, _: u32) -> anyhow::Result<()> {
        Ok(())
    }
    fn discard_frames_for(&mut self, _: u32) -> anyhow::Result<()> {
        Ok(())
    }
    fn frames(&mut self) -> anyhow::Result<Vec<EngineFrame>> {
        Ok(vec![EngineFrame {
            device_handle: 7,
            region_id: 0,
            num_row: 1,
            num_col: 176,
            frame: vec![42; 528],
        }])
    }
    fn destroy_engine(&mut self, _: u32) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("destroy-engine".into());
        ensure!(!self.fail_destroy, "injected engine destruction failure");
        Ok(())
    }
    fn destroy_device(&mut self, _: u32) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("destroy-device".into());
        Ok(())
    }
    fn terminate(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("terminate".into());
        Ok(())
    }
}

#[derive(Default)]
struct Faults {
    short_frame: bool,
    create_device: bool,
    destroy_engine: bool,
    disable: bool,
    normal_rejected: bool,
    cancel_before_start: bool,
    gates: bool,
}
fn exercise(faults: Faults) -> (anyhow::Result<()>, Status, Vec<String>) {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let steps = Rc::new(RefCell::new(script(faults.normal_rejected)));
    let canceled = Arc::new(AtomicBool::new(faults.cancel_before_start));
    let connection = Connection {
        output: Box::new(Transport {
            node: node(false),
            calls: calls.clone(),
            steps: steps.clone(),
            pending: RefCell::new(None),
            output_count: Cell::new(0),
            short_frame: faults.short_frame,
            cancel: canceled.clone(),
        }),
        control_transport: Box::new(Transport {
            node: node(true),
            calls: calls.clone(),
            steps: steps.clone(),
            pending: RefCell::new(None),
            output_count: Cell::new(0),
            short_frame: false,
            cancel: canceled.clone(),
        }),
        validate_identity: &|| Ok(()),
    };
    let (sender, receiver) = mpsc::sync_channel(2);
    let (ready, _started) = mpsc::sync_channel(1);
    let mut response = Some(ready);
    let (reply, gate_reply) = mpsc::sync_channel(1);
    if faults.gates {
        sender
            .send(Message::Gates {
                generation: 1,
                gates: Gates {
                    paused: true,
                    ..Gates::running()
                },
                reply,
            })
            .unwrap();
    }
    let status = Mutex::new(Status::starting("owner".into(), "container".into(), 1));
    let effect: QuickEffect = serde_json::from_value(serde_json::json!({
        "selectedEffectId": 1, "selectedEffectSetting": {"color1":"#2a2a2a"}
    }))
    .unwrap();
    let renderer = Renderer {
        calls: calls.clone(),
        fail_create_device: faults.create_device,
        fail_destroy: faults.destroy_engine,
        fail_disable: faults.disable,
    };
    let time = Cell::new(0u64);
    let result = run_connected(
        connection,
        Input {
            generation: 1,
            effect: &effect,
            receiver: &receiver,
            canceled: &canceled,
            status: &status,
            response: &mut response,
        },
        || Ok(renderer),
        &|_| {},
        &|| {
            let now = time.get();
            time.set(now + 10);
            now
        },
    );
    if faults.gates {
        assert!(gate_reply.try_recv().unwrap().is_err());
    }
    if !faults.cancel_before_start {
        assert!(steps.borrow().is_empty());
    }
    drop(sender);
    let calls = calls.borrow().clone();
    (result, status.into_inner().unwrap(), calls)
}
fn assert_exit_order(calls: &[String]) {
    let terminate = calls.iter().position(|c| c == "terminate").unwrap();
    assert_eq!(
        &calls[terminate + 1..],
        &[
            "feature:2:0:132:[0, 0]",
            "feature:2:0:4:[0, 0]",
            "feature:9:15:2:[1, 0, 1, 0, 0, 1, 42, 42, 42]",
            "close-control",
            "close-output",
        ]
    );
}

#[test]
fn marci_worker_short_frame_stops_renderer_before_exit_and_closes_both_handles() {
    let (result, status, calls) = exercise(Faults {
        short_frame: true,
        ..Default::default()
    });
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("transfer was short")
    );
    assert_eq!(status.last_output.unwrap().transferred_bytes, 780);
    assert_eq!(status.connection_output.unwrap().transferred_bytes, 781);
    assert!(status.normal_mode_confirmed && status.exit_effect_confirmed);
    assert!(status.renderer_released && status.transports_released);
    assert!(status.cleanup_errors.is_empty());
    assert_exit_order(&calls);
}

#[test]
fn marci_worker_cancellation_after_transfer_restores_without_reusing_cancellation_guard() {
    let (result, status, calls) = exercise(Faults::default());
    assert!(result.unwrap_err().is::<Canceled>());
    assert_eq!(status.frames_submitted, 1);
    assert!(status.normal_mode_confirmed && status.exit_effect_confirmed);
    assert!(status.cleanup_errors.is_empty());
    assert_exit_order(&calls);
}

#[test]
fn marci_worker_partial_renderer_initialization_still_destroys_engine_and_restores() {
    let (result, status, calls) = exercise(Faults {
        create_device: true,
        ..Default::default()
    });
    assert!(result.unwrap_err().to_string().contains("creation failure"));
    assert!(!calls.iter().any(|c| c == "destroy-device" || c == "frame"));
    assert!(calls.iter().any(|c| c == "destroy-engine"));
    assert!(status.renderer_released && status.transports_released);
    assert_exit_order(&calls);
}

#[test]
fn marci_worker_cleanup_failures_do_not_skip_remaining_restoration_or_release() {
    let (result, status, calls) = exercise(Faults {
        short_frame: true,
        destroy_engine: true,
        normal_rejected: true,
        ..Default::default()
    });
    assert!(result.is_err());
    assert!(!status.renderer_released && !status.normal_mode_confirmed);
    assert!(status.exit_effect_confirmed && status.transports_released);
    assert_eq!(status.cleanup_errors.len(), 2);
    assert!(calls.iter().any(|c| c == "destroy-device"));
    assert_exit_order(&calls);
}

#[test]
fn marci_worker_failed_gate_change_stops_and_preserves_disable_cleanup_failure() {
    let (result, status, calls) = exercise(Faults {
        gates: true,
        disable: true,
        ..Default::default()
    });
    assert!(result.unwrap_err().to_string().contains("disable failure"));
    assert!(!calls.iter().any(|c| c == "frame"));
    assert!(!status.renderer_released);
    assert!(status.transports_released && status.exit_effect_confirmed);
    assert_eq!(status.cleanup_errors.len(), 1);
    assert_exit_order(&calls);
}

#[test]
fn marci_worker_canceled_before_first_io_only_releases_opened_handles() {
    let (result, status, calls) = exercise(Faults {
        cancel_before_start: true,
        ..Default::default()
    });
    assert!(result.unwrap_err().is::<Canceled>());
    assert_eq!(calls, ["close-control", "close-output"]);
    assert!(!status.exit_required);
    assert!(!status.normal_mode_confirmed && !status.exit_effect_confirmed);
    assert!(status.renderer_released && status.transports_released);
    assert!(status.cleanup_errors.is_empty());
}

#[test]
fn marci_lighting_production_renderer_maps_only_manifest_leds_into_output_report() {
    let mut session = Session::new(lighting_engine::Engine::open().unwrap());
    let effect = razer_device::lighting_engine::SoftwareEffect::Static { color: [255, 0, 0] }
        .engine_effect();
    session
        .initialize_effect(&led_configuration(), &effect)
        .unwrap();
    let frames = session.engine.frames().unwrap();
    assert_eq!(frames.len(), 1);
    let report = Frame::from_engine(&frames[0], session.device_handle.unwrap())
        .unwrap()
        .javascript_report();
    assert_eq!(&report[..7], &[23, 0, 132, 0, 1, 0, 87]);
    for index in 0..176 {
        assert_eq!(
            &report[7 + index * 3..10 + index * 3],
            if index < 30 || (88..118).contains(&index) {
                &[3, 0, 0]
            } else {
                &[0; 3]
            }
        );
    }
    assert_eq!(&report[535..], &[0; 246]);
    assert!(session.cleanup().is_empty());
    assert!(session.engine.frames().unwrap().is_empty());
}
