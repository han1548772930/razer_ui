//! Registered earbuds DND: profile 1, read-before-write and real status acknowledgement.
pub use crate::earbuds_protocol40::ToggleObservation as Observation;
use crate::earbuds_protocol40::{
    CommandHeader, Protocol40Session, Sleeper, WriteOutcome, decode_toggle,
};
use anyhow::{Result, ensure};

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct WriteReport {
    pub observation: bool,
    pub write: WriteOutcome,
}

pub fn read<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    validate: &dyn Fn() -> Result<()>,
) -> Result<bool> {
    let reply = session.send_command(CommandHeader::new([1, 24, 133]), &[1], validate)?;
    ensure!(reply.data.len() >= 2, "DND response omits its state");
    decode_toggle(reply.data[1])
}

pub fn write<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    enabled: bool,
    validate: &dyn Fn() -> Result<()>,
) -> Result<WriteReport> {
    // shouldSkipSet is outside the source setter's catch. A failed GET cannot
    // establish equality with an off state or permit a fictitious successful skip.
    let observed = read(session, validate)?;
    if observed == enabled {
        return Ok(WriteReport {
            observation: observed,
            write: WriteOutcome::Skipped,
        });
    }
    let result = session.send_command(
        CommandHeader::new([2, 24, 5]),
        &[1, u8::from(enabled)],
        validate,
    );
    match result {
        Ok(_) => Ok(WriteReport {
            observation: enabled,
            write: WriteOutcome::Confirmed,
        }),
        Err(error) => {
            validate()?;
            Ok(WriteReport {
                observation: observed,
                write: WriteOutcome::Failed(format!("{error:#}")),
            })
        }
    }
}
