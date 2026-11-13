//! Complete OBM reads retain the same physical owner through wireless confirmation.
use anyhow::Result;
use razer_device::keyboard_onboard_profiles::CompleteReading;

pub(super) trait Session {
    fn confirm_peer(&mut self) -> Result<()>;
    fn read_profiles(&mut self) -> Result<CompleteReading>;
}

pub(super) fn read(session: &mut impl Session) -> Result<CompleteReading> {
    session.confirm_peer()?;
    let reading = session.read_profiles()?;
    session.confirm_peer()?;
    Ok(reading)
}

#[cfg(test)]
#[path = "keyboard_onboard_profiles_tests.rs"]
mod tests;
