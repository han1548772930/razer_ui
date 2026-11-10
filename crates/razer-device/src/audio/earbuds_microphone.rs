//! Earbud microphone mute is a hardware observation, separate from OS mute.
use crate::earbuds_protocol40::{
    CommandHeader, Protocol40Session, Sleeper, ToggleObservation, WriteOutcome, decode_toggle,
};
pub use crate::earbuds_system_volume::Scope;
use crate::{
    simple_audio::{EndpointType, SimpleAudioEndpoint},
    simple_audio_volume::AudioVolumeWriteResult,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Setting {
    enabled: bool,
    volume: u8,
}
impl Setting {
    pub fn new(enabled: bool, volume: u8) -> Result<Self> {
        ensure!(volume <= 100, "Earbud microphone volume is outside 0..100");
        Ok(Self { enabled, volume })
    }
    pub fn enabled(self) -> bool {
        self.enabled
    }
    pub fn volume(self) -> u8 {
        self.volume
    }
}

pub fn select(scope: &Scope, endpoints: Vec<SimpleAudioEndpoint>) -> Result<SimpleAudioEndpoint> {
    endpoints
        .into_iter()
        .find(|endpoint| {
            endpoint.endpoint_type == EndpointType::Microphone
                && endpoint.container_id.contains(scope.container_id())
                && !endpoint.id.is_empty()
        })
        .ok_or_else(|| {
            anyhow::anyhow!("No microphone endpoint matches the observed earbud container")
        })
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    system: Result<crate::earbuds_system_volume::Snapshot, String>,
    hardware_mute: ToggleObservation,
}
impl Snapshot {
    pub fn new(
        system: Result<crate::earbuds_system_volume::Snapshot, String>,
        hardware_mute: ToggleObservation,
    ) -> Self {
        Self {
            system,
            hardware_mute,
        }
    }
    pub fn system(&self) -> &Result<crate::earbuds_system_volume::Snapshot, String> {
        &self.system
    }
    pub fn hardware_mute(&self) -> &ToggleObservation {
        &self.hardware_mute
    }
    pub fn setting(&self) -> Option<Setting> {
        match (&self.system, &self.hardware_mute) {
            (Ok(system), ToggleObservation::Observed(muted)) => {
                Setting::new(!muted, system.setting().volume()).ok()
            }
            _ => None,
        }
    }
    pub fn diagnostic(&self) -> Option<String> {
        let mut errors = Vec::new();
        if let Err(error) = &self.system {
            errors.push(error.clone());
        }
        if let Some(error) = self.hardware_mute.diagnostic() {
            errors.push(error);
        }
        (!errors.is_empty()).then(|| errors.join("; "))
    }
}

#[derive(Clone, Debug)]
pub struct Update {
    request_id: Option<Uuid>,
    observation: Result<Snapshot, String>,
    hardware_write: Option<WriteOutcome>,
    system_write: Option<AudioVolumeWriteResult>,
    error: Option<String>,
}
impl Update {
    pub fn new(observation: Result<Snapshot, String>) -> Self {
        Self {
            request_id: None,
            observation,
            hardware_write: None,
            system_write: None,
            error: None,
        }
    }
    pub fn with_request_id(mut self, id: Uuid) -> Self {
        self.request_id = Some(id);
        self
    }
    pub fn with_writes(
        mut self,
        hardware: Option<WriteOutcome>,
        system: Option<AudioVolumeWriteResult>,
    ) -> Self {
        self.hardware_write = hardware;
        self.system_write = system;
        self
    }
    pub fn with_error(mut self, error: Option<String>) -> Self {
        self.error = error;
        self
    }
    pub fn request_id(&self) -> Option<Uuid> {
        self.request_id
    }
    pub fn observation(&self) -> &Result<Snapshot, String> {
        &self.observation
    }
    pub fn hardware_write(&self) -> Option<&WriteOutcome> {
        self.hardware_write.as_ref()
    }
    pub fn system_write(&self) -> Option<&AudioVolumeWriteResult> {
        self.system_write.as_ref()
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct MuteReport {
    observation: ToggleObservation,
    write: WriteOutcome,
}
impl MuteReport {
    pub fn observation(&self) -> &ToggleObservation {
        &self.observation
    }
    pub fn write(&self) -> &WriteOutcome {
        &self.write
    }
}

pub fn execute<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    muted: Option<bool>,
    validate: &dyn Fn() -> Result<()>,
) -> Result<MuteReport> {
    let read = (|| -> Result<bool> {
        let reply = session.send_command(CommandHeader::new([2, 24, 132]), &[1, 2], validate)?;
        ensure!(
            reply.data.len() >= 3,
            "Earbud microphone mute reply is truncated"
        );
        ensure!(
            reply.data[..2] == [1, 2],
            "Earbud microphone mute reply has another profile or channel"
        );
        decode_toggle(reply.data[2])
    })();
    let mut report = match read {
        Ok(value) => MuteReport {
            observation: ToggleObservation::Observed(value),
            write: WriteOutcome::Skipped,
        },
        Err(error) => {
            validate()?;
            return Ok(MuteReport {
                observation: ToggleObservation::from_read_error(&error),
                write: if muted.is_some() {
                    WriteOutcome::Failed(format!("{error:#}"))
                } else {
                    WriteOutcome::Skipped
                },
            });
        }
    };
    if let Some(muted) = muted {
        if report.observation != ToggleObservation::Observed(muted) {
            match session.send_command(
                CommandHeader::new([3, 24, 4]),
                &[1, 2, u8::from(muted)],
                validate,
            ) {
                Ok(_) => {
                    report.observation = ToggleObservation::Observed(muted);
                    report.write = WriteOutcome::Confirmed;
                }
                Err(error) => {
                    validate()?;
                    report.write = WriteOutcome::Failed(format!("{error:#}"));
                }
            }
        }
    }
    Ok(report)
}
