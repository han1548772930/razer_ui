//! Retain the native engine, HID channel and cancellation until frames stop,
//! verified exit defaults are attempted and native handles have been released.
use anyhow::{Context as _, ensure};
use razer_device::{
    backend::{HidBackend, HidNode},
    controller_lighting::{
        LightingOperation, LightingProtocol, LightingResult, SoftwareEffect, SoftwarePhase,
        SoftwareStatus, capability,
        software::{SoftwareHid, led_configuration},
    },
};
use razer_hid::with_backend;
use serde_json::{Value, json};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[cfg(windows)]
use crate::runtime::{lighting_engine_native as native, lighting_engine_session::Session};

enum Message {
    Update {
        edition: u8,
        effect: SoftwareEffect,
        generation: u64,
    },
    Request {
        operation: LightingOperation,
        response: mpsc::SyncSender<Result<LightingResult, String>>,
    },
    Input {
        generation: u64,
    },
    Wake,
}
struct Job {
    node: HidNode,
    generation: u64,
    sender: mpsc::SyncSender<Message>,
    canceled: Arc<AtomicBool>,
    status: Arc<Mutex<SoftwareStatus>>,
    thread: Option<JoinHandle<()>>,
}
impl Job {
    fn snapshot(&self) -> anyhow::Result<SoftwareStatus> {
        self.status
            .lock()
            .map(|status| status.clone())
            .map_err(|_| anyhow::anyhow!("Lighting worker status was poisoned"))
    }
    fn stop(&mut self) -> anyhow::Result<SoftwareStatus> {
        self.canceled.store(true, Ordering::Release);
        let _ = self.sender.try_send(Message::Wake);
        if let Some(worker) = self.thread.take() {
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("Lighting worker panicked during native cleanup"))?;
        }
        self.snapshot()
    }
    fn stop_result(&mut self) -> anyhow::Result<LightingResult> {
        let restored_before_stop = self.snapshot()?.restore_acknowledged;
        let software = self.stop()?;
        Ok(LightingResult {
            acknowledged_writes: u32::from(!restored_before_stop && software.restore_acknowledged),
            software: Some(software),
            ..Default::default()
        })
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[derive(Default)]
pub(in crate::runtime) struct Controller {
    job: Option<Job>,
    next_generation: u64,
}
impl Controller {
    pub(in crate::runtime) fn execute(
        &mut self,
        node: &HidNode,
        product_id: u32,
        operation: &LightingOperation,
    ) -> anyhow::Result<Value> {
        operation.validate_product(product_id)?;
        let software = match operation {
            LightingOperation::Software { edition, effect } => {
                validate_source_node(node, product_id)?;
                if self
                    .job
                    .as_ref()
                    .is_some_and(|job| job.thread.as_ref().is_some_and(JoinHandle::is_finished))
                {
                    self.job.take().unwrap().stop()?;
                }
                self.next_generation = self
                    .next_generation
                    .checked_add(1)
                    .context("Lighting generation exhausted")?;
                let generation = self.next_generation;
                if let Some(job) = self.job.as_mut() {
                    ensure!(
                        job.node == *node,
                        "Another controller owns this lighting worker"
                    );
                    job.sender
                        .send(Message::Update {
                            edition: *edition,
                            effect: effect.clone(),
                            generation,
                        })
                        .context("Lighting worker disconnected")?;
                    job.generation = generation;
                } else {
                    self.start(node.clone(), *edition, effect.clone(), generation)?;
                }
                Some(SoftwareStatus::starting(generation))
            }
            LightingOperation::PollSoftware => Some(self.selected(node)?.snapshot()?),
            LightingOperation::StopSoftware => {
                let result = self.selected_mut(node)?.stop_result()?;
                return reply(node, product_id, operation, result);
            }
            LightingOperation::SoftwareInput { .. } => {
                let job = self.selected(node)?;
                job.sender
                    .try_send(Message::Input {
                        generation: job.generation,
                    })
                    .context("Lighting input queue is full or disconnected")?;
                Some(job.snapshot()?)
            }
            LightingOperation::SuspendSoftware { .. } => {
                let result = self.request(node, operation.clone())?;
                return reply(node, product_id, operation, result);
            }
            _ if self.job.as_ref().is_some_and(|job| {
                job.node == *node
                    && job
                        .thread
                        .as_ref()
                        .is_some_and(|worker| !worker.is_finished())
            }) =>
            {
                let result = self.request(node, operation.clone())?;
                return reply(node, product_id, operation, result);
            }
            _ => return super::execute(node, product_id, operation),
        };
        reply(
            node,
            product_id,
            operation,
            LightingResult {
                software,
                ..Default::default()
            },
        )
    }
    fn selected(&self, node: &HidNode) -> anyhow::Result<&Job> {
        self.job
            .as_ref()
            .filter(|job| job.node == *node)
            .context("No retained software lighting owner for this collection")
    }
    fn selected_mut(&mut self, node: &HidNode) -> anyhow::Result<&mut Job> {
        self.job
            .as_mut()
            .filter(|job| job.node == *node)
            .context("No retained software lighting owner for this collection")
    }
    fn request(
        &self,
        node: &HidNode,
        operation: LightingOperation,
    ) -> anyhow::Result<LightingResult> {
        let job = self.selected(node)?;
        ensure!(
            !job.canceled.load(Ordering::Acquire),
            "Lighting owner has stopped"
        );
        let (response, receiver) = mpsc::sync_channel(1);
        job.sender
            .send(Message::Request {
                operation,
                response,
            })
            .context("Lighting owner disconnected")?;
        receiver
            .recv_timeout(Duration::from_secs(90))
            .context("Lighting operation timed out")?
            .map_err(anyhow::Error::msg)
    }
    #[cfg(windows)]
    fn start(
        &mut self,
        node: HidNode,
        edition: u8,
        effect: SoftwareEffect,
        generation: u64,
    ) -> anyhow::Result<()> {
        let (sender, receiver) = mpsc::sync_channel(256);
        let canceled = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new(SoftwareStatus::starting(generation)));
        let worker_node = node.clone();
        let worker_signal = canceled.clone();
        let worker_status = status.clone();
        let worker = thread::Builder::new()
            .name("kitsune-lighting-owner".into())
            .spawn(move || {
                run(
                    &worker_node,
                    edition,
                    &effect,
                    generation,
                    &receiver,
                    &worker_signal,
                    &worker_status,
                );
            })?;
        self.job = Some(Job {
            node,
            generation,
            sender,
            canceled,
            status,
            thread: Some(worker),
        });
        Ok(())
    }
    #[cfg(not(windows))]
    fn start(&mut self, _: HidNode, _: u8, _: SoftwareEffect, _: u64) -> anyhow::Result<()> {
        anyhow::bail!("The current Kitsune native software engine requires the Windows adapter")
    }
}

fn reply(
    node: &HidNode,
    product_id: u32,
    operation: &LightingOperation,
    result: LightingResult,
) -> anyhow::Result<Value> {
    Ok(
        json!({"node":node,"product_id":product_id,"operation":operation,
        "device_write_acknowledged":result.acknowledged_writes > 0,
        "device_read_observed":result.effect.is_some() || result.brightness.is_some(),
        "lighting_result":result,"transport_metadata":{"retained_software_owner":true}}),
    )
}

fn validate_source_node(node: &HidNode, product_id: u32) -> anyhow::Result<()> {
    let source = capability(product_id).context("No mounted lighting source capability")?;
    ensure!(
        node.vendor_id == 5426
            && source.accepts(u32::from(node.product_id))
            && !node.path.is_empty(),
        "Lighting collection belongs to another product"
    );
    ensure!(
        node.interface_number == source.claim_interface()
            || cfg!(windows) && node.interface_number == -1,
        "Lighting collection differs from the current claim interface"
    );
    Ok(())
}

fn diagnostic(list: &mut Vec<String>, error: impl Into<String>) {
    if list.len() == 32 {
        list.remove(0);
    }
    list.push(error.into());
}

#[derive(Debug)]
struct LightingCanceled;
impl std::fmt::Display for LightingCanceled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Lighting owner canceled")
    }
}
impl std::error::Error for LightingCanceled {}

fn check_cancellation(canceled: &AtomicBool) -> anyhow::Result<()> {
    if canceled.load(Ordering::Acquire) {
        return Err(LightingCanceled.into());
    }
    Ok(())
}

fn complete_status(status: &Mutex<SoftwareStatus>, result: anyhow::Result<()>) {
    let result = match result {
        Err(error) if error.is::<LightingCanceled>() => Ok(()),
        result => result,
    };
    if let Ok(mut state) = status.lock() {
        if let Err(error) = result {
            diagnostic(&mut state.errors, format!("{error:#}"));
            state.phase = SoftwarePhase::Failed;
        } else if state.phase != SoftwarePhase::Failed {
            state.phase = SoftwarePhase::Stopped;
        }
    }
}

#[cfg(windows)]
fn run(
    node: &HidNode,
    edition: u8,
    effect: &SoftwareEffect,
    generation: u64,
    receiver: &mpsc::Receiver<Message>,
    canceled: &AtomicBool,
    status: &Mutex<SoftwareStatus>,
) {
    let result = run_checked(
        node, edition, effect, generation, receiver, canceled, status,
    );
    complete_status(status, result);
}

#[cfg(windows)]
fn run_checked(
    node: &HidNode,
    edition: u8,
    effect: &SoftwareEffect,
    generation: u64,
    receiver: &mpsc::Receiver<Message>,
    canceled: &AtomicBool,
    status: &Mutex<SoftwareStatus>,
) -> anyhow::Result<()> {
    validate_source_node(node, 4115)?;
    let proof = if node.interface_number == -1 {
        Some(super::super::native::controller_feature_node_proof(
            node, 2,
        )?)
    } else {
        None
    };
    let validate = || -> anyhow::Result<()> {
        let nodes = with_backend(|backend| backend.enumerate())?;
        ensure!(
            nodes.iter().filter(|observed| *observed == node).count() == 1,
            "Kitsune lighting collection changed or is ambiguous"
        );
        if let Some(proof) = &proof {
            ensure!(
                super::super::native::controller_feature_node_proof(node, 2)? == *proof,
                "Kitsune Windows device instance changed"
            );
        }
        Ok(())
    };
    validate()?;
    let device = with_backend(|backend| backend.open(node))?;
    ensure!(
        device.report_lengths()?.feature.get(&4) == Some(&91),
        "Kitsune descriptor lacks its source Feature channel"
    );
    let checked = super::CheckedTransport {
        inner: device.as_ref(),
        validate: &validate,
    };
    let mut hid = SoftwareHid::new(&checked);
    let wait = |duration: Duration| -> anyhow::Result<()> {
        check_cancellation(canceled)?;
        validate()?;
        thread::sleep(duration);
        check_cancellation(canceled)?;
        validate()
    };
    let mut session = None;
    let result = (|| -> anyhow::Result<()> {
        hid.select_custom(&mut |duration| wait(duration))?;
        if let Ok(mut state) = status.lock() {
            state.acknowledged_writes += 1;
        }
        session = Some(Session::new(native::Engine::open()?));
        let session = session.as_mut().unwrap();
        session.initialize(&led_configuration(edition)?, effect)?;
        if let Ok(mut state) = status.lock() {
            state.phase = SoftwarePhase::Running;
        }
        let mut active_generation = generation;
        let mut active_edition = edition;
        let mut next_frame = Instant::now();
        loop {
            if canceled.load(Ordering::Acquire) {
                break;
            }
            let until_frame = next_frame
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(40));
            match receiver.recv_timeout(until_frame) {
                Ok(Message::Update {
                    edition,
                    effect,
                    generation,
                }) => {
                    active_generation = generation;
                    if let Ok(mut state) = status.lock() {
                        state.generation = generation;
                        state.phase = SoftwarePhase::Starting;
                    }
                    ensure!(
                        edition == active_edition,
                        "Lighting edition changed; release the previous device owner first"
                    );
                    hid.select_custom(&mut |duration| wait(duration))?;
                    active_edition = edition;
                    if let Ok(mut state) = status.lock() {
                        state.acknowledged_writes += 1;
                    }
                    session.update(&effect)?;
                    if let Ok(mut state) = status.lock() {
                        state.phase = SoftwarePhase::Running;
                    }
                }
                Ok(Message::Input { generation }) => {
                    if generation == active_generation && !session.suspended {
                        if let Err(e) = session.input() {
                            if let Ok(mut state) = status.lock() {
                                diagnostic(&mut state.errors, format!("Reactive input: {e:#}"));
                            }
                        }
                    }
                }
                Ok(Message::Request {
                    operation,
                    response,
                }) => {
                    let suspend_request =
                        matches!(operation, LightingOperation::SuspendSoftware { .. });
                    let result = (|| -> anyhow::Result<LightingResult> {
                        if let LightingOperation::SuspendSoftware { suspended } = operation {
                            if !suspended {
                                hid.select_custom(&mut |duration| wait(duration))?;
                                if let Ok(mut state) = status.lock() {
                                    state.acknowledged_writes += 1;
                                }
                            }
                            let transition = session.suspend(suspended);
                            let mut state = status
                                .lock()
                                .map_err(|_| anyhow::anyhow!("Lighting status poisoned"))?;
                            state.phase = if session.suspended {
                                SoftwarePhase::Suspended
                            } else {
                                SoftwarePhase::Running
                            };
                            transition?;
                            return Ok(LightingResult {
                                acknowledged_writes: u32::from(!suspended),
                                software: Some(state.clone()),
                                ..Default::default()
                            });
                        }
                        let deadline = Instant::now() + Duration::from_secs(90);
                        let operation_wait = |duration| {
                            ensure!(Instant::now() < deadline, "Lighting command timed out");
                            wait(duration)
                        };
                        let mut result = LightingProtocol::new(
                            &checked,
                            capability(4115).unwrap(),
                            operation_wait,
                        )
                        .execute(&operation)?;
                        if let Ok(mut state) = status.lock() {
                            state.acknowledged_writes += result.acknowledged_writes;
                            result.software = Some(state.clone());
                        }
                        Ok(result)
                    })();
                    let fatal = suspend_request
                        .then(|| result.as_ref().err().map(|error| format!("{error:#}")))
                        .flatten();
                    let _ = response.send(result.map_err(|error| format!("{error:#}")));
                    if let Some(error) = fatal {
                        return Err(anyhow::anyhow!(error));
                    }
                }
                Ok(Message::Wake) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if Instant::now() >= next_frame {
                validate()?;
                let frames = session.engine.frames()?;
                if !session.suspended {
                    for frame in frames {
                        let sent = hid.send_frame(
                            &frame,
                            session
                                .device_handle
                                .context("Missing lighting device handle")?,
                            &mut |duration| wait(duration),
                        )?;
                        if let Ok(mut state) = status.lock() {
                            state.reports_submitted += u64::from(sent.reports_submitted);
                            state.frames_submitted += u64::from(sent.complete);
                            for error in sent.errors {
                                diagnostic(&mut state.errors, error);
                            }
                        }
                        if let Some(error) = sent.interruption {
                            return Err(error);
                        }
                    }
                }
                next_frame = Instant::now() + Duration::from_millis(40);
            }
        }
        Ok(())
    })();
    if let Some(session) = session.as_mut() {
        let errors = session.cleanup();
        if let Ok(mut state) = status.lock() {
            for error in errors {
                diagnostic(&mut state.cleanup_errors, error);
            }
        }
    }
    // Cancellation stops frame production; restoration still validates the
    // retained physical identity and must not target a newly attached device.
    let deadline = Instant::now() + Duration::from_secs(30);
    let restore = hid.restore_on_exit(&mut |duration| {
        ensure!(Instant::now() < deadline, "Lighting restore timed out");
        validate()?;
        thread::sleep(duration);
        validate()
    });
    if let Ok(mut state) = status.lock() {
        match restore {
            Ok(()) => {
                state.restore_acknowledged = true;
                state.acknowledged_writes += 1;
            }
            Err(e) => diagnostic(
                &mut state.cleanup_errors,
                format!("Restore exit spectrum: {e:#}"),
            ),
        }
    }
    result
}

#[cfg(test)]
#[path = "controller_lighting_owner_tests.rs"]
mod tests;
