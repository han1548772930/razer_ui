//! A dock keeps its Rust renderer and exact HID collection until explicit stop.
//! Source profile changes update this owner and cannot orphan frame delivery.
use super::{hid, hid_transport, receiver};
use crate::runtime::{
    lighting_engine::Engine,
    lighting_engine_session::{DeviceEngines, EngineAdapter, SharedEngine},
};
use anyhow::{Context as _, ensure};
use razer_device::{
    lighting_engine::SoftwarePhase,
    receiver_lighting::{
        ApplicationReceipt, ChromaAttributeReceipt, Configuration, LightingHid,
        RestorationDisposition, Status, led_configuration,
    },
};
#[path = "receiver_lighting_monitor.rs"]
mod monitoring;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

enum Message {
    Apply(Configuration, mpsc::SyncSender<Result<(), String>>),
    AttributeResult(
        uuid::Uuid,
        u64,
        ChromaAttributeReceipt,
        mpsc::SyncSender<Result<(), String>>,
    ),
    Wake,
}
#[derive(Debug)]
struct LightingCanceled;
impl std::fmt::Display for LightingCanceled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Dock lighting was canceled")
    }
}
impl std::error::Error for LightingCanceled {}
struct Job {
    id: String,
    path: String,
    container: String,
    canceled: Arc<AtomicBool>,
    updates: mpsc::SyncSender<Message>,
    status: Arc<Mutex<Status>>,
    worker: Option<JoinHandle<()>>,
}
impl Job {
    fn snapshot(&self) -> anyhow::Result<Status> {
        self.status
            .lock()
            .map(|status| status.clone())
            .map_err(|_| anyhow::anyhow!("Dock lighting status lock failed"))
    }
    fn stop(&mut self) -> anyhow::Result<Status> {
        self.canceled.store(true, Ordering::Release);
        let _ = self.updates.try_send(Message::Wake);
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("Dock lighting worker panicked during cleanup"))?;
        }
        self.snapshot()
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[derive(Default)]
pub(super) struct Controller {
    job: Option<Job>,
}
impl Controller {
    pub(super) fn apply(
        &mut self,
        id: String,
        path: String,
        container: String,
        configuration: Configuration,
    ) -> anyhow::Result<Status> {
        configuration.validate()?;
        ensure!(
            !id.is_empty() && id.len() <= 128 && !id.contains('\0'),
            "Dock lighting owner ID is invalid"
        );
        let (reply, completed) = mpsc::sync_channel(1);
        if let Some(job) = self
            .job
            .as_mut()
            .filter(|job| job.worker.as_ref().is_none_or(JoinHandle::is_finished))
        {
            ensure!(
                job.id == id && job.path == path && job.container == container,
                "Stopped dock owner differs from the current connection"
            );
            let previous = job.stop()?;
            ensure!(
                previous.restoration_complete() && previous.software.cleanup_errors.is_empty(),
                "Previous dock restoration/release is unconfirmed: {:?}",
                previous.software
            );
            self.job.take();
        }
        if let Some(job) = self.job.as_mut() {
            ensure!(
                job.id == id && job.path == path && job.container == container,
                "Another dock lighting owner must release its handles first"
            );
            job.updates
                .send(Message::Apply(configuration, reply))
                .context("Dock lighting worker stopped")?;
        } else {
            let canceled = Arc::new(AtomicBool::new(false));
            let status = Arc::new(Mutex::new(Status::starting(
                id.clone(),
                path.clone(),
                container.clone(),
                &configuration,
            )));
            let (updates, incoming) = mpsc::sync_channel(16);
            let signal = canceled.clone();
            let state = status.clone();
            let worker_path = path.clone();
            let worker_container = container.clone();
            let worker = thread::Builder::new()
                .name("receiver-lighting-owner".into())
                .spawn(move || {
                    let result = run(
                        &worker_path,
                        &worker_container,
                        configuration,
                        reply.clone(),
                        &incoming,
                        &signal,
                        &state,
                    );
                    if let Err(error) = &result {
                        let _ = reply.try_send(Err(format!("{error:#}")));
                    }
                    if let Ok(mut status) = state.lock() {
                        match result {
                            Ok(()) => status.software.phase = SoftwarePhase::Stopped,
                            Err(error) if error.downcast_ref::<LightingCanceled>().is_some() => {
                                status.software.phase = SoftwarePhase::Stopped;
                            }
                            Err(error) => {
                                status.software.phase = SoftwarePhase::Failed;
                                status.software.errors.push(format!("{error:#}"));
                            }
                        }
                    }
                })?;
            self.job = Some(Job {
                id,
                path,
                container,
                canceled,
                updates,
                status,
                worker: Some(worker),
            });
        }
        completed
            .recv_timeout(Duration::from_secs(90))
            .context("Dock lighting application timed out")?
            .map_err(anyhow::Error::msg)?;
        self.job.as_ref().unwrap().snapshot()
    }
    pub(super) fn poll(&self, id: &str) -> anyhow::Result<Status> {
        self.job
            .as_ref()
            .filter(|job| job.id == id)
            .context("Dock lighting owner identity differs")?
            .snapshot()
    }
    pub(super) fn chroma_attribute_result(
        &mut self,
        id: &str,
        worker_id: uuid::Uuid,
        request_id: u64,
        receipt: ChromaAttributeReceipt,
    ) -> anyhow::Result<Status> {
        let job = self
            .job
            .as_mut()
            .filter(|job| job.id == id)
            .context("Chroma attribute response belongs to a different dock owner")?;
        let (reply, completed) = mpsc::sync_channel(1);
        job.updates
            .send(Message::AttributeResult(
                worker_id, request_id, receipt, reply,
            ))
            .context("Dock monitor stopped before its local-service receipt")?;
        completed
            .recv_timeout(Duration::from_secs(30))
            .context("Dock local-service receipt delivery timed out")?
            .map_err(anyhow::Error::msg)?;
        job.snapshot()
    }
    pub(super) fn stop(&mut self, id: &str) -> anyhow::Result<Status> {
        self.job
            .as_mut()
            .filter(|job| job.id == id)
            .context("Dock lighting stop belongs to a different owner")?
            .stop()
    }
    pub(super) fn shutdown(&mut self) -> anyhow::Result<Option<Status>> {
        self.job.as_mut().map(Job::stop).transpose()
    }
}

fn run(
    path: &str,
    container: &str,
    mut configuration: Configuration,
    initial_reply: mpsc::SyncSender<Result<(), String>>,
    incoming: &mpsc::Receiver<Message>,
    canceled: &AtomicBool,
    status: &Mutex<Status>,
) -> anyhow::Result<()> {
    let (before, cap) = receiver::select_target(&hid::enumerate_metadata()?, path, container)?;
    ensure!(
        cap.product_id == configuration.product_id,
        "Dock lighting target product differs"
    );
    let bytes = before["feature_report_bytes"]
        .as_u64()
        .and_then(|bytes| usize::try_from(bytes).ok())
        .context("Dock Feature length is absent")?;
    let validate_identity = || -> anyhow::Result<()> {
        let (current, current_cap) = receiver::select_target(
            &hid::enumerate_for_product(cap.vendor_id, cap.product_id)?,
            path,
            container,
        )?;
        ensure!(
            current["device_instance_id"] == before["device_instance_id"]
                && current_cap.product_id == cap.product_id,
            "Dock lighting device instance changed"
        );
        Ok(())
    };
    let validate = || -> anyhow::Result<()> {
        if canceled.load(Ordering::Acquire) {
            return Err(LightingCanceled.into());
        }
        validate_identity()
    };
    let next = || receiver::next_transaction(path, container, cap);
    let _owner_lease = receiver::ReceiverLock::lighting_owner(container, &validate)?;
    let mut lighting = LightingHid::new(cap)?;
    let mut runtime = SharedEngine::new(Engine::open()?);
    let mut engine = DeviceEngines::new(
        runtime.clone(),
        led_configuration(configuration.product_id, configuration.edition_id)?,
    );
    let mut monitor = None;
    let mut custom_selected = false;
    let mut awaiting_current_frame = false;
    let mut pending = Some(initial_reply);
    let mut pending_deadline = Instant::now() + Duration::from_secs(30);
    let result = (|| -> anyhow::Result<()> {
        validate()?;
        monitor = Some(monitoring::Monitor::new(
            runtime.clone(),
            cap.clone(),
            path,
            container,
            before.clone(),
        )?);
        monitor
            .as_mut()
            .unwrap()
            .request(configuration.selected_docking_mouse.clone());
        if configuration.effect.source_id() != 14 {
            let acknowledgements = {
                let _lock = receiver::ReceiverLock::acquire(container)?;
                let device = hid_transport::open(path, bytes)?;
                if let Ok(mut state) = status.lock() {
                    state.restoration = RestorationDisposition::AwaitingAcknowledgements;
                }
                lighting.select_custom(&device, next, &validate)?
            };
            custom_selected = true;
            engine.apply_normal(&configuration.effect.engine_effect()?)?;
            awaiting_current_frame = true;
            if let Ok(mut state) = status.lock() {
                state.software.acknowledged_writes += acknowledgements;
            }
        }
        loop {
            if canceled.load(Ordering::Acquire) {
                break;
            }
            match incoming.recv_timeout(Duration::from_millis(40)) {
                Ok(Message::Apply(next_configuration, reply)) => {
                    next_configuration.validate()?;
                    ensure!(
                        configuration.product_id == next_configuration.product_id
                            && configuration.edition_id == next_configuration.edition_id,
                        "Dock manifest changed before owner release"
                    );
                    if let Some(previous) = pending.take() {
                        let _ =
                            previous.send(Err("Dock profile application was superseded".into()));
                    }
                    let unchanged = configuration
                        .effect
                        .same_ui_effect(&next_configuration.effect);
                    if let Ok(mut state) = status.lock() {
                        state.software.generation += 1;
                        state.effect = next_configuration.effect.clone();
                        // Dock selection is a separate source owner update. Spectrum
                        // disables its battery-level effect; no battery write is sent.
                        state.selected_docking_mouse =
                            next_configuration.selected_docking_mouse.clone();
                        state.source_effect_unchanged = unchanged;
                        state.application = ApplicationReceipt::AwaitingFrame {
                            generation: state.software.generation,
                        };
                    }
                    monitor
                        .as_mut()
                        .unwrap()
                        .request(next_configuration.selected_docking_mouse.clone());
                    if unchanged && next_configuration.effect.source_id() != 14 {
                        if let Ok(mut state) = status.lock() {
                            if state.software.frames_submitted > 0 {
                                state.application = ApplicationReceipt::SubmittedFrame {
                                    generation: state.software.generation,
                                };
                            }
                        }
                        let _ = reply.send(Ok(()));
                    } else if next_configuration.effect.source_id() != 14 {
                        let acknowledgements = {
                            let _lock = receiver::ReceiverLock::acquire(container)?;
                            let device = hid_transport::open(path, bytes)?;
                            if let Ok(mut state) = status.lock() {
                                state.restoration =
                                    RestorationDisposition::AwaitingAcknowledgements;
                            }
                            lighting.select_custom(&device, next, &validate)?
                        };
                        custom_selected = true;
                        engine.apply_normal(&next_configuration.effect.engine_effect()?)?;
                        awaiting_current_frame = true;
                        if let Ok(mut state) = status.lock() {
                            state.software.phase = SoftwarePhase::Starting;
                            state.software.acknowledged_writes += acknowledgements;
                        }
                        pending = Some(reply);
                        pending_deadline = Instant::now() + Duration::from_secs(30);
                    } else {
                        awaiting_current_frame = false;
                        pending = Some(reply);
                        pending_deadline = Instant::now() + Duration::from_secs(30);
                    }
                    configuration = next_configuration;
                }
                Ok(Message::Wake) => {}
                Ok(Message::AttributeResult(worker_id, request_id, receipt, reply)) => {
                    let result = monitor
                        .as_mut()
                        .unwrap()
                        .chroma_attribute_result(worker_id, request_id, receipt)
                        .map_err(|error| format!("{error:#}"));
                    if let Ok(mut state) = status.lock() {
                        state.monitoring = monitor.as_ref().unwrap().status.clone();
                    }
                    let _ = reply.send(result);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            validate()?;
            let monitor = monitor.as_mut().unwrap();
            let effects = monitor.tick(configuration.effect.source_id() == 14, &validate)?;
            if monitor.status.pending_chroma_attribute.is_none()
                && !monitor.status.charging
                && engine.is_special()
            {
                let normal = (configuration.effect.source_id() != 14)
                    .then(|| configuration.effect.engine_effect())
                    .transpose()?;
                engine.remove_special(normal.as_ref())?;
                awaiting_current_frame |= normal.is_some();
            }
            let mut acknowledged = effects.acknowledgements;
            if effects.normal.is_some() || effects.special.is_some() {
                pending_deadline = Instant::now() + Duration::from_secs(30);
                if !custom_selected {
                    let _lock = receiver::ReceiverLock::acquire(container)?;
                    let device = hid_transport::open(path, bytes)?;
                    if let Ok(mut state) = status.lock() {
                        state.restoration = RestorationDisposition::AwaitingAcknowledgements;
                    }
                    acknowledged += lighting.select_custom(&device, next, &validate)?;
                    custom_selected = true;
                }
                if let Some(effect) = effects.normal {
                    engine.apply_normal(&effect)?;
                }
                if let Some(effect) = effects.special {
                    engine.apply_special(&effect)?;
                }
                awaiting_current_frame = true;
            }
            if let Ok(mut state) = status.lock() {
                state.monitoring = monitor.status.clone();
                for error in &monitor.status.cleanup_errors {
                    if !state.software.cleanup_errors.contains(error) {
                        state.software.cleanup_errors.push(error.clone());
                    }
                }
                state.software.acknowledged_writes += acknowledged;
                if state.monitoring.pending_chroma_attribute.is_some() {
                    state.application = ApplicationReceipt::AwaitingLocalService {
                        generation: state.software.generation,
                    };
                    if let Some(reply) = pending.take() {
                        let _ = reply.send(Ok(()));
                    }
                }
                if let Some(reason) = effects.no_write {
                    state.application = ApplicationReceipt::SourceNoWrite {
                        generation: state.software.generation,
                        reason,
                    };
                    state.software.phase = SoftwarePhase::Running;
                    awaiting_current_frame = false;
                    if let Some(reply) = pending.take() {
                        let _ = reply.send(Ok(()));
                    }
                }
            }
            for frame in runtime.frames()? {
                let dock_frame = engine.device_handle == Some(frame.device_handle);
                let delivery = (|| -> anyhow::Result<u32> {
                    if dock_frame {
                        let _lock = receiver::ReceiverLock::acquire(container)?;
                        let device = hid_transport::open(path, bytes)?;
                        lighting.send_frame(&device, &frame, frame.device_handle, &validate)?;
                        Ok(1)
                    } else {
                        monitor
                            .send_frame(&frame, &validate)?
                            .context("Lighting frame belongs to no current dock or peer handle")
                    }
                })();
                let reports = match delivery {
                    Ok(reports) => reports,
                    Err(error) => {
                        validate()?;
                        if monitor.status.diagnostics.len() >= 64 {
                            monitor.status.diagnostics.remove(0);
                        }
                        monitor.status.diagnostics.push(format!("Rendered frame was not submitted; later source frames remain active: {error:#}"));
                        if let Ok(mut state) = status.lock() {
                            state.monitoring = monitor.status.clone();
                        }
                        continue;
                    }
                };
                if let Ok(mut state) = status.lock() {
                    state.software.frames_submitted += 1;
                    state.software.reports_submitted += u64::from(reports);
                    if dock_frame && awaiting_current_frame {
                        state.application = ApplicationReceipt::SubmittedFrame {
                            generation: state.software.generation,
                        };
                        state.software.phase = SoftwarePhase::Running;
                        awaiting_current_frame = false;
                        if let Some(reply) = pending.take() {
                            let _ = reply.send(Ok(()));
                        }
                    }
                }
            }
            ensure!(
                !awaiting_current_frame || Instant::now() < pending_deadline,
                "Dock renderer produced no submitted frame"
            );
        }
        Ok(())
    })();
    if let Some(reply) = pending.take() {
        let detail = result
            .as_ref()
            .err()
            .map(|error| format!("{error:#}"))
            .unwrap_or_else(|| "Dock lighting canceled before first frame".into());
        let _ = reply.send(Err(detail));
    }
    let mut errors = engine.cleanup();
    if let Some(monitor) = monitor.as_mut() {
        errors.extend(monitor.cleanup(&validate_identity));
    }
    if let Err(error) = runtime.terminate() {
        errors.push(format!("Dock lighting runtime termination: {error:#}"));
    }
    if let Ok(mut state) = status.lock() {
        state.software.cleanup_errors.extend(errors);
    }
    if lighting.restoration_required() {
        let restore = (|| {
            let _lock = receiver::ReceiverLock::acquire(container)?;
            let device = hid_transport::open(path, bytes)?;
            lighting.restore_on_exit(
                &device,
                configuration.effect.source_id() == 14,
                next,
                &validate_identity,
            )
        })();
        if let Ok(mut state) = status.lock() {
            match restore {
                Ok(acknowledgements) => {
                    state.restoration = RestorationDisposition::Acknowledged {
                        writes: acknowledgements,
                    };
                    state.software.restore_acknowledged = true;
                    state.software.acknowledged_writes += acknowledgements;
                }
                Err(error) => state
                    .software
                    .cleanup_errors
                    .push(format!("Dock exit lighting restoration: {error:#}")),
            }
        }
    }
    result
}
