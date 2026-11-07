//! Gesture enablement and tap interval for the five current PROTOCOL40 earbuds.
use crate::earbuds_protocol40::{
    CommandHeader, Protocol40Session, Sleeper, ToggleObservation, ToggleRestore, WriteOutcome,
    decode_toggle,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TapSpeed {
    Fast,
    #[default]
    Normal,
    Slow,
    Slowest,
}
impl TapSpeed {
    pub const ALL: [Self; 4] = [Self::Fast, Self::Normal, Self::Slow, Self::Slowest];
    pub fn ticks(self) -> u16 {
        match self {
            Self::Fast => 30,
            Self::Normal => 60,
            Self::Slow => 90,
            Self::Slowest => 120,
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Normal => "normal",
            Self::Slow => "slow",
            Self::Slowest => "slowest",
        }
    }
    pub fn from_ticks(ticks: u16) -> Option<Self> {
        Self::ALL.into_iter().find(|speed| speed.ticks() == ticks)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum TapObservation {
    #[default]
    Unobserved,
    Observed(u16),
    ReadFailed(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TapRestore {
    pub observation: TapObservation,
    pub write: WriteOutcome,
    pub published_local: TapSpeed,
}

pub fn read<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    validate: &dyn Fn() -> Result<()>,
) -> Result<bool> {
    let reply = session.send_command(CommandHeader::new([2, 24, 162]), &[1, 0], validate)?;
    ensure!(reply.data.len() >= 2, "Gesture response omits its state");
    decode_toggle(reply.data[1])
}
pub fn write<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    enabled: bool,
    validate: &dyn Fn() -> Result<()>,
) -> Result<()> {
    session
        .send_command(
            CommandHeader::new([2, 24, 34]),
            &[1, u8::from(enabled)],
            validate,
        )
        .map(|_| ())
}
pub fn restore<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    stored: Option<bool>,
    validate: &dyn Fn() -> Result<()>,
) -> Result<ToggleRestore> {
    let local = stored.unwrap_or(true);
    let observed = match read(session, validate) {
        Ok(value) => value,
        Err(error) => {
            validate()?;
            return Ok(ToggleRestore {
                observation: ToggleObservation::from_read_error(&error),
                write: WriteOutcome::Failed(format!("{error:#}")),
                published_local: local,
            });
        }
    };
    let write = if observed == local {
        WriteOutcome::Skipped
    } else {
        match write(session, local, validate) {
            Ok(()) => WriteOutcome::Confirmed,
            Err(error) => {
                validate()?;
                WriteOutcome::Failed(format!("{error:#}"))
            }
        }
    };
    Ok(ToggleRestore {
        observation: ToggleObservation::Observed(if write == WriteOutcome::Confirmed {
            local
        } else {
            observed
        }),
        write,
        published_local: local,
    })
}
pub fn read_tap<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    validate: &dyn Fn() -> Result<()>,
) -> Result<u16> {
    let reply = session.send_command(CommandHeader::new([2, 24, 167]), &[1, 0], validate)?;
    ensure!(
        reply.data.len() >= 4,
        "Tap speed response omits its interval"
    );
    Ok(u16::from_be_bytes([reply.data[2], reply.data[3]]))
}
pub fn write_tap<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    speed: TapSpeed,
    validate: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let [high, low] = speed.ticks().to_be_bytes();
    session
        .send_command(
            CommandHeader::new([4, 24, 39]),
            &[1, 1, high, low],
            validate,
        )
        .map(|_| ())
}
pub fn restore_tap<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    stored: Option<TapSpeed>,
    validate: &dyn Fn() -> Result<()>,
) -> Result<TapRestore> {
    let local = stored.unwrap_or(TapSpeed::Normal);
    let observed = match read_tap(session, validate) {
        Ok(value) => value,
        Err(error) => {
            validate()?;
            let error = format!("{error:#}");
            return Ok(TapRestore {
                observation: TapObservation::ReadFailed(error.clone()),
                write: WriteOutcome::Failed(error),
                published_local: local,
            });
        }
    };
    let write = if observed == local.ticks() {
        WriteOutcome::Skipped
    } else {
        match write_tap(session, local, validate) {
            Ok(()) => WriteOutcome::Confirmed,
            Err(error) => {
                validate()?;
                WriteOutcome::Failed(format!("{error:#}"))
            }
        }
    };
    Ok(TapRestore {
        observation: TapObservation::Observed(if write == WriteOutcome::Confirmed {
            local.ticks()
        } else {
            observed
        }),
        write,
        published_local: local,
    })
}
/// Current Enhancement callers: 2668 omits the support prop entirely.
pub fn tap_supported(product_id: u32, current: Option<&str>) -> bool {
    let minimum = match product_id {
        2657 => "1.3.6.0",
        2660 | 2664 => "1.3.1.0",
        2689 => "1.0.2.0",
        _ => return false,
    };
    !crate::earbuds_protocol40::firmware_blocked(minimum, current)
}
