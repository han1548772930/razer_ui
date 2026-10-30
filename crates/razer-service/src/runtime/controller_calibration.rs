//! Current V3 trigger worker: actual descriptor-selected HID reads, samples,
//! writes and readback. No vendor DLL, synthetic movement or completion timer.
use anyhow::{Context as _, ensure};
use razer_device::{
    backend::{
        FeatureTransport, HidNode, checked_native_io, checked_native_read, require_complete_send,
    },
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
    pub(super) fn shutdown(&mut self, cleanup: &mut super::shutdown::Cleanup) {
        if let Some(mut active) = self.active.take() {
            active.canceled.store(true, Ordering::Release);
            if let Some(worker) = active.worker.take() {
                cleanup.record(
                    "controller_calibration",
                    worker
                        .join()
                        .map_err(|_| anyhow::anyhow!("Trigger calibration cleanup worker panicked"))
                        .and_then(|result| result),
                );
            }
        }
    }

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

fn record_trigger_write_attempt(attempted: &AtomicBool, report: &[u8]) {
    if report.get(7..9) == Some(&[12, 25]) {
        // A terminal native attempt is distinct from acknowledgement and readback.
        attempted.store(true, Ordering::Release);
    }
}

impl FeatureTransport for CheckedTransport<'_> {
    fn begin_exchange(
        &self,
        options: razer_device::backend::ExchangeOptions,
    ) -> anyhow::Result<Box<dyn razer_device::backend::ExchangeLease + '_>> {
        (self.validate)()?;
        let attempted = self.write_attempted;
        Ok(razer_device::backend::checked_exchange_observing_send(
            self.inner
                .begin_exchange_validated(options, std::rc::Rc::new(self.validate))?,
            self.validate,
            self.validate,
            None,
            move |report| record_trigger_write_attempt(attempted, report),
        ))
    }
    fn begin_exchange_validated<'a>(
        &'a self,
        options: razer_device::backend::ExchangeOptions,
        validate: razer_device::backend::ExchangeValidation<'a>,
    ) -> anyhow::Result<Box<dyn razer_device::backend::ExchangeLease + 'a>> {
        let own = self.validate;
        let combined = std::rc::Rc::new(move || {
            own()?;
            validate()
        });
        let attempted = self.write_attempted;
        Ok(razer_device::backend::checked_exchange_observing_send(
            self.inner.begin_exchange_validated(options, combined)?,
            self.validate,
            self.validate,
            None,
            move |report| record_trigger_write_attempt(attempted, report),
        ))
    }
    fn hid_node(&self) -> Option<&HidNode> {
        self.inner.hid_node()
    }
    fn send_feature_batch(
        &self,
        batch: &razer_device::backend::FeatureBatch,
    ) -> anyhow::Result<razer_device::backend::FeatureBatchReceipt> {
        razer_device::backend::checked_feature_batch(
            self.validate,
            || {
                let result = self.inner.send_feature_batch(batch);
                razer_device::backend::observe_feature_batch(batch, &result, |bytes| {
                    record_trigger_write_attempt(self.write_attempted, bytes);
                });
                result
            },
            self.validate,
        )
    }
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        let count = self.send_feature_counted(report)?;
        require_complete_send(count, report.len())
    }
    fn send_feature_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        checked_native_io(
            self.validate,
            || {
                let result = self.inner.send_feature_counted(report);
                if razer_device::backend::native_io_attempted(&result) {
                    record_trigger_write_attempt(self.write_attempted, report);
                }
                result
            },
            self.validate,
        )
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        checked_native_read(
            self.validate,
            report,
            |report| self.inner.get_feature(report),
            self.validate,
        )
    }
    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        checked_native_read(
            self.validate,
            report,
            |report| self.inner.get_feature_at_collection_max(report),
            self.validate,
        )
    }
    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        let count = self.send_feature_at_collection_max_counted(report)?;
        require_complete_send(count, report.len())
    }
    fn send_feature_at_collection_max_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        checked_native_io(
            self.validate,
            || {
                let result = self.inner.send_feature_at_collection_max_counted(report);
                if razer_device::backend::native_io_attempted(&result) {
                    record_trigger_write_attempt(self.write_attempted, report);
                }
                result
            },
            self.validate,
        )
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
    // Each complete Feature command enters the retained collection's queue;
    // sampling and recovery keep their own identity and cancellation guards.
    let device = with_backend(|backend| backend.open_validated(node, std::rc::Rc::new(&validate)))?;
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
        session.start(&mut protocol)?;
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
                    Err(error) => return Err(error),
                }
                if session.step() == TriggerStep::Complete {
                    return Ok(());
                }
                continue;
            }
            let raw = protocol.raw_analog_input();
            validate()?;
            match session.poll(raw)? {
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
            if Instant::now() >= movement_at
                && let Some(percent) = session.movement_percentage()
            {
                observe(
                    events,
                    json!({"kind":"movement","payload":{
                    "x":0,"y":0,"t":percent}}),
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
    if let Err(error) = &operation
        && session.step() == TriggerStep::Error
    {
        observe(events, progress(&session, Some(&error.to_string())))?;
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
        "persistence":"device_calibration_write_with_exact_readback"}),
    )
}

#[cfg(test)]
mod native_io_tests {
    use super::*;
    use razer_device::backend::{NativeIoOutcome, native_io_outcome};
    use std::cell::{Cell, RefCell};

    struct Native {
        calls: Cell<usize>,
    }

    impl FeatureTransport for Native {
        fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
            panic!("calibration must retain the counted native route")
        }
        fn send_feature_counted(&self, _: &[u8]) -> anyhow::Result<usize> {
            self.calls.set(self.calls.get() + 1);
            Ok(23)
        }
        fn send_feature_at_collection_max_counted(&self, _: &[u8]) -> anyhow::Result<usize> {
            self.calls.set(self.calls.get() + 1);
            Ok(29)
        }
        fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
            self.calls.set(self.calls.get() + 1);
            report[1] = 0x71;
            Ok(17)
        }
        fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
            self.calls.set(self.calls.get() + 1);
            report[1] = 0x73;
            Ok(19)
        }
        fn metadata(&self) -> Value {
            Value::Null
        }
    }

    #[test]
    fn calibration_attempt_starts_after_admission_and_survives_failed_delivery() {
        for collection in [false, true] {
            for blocked in [false, true] {
                let native = Native {
                    calls: Cell::new(0),
                };
                let checks = Cell::new(0);
                let validate = || {
                    let previous = checks.replace(checks.get() + 1);
                    ensure!(!blocked && previous == 0, "calibration owner expired");
                    Ok(())
                };
                let attempted = AtomicBool::new(false);
                let checked = CheckedTransport {
                    inner: &native,
                    validate: &validate,
                    write_attempted: &attempted,
                };
                let mut report = [0; 91];
                report[7..9].copy_from_slice(&[12, 25]);
                let error = if collection {
                    checked.send_feature_at_collection_max_counted(&report)
                } else {
                    checked.send_feature_counted(&report)
                }
                .unwrap_err();
                let expected = if blocked {
                    NativeIoOutcome::NotCalled
                } else {
                    NativeIoOutcome::Returned {
                        byte_count: if collection { 29 } else { 23 },
                    }
                };
                assert_eq!(native_io_outcome(&error), Some(expected));
                assert!(format!("{error:#}").contains("calibration owner expired"));
                assert_eq!(attempted.load(Ordering::Acquire), !blocked);
                assert_eq!(native.calls.get(), usize::from(!blocked));
            }
        }
    }

    #[test]
    fn calibration_short_send_and_late_reads_never_publish_success() {
        let native = Native {
            calls: Cell::new(0),
        };
        let attempted = AtomicBool::new(false);
        let checked = CheckedTransport {
            inner: &native,
            validate: &|| Ok(()),
            write_attempted: &attempted,
        };
        let mut report = [0; 91];
        report[7..9].copy_from_slice(&[12, 25]);
        let error = checked.send_feature(&report).unwrap_err();
        assert_eq!(
            native_io_outcome(&error),
            Some(NativeIoOutcome::Returned { byte_count: 23 })
        );
        assert!(attempted.load(Ordering::Acquire));
        for collection in [false, true] {
            let checks = Cell::new(0);
            let validate = || {
                let previous = checks.replace(checks.get() + 1);
                ensure!(previous == 0, "calibration read owner expired");
                Ok(())
            };
            let attempted = AtomicBool::new(false);
            let checked = CheckedTransport {
                inner: &native,
                validate: &validate,
                write_attempted: &attempted,
            };
            let error = if collection {
                checked.get_feature_at_collection_max(&mut report)
            } else {
                checked.get_feature(&mut report)
            }
            .unwrap_err();
            assert_eq!(
                native_io_outcome(&error),
                Some(NativeIoOutcome::Returned {
                    byte_count: if collection { 19 } else { 17 },
                })
            );
            assert_eq!(report[1], if collection { 0x73 } else { 0x71 });
            assert!(!attempted.load(Ordering::Acquire));
        }
    }

    #[test]
    fn calibration_batch_does_not_mark_an_unreached_trigger_write_as_attempted() {
        struct BatchNative;
        impl FeatureTransport for BatchNative {
            fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
                unreachable!()
            }
            fn send_feature_at_collection_max_counted(
                &self,
                report: &[u8],
            ) -> anyhow::Result<usize> {
                match report[0] {
                    0 => Ok(0),
                    1 => Err(razer_device::backend::native_io_failure(
                        NativeIoOutcome::NotCalled,
                        anyhow::anyhow!("preflight failed"),
                    )),
                    2 => Err(razer_device::backend::native_io_failure(
                        NativeIoOutcome::Rejected,
                        anyhow::anyhow!("native rejected"),
                    )),
                    _ => Ok(report.len()),
                }
            }
            fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
                unreachable!()
            }
            fn metadata(&self) -> Value {
                Value::Null
            }
        }
        let native = BatchNative;
        for first in [0, 1, 2, 3] {
            let mut first_report = [0; 91];
            first_report[0] = first;
            let mut trigger_report = [0; 91];
            trigger_report[0] = 2;
            trigger_report[7..9].copy_from_slice(&[12, 25]);
            let mut batch = razer_device::backend::FeatureBatch::new();
            batch.push_report(&first_report).unwrap();
            batch.push_report(&trigger_report).unwrap();
            let attempted = AtomicBool::new(false);
            let checked = CheckedTransport {
                inner: &native,
                validate: &|| Ok(()),
                write_attempted: &attempted,
            };
            let result = checked.send_feature_batch(&batch);
            assert_eq!(result.is_ok(), first == 0);
            assert_eq!(attempted.load(Ordering::Acquire), first == 3);
        }
    }

    #[test]
    fn calibration_terminal_send_facts_survive_cancellation_and_stale_owner_cleanup() {
        struct TerminalNative<'a> {
            result: RefCell<Option<anyhow::Result<usize>>>,
            current: &'a Cell<bool>,
            attempted: &'a AtomicBool,
            entries: Cell<usize>,
        }
        impl TerminalNative<'_> {
            fn send(&self) -> anyhow::Result<usize> {
                assert!(!self.attempted.load(Ordering::Acquire));
                self.entries.set(self.entries.get() + 1);
                self.current.set(false);
                self.result.borrow_mut().take().unwrap()
            }
        }
        impl FeatureTransport for TerminalNative<'_> {
            fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
                panic!("calibration must preserve the counted native result")
            }
            fn send_feature_counted(&self, _: &[u8]) -> anyhow::Result<usize> {
                self.send()
            }
            fn send_feature_at_collection_max_counted(&self, _: &[u8]) -> anyhow::Result<usize> {
                self.send()
            }
            fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
                panic!("owner loss must stop the command before acknowledgement")
            }
            fn metadata(&self) -> Value {
                Value::Null
            }
        }
        for owner_error in ["calibration canceled", "calibration connection is stale"] {
            for leased in [false, true] {
                for collection in [false, true] {
                    for outcome in [
                        None,
                        Some(NativeIoOutcome::NotCalled),
                        Some(NativeIoOutcome::Rejected),
                        Some(NativeIoOutcome::Returned { byte_count: 0 }),
                        Some(NativeIoOutcome::Returned { byte_count: 91 }),
                    ] {
                        let current = Cell::new(true);
                        let attempted = AtomicBool::new(false);
                        let result = match outcome {
                            Some(NativeIoOutcome::Returned { byte_count: 91 }) => Ok(91),
                            _ => {
                                let error: anyhow::Error =
                                    std::io::Error::other("calibration native failure").into();
                                Err(match outcome {
                                    Some(outcome) => {
                                        razer_device::backend::native_io_failure(outcome, error)
                                    }
                                    None => error,
                                })
                            }
                        };
                        let native = TerminalNative {
                            result: RefCell::new(Some(result)),
                            current: &current,
                            attempted: &attempted,
                            entries: Cell::new(0),
                        };
                        let validate = || {
                            ensure!(current.get(), "{owner_error}");
                            Ok(())
                        };
                        let checked = CheckedTransport {
                            inner: &native,
                            validate: &validate,
                            write_attempted: &attempted,
                        };
                        let mut report = [0; 91];
                        report[7..9].copy_from_slice(&[12, 25]);
                        let send = |transport: &dyn FeatureTransport| {
                            if collection {
                                transport.send_feature_at_collection_max_counted(&report)
                            } else {
                                transport.send_feature_counted(&report)
                            }
                        };
                        let result = if leased {
                            let lease = checked
                                .begin_exchange(razer_device::backend::ExchangeOptions::plain())
                                .unwrap();
                            let result = send(lease.transport());
                            assert!(lease.finish().is_err());
                            result
                        } else {
                            send(&checked)
                        };
                        let error = result.unwrap_err();
                        assert_eq!(native_io_outcome(&error), outcome);
                        assert!(format!("{error:#}").contains(owner_error));
                        assert_eq!(
                            error.downcast_ref::<std::io::Error>().is_some(),
                            outcome != Some(NativeIoOutcome::Returned { byte_count: 91 })
                        );
                        assert_eq!(native.entries.get(), 1);
                        assert_eq!(
                            attempted.load(Ordering::Acquire),
                            matches!(
                                outcome,
                                Some(NativeIoOutcome::Returned { .. } | NativeIoOutcome::Rejected)
                            )
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn calibration_canceled_batch_drains_before_reconciling_trigger_write_for_cleanup() {
        struct BatchNative<'a> {
            current: &'a Cell<bool>,
            attempted: &'a AtomicBool,
            events: RefCell<Vec<&'static str>>,
        }
        impl FeatureTransport for BatchNative<'_> {
            fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
                panic!("the whole native batch must use counted reports")
            }
            fn send_feature_at_collection_max_counted(
                &self,
                report: &[u8],
            ) -> anyhow::Result<usize> {
                assert!(!self.attempted.load(Ordering::Acquire));
                self.events.borrow_mut().push(match report[8] {
                    152 => "raw input sent",
                    25 => "trigger write sent",
                    _ => panic!("unexpected calibration report"),
                });
                self.current.set(false);
                Ok(report.len())
            }
            fn send_feature_batch(
                &self,
                batch: &razer_device::backend::FeatureBatch,
            ) -> anyhow::Result<razer_device::backend::FeatureBatchReceipt> {
                razer_device::backend::execute_feature_batch(
                    batch,
                    |report| self.send_feature_at_collection_max_counted(report),
                    |milliseconds| {
                        assert_eq!(milliseconds, 0);
                        self.events.borrow_mut().push("row wait");
                        Ok(())
                    },
                )
            }
            fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
                panic!("a canceled frame cannot invent calibration readback")
            }
            fn metadata(&self) -> Value {
                Value::Null
            }
        }
        for leased in [false, true] {
            let current = Cell::new(true);
            let attempted = AtomicBool::new(false);
            let native = BatchNative {
                current: &current,
                attempted: &attempted,
                events: RefCell::new(Vec::new()),
            };
            let validate = || {
                ensure!(current.get(), "calibration canceled during native batch");
                Ok(())
            };
            let checked = CheckedTransport {
                inner: &native,
                validate: &validate,
                write_attempted: &attempted,
            };
            let mut query = [0; 91];
            query[6..9].copy_from_slice(&[80, 12, 152]);
            let mut write = [0; 91];
            write[6..9].copy_from_slice(&[51, 12, 25]);
            let mut batch = razer_device::backend::FeatureBatch::new();
            batch.push_report(&query).unwrap();
            batch.push_sleep(0);
            batch.push_report(&write).unwrap();
            let result = if leased {
                let lease = checked
                    .begin_exchange(razer_device::backend::ExchangeOptions::batch())
                    .unwrap();
                let result = lease.transport().send_feature_batch(&batch);
                assert!(lease.finish().is_err());
                result
            } else {
                checked.send_feature_batch(&batch)
            };
            let error = result.unwrap_err();
            assert_eq!(
                native_io_outcome(&error),
                Some(NativeIoOutcome::Returned { byte_count: 182 })
            );
            let failure = razer_device::backend::FeatureBatchFailure::from_error(&error).unwrap();
            assert_eq!(failure.raw_return, Some(182));
            assert_eq!(failure.progress.reports_submitted(), 2);
            assert_eq!(failure.progress.completed_operations, 3);
            assert_eq!(failure.progress.report_attempts.len(), 2);
            assert_eq!(
                *native.events.borrow(),
                ["raw input sent", "row wait", "trigger write sent"]
            );
            assert!(attempted.load(Ordering::Acquire));
            assert!(format!("{error:#}").contains("calibration canceled during native batch"));
        }
    }
}
