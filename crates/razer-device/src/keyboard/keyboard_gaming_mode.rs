//! Current source-gated wired game-mode indicator LED, not key suppression.
//! Source task acknowledges SET; optional independent GET is a Rust policy.
use crate::{
    backend::FeatureTransport,
    keyboard_settings::{self, KeyboardSettingsCapability},
};
use anyhow::{Context as _, ensure};
use serde::Serialize;
use std::fmt;

include!("keyboard_gaming_mode_data_generated.rs");
const GET_LED: [u8; 3] = [3, 3, 128];
const SET_LED: [u8; 3] = [3, 3, 0];
const LED_CLASS: u8 = 0;
const GAME_MODE_LED: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameModePolicy {
    Disabled,
    Enabled,
    DuringGame,
}

/// Source policy uses a running-games observation for mode 2. Its missing or
/// rejected storage read resolves to off. This does not query games or storage.
pub fn effective_indicator(policy: GameModePolicy, observed_game_running: Option<bool>) -> bool {
    match policy {
        GameModePolicy::Disabled => false,
        GameModePolicy::Enabled => true,
        GameModePolicy::DuringGame => observed_game_running.unwrap_or(false),
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct GameModeIndicator {
    pub led_class_id: u8,
    pub led_id: u8,
    /// Actual getter byte; unknown values are retained without guessing a state.
    pub raw_state: u8,
}

impl GameModeIndicator {
    pub fn enabled(&self) -> Option<bool> {
        match self.raw_state {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct IndicatorWriteReport {
    pub requested_enabled: bool,
    pub attempted: bool,
    /// Correlated setter data, possibly empty; never used as getter evidence.
    pub acknowledgment_data: Option<Vec<u8>>,
    pub observed: Option<GameModeIndicator>,
}

impl IndicatorWriteReport {
    pub fn verified(&self) -> bool {
        self.acknowledgment_data.is_some()
            && self.observed.as_ref().and_then(GameModeIndicator::enabled)
                == Some(self.requested_enabled)
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum IndicatorFailurePhase {
    Prepare,
    Write,
    Readback,
}

#[derive(Debug)]
pub struct IndicatorWriteFailure {
    pub phase: IndicatorFailurePhase,
    pub report: IndicatorWriteReport,
    pub error: anyhow::Error,
}

impl fmt::Display for IndicatorWriteFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Gaming LED {:?}: {:#}", self.phase, self.error)
    }
}
impl std::error::Error for IndicatorWriteFailure {}

fn capability(product_id: u32) -> anyhow::Result<&'static KeyboardSettingsCapability> {
    ensure!(
        product_id == 515 || AUDITED_INDICATOR_PRODUCTS.contains(&product_id),
        "Current product registration has no audited wired gaming indicator"
    );
    keyboard_settings::capability(product_id)
        .context("Gaming indicator product has no audited wired transport")
}

fn parse(data: &[u8]) -> anyhow::Result<GameModeIndicator> {
    ensure!(data.len() >= 3, "Gaming indicator getter is truncated");
    ensure!(
        data[..2] == [LED_CLASS, GAME_MODE_LED],
        "Gaming indicator getter LED class or ID differs"
    );
    Ok(GameModeIndicator {
        led_class_id: data[0],
        led_id: data[1],
        raw_state: data[2],
    })
}

fn read_with_cap(
    device: &dyn FeatureTransport,
    cap: &KeyboardSettingsCapability,
    sequence: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<GameModeIndicator> {
    capability(cap.product_id)?;
    validate()?;
    let data = keyboard_settings::exchange(
        device,
        cap,
        GET_LED,
        &[LED_CLASS, GAME_MODE_LED],
        sequence,
        validate,
    )?;
    let observed = parse(&data)?;
    validate()?;
    Ok(observed)
}

/// Read actual LED class 0 / GameModeLED 8, with wire transaction 0..30.
pub fn read(
    device: &dyn FeatureTransport,
    product_id: u32,
    sequence: u8,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<GameModeIndicator> {
    read_with_cap(device, capability(product_id)?, sequence, &validate)
}

fn apply_with_cap(
    device: &dyn FeatureTransport,
    cap: &KeyboardSettingsCapability,
    enabled: bool,
    confirm_with_readback: bool,
    next_sequence: &mut impl FnMut() -> anyhow::Result<u8>,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> Result<IndicatorWriteReport, IndicatorWriteFailure> {
    let mut report = IndicatorWriteReport {
        requested_enabled: enabled,
        ..IndicatorWriteReport::default()
    };
    let failure = |phase, report, error| IndicatorWriteFailure {
        phase,
        report,
        error,
    };
    let sequence = (|| {
        capability(cap.product_id)?;
        validate()?;
        let sequence = next_sequence()?;
        keyboard_settings::wire_transaction(sequence)?;
        Ok(sequence)
    })()
    .map_err(|error| failure(IndicatorFailurePhase::Prepare, report.clone(), error))?;
    report.attempted = true;
    let result = keyboard_settings::exchange(
        device,
        cap,
        SET_LED,
        &[LED_CLASS, GAME_MODE_LED, u8::from(enabled)],
        sequence,
        validate,
    );
    report.acknowledgment_data =
        crate::feature_exchange::acknowledged_reply(&result).map(<[u8]>::to_vec);
    result.map_err(|error| failure(IndicatorFailurePhase::Write, report.clone(), error))?;
    validate().map_err(|error| failure(IndicatorFailurePhase::Write, report.clone(), error))?;
    if confirm_with_readback {
        let observed = next_sequence()
            .and_then(|sequence| read_with_cap(device, cap, sequence, validate))
            .map_err(|error| failure(IndicatorFailurePhase::Readback, report.clone(), error))?;
        let matches = observed.enabled() == Some(enabled);
        report.observed = Some(observed);
        if !matches {
            return Err(failure(
                IndicatorFailurePhase::Readback,
                report,
                anyhow::anyhow!("Independent gaming LED GET differs; SET was not rolled back"),
            ));
        }
    }
    Ok(report)
}

/// Preserve original SET acknowledgement semantics. An optional independent
/// GET is required for verified(); it does not prove key filtering or storage.
pub fn apply(
    device: &dyn FeatureTransport,
    product_id: u32,
    enabled: bool,
    confirm_with_readback: bool,
    mut next_sequence: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> Result<IndicatorWriteReport, IndicatorWriteFailure> {
    let cap = capability(product_id).map_err(|error| IndicatorWriteFailure {
        phase: IndicatorFailurePhase::Prepare,
        report: IndicatorWriteReport {
            requested_enabled: enabled,
            ..IndicatorWriteReport::default()
        },
        error,
    })?;
    apply_with_cap(
        device,
        cap,
        enabled,
        confirm_with_readback,
        &mut next_sequence,
        &validate,
    )
}

#[cfg(test)]
#[path = "keyboard_gaming_mode_tests.rs"]
mod tests;
