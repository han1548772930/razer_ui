//! Razer Hanbo AIO (3893) thermal profile and `rzDeviceElsa` command encoding.
//!
//! Current sources: product 3893 MW `main` (DEFAULTPROFILE ports and the
//! `hanboConfigure.hanboModeSettings` feature), `6259` task runner `ko`
//! (`Fans` selects the fan controller, every other thermal port the pump), and
//! `rzDeviceElsa` (`_sendCommand`, command headers and payload layouts).
//! Device settings require a matching status-1 reply. Telemetry requires both
//! current getter responses; an Output write alone proves neither result.
use crate::backend::{FeatureTransport, HidNode};
use anyhow::{Context as _, Result, bail, ensure};
use serde::{Deserialize, Serialize};

#[path = "hanbo_startup.rs"]
mod startup;
pub use startup::{
    DeviceInformation, DeviceVersion, ErrorcodeInfo, StartupQuery, StartupReading,
    decode_device_information, decode_errorcode_info, read_startup,
};

pub const PRODUCT_ID: u32 = 3893;
pub const VENDOR_ID: u16 = 5426;
pub const CLAIM_INTERFACE: i32 = 0;
/// `_sendCommand` allocates 65 data bytes, then prefixes claimInterface 0.
pub const SOURCE_REPORT_LENGTH: usize = 65;
pub const REPORT_LENGTH: usize = SOURCE_REPORT_LENGTH + 1;
/// The source compares the native write count against reportLength, not the
/// prefixed buffer length. Do not infer a successful transfer from 66 bytes.
pub const EXPECTED_OUTPUT_COUNT: usize = SOURCE_REPORT_LENGTH;
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

/// Current MW module 68560 `MAX_RETRY_OUT`, `MAX_RETRY_IN` and sleep constants.
pub const OUTPUT_ATTEMPTS: usize = 20;
pub const INPUT_ATTEMPTS: usize = 10;
pub const OUTPUT_INPUT_DELAY_MS: u64 = 5;
pub const INPUT_RETRY_DELAY_MS: u64 = 5;
/// A finite Rust read timeout admits cancellation between interrupt reads.
pub const INPUT_TIMEOUT_MS: u32 = 50;

pub fn validate_node(node: &HidNode, product_id: u32) -> Result<()> {
    ensure!(
        product_id == PRODUCT_ID
            && u32::from(node.product_id) == PRODUCT_ID
            && node.vendor_id == VENDOR_ID
            && node.interface_number == CLAIM_INTERFACE,
        "Hanbo collection differs from source VID/PID/interface"
    );
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "u8", into = "u8")]
pub enum ControllerMode {
    NoMode,
    Quiet,
    Normal,
    Performance,
    Unknown(u8),
}

impl From<u8> for ControllerMode {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::NoMode,
            1 => Self::Quiet,
            2 => Self::Normal,
            3 => Self::Performance,
            value => Self::Unknown(value),
        }
    }
}

impl From<ControllerMode> for u8 {
    fn from(value: ControllerMode) -> Self {
        match value {
            ControllerMode::NoMode => 0,
            ControllerMode::Quiet => 1,
            ControllerMode::Normal => 2,
            ControllerMode::Performance => 3,
            ControllerMode::Unknown(value) => value,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "u8", into = "u8")]
pub enum ControllerPresence {
    Absent,
    Present,
    Unknown(u8),
}

impl From<u8> for ControllerPresence {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Absent,
            1 => Self::Present,
            value => Self::Unknown(value),
        }
    }
}

impl From<ControllerPresence> for u8 {
    fn from(value: ControllerPresence) -> Self {
        match value {
            ControllerPresence::Absent => 0,
            ControllerPresence::Present => 1,
            ControllerPresence::Unknown(value) => value,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FanControllerInfo {
    pub channel_number: u8,
    pub presence: ControllerPresence,
    pub set_mode: ControllerMode,
    pub actual_mode: ControllerMode,
    pub rpm: u16,
    pub set_duty_cycle: u8,
    pub actual_duty_cycle: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PumpControllerFunctionalityInfo {
    pub presence: ControllerPresence,
    pub set_mode: ControllerMode,
    pub actual_mode: ControllerMode,
    /// `data[5] + data[6] / 10`, without assuming a decimal-byte range.
    pub temperature_tenths_celsius: u16,
    pub rpm: u16,
    pub set_duty_cycle: u8,
    pub actual_duty_cycle: u8,
}

impl PumpControllerFunctionalityInfo {
    pub fn temperature_celsius(&self) -> f64 {
        f64::from(self.temperature_tenths_celsius) / 10.
    }
}

/// MW 34244 publishes a sample only after both ordered getters finish.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Telemetry {
    pub fan: FanControllerInfo,
    pub pump: PumpControllerFunctionalityInfo,
}

impl Telemetry {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.pump.temperature_tenths_celsius <= u16::from(u8::MAX) * 11,
            "Hanbo liquid temperature cannot be represented by the source bytes"
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoolerQuery {
    FanControllerInfo,
    PumpControllerFunctionalityInfo,
}

impl CoolerQuery {
    pub const fn header(self) -> [u8; 2] {
        match self {
            Self::FanControllerInfo => [32, 1],
            Self::PumpControllerFunctionalityInfo => [18, 1],
        }
    }

    pub const fn reply_header(self) -> [u8; 2] {
        match self {
            Self::FanControllerInfo => [33, 2],
            Self::PumpControllerFunctionalityInfo => [19, 2],
        }
    }

    pub fn report(self) -> [u8; REPORT_LENGTH] {
        output_report(self.header(), &[])
    }
}

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
    /// Despite its getter name, source `getCPUGPUTemperature` sends both
    /// observed temperatures to the controller and requires a status-1 ACK.
    CpuGpuTemperature {
        cpu_integer: u8,
        cpu_decimal: u8,
        gpu_integer: u8,
        gpu_decimal: u8,
    },
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
    pub fn for_temperatures(cpu: Option<f64>, gpu: Option<f64>) -> Result<Self> {
        let cpu = cpu.context("Hanbo CPU temperature is unobserved")?;
        let gpu = gpu.context("Hanbo GPU temperature is unobserved")?;
        let (cpu_integer, cpu_decimal) = temperature_parts(cpu)?;
        let (gpu_integer, gpu_decimal) = temperature_parts(gpu)?;
        Ok(Self::CpuGpuTemperature {
            cpu_integer,
            cpu_decimal,
            gpu_integer,
            gpu_decimal,
        })
    }

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
            Self::CpuGpuTemperature { .. } => [192, 1],
            Self::FanParameter { .. } => [34, 1],
            Self::FanCurve { .. } => [200, 1],
            Self::PumpParameter { .. } => [20, 1],
            Self::PumpCurve { .. } => [24, 1],
        }
    }

    pub const fn reply_header(&self) -> [u8; 2] {
        match self {
            Self::CpuGpuTemperature { .. } => [193, 2],
            Self::FanParameter { .. } => [35, 2],
            Self::FanCurve { .. } => [201, 2],
            Self::PumpParameter { .. } => [21, 2],
            Self::PumpCurve { .. } => [25, 2],
        }
    }

    fn payload(&self) -> Vec<u8> {
        match self {
            Self::CpuGpuTemperature {
                cpu_integer,
                cpu_decimal,
                gpu_integer,
                gpu_decimal,
            } => {
                vec![*cpu_integer, *cpu_decimal, *gpu_integer, *gpu_decimal, 0]
            }
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
        output_report(self.header(), &self.payload())
    }
}

/// Elsa `He` splits Number.toString(), then assigns parseInt/parseFloat to a
/// Uint8Array. Its decimal fraction is not multiplied by ten; retain the
/// source's truncation instead of silently changing its wire representation.
fn temperature_parts(value: f64) -> Result<(u8, u8)> {
    let text = crate::ecmascript_number::to_string(value)?;
    let (integer, decimal) = text.split_once('.').unwrap_or((&text, "0"));
    let end = integer
        .char_indices()
        .find(|(index, character)| {
            !character.is_ascii_digit() && !(*index == 0 && *character == '-')
        })
        .map_or(integer.len(), |(index, _)| index);
    let integer = integer[..end].parse::<f64>()?;
    let decimal = format!("0.{decimal}").parse::<f64>()?;
    Ok((
        crate::ecmascript_number::to_uint8(integer),
        crate::ecmascript_number::to_uint8(decimal),
    ))
}

fn output_report(header: [u8; 2], payload: &[u8]) -> [u8; REPORT_LENGTH] {
    let mut report = [0u8; REPORT_LENGTH];
    report[0] = REPORT_ID;
    report[1..3].copy_from_slice(&header);
    for (slot, byte) in report[3..].iter_mut().zip(payload) {
        *slot = *byte;
    }
    report
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
    if !matching_packet(input, command.reply_header()) {
        return None;
    }
    Some(match input[2] {
        1 => CoolerReply::Accepted,
        status => CoolerReply::Rejected { status },
    })
}

fn matching_packet(input: &[u8], header: [u8; 2]) -> bool {
    input.len() == INPUT_LENGTH && input[..2] == header
}

pub fn decode_fan_controller_info(input: &[u8]) -> Option<FanControllerInfo> {
    if !matching_packet(input, CoolerQuery::FanControllerInfo.reply_header()) {
        return None;
    }
    Some(FanControllerInfo {
        channel_number: input[2],
        presence: input[3].into(),
        set_mode: input[4].into(),
        actual_mode: input[5].into(),
        rpm: u16::from_be_bytes([input[6], input[7]]),
        set_duty_cycle: input[8],
        actual_duty_cycle: input[9],
    })
}

pub fn decode_pump_controller_functionality_info(
    input: &[u8],
) -> Option<PumpControllerFunctionalityInfo> {
    if !matching_packet(
        input,
        CoolerQuery::PumpControllerFunctionalityInfo.reply_header(),
    ) {
        return None;
    }
    Some(PumpControllerFunctionalityInfo {
        presence: input[2].into(),
        set_mode: input[3].into(),
        actual_mode: input[4].into(),
        temperature_tenths_celsius: u16::from(input[5]) * 10 + u16::from(input[6]),
        rpm: u16::from_be_bytes([input[7], input[8]]),
        set_duty_cycle: input[9],
        actual_duty_cycle: input[10],
    })
}

/// One write/read exchange over an opened Elsa collection. `validate` lets
/// the owner reject the exchange after a device/session replacement.
pub fn exchange(
    transport: &dyn FeatureTransport,
    command: &CoolerCommand,
    validate: impl FnMut() -> Result<()>,
) -> Result<()> {
    exchange_with_wait(transport, command, validate, std::thread::sleep)
}

fn exchange_with_wait(
    transport: &dyn FeatureTransport,
    command: &CoolerCommand,
    mut validate: impl FnMut() -> Result<()>,
    mut wait: impl FnMut(std::time::Duration),
) -> Result<()> {
    let input = exchange_packet(
        transport,
        &command.report(),
        command.reply_header(),
        &mut validate,
        &mut wait,
    )?;
    match decode_reply(command, &input).context("Hanbo setter reply header changed")? {
        CoolerReply::Accepted => Ok(()),
        CoolerReply::Rejected { status } => {
            bail!(
                "Hanbo command {:?} rejected, status {status}",
                command.header()
            )
        }
    }
}

/// Ordered current-source getters; a failed pump read does not publish the
/// earlier fan response as a complete sample.
pub fn read_telemetry(
    transport: &dyn FeatureTransport,
    validate: impl FnMut() -> Result<()>,
) -> Result<Telemetry> {
    read_telemetry_with_wait(transport, validate, std::thread::sleep)
}

fn read_telemetry_with_wait(
    transport: &dyn FeatureTransport,
    mut validate: impl FnMut() -> Result<()>,
    mut wait: impl FnMut(std::time::Duration),
) -> Result<Telemetry> {
    let fan = CoolerQuery::FanControllerInfo;
    let fan = exchange_packet(
        transport,
        &fan.report(),
        fan.reply_header(),
        &mut validate,
        &mut wait,
    )?;
    let fan = decode_fan_controller_info(&fan).context("Hanbo fan reply header changed")?;
    let pump = CoolerQuery::PumpControllerFunctionalityInfo;
    let pump = exchange_packet(
        transport,
        &pump.report(),
        pump.reply_header(),
        &mut validate,
        &mut wait,
    )?;
    let pump = decode_pump_controller_functionality_info(&pump)
        .context("Hanbo pump reply header changed")?;
    validate()?;
    Ok(Telemetry { fan, pump })
}

fn exchange_packet(
    transport: &dyn FeatureTransport,
    report: &[u8; REPORT_LENGTH],
    reply_header: [u8; 2],
    validate: &mut dyn FnMut() -> Result<()>,
    wait: &mut dyn FnMut(std::time::Duration),
) -> Result<[u8; INPUT_LENGTH]> {
    let mut last_error = String::from("no input response");
    for _ in 0..OUTPUT_ATTEMPTS {
        validate()?;
        let count = transport
            .write_output_counted(report)
            .context("Hanbo Output Report 发送失败")?;
        validate()?;
        if count != EXPECTED_OUTPUT_COUNT {
            last_error = format!(
                "native write returned {count} bytes; source requires {EXPECTED_OUTPUT_COUNT}"
            );
            continue;
        }
        wait(std::time::Duration::from_millis(OUTPUT_INPUT_DELAY_MS));
        for attempt in 0..INPUT_ATTEMPTS {
            if attempt > 0 {
                wait(std::time::Duration::from_millis(INPUT_RETRY_DELAY_MS));
            }
            validate()?;
            let mut input = [0u8; INPUT_LENGTH + 1];
            let received = transport.read_interrupt(&mut input, INPUT_TIMEOUT_MS);
            validate()?;
            let count = match received {
                Ok(count) => count,
                Err(error) => {
                    last_error = format!("Hanbo Input Report 读取失败: {error:#}");
                    break;
                }
            };
            if count == INPUT_LENGTH && matching_packet(&input[..count], reply_header) {
                let mut packet = [0u8; INPUT_LENGTH];
                packet.copy_from_slice(&input[..INPUT_LENGTH]);
                return Ok(packet);
            }
            last_error = if count == INPUT_LENGTH {
                format!("invalid response header {:?}", &input[..2])
            } else {
                format!("invalid input count {count}; source requires {INPUT_LENGTH}")
            };
        }
    }
    bail!(
        "Hanbo command {:?} received no matching reply: {last_error}",
        &report[1..3]
    )
}

#[cfg(test)]
#[path = "hanbo_cooler_tests.rs"]
mod tests;
