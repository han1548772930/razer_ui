//! Razer Hanbo AIO (3893) thermal profile and `rzDeviceElsa` command encoding.
//!
//! Current sources: product 3893 MW `main` (DEFAULTPROFILE ports and the
//! `hanboConfigure.hanboModeSettings` feature), `6259` task runner `ko`
//! (`Fans` selects the fan controller, every other thermal port the pump), and
//! `rzDeviceElsa` (`_sendCommand`, command headers and payload layouts).
//! The UI page only edits this typed profile; transport success is never a
//! device acknowledgement, which requires a matching reply with status 1.
use crate::backend::FeatureTransport;
use anyhow::{Context as _, Result, bail, ensure};
use serde::{Deserialize, Serialize};

pub const PRODUCT_ID: u32 = 3893;
/// `rzDeviceElsa.reportLength`; `hid.write` sends claimInterface 0 as the ID.
pub const REPORT_LENGTH: usize = 65;
pub const REPORT_ID: u8 = 0;
/// `_getHIDInputReportResult` accepts only exactly 64 returned bytes.
pub const INPUT_LENGTH: usize = 64;
/// Source ports: `Fans` id 1 owns seven curve points, `Pump Cap` id 2 five.
pub const FAN_PORT_ID: u32 = 1;
pub const PUMP_PORT_ID: u32 = 2;
pub const FAN_PORT_NAME: &str = "Fans";
pub const PUMP_PORT_NAME: &str = "Pump Cap";
pub const FAN_CURVE_POINTS: usize = 7;
pub const PUMP_CURVE_POINTS: usize = 5;
/// UI `Vg`/`Wg` and `zg`/`kg` chart axes.
pub const FAN_CURVE_TEMPERATURES: [u32; FAN_CURVE_POINTS] = [20, 30, 40, 50, 60, 70, 80];
pub const PUMP_CURVE_TEMPERATURES: [u32; PUMP_CURVE_POINTS] = [20, 30, 40, 50, 60];
pub const CURVE_SPEEDS: [u32; 5] = [0, 25, 50, 75, 100];

/// Rust policy for a bounded exchange; the source retry and sleep constants
/// come from a shared base module that is not part of the retained capture.
pub const OUTPUT_ATTEMPTS: usize = 3;
pub const INPUT_ATTEMPTS: usize = 5;
pub const INPUT_TIMEOUT_MS: u32 = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThermalMode {
    Quiet,
    Normal,
    Performance,
    Advanced,
}

impl ThermalMode {
    /// UI `bg`/`wg` order.
    pub const ALL: [Self; 4] = [Self::Quiet, Self::Normal, Self::Performance, Self::Advanced];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Quiet => "QUIET",
            Self::Normal => "NORMAL",
            Self::Performance => "PERFORMANCE",
            Self::Advanced => "ADVANCED",
        }
    }

    /// `hanboModeSettings`; Advanced writes a curve instead.
    pub const fn preset(self) -> Option<(u8, u8)> {
        match self {
            Self::Quiet => Some((1, 20)),
            Self::Normal => Some((2, 50)),
            Self::Performance => Some((3, 80)),
            Self::Advanced => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoolerChannel {
    Fans,
    Pump,
}

impl CoolerChannel {
    pub const fn curve_points(self) -> usize {
        match self {
            Self::Fans => FAN_CURVE_POINTS,
            Self::Pump => PUMP_CURVE_POINTS,
        }
    }

    pub const fn temperatures(self) -> &'static [u32] {
        match self {
            Self::Fans => &FAN_CURVE_TEMPERATURES,
            Self::Pump => &PUMP_CURVE_TEMPERATURES,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveThermalIndicator {
    pub mode_selected: ThermalMode,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortThermal {
    pub active_thermal_indicator: ActiveThermalIndicator,
    /// Fan speed percentages in UI chart order, one decimal place.
    pub advanced_setting: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThermalProfile {
    pub fans: PortThermal,
    pub pump: PortThermal,
}

impl Default for ThermalProfile {
    /// MW `DEFAULTPROFILE.ports` for editions 0 and 128.
    fn default() -> Self {
        Self {
            fans: PortThermal {
                active_thermal_indicator: ActiveThermalIndicator {
                    mode_selected: ThermalMode::Normal,
                },
                advanced_setting: vec![20., 30., 40., 50., 60., 80., 100.],
            },
            pump: PortThermal {
                active_thermal_indicator: ActiveThermalIndicator {
                    mode_selected: ThermalMode::Normal,
                },
                advanced_setting: vec![20., 40., 60., 80., 100.],
            },
        }
    }
}

impl ThermalProfile {
    pub fn port(&self, channel: CoolerChannel) -> &PortThermal {
        match channel {
            CoolerChannel::Fans => &self.fans,
            CoolerChannel::Pump => &self.pump,
        }
    }

    pub fn port_mut(&mut self, channel: CoolerChannel) -> &mut PortThermal {
        match channel {
            CoolerChannel::Fans => &mut self.fans,
            CoolerChannel::Pump => &mut self.pump,
        }
    }

    /// Admit a saved port only when its mode is known and its curve has the
    /// channel's point count and the 0-100 range the chart can display.
    pub fn restore_port(&mut self, channel: CoolerChannel, saved: PortThermal) -> bool {
        let valid = saved.advanced_setting.len() == channel.curve_points()
            && saved
                .advanced_setting
                .iter()
                .all(|value| value.is_finite() && (0. ..=100.).contains(value));
        if valid {
            *self.port_mut(channel) = saved;
        }
        valid
    }
}

/// Source `rzDeviceElsa` command, after the task runner selected its channel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CoolerCommand {
    /// `setFanControllerParameter(mode, speed)`, header [34,1] reply [35,2].
    FanParameter { mode: u8, speed: u8 },
    /// `setCPUGPUFanControllerSpeedsCurve(points + [100,100], 0)`, [200,1]/[201,2].
    FanCurve { points: [u8; 9] },
    /// `setCoolerPumpControllerParameter(mode, speed)`, [20,1]/[21,2].
    PumpParameter { mode: u8, speed: u8 },
    /// `setCoolerPumpControllerSpeedsCurve(1, points + [100;4])`, [24,1]/[25,2].
    PumpCurve { points: [u8; 9] },
}

/// `Uint8Array` conversion truncates toward zero; the chart keeps 0..=100.
fn curve_byte(value: f64) -> u8 {
    value.trunc().clamp(0., 255.) as u8
}

impl CoolerCommand {
    pub fn for_port(channel: CoolerChannel, thermal: &PortThermal) -> Result<Self> {
        let mode = thermal.active_thermal_indicator.mode_selected;
        if let Some((mode, speed)) = mode.preset() {
            return Ok(match channel {
                CoolerChannel::Fans => Self::FanParameter { mode, speed },
                CoolerChannel::Pump => Self::PumpParameter { mode, speed },
            });
        }
        // Source `new Uint8Array(...)` length check throws for any other size.
        ensure!(
            thermal.advanced_setting.len() == channel.curve_points(),
            "Hanbo {channel:?} curve has {} points",
            thermal.advanced_setting.len()
        );
        let mut points = [100u8; 9];
        for (slot, value) in points.iter_mut().zip(&thermal.advanced_setting) {
            *slot = curve_byte(*value);
        }
        Ok(match channel {
            CoolerChannel::Fans => Self::FanCurve { points },
            CoolerChannel::Pump => Self::PumpCurve { points },
        })
    }

    pub const fn header(&self) -> [u8; 2] {
        match self {
            Self::FanParameter { .. } => [34, 1],
            Self::FanCurve { .. } => [200, 1],
            Self::PumpParameter { .. } => [20, 1],
            Self::PumpCurve { .. } => [24, 1],
        }
    }

    pub const fn reply_header(&self) -> [u8; 2] {
        match self {
            Self::FanParameter { .. } => [35, 2],
            Self::FanCurve { .. } => [201, 2],
            Self::PumpParameter { .. } => [21, 2],
            Self::PumpCurve { .. } => [25, 2],
        }
    }

    fn payload(&self) -> Vec<u8> {
        match self {
            Self::FanParameter { mode, speed } | Self::PumpParameter { mode, speed } => {
                vec![*mode, *speed, 0]
            }
            // Fan curve: o[0]=0 (CPU/GPU selector default), o[1]=0.
            Self::FanCurve { points } => [[0u8, 0].as_slice(), points, &[0]].concat(),
            // Pump curve: o[0]=1 is the runner's first argument, o[1]=0.
            Self::PumpCurve { points } => [[1u8, 0].as_slice(), points, &[0]].concat(),
        }
    }

    /// Complete Output report: report ID, header, payload, zero padding.
    pub fn report(&self) -> [u8; REPORT_LENGTH] {
        let mut report = [0u8; REPORT_LENGTH];
        report[0] = REPORT_ID;
        report[1..3].copy_from_slice(&self.header());
        for (slot, byte) in report[3..].iter_mut().zip(self.payload()) {
            *slot = byte;
        }
        report
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoolerReply {
    /// Header matched and `data[2] === 1`.
    Accepted,
    /// Header matched; the source `Oe` check throws "command failed".
    Rejected { status: u8 },
}

/// Returns None for a packet the source would discard and read again.
pub fn decode_reply(command: &CoolerCommand, input: &[u8]) -> Option<CoolerReply> {
    if input.len() != INPUT_LENGTH || input[..2] != command.reply_header() {
        return None;
    }
    Some(match input[2] {
        1 => CoolerReply::Accepted,
        status => CoolerReply::Rejected { status },
    })
}

/// One write/read exchange over an opened Elsa collection. `validate` lets
/// the owner reject the exchange after a device/session replacement.
pub fn exchange(
    transport: &dyn FeatureTransport,
    command: &CoolerCommand,
    mut validate: impl FnMut() -> Result<()>,
) -> Result<()> {
    let report = command.report();
    for _ in 0..OUTPUT_ATTEMPTS {
        validate()?;
        transport
            .write_output(&report)
            .context("Hanbo Output Report 发送失败")?;
        for _ in 0..INPUT_ATTEMPTS {
            validate()?;
            let mut input = [0u8; INPUT_LENGTH + 1];
            let count = transport
                .read_interrupt(&mut input, INPUT_TIMEOUT_MS)
                .context("Hanbo Input Report 读取失败")?;
            // Unnumbered input may arrive with or without the zero ID prefix.
            let packet = match count {
                INPUT_LENGTH => &input[..INPUT_LENGTH],
                n if n == INPUT_LENGTH + 1 && input[0] == REPORT_ID => &input[1..=INPUT_LENGTH],
                _ => continue,
            };
            match decode_reply(command, packet) {
                Some(CoolerReply::Accepted) => return Ok(()),
                Some(CoolerReply::Rejected { status }) => {
                    bail!(
                        "Hanbo command {:?} rejected, status {status}",
                        command.header()
                    )
                }
                None => continue,
            }
        }
    }
    bail!(
        "Hanbo command {:?} received no matching reply",
        command.header()
    )
}

#[cfg(test)]
#[path = "hanbo_cooler_tests.rs"]
mod tests;
