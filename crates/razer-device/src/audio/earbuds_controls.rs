//! Earbuds auto-pause and PID2657 ANC control state. Device observation,
//! local persisted profile value and the page draft are separate fields.
//! Every device task carries the connection generation; results from an
//! older generation are rejected instead of overwriting the new connection.
use crate::earbuds_protocol40::{
    self as proto, AncChange, AncMode, AncRuntimeState, AncSetting, AncWrite, AutoPauseObservation,
    AutoPauseProfileField, Protocol40Session, Sleeper, WriteOutcome,
};
use anyhow::{Result, bail};

pub use proto::AncLevel;

/// Page draft (Redux reducer equivalent). Initial values are the source
/// reducer defaults, not device values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EarbudsDraft {
    pub auto_pause: bool,
    pub anc: Option<AncSetting>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EarbudsControls {
    product_id: u32,
    generation: u64,
    connected: bool,
    pub auto_pause_device: AutoPauseObservation,
    pub auto_pause_persisted: Option<bool>,
    pub anc_runtime: Option<AncRuntimeState>,
    pub anc_persisted: Option<AncSetting>,
    pub draft: EarbudsDraft,
    /// `batteryReducer.hasDisconnectedEarbud`; disables every widget.
    pub earbud_disconnected: bool,
    pub firmware_version: Option<String>,
    pub last_auto_pause_write: Option<WriteOutcome>,
    pub last_anc_write: Option<AncWrite>,
}

/// A queued `NORMAL_SKIPPABLE` device task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EarbudsTask {
    SetAutoPause {
        generation: u64,
        enabled: bool,
    },
    SetAnc {
        generation: u64,
        setting: AncSetting,
    },
}

impl EarbudsTask {
    pub fn generation(self) -> u64 {
        match self {
            Self::SetAutoPause { generation, .. } | Self::SetAnc { generation, .. } => generation,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EarbudsTaskResult {
    AutoPause(WriteOutcome),
    Anc {
        write: AncWrite,
        runtime: AncRuntimeState,
    },
}

/// Connect/reconnect restore result, in source feature registration order
/// (`activeNoiseCancellation` before `audioAutoPauseControlFeature`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EarbudsRestore {
    pub generation: u64,
    pub anc: Option<(AncWrite, AncRuntimeState, AncSetting)>,
    pub auto_pause: Option<proto::AutoPauseRestore>,
}

/// Local profile view handed to restore; `None` fields use source fallbacks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EarbudsProfileView {
    pub auto_pause: AutoPauseProfileField,
    pub anc: Option<AncSetting>,
}

impl EarbudsControls {
    pub fn new(product_id: u32) -> Result<Self> {
        if !proto::supports_auto_pause(product_id) {
            bail!("PID {product_id} does not register earbuds auto-pause");
        }
        let anc = proto::supports_anc(product_id);
        Ok(Self {
            product_id,
            generation: 0,
            connected: false,
            auto_pause_device: AutoPauseObservation::Unobserved,
            auto_pause_persisted: None,
            anc_runtime: anc.then(AncRuntimeState::default),
            anc_persisted: None,
            draft: EarbudsDraft {
                auto_pause: false,
                anc: anc.then_some(proto::ANC_UI_INITIAL),
            },
            earbud_disconnected: false,
            firmware_version: None,
            last_auto_pause_write: None,
            last_anc_write: None,
        })
    }

    pub fn product_id(&self) -> u32 {
        self.product_id
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn is_current(&self, generation: u64) -> bool {
        self.connected && generation == self.generation
    }

    /// `DeviceConnected` (initial or reconnect): a new generation; prior
    /// device observations are no longer current.
    pub fn connect(&mut self) -> u64 {
        self.generation += 1;
        self.connected = true;
        self.auto_pause_device = AutoPauseObservation::Unobserved;
        self.generation
    }

    pub fn disconnect(&mut self) {
        self.generation += 1;
        self.connected = false;
        self.auto_pause_device = AutoPauseObservation::Unobserved;
    }

    /// Local profile loaded for the page (`ui_profile_consumer`).
    pub fn load_profile(&mut self, profile: EarbudsProfileView) {
        self.auto_pause_persisted = profile.auto_pause.is_enabled;
        if self.anc_runtime.is_some() {
            self.anc_persisted = profile.anc;
        }
        if let Some(enabled) = profile.auto_pause.is_enabled {
            self.draft.auto_pause = enabled;
        }
        if let (Some(_), Some(anc)) = (self.draft.anc, profile.anc) {
            self.draft.anc = Some(anc);
        }
    }

    pub fn apply_restore(&mut self, restore: EarbudsRestore) -> Result<()> {
        if !self.is_current(restore.generation) {
            bail!(
                "stale earbuds restore for generation {}",
                restore.generation
            );
        }
        if let Some((write, runtime, published)) = restore.anc {
            self.anc_runtime = Some(runtime);
            self.last_anc_write = Some(write);
            self.draft.anc = Some(published);
        }
        if let Some(auto_pause) = restore.auto_pause {
            self.auto_pause_device = auto_pause.observation;
            self.last_auto_pause_write = Some(auto_pause.write);
            // MW_UPDATE_AUDIO_AUTO_PAUSE_CONTROL_TO_UI replaces the field.
            self.draft.auto_pause = auto_pause.published_local;
        }
        Ok(())
    }

    /// `km` toggle: reducer updates first, then the MW event is queued. The
    /// toggle is inert while an earbud is disconnected (pointer-events none).
    pub fn toggle_auto_pause(&mut self) -> Option<EarbudsTask> {
        if self.earbud_disconnected || !self.connected {
            return None;
        }
        let enabled = !self.draft.auto_pause;
        self.draft.auto_pause = enabled;
        Some(EarbudsTask::SetAutoPause {
            generation: self.generation,
            enabled,
        })
    }

    /// `Nm` actions: switch, ANC/Ambient tab and Reduce checkbox. Tab and
    /// Reduce only act while enabled; Reduce additionally needs ANC mode.
    pub fn change_anc(&mut self, change: AncChange) -> Option<EarbudsTask> {
        let current = self.draft.anc?;
        if self.earbud_disconnected || !self.connected {
            return None;
        }
        if (change.mode.is_some() || change.level.is_some()) && !current.enabled {
            return None;
        }
        if change.level.is_some() && current.mode != AncMode::Anc {
            return None;
        }
        // The reducer merges into the old state and posts the full triple.
        let merged = AncSetting {
            enabled: change.enabled.unwrap_or(current.enabled),
            mode: change.mode.unwrap_or(current.mode),
            level: change.level.unwrap_or(current.level),
        };
        self.draft.anc = Some(merged);
        Some(EarbudsTask::SetAnc {
            generation: self.generation,
            setting: merged,
        })
    }

    /// Reduce row visibility (`isSupportReduceANC: !U5(min, fw) > 0`).
    pub fn reduce_supported(&self) -> bool {
        self.anc_runtime.is_some() && !proto::reduce_blocked(self.firmware_version.as_deref())
    }

    /// Hardware `[5,33,state]`: updates profile and UI, not runtimeState.
    pub fn anc_notification(&mut self, generation: u64, record: &[u8]) -> Option<AncSetting> {
        if !self.is_current(generation) {
            return None;
        }
        let base = self.anc_persisted.or(self.draft.anc)?;
        let updated = proto::apply_anc_notification(base, record)?;
        self.anc_persisted = Some(updated);
        self.draft.anc = Some(updated);
        Some(updated)
    }

    /// Accepts a task result only for the current generation. The draft is
    /// never rolled back: the source reducer has no completion branch.
    pub fn complete(&mut self, task: EarbudsTask, result: EarbudsTaskResult) -> Result<()> {
        if !self.is_current(task.generation()) {
            bail!(
                "stale earbuds task result for generation {}",
                task.generation()
            );
        }
        match result {
            EarbudsTaskResult::AutoPause(write) => self.last_auto_pause_write = Some(write),
            EarbudsTaskResult::Anc { write, runtime } => {
                self.anc_runtime = Some(runtime);
                self.last_anc_write = Some(write);
            }
        }
        Ok(())
    }

    /// Called only after the selected profile's atomic save has completed.
    /// An optimistic control change is not a persistence acknowledgement.
    pub fn acknowledge_persistence(&mut self, task: EarbudsTask) -> Result<()> {
        if !self.is_current(task.generation()) {
            bail!("stale earbuds profile persistence acknowledgement");
        }
        match task {
            EarbudsTask::SetAutoPause { enabled, .. } => {
                self.auto_pause_persisted = Some(enabled);
            }
            EarbudsTask::SetAnc { setting, .. } => self.anc_persisted = Some(setting),
        }
        Ok(())
    }

    /// Snapshot of the runtime state a worker needs for one task.
    pub fn anc_runtime_for_task(&self) -> AncRuntimeState {
        self.anc_runtime.unwrap_or_default()
    }
}

/// Worker side of connect/reconnect. `validate` must fail once the owner's
/// generation changes; a stale chain returns an error and publishes nothing.
pub fn restore<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    product_id: u32,
    generation: u64,
    mut anc_runtime: AncRuntimeState,
    profile: EarbudsProfileView,
    validate: &dyn Fn() -> Result<()>,
) -> Result<EarbudsRestore> {
    let anc = if proto::supports_anc(product_id) {
        let local = profile.anc.unwrap_or(proto::ANC_PROFILE_DEFAULT);
        let write = proto::set_anc(session, &mut anc_runtime, local, validate)?;
        Some((write, anc_runtime, local))
    } else {
        None
    };
    let auto_pause = proto::supports_auto_pause(product_id)
        .then(|| proto::restore_auto_pause(session, profile.auto_pause, validate))
        .transpose()?;
    validate()?;
    Ok(EarbudsRestore {
        generation,
        anc,
        auto_pause,
    })
}

pub fn run_task<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    task: EarbudsTask,
    mut anc_runtime: AncRuntimeState,
    validate: &dyn Fn() -> Result<()>,
) -> Result<EarbudsTaskResult> {
    match task {
        EarbudsTask::SetAutoPause { enabled, .. } => {
            let outcome = match session.set_auto_pause(enabled, validate) {
                Ok(()) => WriteOutcome::Confirmed,
                Err(error) => {
                    validate()?;
                    WriteOutcome::Failed(format!("{error:#}"))
                }
            };
            Ok(EarbudsTaskResult::AutoPause(outcome))
        }
        EarbudsTask::SetAnc { setting, .. } => {
            let write = proto::set_anc(session, &mut anc_runtime, setting, validate)?;
            Ok(EarbudsTaskResult::Anc {
                write,
                runtime: anc_runtime,
            })
        }
    }
}

#[cfg(test)]
#[path = "earbuds_controls_tests.rs"]
mod tests;
