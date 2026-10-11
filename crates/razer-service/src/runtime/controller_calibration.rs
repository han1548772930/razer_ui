//! Current V3 trigger worker: actual descriptor-selected HID reads, samples,
//! writes and readback. No vendor DLL, synthetic movement or completion timer.
use anyhow::{Context as _, ensure};
use razer_device::{
    backend::{FeatureTransport, HidBackend, HidNode},
    controller_calibration::{
        HoldChange, TriggerCalibrationSession, TriggerCalibrationTransport, TriggerPart,
        TriggerStep,
    },
    controller_calibration_protocol::CalibrationProtocolSession,
    device_identity::{self, IdentityLookup},
};
use razer_hid::with_backend;
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const MAX_OBSERVATIONS: usize = 1024;
type Observations = Arc<Mutex<VecDeque<Value>>>;

struct Operation {
    id: String,
    canceled: Arc<AtomicBool>,
    observations: Observations,
    worker: Option<JoinHandle<anyhow::Result<Value>>>,
}

#[derive(Default)]
pub(super) struct Controller {
    active: Option<Operation>,
}

impl Controller {
    pub(super) fn start(
        &mut self,
        id: String,
        node: HidNode,
        product_id: u32,
        part_id: u8,
    ) -> anyhow::Result<Value> {
        ensure!(
            !id.is_empty() && id.len() <= 128 && !id.contains('\0'),
            "校准操作 ID 无效"
        );
        ensure!(self.active.is_none(), "之前校准操作尚未完成清理或领取结果");
        let part = TriggerPart::from_ui_part(part_id).context("校准目标不是扳机")?;
        verify_product(&node, product_id)?;
        let canceled = Arc::new(AtomicBool::new(false));
        let observations = Arc::new(Mutex::new(VecDeque::new()));
        let signal = Arc::clone(&canceled);
        let events = Arc::clone(&observations);
        let worker = thread::Builder::new()
            .name("controller-trigger-calibration".into())
            .spawn(move || execute(&node, product_id, part, &signal, &events))?;
        self.active = Some(Operation {
            id: id.clone(),
            canceled,
            observations,
            worker: Some(worker),
        });
        // This is operation acceptance, not the source progress callback or a
        // successful hardware operation. The worker's first raw queries follow.
        Ok(json!({"operation_id":id,"state":"running","terminal":false,"part_id":part_id}))
    }

    pub(super) fn poll(&mut self, id: &str) -> anyhow::Result<Value> {
        let operation = self.active.as_mut().context("没有校准操作")?;
        ensure!(operation.id == id, "校准操作 ID 不匹配");
        let events: Vec<Value> = operation
            .observations
            .lock()
            .map_err(|_| anyhow::anyhow!("校准观察队列不可用"))?
            .drain(..)
            .collect();
        if !operation
            .worker
            .as_ref()
            .is_some_and(JoinHandle::is_finished)
        {
            return Ok(json!({"operation_id":id,
                "state":if operation.canceled.load(Ordering::Acquire){"canceling"}else{"running"},
                "terminal":false,"observations":events}));
        }
        let outcome = operation.worker.take().unwrap().join();
        let canceled = operation.canceled.load(Ordering::Acquire);
        // A final observation can be queued between the first drain and join.
        let mut events = events;
        events.extend(
            operation
                .observations
                .lock()
                .map_err(|_| anyhow::anyhow!("校准观察队列不可用"))?
                .drain(..),
        );
        self.active = None;
        Ok(match outcome {
            Ok(Ok(result)) => json!({"operation_id":id,"state":result["state"],"terminal":true,
                "observations":events,"result":result}),
            Ok(Err(error)) => json!({"operation_id":id,
                "state":if canceled{"canceled"}else{"failed"},
                "terminal":true,"observations":events,"error":format!("{error:#}"),
                "device_write_verified":false}),
            Err(_) => {
                json!({"operation_id":id,"state":"failed","terminal":true,"observations":events,
                "error":"校准线程异常终止；设备写回未确认","device_write_verified":false})
            }
        })
    }

    pub(super) fn cancel(&mut self, id: &str) -> anyhow::Result<Value> {
        let operation = self.active.as_ref().context("没有校准操作")?;
        ensure!(operation.id == id, "校准操作 ID 不匹配");
        operation.canceled.store(true, Ordering::Release);
        Ok(json!({"operation_id":id,"state":"canceling","terminal":false}))
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        if let Some(mut operation) = self.active.take() {
            operation.canceled.store(true, Ordering::Release);
            if let Some(worker) = operation.worker.take() {
                // Handles and their inter-worker file lock must finish dropping
                // before worker shutdown reports successful cleanup.
                let _ = worker.join();
            }
        }
    }
}

fn verify_product(node: &HidNode, product_id: u32) -> anyhow::Result<()> {
    let physical: &[u16] = match product_id {
        2676 => &[2676, 2677],
        2684 => &[2684, 2685],
        _ => anyhow::bail!("此产品没有当前 V3 扳机校准证据"),
    };
    ensure!(
        node.vendor_id == 5426 && physical.contains(&node.product_id) && !node.path.is_empty(),
        "校准 collection 与当前源有线产品/接口不匹配；无线身份接入尚未完成"
    );
    let IdentityLookup::Unique(identity) = device_identity::lookup(u32::from(node.product_id))
    else {
        anyhow::bail!("校准物理设备没有唯一源身份");
    };
    ensure!(
        identity.product_id == product_id && !identity.is_dongle && !identity.is_ble,
        "校准实际物理设备不属于请求产品"
    );
    match node.interface_number {
        1 => {}
        #[cfg(windows)]
        -1 => {
            super::native::controller_calibration_node_proof(node)?;
        }
        _ => anyhow::bail!("校准接口号没有源 claim1 的实际平台观察证据"),
    }
    Ok(())
}

fn observe(events: &Observations, value: Value) -> anyhow::Result<()> {
    let mut queue = events
        .lock()
        .map_err(|_| anyhow::anyhow!("校准观察队列不可用"))?;
    // Do not permit a disconnected UI consumer to accumulate unbounded memory.
    // Movement storage is latest-value state; remove an old movement first.
    if queue.len() >= MAX_OBSERVATIONS {
        let index = queue
            .iter()
            .position(|v| v["kind"] == "movement")
            .context("校准观察消费者未领取进度")?;
        queue.remove(index);
    }
    queue.push_back(value);
    Ok(())
}

fn progress(session: &TriggerCalibrationSession, error: Option<&str>) -> Value {
    let snapshot = session.progress();
    let step = snapshot.step().ui_step();
    let description = match snapshot.step() {
        TriggerStep::None => "Step_None",
        TriggerStep::Press => "Step_1_Trigger_Press",
        TriggerStep::Release => "Step_2_Trigger_Release",
        TriggerStep::Complete => "Step_3_Trigger_Complete",
        TriggerStep::Error => "Step_Error",
    };
    json!({"kind":"progress","payload":{"step":step,"stepDesc":description,
        "isStepValid":snapshot.valid(),"error":error.unwrap_or("")}})
}

struct CheckedTransport<'a> {
    inner: &'a dyn FeatureTransport,
    validate: &'a dyn Fn() -> anyhow::Result<()>,
    write_attempted: &'a AtomicBool,
}
impl FeatureTransport for CheckedTransport<'_> {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        (self.validate)()?;
        if report.get(7..9) == Some(&[12, 25]) {
            // Attempt is distinct from firmware acknowledgement and readback.
            self.write_attempted.store(true, Ordering::Release);
        }
        self.inner.send_feature(report)?;
        (self.validate)()
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        (self.validate)()?;
        let count = self.inner.get_feature(report)?;
        (self.validate)()?;
        Ok(count)
    }
    fn metadata(&self) -> Value {
        self.inner.metadata()
    }
}

fn execute(
    node: &HidNode,
    product_id: u32,
    part: TriggerPart,
    canceled: &AtomicBool,
    events: &Observations,
) -> anyhow::Result<Value> {
    verify_product(node, product_id)?;
    #[cfg(windows)]
    let windows_proof = if node.interface_number == -1 {
        Some(super::native::controller_calibration_node_proof(node)?)
    } else {
        None
    };
    let validate = || {
        ensure!(!canceled.load(Ordering::Acquire), "校准已取消");
        let current = with_backend(|backend| backend.enumerate())?;
        ensure!(
            current.iter().filter(|observed| *observed == node).count() == 1,
            "校准期间实际 HID collection 身份变化或不唯一"
        );
        #[cfg(windows)]
        if let Some(proof) = windows_proof.as_ref() {
            ensure!(
                super::native::controller_calibration_node_proof(node)? == *proof,
                "校准期间实际 Windows 实例/容器/接口变化"
            );
        }
        Ok(())
    };
    let wait = |duration: Duration| {
        let until = Instant::now() + duration;
        validate()?;
        while let Some(remaining) = until.checked_duration_since(Instant::now()) {
            thread::sleep(remaining.min(Duration::from_millis(10)));
            validate()?;
        }
        Ok(())
    };
    validate()?;
    // NativeBackend opens this exact re-observed collection and holds its
    // OS file lock across all polling, sampling, write and verification calls.
    let device = with_backend(|backend| backend.open(node))?;
    ensure!(
        device.report_lengths()?.feature.get(&10) == Some(&91),
        "实际 HID descriptor 的 Report10/长度与 SagePC 源不符"
    );
    let attempted = AtomicBool::new(false);
    let checked = CheckedTransport {
        inner: device.as_ref(),
        validate: &validate,
        write_attempted: &attempted,
    };
    let mut protocol = CalibrationProtocolSession::new(&checked, &wait);
    let mut session =
        TriggerCalibrationSession::new(product_id, part).context("此产品缺少当前扳机参数")?;
    let mut movement_removed = false;
    let operation = (|| -> anyhow::Result<()> {
        session.start(&mut protocol);
        validate()?;
        observe(events, progress(&session, None))?;
        let mut hold_at: Option<Instant> = None;
        let mut movement_at = Instant::now();
        loop {
            let poll_delay = TriggerCalibrationSession::poll_delay();
            wait(hold_at.map_or(poll_delay, |at| {
                at.saturating_duration_since(Instant::now()).min(poll_delay)
            }))?;
            if hold_at.is_some_and(|time| Instant::now() >= time) {
                hold_at = None;
                // A due source hold starts sampling before another raw poll.
                // Polling pauses while five ADC samples and the final native
                // write+readback run on this same retained collection.
                let result = session.hold_complete(&mut protocol, &wait);
                validate()?;
                if matches!(session.step(), TriggerStep::Complete | TriggerStep::Error) {
                    // Source awaits storage removal before terminal progress.
                    observe(
                        events,
                        json!({"kind":"remove_movement","key":"CalibrationUserMovement"}),
                    )?;
                    movement_removed = true;
                }
                match result {
                    Ok(()) => observe(events, progress(&session, None))?,
                    Err(error) => {
                        // Source error is the top-level calibration string;
                        // retain transport context separately in result.error.
                        observe(events, progress(&session, Some(&error.to_string())))?;
                        return Err(error);
                    }
                }
                if session.step() == TriggerStep::Complete {
                    return Ok(());
                }
                continue;
            }
            let raw = protocol.raw_analog_input().ok();
            validate()?;
            match session.poll(raw) {
                HoldChange::Start(duration) => {
                    hold_at = Some(Instant::now() + duration);
                    observe(events, progress(&session, None))?;
                }
                HoldChange::Cancel => {
                    hold_at = None;
                    observe(events, progress(&session, None))?;
                }
                HoldChange::Unchanged => {}
            }
            if Instant::now() >= movement_at {
                observe(
                    events,
                    json!({"kind":"movement","payload":{
                    "x":0,"y":0,"t":session.movement_percentage()}}),
                )?;
                movement_at = Instant::now() + TriggerCalibrationSession::movement_throttle();
            }
        }
    })();
    session.stop();
    if !movement_removed {
        observe(
            events,
            json!({"kind":"remove_movement","key":"CalibrationUserMovement"}),
        )?;
    }
    let canceled = canceled.load(Ordering::Acquire);
    let verified = operation.is_ok();
    Ok(
        json!({"node":node,"product_id":product_id,"part_id":part.ui_part(),
        "state":if verified{"completed"}else if canceled{"canceled"}else{"failed"},
        "device_write_attempted":attempted.load(Ordering::Acquire),
        "device_write_verified":verified,
        "error":operation.err().map(|e|format!("{e:#}")),
        "transport_metadata":device.metadata(),
        "persistence":"device_calibration_write_with_exact_readback",
        "evidence":"docs/re/gamepad-calibration-middleware-current.md"}),
    )
}
