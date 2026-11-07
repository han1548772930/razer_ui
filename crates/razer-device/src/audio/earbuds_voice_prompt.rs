//! Earbud prompt index All=255. User SET is direct; profile restore reads first.
use crate::earbuds_protocol40::{
    CommandHeader, Protocol40Session, Sleeper, ToggleObservation, ToggleRestore, WriteOutcome,
    decode_toggle,
};
use anyhow::{Result, ensure};

pub fn read<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    validate: &dyn Fn() -> Result<()>,
) -> Result<bool> {
    let reply = session.send_command(CommandHeader::new([2, 24, 144]), &[1, 255], validate)?;
    ensure!(
        reply.data.len() >= 3,
        "Voice prompt response omits its state"
    );
    decode_toggle(reply.data[2])
}

pub fn write<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    enabled: bool,
    validate: &dyn Fn() -> Result<()>,
) -> Result<()> {
    session
        .send_command(
            CommandHeader::new([3, 24, 16]),
            &[1, 255, u8::from(enabled)],
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
