//! A serial native owner drains accepted commands before its service shutdown.
use anyhow::ensure;
use razer_device::keyboard_submission::{self, Intent};
use razer_discovery::{direct, discovery};
use razer_ipc::{ServiceClient, ServiceRequest};
use std::{
    collections::{BTreeSet, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

#[derive(Debug)]
pub(super) struct Command {
    pub(super) generation: u64,
    pub(super) intent: Intent,
}

pub(super) enum Reply {
    Finished(u64, Result<(), String>),
    CleanupFailed(String),
    Stopped,
}

pub(super) struct Admission<T> {
    commands: Option<mpsc::SyncSender<Command>>,
    pending: BTreeSet<u64>,
    waiting: VecDeque<T>,
}
impl<T> Admission<T> {
    pub(super) fn new(commands: mpsc::SyncSender<Command>) -> Self {
        Self {
            commands: Some(commands),
            pending: BTreeSet::new(),
            waiting: VecDeque::new(),
        }
    }

    pub(super) fn stop_after_pending(&mut self) {
        self.commands.take();
    }

    pub(super) fn submit(&mut self, command: Command, successor: T) -> Result<(), String> {
        if let Some(commands) = &self.commands {
            let generation = command.generation;
            commands
                .try_send(command)
                .map_err(|error| format!("Keyboard queue unavailable: {error}"))?;
            self.pending.insert(generation);
        } else if self.waiting.len() < 128 {
            self.waiting.push_back(successor);
        } else {
            return Err("Keyboard successor queue is full".into());
        }
        Ok(())
    }

    pub(super) fn finish(&mut self, generation: u64) {
        self.pending.remove(&generation);
    }

    pub(super) fn retire(&mut self) -> (BTreeSet<u64>, VecDeque<T>) {
        (
            std::mem::take(&mut self.pending),
            std::mem::take(&mut self.waiting),
        )
    }
}

trait Transport {
    fn submit(&mut self, intent: Intent, valid: &AtomicBool) -> anyhow::Result<()>;
    fn shutdown(&mut self) -> anyhow::Result<()>;
}

struct NativeTransport {
    client: ServiceClient,
    observation: discovery::ObservedDevice,
    profile: String,
}
impl Transport for NativeTransport {
    fn submit(&mut self, intent: Intent, valid: &AtomicBool) -> anyhow::Result<()> {
        let guard = || -> anyhow::Result<()> {
            ensure!(
                valid.load(Ordering::Acquire),
                "Keyboard connection/profile task was canceled"
            );
            Ok(())
        };
        guard()?;
        let capability = keyboard_submission::validate(self.observation.product_id(), &intent)?;
        let route = direct::resolve(
            &mut self.client,
            &self.observation,
            &capability.device_read_capability(),
        )?;
        route.submit_keyboard_control(&mut self.client, &self.profile, intent, guard)?;
        Ok(())
    }

    fn shutdown(&mut self) -> anyhow::Result<()> {
        ensure!(
            !self.client.is_stopped(),
            "Keyboard service terminated before acknowledging shutdown"
        );
        self.client
            .set_request_guard(Arc::new(AtomicBool::new(true)));
        let receipt = self.client.request(ServiceRequest::Shutdown)?;
        verify_shutdown(&receipt)
    }
}

fn verify_shutdown(receipt: &serde_json::Value) -> anyhow::Result<()> {
    ensure!(
        receipt["stopped"] == true,
        "Keyboard service did not acknowledge shutdown"
    );
    Ok(())
}

fn guarded<T>(operation: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation))
        .map_err(|_| anyhow::anyhow!("Keyboard submission owner panicked"))?
}

pub(super) fn run(
    observation: discovery::ObservedDevice,
    profile: String,
    valid: Arc<AtomicBool>,
    commands: mpsc::Receiver<Command>,
    replies: mpsc::Sender<Reply>,
) {
    let signal = valid.clone();
    run_with(valid, commands, replies, move || {
        let mut client = ServiceClient::spawn()?;
        client.set_request_guard(signal);
        Ok(NativeTransport {
            client,
            observation,
            profile,
        })
    });
}

fn run_with<T: Transport>(
    valid: Arc<AtomicBool>,
    commands: mpsc::Receiver<Command>,
    replies: mpsc::Sender<Reply>,
    factory: impl FnOnce() -> anyhow::Result<T>,
) {
    let result = guarded(factory).and_then(|mut transport| {
        while valid.load(Ordering::Acquire) {
            let command = match commands.recv_timeout(Duration::from_millis(16)) {
                Ok(command) => command,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            let result = guarded(|| {
                ensure!(
                    valid.load(Ordering::Acquire),
                    "Keyboard connection/profile task was canceled"
                );
                transport.submit(command.intent, &valid)
            })
            .map_err(|error| format!("{error:#}"));
            if replies
                .send(Reply::Finished(command.generation, result))
                .is_err()
            {
                break;
            }
        }
        // Dropping the final sender is a graceful drain. Scope invalidation
        // rejects the remaining commands and still performs service cleanup.
        for command in commands.try_iter() {
            let _ = replies.send(Reply::Finished(
                command.generation,
                Err("Keyboard connection/profile task was canceled".into()),
            ));
        }
        let cleanup = guarded(|| transport.shutdown());
        let destruction = guarded(|| {
            drop(transport);
            Ok(())
        });
        match (cleanup, destruction) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(cleanup), Err(destruction)) => {
                Err(anyhow::anyhow!("{cleanup:#}; {destruction:#}"))
            }
        }
    });
    if let Err(error) = result {
        let reason = format!("{error:#}");
        for command in commands.try_iter() {
            let _ = replies.send(Reply::Finished(command.generation, Err(reason.clone())));
        }
        let _ = replies.send(Reply::CleanupFailed(reason));
    }
    let _ = replies.send(Reply::Stopped);
}

#[cfg(test)]
#[path = "keyboard_actuation_worker_tests.rs"]
mod tests;
