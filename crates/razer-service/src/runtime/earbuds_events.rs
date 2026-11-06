//! Per-client earbuds subscription: a real begin acknowledgement and joined stop.
use anyhow::{Context as _, Result, anyhow, ensure};
use razer_device::{
    earbuds_events::{self, Notification, PollReply, StartReply, StopReply, Target},
    receiver_pairing::HardwareEvents,
};
use serde_json::Value;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

struct Active {
    id: String,
    canceled: Arc<AtomicBool>,
    events: Receiver<Notification>,
    worker: JoinHandle<Result<(), String>>,
}
struct Terminal {
    id: String,
    error: Option<String>,
}

#[derive(Default)]
pub(super) struct Controller {
    active: Option<Active>,
    terminal: Option<Terminal>,
}

fn joined(worker: JoinHandle<Result<(), String>>) -> Result<(), String> {
    worker
        .join()
        .unwrap_or_else(|_| Err("Earbud notification worker panicked".into()))
}

fn finish(source: &mut dyn HardwareEvents, result: Result<()>) -> Result<(), String> {
    match (result, source.finish()) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(format!("{error:#}")),
        (Ok(()), Err(error)) => Err(format!("Notification cleanup: {error:#}")),
        (Err(error), Err(cleanup)) => Err(format!("{error:#}; notification cleanup: {cleanup:#}")),
    }
}

impl Controller {
    pub(super) fn start(&mut self, id: String, target: Target) -> Result<Value> {
        target.product_id()?;
        self.start_with(id, move || {
            #[cfg(windows)]
            let source = super::native::earbuds_event_source(&target)?;
            #[cfg(not(windows))]
            let source = collection_source(target.clone())?;
            Ok(super::earbuds_auto_pause::registered_events(source, target))
        })
        .and_then(|reply| serde_json::to_value(reply).map_err(Into::into))
    }

    fn start_with(
        &mut self,
        id: String,
        factory: impl FnOnce() -> Result<Box<dyn HardwareEvents>> + Send + 'static,
    ) -> Result<StartReply> {
        ensure!(
            !id.is_empty() && id.len() <= 128 && self.active.is_none(),
            "Earbud event owner is invalid or already active"
        );
        ensure!(
            self.terminal.as_ref().is_none_or(|old| old.id != id),
            "A released earbud event identity cannot be reused"
        );
        let (owner, request_canceled) = super::hid_broker::current_request_context();
        super::hid_broker::check_current_request()?;
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        // A normal Stop lets the bounded current Input read finish. Only a
        // canceled startup aborts broker admission; otherwise its expected
        // cancellation error would incorrectly become a cleanup failure.
        let startup_canceled = Arc::new(AtomicBool::new(false));
        let io_canceled = startup_canceled.clone();
        let (sender, events) = mpsc::sync_channel(256);
        let (ready, started) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("razer-earbud-events".into())
            .spawn(move || {
                super::hid_broker::with_request_context(owner, io_canceled, || {
                    let mut source = match factory() {
                        Ok(source) => source,
                        Err(error) => {
                            let error = format!("{error:#}");
                            let _ = ready.send(Err(error.clone()));
                            return Err(error);
                        }
                    };
                    if let Err(error) = source.begin() {
                        let result = finish(source.as_mut(), Err(error));
                        let _ = ready.send(result.clone());
                        return result;
                    }
                    let result = (|| {
                        ready
                            .send(Ok(()))
                            .context("Earbud subscription starter disappeared")?;
                        while !signal.load(Ordering::Acquire) {
                            let bytes = source.receive(Duration::from_millis(50))?;
                            if signal.load(Ordering::Acquire) {
                                break;
                            }
                            if let Some(bytes) = bytes
                                && let Some(event) = earbuds_events::notification(&bytes)?
                            {
                                sender
                                    .try_send(event)
                                    .context("Earbud notification consumer is absent or full")?;
                            }
                        }
                        Ok(())
                    })();
                    finish(source.as_mut(), result)
                })
            })?;
        let startup = loop {
            if request_canceled.load(Ordering::Acquire) {
                canceled.store(true, Ordering::Release);
                startup_canceled.store(true, Ordering::Release);
            }
            match started.recv_timeout(Duration::from_millis(50)) {
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                result => break result,
            }
        };
        match startup {
            Ok(Ok(())) if !request_canceled.load(Ordering::Acquire) => {
                self.terminal = None;
                self.active = Some(Active {
                    id: id.clone(),
                    canceled,
                    events,
                    worker,
                });
                Ok(StartReply::started(id))
            }
            startup => {
                canceled.store(true, Ordering::Release);
                startup_canceled.store(true, Ordering::Release);
                let result = joined(worker);
                let error = result.err().unwrap_or_else(|| {
                    if request_canceled.load(Ordering::Acquire) {
                        "Earbud subscription initialization was canceled".into()
                    } else {
                        format!("Earbud subscription did not initialize: {startup:?}")
                    }
                });
                self.terminal = Some(Terminal {
                    id,
                    error: Some(error.clone()),
                });
                Err(anyhow!(error))
            }
        }
    }

    fn poll_reply(&mut self, id: &str) -> Result<PollReply> {
        if let Some(active) = &self.active {
            ensure!(active.id == id, "Earbud notification owner differs");
            let mut events: Vec<_> = active.events.try_iter().collect();
            if !active.worker.is_finished() {
                return Ok(PollReply::new(id.into(), events, false, None));
            }
            let active = self.active.take().expect("checked event owner");
            let error = joined(active.worker).err();
            events.extend(active.events.try_iter());
            self.terminal = Some(Terminal {
                id: id.into(),
                error: error.clone(),
            });
            return Ok(PollReply::new(id.into(), events, true, error));
        }
        let terminal = self
            .terminal
            .as_ref()
            .context("Earbud notification owner is absent")?;
        ensure!(terminal.id == id, "Earbud notification owner differs");
        Ok(PollReply::new(
            id.into(),
            vec![],
            true,
            terminal.error.clone(),
        ))
    }

    pub(super) fn poll(&mut self, id: &str) -> Result<Value> {
        serde_json::to_value(self.poll_reply(id)?).map_err(Into::into)
    }

    fn stop_reply(&mut self, id: &str) -> Result<StopReply> {
        if let Some(active) = &self.active {
            ensure!(active.id == id, "Earbud notification stop owner differs");
            let active = self.active.take().expect("checked event owner");
            active.canceled.store(true, Ordering::Release);
            self.terminal = Some(Terminal {
                id: id.into(),
                error: joined(active.worker).err(),
            });
        }
        let terminal = self
            .terminal
            .as_ref()
            .context("Earbud notification owner is absent")?;
        ensure!(terminal.id == id, "Earbud notification stop owner differs");
        if let Some(error) = &terminal.error {
            return Err(anyhow!(error.clone()));
        }
        Ok(StopReply::stopped(id.into()))
    }

    pub(super) fn stop(&mut self, id: &str) -> Result<Value> {
        serde_json::to_value(self.stop_reply(id)?).map_err(Into::into)
    }

    pub(super) fn shutdown(&mut self, cleanup: &mut super::shutdown::Cleanup) {
        let id = self
            .active
            .as_ref()
            .map(|active| active.id.clone())
            .or_else(|| self.terminal.as_ref().map(|terminal| terminal.id.clone()));
        if let Some(id) = id {
            cleanup.record("earbuds_events", self.stop_reply(&id));
        }
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        if let Some(active) = self.active.take() {
            active.canceled.store(true, Ordering::Release);
            let _ = joined(active.worker);
        }
    }
}

#[cfg(not(windows))]
fn collection_source(target: Target) -> Result<Box<dyn HardwareEvents>> {
    use razer_device::backend::{HidNode, InterruptTransport};
    use razer_hid::with_backend;
    struct Collection {
        node: HidNode,
        reader: Option<Box<dyn InterruptTransport>>,
    }
    impl HardwareEvents for Collection {
        fn begin(&mut self) -> Result<()> {
            ensure!(
                self.reader.is_none(),
                "Earbud event collection is already open"
            );
            let reader = with_backend(|backend| backend.open_interrupt(&self.node))?;
            let lengths = reader.report_lengths();
            let maximum = lengths
                .input
                .values()
                .max()
                .context("Selected earbud collection has no Input Report")?;
            ensure!(
                *maximum <= 100,
                "Earbud Input Report exceeds the source read capacity"
            );
            self.reader = Some(reader);
            Ok(())
        }
        fn receive(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>> {
            with_backend(|backend| {
                ensure!(
                    backend
                        .enumerate()?
                        .iter()
                        .filter(|node| **node == self.node)
                        .count()
                        == 1,
                    "Earbud event collection changed"
                );
                Ok(())
            })?;
            let mut bytes = vec![0; 100];
            let count = self
                .reader
                .as_ref()
                .context("Earbud Input reader is absent")?
                .read(&mut bytes, u32::try_from(timeout.as_millis())?)?;
            ensure!(
                count <= bytes.len(),
                "Earbud Input count exceeds the supplied buffer"
            );
            bytes.truncate(count);
            Ok((count > 0).then_some(bytes))
        }
        fn finish(&mut self) -> Result<()> {
            self.reader = None;
            Ok(())
        }
    }
    match target {
        Target::Collection(node) => Ok(Box::new(Collection { node, reader: None })),
        Target::WindowsContainer(_) => {
            anyhow::bail!("Windows ContainerId requires the native identity adapter")
        }
    }
}

#[cfg(test)]
#[path = "earbuds_events_tests.rs"]
mod tests;
