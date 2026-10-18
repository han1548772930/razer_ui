//! Independent Elsa startup getters, separate from the periodic thermal sample.
use super::{
    FeatureTransport, INPUT_LENGTH, REPORT_LENGTH, exchange_packet, matching_packet, output_report,
};
use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupQuery {
    DeviceInformation,
    ErrorcodeInfo,
}

impl StartupQuery {
    pub const fn header(self) -> [u8; 2] {
        match self {
            Self::DeviceInformation => [1, 1],
            Self::ErrorcodeInfo => [160, 1],
        }
    }

    pub const fn reply_header(self) -> [u8; 2] {
        match self {
            Self::DeviceInformation => [2, 2],
            Self::ErrorcodeInfo => [161, 2],
        }
    }

    pub fn report(self) -> [u8; REPORT_LENGTH] {
        output_report(self.header(), &[])
    }
}

/// Minor version is the complete source byte; the displayed minor components
/// are its high/low nibbles. Unknown values remain observable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceVersion {
    pub major: u8,
    pub minor: u8,
}

impl std::fmt::Display for DeviceVersion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}.{}.{}",
            self.major,
            self.minor >> 4,
            self.minor & 15
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInformation {
    /// Current getter uses data.slice(2,17), retaining all fifteen bytes.
    pub device_id: [u8; 15],
    /// LE16. Source names edition 0 fan2 and edition 128 fan3.
    pub edition: u16,
    /// Source modes 1 main application, 2 reprogrammer, 3 factory test.
    pub mode: u8,
    pub firmware: DeviceVersion,
    /// Source names 0 release and 1 debug; other byte values stay unknown.
    pub firmware_debug_flag: u8,
    pub hardware: DeviceVersion,
}

impl DeviceInformation {
    pub fn device_id_ascii(&self) -> String {
        self.device_id
            .iter()
            .copied()
            .filter(|byte| (32..127).contains(byte))
            .map(char::from)
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorcodeInfo {
    pub pump_not_connected: bool,
    pub temperature_error: bool,
    pub pump_tach_frequency_error: bool,
    pub fan_not_connected: bool,
    pub fan_number_error: bool,
    pub fan_tach_frequency_error: bool,
    pub fan_led_broken: bool,
    pub power_not_connected: bool,
}

impl ErrorcodeInfo {
    pub fn has_errors(&self) -> bool {
        self.pump_not_connected
            || self.temperature_error
            || self.pump_tach_frequency_error
            || self.fan_not_connected
            || self.fan_number_error
            || self.fan_tach_frequency_error
            || self.fan_led_broken
            || self.power_not_connected
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum StartupReading {
    DeviceInformation(DeviceInformation),
    ErrorcodeInfo(ErrorcodeInfo),
}

impl StartupReading {
    pub const fn query(&self) -> StartupQuery {
        match self {
            Self::DeviceInformation(_) => StartupQuery::DeviceInformation,
            Self::ErrorcodeInfo(_) => StartupQuery::ErrorcodeInfo,
        }
    }
}

pub fn decode_device_information(input: &[u8]) -> Option<DeviceInformation> {
    if !matching_packet(input, StartupQuery::DeviceInformation.reply_header()) {
        return None;
    }
    let mut device_id = [0; 15];
    device_id.copy_from_slice(&input[2..17]);
    Some(DeviceInformation {
        device_id,
        edition: u16::from_le_bytes([input[26], input[27]]),
        mode: input[28],
        firmware: DeviceVersion {
            major: input[29],
            minor: input[30],
        },
        firmware_debug_flag: input[31],
        hardware: DeviceVersion {
            major: input[32],
            minor: input[33],
        },
    })
}

pub fn decode_errorcode_info(input: &[u8]) -> Option<ErrorcodeInfo> {
    if !matching_packet(input, StartupQuery::ErrorcodeInfo.reply_header()) {
        return None;
    }
    let flags = input[2];
    Some(ErrorcodeInfo {
        pump_not_connected: flags & 128 != 0,
        temperature_error: flags & 64 != 0,
        pump_tach_frequency_error: flags & 32 != 0,
        fan_not_connected: flags & 16 != 0,
        fan_number_error: flags & 8 != 0,
        fan_tach_frequency_error: flags & 4 != 0,
        fan_led_broken: flags & 2 != 0,
        power_not_connected: flags & 1 != 0,
    })
}

pub fn read_startup(
    transport: &dyn FeatureTransport,
    query: StartupQuery,
    validate: impl FnMut() -> Result<()>,
) -> Result<StartupReading> {
    read_startup_with_wait(transport, query, validate, std::thread::sleep)
}

pub(super) fn read_startup_with_wait(
    transport: &dyn FeatureTransport,
    query: StartupQuery,
    mut validate: impl FnMut() -> Result<()>,
    mut wait: impl FnMut(std::time::Duration),
) -> Result<StartupReading> {
    let input: [u8; INPUT_LENGTH] = exchange_packet(
        transport,
        &query.report(),
        query.reply_header(),
        &mut validate,
        &mut wait,
    )?;
    validate()?;
    match query {
        StartupQuery::DeviceInformation => {
            decode_device_information(&input).map(StartupReading::DeviceInformation)
        }
        StartupQuery::ErrorcodeInfo => {
            decode_errorcode_info(&input).map(StartupReading::ErrorcodeInfo)
        }
    }
    .context("Hanbo startup getter reply header changed")
}
