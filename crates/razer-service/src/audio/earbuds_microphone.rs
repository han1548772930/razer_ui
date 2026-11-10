//! One microphone session owns OS volume, HID mute and their independent results.
use anyhow::{Context as _, Result, ensure};
use razer_device::{
    earbuds_auto_pause::Operation,
    earbuds_microphone::{MuteReport, Scope, Setting, Snapshot, Update, select},
    earbuds_protocol40::{ToggleObservation, WriteOutcome},
    earbuds_system_volume::Snapshot as SystemSnapshot,
    simple_audio::SimpleAudioEndpoint,
    simple_audio_volume::{AudioVolumeReadResult, AudioVolumeWriteResult},
};
use razer_discovery::discovery::ObservedDevice;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, RecvTimeoutError},
    },
    time::Duration,
};
use uuid::Uuid;

pub enum Command {
    Edit {
        scope_id: Uuid,
        request_id: Uuid,
        setting: Setting,
    },
    HardwareMute {
        scope_id: Uuid,
        muted: u8,
    },
}
struct HardwareStep {
    report: Result<MuteReport, String>,
    cleanup_error: Option<String>,
}
trait Backend {
    fn enumerate(&mut self) -> Result<Vec<SimpleAudioEndpoint>>;
    fn read(&mut self, id: &str) -> Result<AudioVolumeReadResult>;
    fn write(&mut self, id: &str, muted: bool, volume: u8) -> Result<AudioVolumeWriteResult>;
    fn hardware(&mut self, muted: Option<bool>) -> HardwareStep;
    fn changed(&mut self) -> Result<bool>;
    fn close(&mut self) -> Result<()>;
}
struct Session<B> {
    scope: Scope,
    endpoint: Option<SimpleAudioEndpoint>,
    backend: B,
}
fn failure_list(messages: impl IntoIterator<Item = Option<String>>) -> Option<String> {
    let messages = messages.into_iter().flatten().collect::<Vec<_>>();
    (!messages.is_empty()).then(|| messages.join("; "))
}
impl<B: Backend> Session<B> {
    fn endpoint(&mut self) -> Result<SimpleAudioEndpoint> {
        let endpoint = select(&self.scope, self.backend.enumerate()?)?;
        if let Some(previous) = &self.endpoint {
            ensure!(
                previous.id == endpoint.id,
                "Earbud microphone endpoint changed; reopen the session"
            );
        }
        self.endpoint = Some(endpoint.clone());
        Ok(endpoint)
    }
    fn system(&mut self) -> Result<SystemSnapshot> {
        let endpoint = self.endpoint()?;
        let read = self.backend.read(&endpoint.id)?;
        SystemSnapshot::from_read(endpoint, read)
    }
    fn observe(&mut self) -> Update {
        // audioBasicFeature reads system volume before overriding only the
        // displayed enabled field with hardware mute. OS mute stays separate.
        let system = self.system().map_err(|error| format!("{error:#}"));
        let hardware = self.backend.hardware(None);
        let mute = match hardware.report {
            Ok(report) => report.observation().clone(),
            Err(error) => ToggleObservation::ReadFailed(error),
        };
        let snapshot = Snapshot::new(system, mute);
        let error = failure_list([snapshot.diagnostic(), hardware.cleanup_error]);
        Update::new(Ok(snapshot)).with_error(error)
    }
    fn edit(&mut self, setting: Setting, active: &AtomicBool) -> Update {
        if !active.load(Ordering::Acquire) {
            return Update::new(Err("Earbud microphone request was cancelled".into()));
        }
        let endpoint = match self.endpoint() {
            Ok(endpoint) => endpoint,
            Err(error) => return Update::new(Err(format!("{error:#}"))),
        };
        // Native setMicVolume resolves the endpoint, reads/changes device mute,
        // then reads OS mute and submits the requested level preserving OS mute.
        let hardware = self.backend.hardware(Some(!setting.enabled()));
        let (mute, hardware_write, hardware_error) = match hardware.report {
            Ok(report) => {
                let error = match report.write() {
                    WriteOutcome::Failed(error) => Some(error.clone()),
                    _ => None,
                };
                (
                    report.observation().clone(),
                    Some(report.write().clone()),
                    error,
                )
            }
            Err(error) => (
                ToggleObservation::ReadFailed(error.clone()),
                None,
                Some(error),
            ),
        };
        let mut write = None;
        let mut system = Err("Microphone system volume has not been read".into());
        let system_result = (|| -> Result<()> {
            ensure!(
                active.load(Ordering::Acquire),
                "Microphone request cancelled before OS volume submission"
            );
            let current = self.backend.read(&endpoint.id)?;
            let previous = SystemSnapshot::from_read(endpoint.clone(), current)?;
            let was_muted = !previous.setting().enabled();
            system = Err("Microphone OS submission has not been confirmed".into());
            let result = self
                .backend
                .write(&endpoint.id, was_muted, setting.volume())?;
            let observed = result
                .observation
                .clone()
                .context("Microphone OS write has no readback")
                .and_then(|reading| SystemSnapshot::from_read(endpoint.clone(), reading));
            let accepted = result.source_response.result;
            let reason = result.source_response.reason.clone();
            write = Some(result);
            system = observed.map_err(|error| format!("{error:#}"));
            let observed = system
                .as_ref()
                .map_err(|error| anyhow::anyhow!(error.clone()))?;
            ensure!(
                active.load(Ordering::Acquire),
                "Microphone request cancelled after OS submission"
            );
            ensure!(accepted, "Microphone OS volume write failed: {reason}");
            ensure!(
                observed.setting().volume() == setting.volume()
                    && observed.setting().enabled() != was_muted,
                "Microphone OS readback differs from the requested volume or preserved mute"
            );
            ensure!(
                self.endpoint()?.id == endpoint.id,
                "Microphone endpoint changed after submission"
            );
            Ok(())
        })();
        if let Err(error) = &system_result {
            if system.is_err() {
                system = Err(format!("{error:#}"));
            }
        }
        let snapshot = Snapshot::new(system, mute);
        let error = failure_list([
            hardware_error,
            hardware.cleanup_error,
            system_result.err().map(|error| format!("{error:#}")),
            snapshot.diagnostic(),
        ]);
        Update::new(Ok(snapshot))
            .with_writes(hardware_write, write)
            .with_error(error)
    }
    fn hardware_notice(&mut self, raw: u8) -> Update {
        let hardware = match raw {
            0 => ToggleObservation::Observed(false),
            1 => ToggleObservation::Observed(true),
            raw => ToggleObservation::Unknown(raw),
        };
        let snapshot = Snapshot::new(
            self.system().map_err(|error| format!("{error:#}")),
            hardware,
        );
        let error = snapshot.diagnostic();
        Update::new(Ok(snapshot)).with_error(error)
    }
}

#[cfg(windows)]
struct WindowsBackend {
    route: ObservedDevice,
    active: Arc<AtomicBool>,
    monitor: crate::earbuds_system_volume::windows::Monitor,
}
#[cfg(windows)]
impl Backend for WindowsBackend {
    fn enumerate(&mut self) -> Result<Vec<SimpleAudioEndpoint>> {
        crate::simple_audio::enumerate()
    }
    fn read(&mut self, id: &str) -> Result<AudioVolumeReadResult> {
        crate::simple_audio_volume::read(id)
    }
    fn write(&mut self, id: &str, muted: bool, volume: u8) -> Result<AudioVolumeWriteResult> {
        crate::simple_audio_volume::write(id, muted, volume)
    }
    fn hardware(&mut self, muted: Option<bool>) -> HardwareStep {
        let operation = muted.map_or(Operation::ReadMicrophoneMute, |muted| {
            Operation::SetMicrophoneMute { muted }
        });
        let completion =
            crate::earbuds_auto_pause::device(self.route.clone(), operation, self.active.clone());
        let report = completion.result.and_then(|report| {
            if report.product_id != self.route.product_id() || report.operation != operation {
                return Err("Microphone HID reply belongs to another request".into());
            }
            report
                .microphone_mute
                .ok_or_else(|| "Microphone HID reply omitted mute state".into())
        });
        HardwareStep {
            report,
            cleanup_error: completion.cleanup_failure,
        }
    }
    fn changed(&mut self) -> Result<bool> {
        self.monitor.capture_changed()
    }
    fn close(&mut self) -> Result<()> {
        self.monitor.finish()
    }
}

/// The shell retains the shared earbud notification lease around this entire
/// blocking worker, including COM callback draining and every HID cleanup.
pub fn run(
    route: ObservedDevice,
    scope: Scope,
    active: Arc<AtomicBool>,
    commands: Receiver<Command>,
    events: async_channel::Sender<Update>,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        let monitor = crate::earbuds_system_volume::windows::Monitor::open_capture()
            .map_err(|error| format!("{error:#}"))?;
        let backend = WindowsBackend {
            route,
            active: active.clone(),
            monitor,
        };
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
        let _ = (route, scope, active, commands, events);
        Err("Earbud microphone system volume requires Windows Core Audio".into())
    }
}

fn run_session<B: Backend>(
    mut session: Session<B>,
    active: Arc<AtomicBool>,
    commands: Receiver<Command>,
    events: async_channel::Sender<Update>,
) -> Result<(), String> {
    let mut errors = Vec::new();
    if active.load(Ordering::Acquire) {
        let _ = events.send_blocking(session.observe());
    }
    while active.load(Ordering::Acquire) && !events.is_closed() {
        match commands.recv_timeout(Duration::from_millis(100)) {
            Ok(Command::Edit {
                scope_id,
                request_id,
                setting,
            }) => {
                let update = if scope_id == session.scope.id() {
                    session.edit(setting, &active)
                } else {
                    Update::new(Err("Microphone command belongs to another session".into()))
                };
                let _ = events.send_blocking(update.with_request_id(request_id));
            }
            Ok(Command::HardwareMute { scope_id, muted }) if scope_id == session.scope.id() => {
                let update = session.hardware_notice(muted);
                let setting = update
                    .observation()
                    .as_ref()
                    .ok()
                    .and_then(Snapshot::setting);
                let _ = events.send_blocking(update);
                // The current notification consumer republishes the hardware
                // state, then queues the complete unsaved MIC setting.
                if let Some(setting) = setting {
                    let completion = session.edit(setting, &active);
                    if completion.error().is_some() {
                        let _ = events.send_blocking(completion);
                    }
                }
            }
            Ok(Command::HardwareMute { .. }) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if !active.load(Ordering::Acquire) {
            break;
        }
        match session.backend.changed() {
            Ok(true) => {
                let _ = events.send_blocking(session.observe());
            }
            Ok(false) => {}
            Err(error) => {
                errors.push(format!("{error:#}"));
                break;
            }
        }
    }
    loop {
        match session.backend.close() {
            Ok(()) => break,
            Err(error) => {
                let error = format!("{error:#}");
                if !errors.contains(&error) {
                    errors.push(error);
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

#[cfg(test)]
#[path = "earbuds_microphone_tests.rs"]
mod tests;
