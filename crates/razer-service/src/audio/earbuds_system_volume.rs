//! Retained local Core Audio session for the five earbud Sound volume cards.
use anyhow::{Result, ensure};
use razer_device::earbuds_system_volume::{Mutation, Scope, Setting, Snapshot, Update, select};
use razer_device::simple_audio::SimpleAudioEndpoint;
use razer_device::simple_audio_volume::{AudioVolumeReadResult, AudioVolumeWriteResult};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, RecvTimeoutError},
};
use std::time::Duration;
use uuid::Uuid;

#[cfg(windows)]
#[path = "../platform/windows/earbuds_system_volume.rs"]
mod windows;

pub struct Command {
    scope_id: Uuid,
    request_id: Uuid,
    setting: Setting,
}
impl Command {
    pub fn new(scope_id: Uuid, request_id: Uuid, setting: Setting) -> Self {
        Self {
            scope_id,
            request_id,
            setting,
        }
    }
}

#[derive(Default)]
struct Notifications {
    speaker: bool,
    default_endpoint: Option<String>,
    topology: bool,
}
trait Backend {
    fn enumerate(&mut self) -> Result<Vec<SimpleAudioEndpoint>>;
    fn read(&mut self, id: &str) -> Result<AudioVolumeReadResult>;
    fn write(&mut self, id: &str, mute: bool, volume: u8) -> Result<AudioVolumeWriteResult>;
    fn notifications(&mut self) -> Result<Notifications>;
    fn close(&mut self) -> Result<()>;
}
struct Session<B> {
    scope: Scope,
    endpoint: Option<SimpleAudioEndpoint>,
    backend: B,
}
impl<B: Backend> Session<B> {
    fn observe(&mut self) -> Result<Snapshot> {
        let endpoint = select(&self.scope, self.backend.enumerate()?)?;
        if let Some(previous) = &self.endpoint {
            ensure!(
                previous.id == endpoint.id,
                "Selected earbud endpoint changed; reopen the device session"
            );
        }
        let reading = self.backend.read(&endpoint.id)?;
        let snapshot = Snapshot::from_read(endpoint.clone(), reading)?;
        self.endpoint = Some(endpoint);
        Ok(snapshot)
    }
    fn refresh(&mut self) -> Update {
        Update::new(self.observe().map_err(|e| format!("{e:#}")))
    }
    fn notification(&mut self) -> Result<Option<Update>> {
        let notice = self.backend.notifications()?;
        // audioBasicFeature re-reads the selected endpoint on every default
        // speaker-volume event. Ls forwards default-change only for this product.
        let selected_default = notice.default_endpoint.as_ref().is_some_and(|id| {
            self.endpoint
                .as_ref()
                .is_some_and(|endpoint| endpoint.id == *id)
        });
        Ok((notice.speaker || selected_default || notice.topology).then(|| self.refresh()))
    }
    fn edit(&mut self, command: Command, active: &AtomicBool) -> Update {
        let mut mutation = None;
        let mut observed = None;
        let result = (|| -> Result<()> {
            ensure!(
                command.scope_id == self.scope.id(),
                "Volume command belongs to another session"
            );
            ensure!(
                command.setting.volume() <= 100,
                "Earbud volume is outside 0..100"
            );
            ensure!(
                active.load(Ordering::Acquire),
                "Earbud volume edit was cancelled before reading"
            );
            let previous = self.observe()?;
            let desired = command.setting;
            observed = Some(previous.clone());
            ensure!(
                active.load(Ordering::Acquire),
                "Earbud volume edit was cancelled before submission"
            );
            if !desired.needs_write(previous.setting()) {
                mutation = Some(Mutation::Unchanged);
                return Ok(());
            }
            observed = None;
            let write = self.backend.write(
                &previous.endpoint().id,
                !desired.enabled(),
                desired.volume(),
            )?;
            let confirmation = write
                .observation
                .clone()
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        write
                            .observation_error
                            .clone()
                            .unwrap_or_else(|| "Volume submission has no readback".into())
                    )
                })
                .and_then(|reading| Snapshot::from_read(previous.endpoint().clone(), reading));
            let source_ok = write.source_response.result;
            let reason = write.source_response.reason.clone();
            mutation = Some(Mutation::Written(write));
            // Even a failed mute can have changed volume. Keep the real
            // readback separately from the operation's success/failure.
            let confirmed = confirmation?;
            ensure!(
                active.load(Ordering::Acquire),
                "Volume may have changed after cancellation; observation is no longer current"
            );
            let current = select(&self.scope, self.backend.enumerate()?)?;
            ensure!(
                current.id == previous.endpoint().id,
                "Earbud endpoint changed after volume submission"
            );
            observed = Some(confirmed);
            ensure!(source_ok, "Volume or mute submission failed: {reason}");
            ensure!(
                observed
                    .as_ref()
                    .is_some_and(|value| value.setting() == desired),
                "Volume readback differs from the requested setting"
            );
            Ok(())
        })();
        let error = result.err().map(|e| format!("{e:#}"));
        let observation = observed.ok_or_else(|| {
            error
                .clone()
                .unwrap_or_else(|| "Volume has not been observed".into())
        });
        Update::new(observation)
            .with_request_id(command.request_id)
            .with_mutation(mutation)
            .with_error(error)
    }
}

/// Called on one owned background thread. Commands and notifications never run
/// in render or on the UI thread. The caller cancels, then awaits this function.
pub fn run(
    scope: Scope,
    active: Arc<AtomicBool>,
    commands: Receiver<Command>,
    events: async_channel::Sender<Update>,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        let backend = windows::Monitor::open().map_err(|e| format!("{e:#}"))?;
        run_session(
            Session {
                scope,
                endpoint: None,
                backend,
            },
            active,
            commands,
            events,
        )
    }
    #[cfg(not(windows))]
    {
        let _ = (scope, active, commands, events);
        Err("Earbud system volume requires Windows Core Audio".into())
    }
}

fn run_session<B: Backend>(
    mut session: Session<B>,
    active: Arc<AtomicBool>,
    commands: Receiver<Command>,
    events: async_channel::Sender<Update>,
) -> Result<(), String> {
    let mut failure = None;
    if active.load(Ordering::Acquire) {
        let _ = events.send_blocking(session.refresh());
    }
    while active.load(Ordering::Acquire) && !events.is_closed() {
        match commands.recv_timeout(Duration::from_millis(100)) {
            Ok(command) => {
                let _ = events.send_blocking(session.edit(command, &active));
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if !active.load(Ordering::Acquire) {
            break;
        }
        match session.notification() {
            Ok(Some(update)) => {
                let _ = events.send_blocking(update);
            }
            Ok(None) => {}
            Err(error) => {
                let message = format!("{error:#}");
                let _ = events
                    .send_blocking(Update::new(Err(message.clone())).with_error(Some(message)));
            }
        }
    }
    // Failed unregistration retains the COM owner on its original thread.
    // Report the failure and keep cleanup pending until the resources release.
    loop {
        match session.backend.close() {
            Ok(()) => break,
            Err(error) => {
                let message = format!("Core Audio cleanup is pending: {error:#}");
                if failure.as_ref() != Some(&message) {
                    let _ = events.send_blocking(
                        Update::new(Err(message.clone())).with_error(Some(message.clone())),
                    );
                    failure = Some(message);
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
    failure.map_or(Ok(()), Err)
}

#[cfg(test)]
#[path = "earbuds_system_volume_tests.rs"]
mod tests;
