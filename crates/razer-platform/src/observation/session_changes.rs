//! Current host lock-screen and statically recovered SysUtils power events.
//! Events are observations, never inferred from ordinary window activation.
use async_channel::Receiver;

#[derive(Debug)]
pub enum SessionChange {
    Locked,
    Unlocked,
    Suspended,
    Resumed,
    Failed(String),
}

pub struct SessionMonitor {
    #[cfg(windows)]
    _owner: crate::platform::windows::session_changes::Owner,
}

impl SessionMonitor {
    pub fn start() -> Result<(Self, Receiver<SessionChange>), String> {
        #[cfg(windows)]
        {
            let (events, changes) = async_channel::unbounded();
            let owner = crate::platform::windows::session_changes::Owner::start(events)?;
            Ok((Self { _owner: owner }, changes))
        }
        #[cfg(not(windows))]
        {
            Err("Screen lock observation is not implemented on this platform".into())
        }
    }
}
