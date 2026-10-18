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
    owner: crate::platform::windows::session_changes::Owner,
}

impl SessionMonitor {
    pub fn start() -> Result<(Self, Receiver<SessionChange>), String> {
        #[cfg(windows)]
        {
            let (events, changes) = async_channel::unbounded();
            let owner = crate::platform::windows::session_changes::Owner::start(events)?;
            Ok((Self { owner }, changes))
        }
        #[cfg(not(windows))]
        {
            Err("Screen lock observation is not implemented on this platform".into())
        }
    }

    /// Consume on a background executor and wait until the observation thread
    /// has released its native registrations, window and callback context.
    pub fn shutdown(self) -> Result<(), String> {
        #[cfg(windows)]
        {
            self.owner.shutdown()
        }
        #[cfg(not(windows))]
        {
            Ok(())
        }
    }
}
