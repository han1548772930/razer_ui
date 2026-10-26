//! One retained owner serializes Marci rendering, both HID handles and cleanup.
use anyhow::{Context as _, ensure};
use razer_device::marci_lighting::{Gates, Operation, Status};
use razer_device::receiver_lighting::QuickEffect;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

#[cfg(windows)]
#[path = "marci_lighting_worker.rs"]
mod worker;

enum Message {
    Apply {
        generation: u64,
        effect: QuickEffect,
        reply: mpsc::SyncSender<Result<Status, String>>,
    },
    Gates {
        generation: u64,
        gates: Gates,
        reply: mpsc::SyncSender<Result<Status, String>>,
    },
    Wake,
}
struct Job {
    owner_id: String,
    container: String,
    generation: u64,
    sender: mpsc::SyncSender<Message>,
    canceled: Arc<AtomicBool>,
    status: Arc<Mutex<Status>>,
    thread: Option<JoinHandle<()>>,
}
impl Job {
    fn snapshot(&self) -> anyhow::Result<Status> {
        self.status
            .lock()
            .map(|s| s.clone())
            .map_err(|_| anyhow::anyhow!("Marci worker state is poisoned"))
    }
    fn stop(&mut self) -> anyhow::Result<Status> {
        self.canceled.store(true, Ordering::Release);
        let _ = self.sender.try_send(Message::Wake);
        if let Some(worker) = self.thread.take() {
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("Marci worker panicked during cleanup"))?;
        }
        self.snapshot()
    }
    fn receive(&self, receiver: mpsc::Receiver<Result<Status, String>>) -> anyhow::Result<Status> {
        receiver
            .recv_timeout(Duration::from_secs(90))
            .context("Marci command timed out")?
            .map_err(anyhow::Error::msg)
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
}
impl Controller {
    pub(in crate::runtime) fn execute(
        &mut self,
        container: &str,
        owner_id: &str,
        operation: Operation,
    ) -> anyhow::Result<serde_json::Value> {
        ensure!(
            !container.is_empty() && !owner_id.is_empty(),
            "Marci owner identity is absent"
        );
        if let Some(job) = &self.job {
            ensure!(
                job.owner_id == owner_id && job.container.eq_ignore_ascii_case(container),
                "Another physical owner retains Marci lighting"
            );
        }
        let status = match operation {
            Operation::Apply { generation, effect } => {
                validate_effect(&effect)?;
                ensure!(generation > 0, "Marci generation must be positive");
                if let Some(job) = &mut self.job {
                    ensure!(
                        !job.canceled.load(Ordering::Acquire)
                            && generation > job.generation
                            && job.thread.as_ref().is_some_and(|t| !t.is_finished()),
                        "Marci owner was stopped or generation is stale"
                    );
                    let (reply, receiver) = mpsc::sync_channel(1);
                    job.sender
                        .send(Message::Apply {
                            generation,
                            effect,
                            reply,
                        })
                        .context("Marci worker disconnected")?;
                    let status = job.receive(receiver)?;
                    job.generation = generation;
                    status
                } else {
                    self.start(container, owner_id, generation, effect)?
                }
            }
            Operation::SetGates { generation, gates } => {
                let job = self.job.as_ref().context("No retained Marci owner")?;
                ensure!(
                    !job.canceled.load(Ordering::Acquire) && generation == job.generation,
                    "Marci gate belongs to a stopped or replaced generation"
                );
                let (reply, receiver) = mpsc::sync_channel(1);
                job.sender
                    .send(Message::Gates {
                        generation,
                        gates,
                        reply,
                    })
                    .context("Marci worker disconnected")?;
                job.receive(receiver)?
            }
            Operation::Poll => self
                .job
                .as_ref()
                .context("No retained Marci owner")?
                .snapshot()?,
            Operation::Stop => self
                .job
                .as_mut()
                .context("No retained Marci owner")?
                .stop()?,
        };
        serde_json::to_value(status).map_err(Into::into)
    }
    pub(in crate::runtime) fn shutdown(&mut self, cleanup: &mut super::shutdown::Cleanup) {
        if let Some(mut job) = self.job.take() {
            let result = job.stop();
            if let Ok(status) = &result {
                if !status.cleanup_completed() {
                    cleanup.fail(
                        "marci_lighting",
                        format!("Marci cleanup was not confirmed: {status:?}"),
                    );
                }
            }
            cleanup.record("marci_lighting", result);
        }
    }
    #[cfg(windows)]
    fn start(
        &mut self,
        container: &str,
        owner: &str,
        generation: u64,
        effect: QuickEffect,
    ) -> anyhow::Result<Status> {
        let (sender, receiver) = mpsc::sync_channel(64);
        let canceled = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new(Status::starting(
            owner.into(),
            container.into(),
            generation,
        )));
        let (response, ready) = mpsc::sync_channel(1);
        let worker_container = container.to_owned();
        let worker_canceled = canceled.clone();
        let worker_status = status.clone();
        let thread = thread::Builder::new()
            .name("marci-lighting-owner".into())
            .spawn(move || {
                worker::run(
                    &worker_container,
                    generation,
                    &effect,
                    &receiver,
                    &worker_canceled,
                    &worker_status,
                    response,
                );
            })?;
        self.job = Some(Job {
            owner_id: owner.into(),
            container: container.into(),
            generation,
            sender,
            canceled,
            status,
            thread: Some(thread),
        });
        self.job.as_ref().unwrap().receive(ready)
    }
    #[cfg(not(windows))]
    fn start(&mut self, _: &str, _: &str, _: u64, _: QuickEffect) -> anyhow::Result<Status> {
        anyhow::bail!("Marci collection-to-container binding is unavailable on this platform")
    }
}

fn validate_effect(effect: &QuickEffect) -> anyhow::Result<()> {
    ensure!(
        [1, 2, 3, 4, 7, 8, 11, 12, 13, 19].contains(&effect.source_id()),
        "Marci Quick effect {} has not yet been independently implemented",
        effect.source_id()
    );
    effect.engine_effect_for_geometry(&razer_device::marci_lighting::led_configuration())?;
    Ok(())
}

fn update(status: &Mutex<Status>, operation: impl FnOnce(&mut Status)) {
    if let Ok(mut status) = status.lock() {
        operation(&mut status);
    }
}
fn snapshot(status: &Mutex<Status>) -> Result<Status, String> {
    status
        .lock()
        .map(|s| s.clone())
        .map_err(|_| "Marci worker state is poisoned".into())
}
