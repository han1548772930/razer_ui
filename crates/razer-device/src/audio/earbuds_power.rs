//! Global earbud sleep settings; profile selection does not restore these values.
use crate::earbuds_protocol40::{
    CommandHeader, Protocol40Session, Sleeper, ToggleObservation, WriteOutcome, decode_toggle,
};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};

pub const MINUTE_CHOICES: [u16; 5] = [5, 15, 30, 45, 60];
const GET_MODE: CommandHeader = CommandHeader::new([1, 7, 136]);
const SET_MODE: CommandHeader = CommandHeader::new([2, 7, 8]);
const GET_TIME: CommandHeader = CommandHeader::new([0, 7, 131]);
const SET_TIME: CommandHeader = CommandHeader::new([2, 7, 3]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Setting {
    enabled: bool,
    seconds: u16,
}
impl Setting {
    pub const fn new(enabled: bool, seconds: u16) -> Self {
        Self { enabled, seconds }
    }
    pub fn enabled(self) -> bool {
        self.enabled
    }
    pub fn seconds(self) -> u16 {
        self.seconds
    }
    pub fn minutes(self) -> f64 {
        f64::from(self.seconds) / 60.
    }
    pub fn with_enabled(self, enabled: bool) -> Self {
        Self { enabled, ..self }
    }
    pub fn with_minutes(self, minutes: u16) -> Result<Self> {
        ensure!(
            MINUTE_CHOICES.contains(&minutes),
            "Earbud sleep time is not a source option"
        );
        Ok(Self {
            seconds: minutes * 60,
            ..self
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum TimeObservation {
    #[default]
    Unobserved,
    Observed(u16),
    ReadFailed(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Observation {
    mode: ToggleObservation,
    time: TimeObservation,
}
impl Observation {
    pub fn mode(&self) -> &ToggleObservation {
        &self.mode
    }
    pub fn time(&self) -> &TimeObservation {
        &self.time
    }
    pub fn setting(&self) -> Option<Setting> {
        match (&self.mode, &self.time) {
            (ToggleObservation::Observed(enabled), TimeObservation::Observed(seconds)) => {
                Some(Setting::new(*enabled, *seconds))
            }
            _ => None,
        }
    }
    pub fn diagnostic(&self) -> Option<String> {
        self.mode.diagnostic().or_else(|| match &self.time {
            TimeObservation::ReadFailed(error) => Some(error.clone()),
            _ => None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Report {
    observation: Observation,
    mode_write: WriteOutcome,
    time_write: WriteOutcome,
}
impl Report {
    pub fn observation(&self) -> &Observation {
        &self.observation
    }
    pub fn mode_write(&self) -> &WriteOutcome {
        &self.mode_write
    }
    pub fn time_write(&self) -> &WriteOutcome {
        &self.time_write
    }
    pub fn outcome(&self) -> WriteOutcome {
        for write in [&self.mode_write, &self.time_write] {
            if let WriteOutcome::Failed(error) = write {
                return WriteOutcome::Failed(error.clone());
            }
        }
        if matches!(self.mode_write, WriteOutcome::Confirmed)
            || matches!(self.time_write, WriteOutcome::Confirmed)
        {
            WriteOutcome::Confirmed
        } else {
            WriteOutcome::Skipped
        }
    }
}

pub fn read<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    validate: &dyn Fn() -> Result<()>,
) -> Result<Observation> {
    let mode = (|| {
        let reply = session.send_command(GET_MODE, &[1], validate)?;
        ensure!(
            reply.data.len() >= 2,
            "Earbud sleep-mode reply omits profile or mode"
        );
        ensure!(
            reply.data[0] == 1,
            "Earbud sleep-mode reply belongs to another profile"
        );
        Ok(reply.data[1])
    })();
    let mode = match mode {
        Ok(raw) => match decode_toggle(raw) {
            Ok(enabled) => ToggleObservation::Observed(enabled),
            Err(error) => ToggleObservation::from_read_error(&error),
        },
        Err(error) => {
            validate()?;
            return Ok(Observation {
                mode: ToggleObservation::from_read_error(&error),
                time: TimeObservation::Unobserved,
            });
        }
    };
    let time = (|| -> Result<u16> {
        let reply = session.send_command(GET_TIME, &[], validate)?;
        let bytes: [u8; 2] = reply
            .data
            .get(..2)
            .context("Earbud sleep-time reply is truncated")?
            .try_into()?;
        Ok(u16::from_be_bytes(bytes))
    })();
    let time = match time {
        Ok(seconds) => TimeObservation::Observed(seconds),
        Err(error) => {
            validate()?;
            TimeObservation::ReadFailed(format!("{error:#}"))
        }
    };
    Ok(Observation { mode, time })
}

pub fn execute<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    setting: Option<Setting>,
    validate: &dyn Fn() -> Result<()>,
) -> Result<Report> {
    // The facade reads mode then time before either setter. Its stale runtime
    // cache and swallowed FAILED results cannot establish a successful skip.
    let mut report = Report {
        observation: read(session, validate)?,
        mode_write: WriteOutcome::Skipped,
        time_write: WriteOutcome::Skipped,
    };
    let Some(setting) = setting else {
        return Ok(report);
    };
    let Some(previous) = report.observation.setting() else {
        report.mode_write = WriteOutcome::Failed(
            report
                .observation
                .diagnostic()
                .unwrap_or_else(|| "Earbud sleep settings have not been observed".into()),
        );
        return Ok(report);
    };
    if previous.enabled != setting.enabled {
        match session.send_command(SET_MODE, &[1, u8::from(setting.enabled)], validate) {
            Ok(_) => {
                report.mode_write = WriteOutcome::Confirmed;
                report.observation.mode = ToggleObservation::Observed(setting.enabled);
            }
            Err(error) => {
                validate()?;
                report.mode_write = WriteOutcome::Failed(format!("{error:#}"));
                return Ok(report);
            }
        }
    }
    if previous.seconds != setting.seconds {
        match session.send_command(SET_TIME, &setting.seconds.to_be_bytes(), validate) {
            Ok(_) => {
                report.time_write = WriteOutcome::Confirmed;
                report.observation.time = TimeObservation::Observed(setting.seconds);
            }
            Err(error) => {
                validate()?;
                report.time_write = WriteOutcome::Failed(format!("{error:#}"));
            }
        }
    }
    Ok(report)
}
