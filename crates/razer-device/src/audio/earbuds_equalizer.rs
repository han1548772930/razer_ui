//! Firmware EQ submissions are local configuration and acknowledged writes,
//! never independently observed EQ: the mounted feature has no getter.
use crate::earbuds_protocol40::{CommandHeader, Protocol40Session, Sleeper, WriteOutcome};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    Game,
    Music,
    Movie,
    Custom,
    Voice,
}

impl Preset {
    pub fn key(self) -> &'static str {
        match self {
            Self::Game => "game",
            Self::Music => "music",
            Self::Movie => "movie",
            Self::Custom => "custom",
            Self::Voice => "voice",
        }
    }
    fn index(self, product_id: u32) -> u8 {
        match self {
            Self::Game => 1,
            Self::Music => 2,
            Self::Movie => 3,
            Self::Custom => 31,
            Self::Voice if product_id != 2689 => 4,
            Self::Voice => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Standard,
    Esports,
}
impl Mode {
    pub fn key(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Esports => "esports",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Setting {
    pub mode: Mode,
    pub preset: Preset,
    pub bands: [i8; 10],
}
impl Setting {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.bands.iter().all(|level| (-6..=6).contains(level)),
            "Earbud EQ band is outside -6..6"
        );
        Ok(())
    }
}

/// Confirmation of one submitted command, not an independently read EQ value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Submission {
    NotSubmitted,
    Confirmed,
    Failed { message: String, acknowledged: bool },
    Cancelled { message: String, acknowledged: bool },
}

impl Submission {
    fn error(&self) -> Option<&str> {
        match self {
            Self::Failed { message, .. } | Self::Cancelled { message, .. } => Some(message),
            _ => None,
        }
    }

    /// A validated ACK retained after a final guard/release failure is
    /// diagnostic evidence only; it does not authorize another command.
    pub fn acknowledged(&self) -> bool {
        matches!(
            self,
            Self::Confirmed
                | Self::Failed {
                    acknowledged: true,
                    ..
                }
                | Self::Cancelled {
                    acknowledged: true,
                    ..
                }
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub setting: Setting,
    pub preset: Submission,
    pub bands: Submission,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Failure {
    pub product_id: u32,
    pub submission: Report,
    pub message: String,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Failure {}

impl Report {
    pub fn validate(&self) -> Result<()> {
        self.setting.validate()?;
        ensure!(
            self.preset != Submission::NotSubmitted,
            "EQ preset command was never attempted"
        );
        ensure!(
            (self.preset == Submission::Confirmed) == (self.bands != Submission::NotSubmitted),
            "EQ band confirmation does not follow its preset submission"
        );
        Ok(())
    }
    pub fn diagnostic(&self) -> Option<String> {
        if let Some(error) = self.preset.error() {
            if self.preset.acknowledged() {
                return Some(format!(
                    "Earbud EQ preset was acknowledged; submission stopped before the band command: {error}"
                ));
            }
            return Some(format!("Earbud EQ preset was not confirmed: {error}"));
        }
        if let Some(error) = self.bands.error() {
            if self.bands.acknowledged() {
                return Some(format!(
                    "Earbud EQ commands were acknowledged; completion failed: {error}"
                ));
            }
            return Some(format!(
                "Earbud EQ preset was acknowledged; band command did not complete: {error}"
            ));
        }
        if self.preset != Submission::Confirmed || self.bands != Submission::Confirmed {
            return Some("Earbud EQ submission did not complete both commands".into());
        }
        None
    }

    pub fn outcome(&self) -> WriteOutcome {
        self.diagnostic()
            .map(WriteOutcome::Failed)
            .unwrap_or(WriteOutcome::Confirmed)
    }
}

fn failed_submission(
    error: anyhow::Error,
    command: [u8; 3],
    validate: &dyn Fn() -> Result<()>,
) -> Submission {
    let acknowledged = crate::feature_exchange::CompletedFeatureExchange::from_error(&error)
        .is_some_and(|reply| reply.command() == command);
    match validate() {
        Ok(()) => Submission::Failed {
            message: format!("{error:#}"),
            acknowledged,
        },
        Err(cancelled) => Submission::Cancelled {
            message: format!("{error:#}; {cancelled:#}"),
            acknowledged,
        },
    }
}

pub fn apply<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    product_id: u32,
    setting: Setting,
    validate: &dyn Fn() -> Result<()>,
) -> Result<Report> {
    ensure!(
        crate::earbuds_protocol40::supports_auto_pause(product_id),
        "Product has no current earbud firmware EQ"
    );
    setting.validate()?;
    let mut report = Report {
        setting,
        preset: Submission::NotSubmitted,
        bands: Submission::NotSubmitted,
    };
    match session.send_command(
        CommandHeader::new([4, 24, 0]),
        &[1, 1, 0, setting.preset.index(product_id)],
        validate,
    ) {
        Ok(_) => report.preset = Submission::Confirmed,
        Err(error) => {
            report.preset = failed_submission(error, [4, 24, 0], validate);
            return Ok(report);
        }
    }
    let mut payload = [0; 12];
    payload[..2].copy_from_slice(&[1, 1]);
    for (byte, level) in payload[2..].iter_mut().zip(setting.bands) {
        *byte = if level < 0 {
            0x80 | level.unsigned_abs()
        } else {
            level as u8
        };
    }
    match session.send_command(CommandHeader::new([12, 24, 1]), &payload, validate) {
        Ok(_) => report.bands = Submission::Confirmed,
        Err(error) => report.bands = failed_submission(error, [12, 24, 1], validate),
    }
    Ok(report)
}
