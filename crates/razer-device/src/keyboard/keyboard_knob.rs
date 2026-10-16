//! Razer Pro Type Ergo (PID 708) command-dial/knob hardware functions.
//!
//! The current source registers `useNewKnobModeSetting` only for PID 708 and
//! advertises OBM knob ID 39. A profile contains eight six-byte mode records;
//! the current mode is a separate eight-byte record. The original task writes
//! the configuration and current mode per OBM slot and updates local task
//! storage. This module reports each hardware phase separately and never calls
//! a local cache a device acknowledgement.
use crate::{
    backend::FeatureTransport,
    feature_exchange, keyboard_function_swap,
    keyboard_settings::{self, KeyboardSettingsCapability},
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const AUDITED_PRODUCT: u32 = 708;
pub const KNOB_ID: u8 = 39;
pub const CONFIGURATION_RECORDS: usize = 8;
const GET_CONFIG: [u8; 3] = [50, 3, 146];
const SET_CONFIG: [u8; 3] = [50, 3, 18];
const GET_CURRENT: [u8; 3] = [8, 3, 147];
const SET_CURRENT: [u8; 3] = [8, 3, 19];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnobModeConfig {
    pub disabled: u8,
    pub stage: u8,
    pub mode_id: u8,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl KnobModeConfig {
    fn encode(self, out: &mut Vec<u8>) {
        out.extend([
            self.disabled,
            self.stage,
            self.mode_id,
            self.red,
            self.green,
            self.blue,
        ]);
    }
}

/// Caller-owned UI mode values. Builtin modes reach hardware; custom modes
/// stay in the mapping engine and use the source fallback for hardware mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnobUiMode {
    pub uid: String,
    pub is_custom: bool,
    pub enabled: bool,
    pub mode_id: u8,
    pub rgb: [u8; 3],
}

const ZERO_MODE: KnobModeConfig = KnobModeConfig {
    disabled: 0,
    stage: 0,
    mode_id: 0,
    red: 0,
    green: 0,
    blue: 0,
};

pub const ALL_DISABLED_FALLBACK: KnobConfiguration = [
    KnobModeConfig {
        disabled: 0,
        stage: 0,
        mode_id: 0,
        red: 0,
        green: 0,
        blue: 255,
    },
    KnobModeConfig {
        disabled: 0,
        stage: 1,
        mode_id: 1,
        red: 0,
        green: 0,
        blue: 255,
    },
    KnobModeConfig {
        disabled: 0,
        stage: 2,
        mode_id: 2,
        red: 0,
        green: 0,
        blue: 255,
    },
    KnobModeConfig {
        disabled: 0,
        stage: 3,
        mode_id: 3,
        red: 0,
        green: 0,
        blue: 255,
    },
    KnobModeConfig {
        disabled: 1,
        stage: 4,
        mode_id: 4,
        red: 64,
        green: 224,
        blue: 208,
    },
    KnobModeConfig {
        disabled: 1,
        stage: 5,
        mode_id: 5,
        red: 255,
        green: 0,
        blue: 255,
    },
    KnobModeConfig {
        disabled: 1,
        stage: 6,
        mode_id: 6,
        red: 255,
        green: 255,
        blue: 255,
    },
    KnobModeConfig {
        disabled: 1,
        stage: 7,
        mode_id: 7,
        red: 255,
        green: 165,
        blue: 0,
    },
];

fn validate_modes(modes: &[KnobUiMode]) -> anyhow::Result<()> {
    let builtin: Vec<_> = modes.iter().filter(|mode| !mode.is_custom).collect();
    ensure!(
        builtin.len() <= CONFIGURATION_RECORDS,
        "Knob has at most eight builtin mode records"
    );
    let mut ids = std::collections::BTreeSet::new();
    let mut uids = std::collections::BTreeSet::new();
    for mode in modes {
        ensure!(
            !mode.uid.is_empty() && uids.insert(&mode.uid),
            "Knob mode UID is empty or duplicate"
        );
        if !mode.is_custom {
            ensure!(
                mode.mode_id < 8 && ids.insert(mode.mode_id),
                "Builtin knob mode ID must be unique in 0..7"
            );
        }
    }
    Ok(())
}

fn ui_record(mode: &KnobUiMode, stage: usize) -> KnobModeConfig {
    KnobModeConfig {
        disabled: u8::from(!mode.enabled),
        stage: stage as u8,
        mode_id: mode.mode_id,
        red: mode.rgb[0],
        green: mode.rgb[1],
        blue: mode.rgb[2],
    }
}

/// Source AW: remove custom modes, assign stage among all builtin modes,
/// invert enabled, and restore exact product defaults when none is enabled.
/// Short lists are zero-filled exactly like the source Uint8Array buffer.
pub fn compile_configuration(modes: &[KnobUiMode]) -> anyhow::Result<KnobConfiguration> {
    validate_modes(modes)?;
    let builtin: Vec<_> = modes.iter().filter(|mode| !mode.is_custom).collect();
    if !builtin.iter().any(|mode| mode.enabled) {
        return Ok(ALL_DISABLED_FALLBACK);
    }
    let mut records = [ZERO_MODE; CONFIGURATION_RECORDS];
    for (stage, mode) in builtin.into_iter().enumerate() {
        records[stage] = ui_record(mode, stage);
    }
    Ok(records)
}

/// Source XA/AV: a selected custom mode uses the first enabled builtin mode,
/// or the first mode with ID 0 if all builtins are disabled. Stage includes
/// disabled builtin predecessors. A custom fallback has source index -1,
/// encoded as 255 by the original Uint8Array; it is not silently changed.
pub fn compile_current(
    modes: &[KnobUiMode],
    selected_uid: &str,
) -> anyhow::Result<CurrentKnobMode> {
    validate_modes(modes)?;
    let selected = modes
        .iter()
        .find(|mode| mode.uid == selected_uid)
        .context("Selected knob mode UID is absent")?;
    let builtin: Vec<_> = modes.iter().filter(|mode| !mode.is_custom).collect();
    let chosen = if selected.is_custom {
        builtin
            .iter()
            .copied()
            .find(|mode| mode.enabled)
            .or_else(|| modes.iter().find(|mode| mode.mode_id == 0))
            .context("Custom knob mode has no source mode-0 fallback")?
    } else {
        selected
    };
    let stage = builtin
        .iter()
        .position(|mode| mode.uid == chosen.uid)
        .unwrap_or(255);
    Ok(ui_record(chosen, stage).into())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentKnobMode {
    pub disabled: u8,
    pub stage: u8,
    pub mode_id: u8,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl From<KnobModeConfig> for CurrentKnobMode {
    fn from(mode: KnobModeConfig) -> Self {
        Self {
            disabled: mode.disabled,
            stage: mode.stage,
            mode_id: mode.mode_id,
            red: mode.red,
            green: mode.green,
            blue: mode.blue,
        }
    }
}

pub type KnobConfiguration = [KnobModeConfig; CONFIGURATION_RECORDS];

#[derive(Clone, Debug, Serialize)]
pub struct KnobReadReport {
    pub profile_id: u8,
    pub knob_id: u8,
    pub configuration: KnobConfiguration,
    pub current_mode: CurrentKnobMode,
}

#[derive(Clone, Debug, Serialize)]
pub struct KnobPhaseReport {
    pub attempted: bool,
    pub send_attempts: usize,
    pub acknowledgment_data: Option<Vec<u8>>,
}

impl Default for KnobPhaseReport {
    fn default() -> Self {
        Self {
            attempted: false,
            send_attempts: 0,
            acknowledgment_data: None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct KnobWriteReport {
    pub profile_id: u8,
    pub knob_id: u8,
    pub requested_configuration: Option<KnobConfiguration>,
    pub requested_current_mode: Option<CurrentKnobMode>,
    pub configuration: KnobPhaseReport,
    pub current_mode: KnobPhaseReport,
    pub observed_configuration: Option<KnobConfiguration>,
    pub observed_current_mode: Option<CurrentKnobMode>,
    verified: bool,
}

impl KnobWriteReport {
    pub fn verified(&self) -> bool {
        self.verified
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum KnobFailurePhase {
    Prepare,
    ConfigurationWrite,
    CurrentModeWrite,
    ConfigurationReadback,
    CurrentModeReadback,
}

#[derive(Debug)]
pub struct KnobWriteFailure {
    pub phase: KnobFailurePhase,
    pub report: KnobWriteReport,
    pub error: anyhow::Error,
}

impl fmt::Display for KnobWriteFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Knob {:?}: {:#}", self.phase, self.error)
    }
}
impl std::error::Error for KnobWriteFailure {}

fn capability(product_id: u32) -> anyhow::Result<&'static KeyboardSettingsCapability> {
    ensure!(
        product_id == AUDITED_PRODUCT,
        "Knob mode setting is source-gated to Pro Type Ergo PID 708"
    );
    keyboard_settings::capability(product_id).context("Missing audited PID 708 wired transport")
}

fn validate_ids(profile_id: u8, knob_id: u8) -> anyhow::Result<()> {
    ensure!(
        profile_id == 1,
        "PID 708 single-profile knob tasks address OBM slot 1"
    );
    ensure!(knob_id == KNOB_ID, "PID 708 current source uses knob ID 39");
    Ok(())
}

fn parse_configuration(
    data: &[u8],
    profile_id: u8,
    knob_id: u8,
) -> anyhow::Result<KnobConfiguration> {
    ensure!(
        data.len() >= 2 + CONFIGURATION_RECORDS * 6,
        "Knob configuration response is truncated"
    );
    ensure!(
        data[0] == profile_id && data[1] == knob_id,
        "Knob configuration identity differs"
    );
    Ok(std::array::from_fn(|index| {
        let offset = 2 + index * 6;
        KnobModeConfig {
            disabled: data[offset],
            stage: data[offset + 1],
            mode_id: data[offset + 2],
            red: data[offset + 3],
            green: data[offset + 4],
            blue: data[offset + 5],
        }
    }))
}

fn parse_current(data: &[u8], profile_id: u8, knob_id: u8) -> anyhow::Result<CurrentKnobMode> {
    ensure!(data.len() >= 8, "Current knob mode response is truncated");
    ensure!(
        data[0] == profile_id && data[1] == knob_id,
        "Current knob identity differs"
    );
    Ok(CurrentKnobMode {
        disabled: data[2],
        stage: data[3],
        mode_id: data[4],
        red: data[5],
        green: data[6],
        blue: data[7],
    })
}

fn read_configuration_with_cap(
    device: &dyn FeatureTransport,
    cap: &KeyboardSettingsCapability,
    profile_id: u8,
    knob_id: u8,
    sequence: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<KnobConfiguration> {
    capability(cap.product_id)?;
    validate_ids(profile_id, knob_id)?;
    keyboard_function_swap::validate_wired_capability(cap)?;
    let data = feature_exchange::exchange(
        device,
        &cap.transport,
        GET_CONFIG,
        &[profile_id, knob_id],
        keyboard_settings::wire_transaction(sequence)?,
        validate,
    )?;
    validate()?;
    parse_configuration(&data, profile_id, knob_id)
}

fn read_current_with_cap(
    device: &dyn FeatureTransport,
    cap: &KeyboardSettingsCapability,
    profile_id: u8,
    knob_id: u8,
    sequence: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<CurrentKnobMode> {
    capability(cap.product_id)?;
    validate_ids(profile_id, knob_id)?;
    keyboard_function_swap::validate_wired_capability(cap)?;
    let data = feature_exchange::exchange(
        device,
        &cap.transport,
        GET_CURRENT,
        &[profile_id, knob_id],
        keyboard_settings::wire_transaction(sequence)?,
        validate,
    )?;
    validate()?;
    parse_current(&data, profile_id, knob_id)
}

pub fn read_configuration(
    device: &dyn FeatureTransport,
    product_id: u32,
    profile_id: u8,
    knob_id: u8,
    sequence: u8,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<KnobConfiguration> {
    read_configuration_with_cap(
        device,
        capability(product_id)?,
        profile_id,
        knob_id,
        sequence,
        &validate,
    )
}

pub fn read_current(
    device: &dyn FeatureTransport,
    product_id: u32,
    profile_id: u8,
    knob_id: u8,
    sequence: u8,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<CurrentKnobMode> {
    read_current_with_cap(
        device,
        capability(product_id)?,
        profile_id,
        knob_id,
        sequence,
        &validate,
    )
}

pub fn read(
    device: &dyn FeatureTransport,
    product_id: u32,
    profile_id: u8,
    knob_id: u8,
    mut next_sequence: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<KnobReadReport> {
    let cap = capability(product_id)?;
    let configuration = read_configuration_with_cap(
        device,
        cap,
        profile_id,
        knob_id,
        next_sequence()?,
        &validate,
    )?;
    let current_mode = read_current_with_cap(
        device,
        cap,
        profile_id,
        knob_id,
        next_sequence()?,
        &validate,
    )?;
    Ok(KnobReadReport {
        profile_id,
        knob_id,
        configuration,
        current_mode,
    })
}

fn apply_with_cap(
    device: &dyn FeatureTransport,
    cap: &KeyboardSettingsCapability,
    profile_id: u8,
    knob_id: u8,
    configuration: Option<&KnobConfiguration>,
    current_mode: Option<CurrentKnobMode>,
    confirm_with_readback: bool,
    next_sequence: &mut impl FnMut() -> anyhow::Result<u8>,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> Result<KnobWriteReport, KnobWriteFailure> {
    let mut report = KnobWriteReport {
        profile_id,
        knob_id,
        requested_configuration: configuration.copied(),
        requested_current_mode: current_mode,
        configuration: KnobPhaseReport::default(),
        current_mode: KnobPhaseReport::default(),
        observed_configuration: None,
        observed_current_mode: None,
        verified: false,
    };
    let fail = |phase, report, error| KnobWriteFailure {
        phase,
        report,
        error,
    };
    (|| {
        capability(cap.product_id)?;
        validate_ids(profile_id, knob_id)?;
        ensure!(
            configuration.is_some() || current_mode.is_some(),
            "No knob hardware change requested"
        );
        keyboard_function_swap::validate_wired_capability(cap)?;
        validate()?;
        Ok::<(), anyhow::Error>(())
    })()
    .map_err(|error| fail(KnobFailurePhase::Prepare, report.clone(), error))?;

    if let Some(configuration) = configuration {
        let mut payload = Vec::with_capacity(50);
        payload.extend([profile_id, knob_id]);
        for mode in configuration {
            mode.encode(&mut payload);
        }
        let data = feature_exchange::exchange_with_report_data_size_observing_send(
            device,
            &cap.transport,
            SET_CONFIG,
            &payload,
            keyboard_settings::wire_transaction(next_sequence().map_err(|error| {
                fail(KnobFailurePhase::ConfigurationWrite, report.clone(), error)
            })?)
            .map_err(|error| fail(KnobFailurePhase::ConfigurationWrite, report.clone(), error))?,
            SET_CONFIG[0],
            validate,
            &mut || {
                report.configuration.attempted = true;
                report.configuration.send_attempts += 1;
            },
        )
        .map_err(|error| fail(KnobFailurePhase::ConfigurationWrite, report.clone(), error))?;
        report.configuration.acknowledgment_data = Some(data);
    }

    if let Some(current_mode) = current_mode {
        let mode_payload = [
            profile_id,
            knob_id,
            current_mode.disabled,
            current_mode.stage,
            current_mode.mode_id,
            current_mode.red,
            current_mode.green,
            current_mode.blue,
        ];
        let data = feature_exchange::exchange_with_report_data_size_observing_send(
            device,
            &cap.transport,
            SET_CURRENT,
            &mode_payload,
            keyboard_settings::wire_transaction(next_sequence().map_err(|error| {
                fail(KnobFailurePhase::CurrentModeWrite, report.clone(), error)
            })?)
            .map_err(|error| fail(KnobFailurePhase::CurrentModeWrite, report.clone(), error))?,
            SET_CURRENT[0],
            validate,
            &mut || {
                report.current_mode.attempted = true;
                report.current_mode.send_attempts += 1;
            },
        )
        .map_err(|error| fail(KnobFailurePhase::CurrentModeWrite, report.clone(), error))?;
        report.current_mode.acknowledgment_data = Some(data);
    }
    let final_write_phase = if current_mode.is_some() {
        KnobFailurePhase::CurrentModeWrite
    } else {
        KnobFailurePhase::ConfigurationWrite
    };
    validate().map_err(|error| fail(final_write_phase, report.clone(), error))?;

    if confirm_with_readback {
        if let Some(configuration) = configuration {
            let observed_configuration = read_configuration_with_cap(
                device,
                cap,
                profile_id,
                knob_id,
                next_sequence().map_err(|error| {
                    fail(
                        KnobFailurePhase::ConfigurationReadback,
                        report.clone(),
                        error,
                    )
                })?,
                validate,
            )
            .map_err(|error| {
                fail(
                    KnobFailurePhase::ConfigurationReadback,
                    report.clone(),
                    error,
                )
            })?;
            report.observed_configuration = Some(observed_configuration);
            if observed_configuration != *configuration {
                return Err(fail(
                    KnobFailurePhase::ConfigurationReadback,
                    report,
                    anyhow::anyhow!(
                        "Independent knob configuration GET differs; writes were not rolled back"
                    ),
                ));
            }
        }
        if let Some(current_mode) = current_mode {
            let observed_current_mode = read_current_with_cap(
                device,
                cap,
                profile_id,
                knob_id,
                next_sequence().map_err(|error| {
                    fail(KnobFailurePhase::CurrentModeReadback, report.clone(), error)
                })?,
                validate,
            )
            .map_err(|error| fail(KnobFailurePhase::CurrentModeReadback, report.clone(), error))?;
            report.observed_current_mode = Some(observed_current_mode);
            if observed_current_mode != current_mode {
                return Err(fail(
                    KnobFailurePhase::CurrentModeReadback,
                    report,
                    anyhow::anyhow!(
                        "Independent current knob mode GET differs; writes were not rolled back"
                    ),
                ));
            }
        }
        report.verified = true;
    }
    Ok(report)
}

/// Write the requested PID 708 OBM fields. Configuration, when present, contains
/// all eight records and precedes current mode. Optional independent GETs prove
/// those hardware fields only; no original task cache or mapping is persisted.
pub fn apply(
    device: &dyn FeatureTransport,
    product_id: u32,
    profile_id: u8,
    knob_id: u8,
    configuration: Option<&KnobConfiguration>,
    current_mode: Option<CurrentKnobMode>,
    confirm_with_readback: bool,
    mut next_sequence: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> Result<KnobWriteReport, KnobWriteFailure> {
    let cap = match capability(product_id) {
        Ok(cap) => cap,
        Err(error) => {
            return Err(KnobWriteFailure {
                phase: KnobFailurePhase::Prepare,
                report: KnobWriteReport {
                    profile_id,
                    knob_id,
                    requested_configuration: configuration.copied(),
                    requested_current_mode: current_mode,
                    configuration: KnobPhaseReport::default(),
                    current_mode: KnobPhaseReport::default(),
                    observed_configuration: None,
                    observed_current_mode: None,
                    verified: false,
                },
                error,
            });
        }
    };
    apply_with_cap(
        device,
        cap,
        profile_id,
        knob_id,
        configuration,
        current_mode,
        confirm_with_readback,
        &mut next_sequence,
        &validate,
    )
}

#[cfg(test)]
#[path = "keyboard_knob_tests.rs"]
mod tests;
