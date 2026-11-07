//! Current firmware selector for the five PROTOCOL40 earbuds deployments.
//! Four current facades try dock extended firmware first; 2689 calls the
//! ordinary extended getter directly. Both use the DockDongle transaction type.
use crate::earbuds_protocol40::{
    self as protocol, CommandHeader, Protocol40Session, Sleeper, TransactionDeviceType,
};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};

const DOCK_EXTENDED: CommandHeader = CommandHeader::new([4, 0, 147]);
const EXTENDED: CommandHeader = CommandHeader::new([0, 0, 135]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    major: u8,
    minor: u8,
    internal: u8,
    reserved: u8,
}

impl Version {
    fn from_payload(data: &[u8]) -> Result<Self> {
        ensure!(data.len() >= 4, "Firmware reply omits version components");
        Ok(Self {
            major: data[0],
            minor: data[1],
            internal: data[2],
            reserved: data[3],
        })
    }

    pub fn components(self) -> [u8; 4] {
        [self.major, self.minor, self.internal, self.reserved]
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}.{}.{}.{}",
            self.major, self.minor, self.internal, self.reserved
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    DockExtended,
    Extended,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reading {
    version: Version,
    source: Source,
    dock_failure: Option<String>,
}

impl Reading {
    pub fn version(&self) -> Version {
        self.version
    }
    pub fn source(&self) -> Source {
        self.source
    }
    pub fn dock_failure(&self) -> Option<&str> {
        self.dock_failure.as_deref()
    }
}

/// Read actual firmware through the source selector. A failure never becomes
/// the unrelated multi-firmware branch's synthetic "1.0.0.0" result.
pub fn read<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    product_id: u32,
    validate: &dyn Fn() -> Result<()>,
) -> Result<Reading> {
    ensure!(
        protocol::supports_auto_pause(product_id),
        "Product has no audited earbuds firmware selector"
    );
    validate()?;
    let mut dock_failure = None;
    if product_id != 2689 {
        match session.send_command_as(
            DOCK_EXTENDED,
            &[],
            TransactionDeviceType::DockDongle,
            validate,
        ) {
            Ok(reply) if !reply.data.is_empty() => {
                // A partially returned version stays unknown. Do not turn
                // missing components into zeros or infer a feature gate.
                let version = Version::from_payload(&reply.data)?;
                return Ok(Reading {
                    version,
                    source: Source::DockExtended,
                    dock_failure: None,
                });
            }
            Ok(_) => dock_failure = Some("Dock firmware reply has no version fields".into()),
            Err(error) => dock_failure = Some(format!("{error:#}")),
        }
        validate().with_context(|| {
            format!(
                "Dock firmware query: {}",
                dock_failure.as_deref().unwrap_or("no version")
            )
        })?;
    }
    let reply = session
        .send_command_as(EXTENDED, &[], TransactionDeviceType::DockDongle, validate)
        .with_context(|| match &dock_failure {
            Some(error) => format!("Extended firmware query after dock failure: {error}"),
            None => "Extended firmware query".into(),
        })?;
    let version = Version::from_payload(&reply.data)?;
    Ok(Reading {
        version,
        source: Source::Extended,
        dock_failure,
    })
}

#[cfg(test)]
#[path = "earbuds_firmware_tests.rs"]
mod tests;
