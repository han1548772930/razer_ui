//! Read-only OS observations for the source's display and system-idle handlers.
use async_channel::Receiver;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    DisplayOff,
    DisplayOn,
    DisplayDimmed,
    Idle { milliseconds: u32 },
    Shutdown,
    Failed(String),
}

pub struct Monitor {
    #[cfg(windows)]
    owner: crate::platform::windows::power_idle::Owner,
}
impl Monitor {
    pub fn start() -> Result<(Self, Receiver<Event>), String> {
        #[cfg(windows)]
        {
            let (events, receiver)=async_channel::unbounded();
            let owner=crate::platform::windows::power_idle::Owner::start(events)?;
            Ok((Self {owner},receiver))
        }
        #[cfg(not(windows))]
        { Err("Display-power and idle observation is unavailable on this platform".into()) }
    }
    /// Consumes the monitor on a background executor and waits for callback,
    /// timer, registration and window release before returning.
    pub fn stop(self) -> Result<(),String> {
        #[cfg(windows)]
        { self.owner.stop() }
        #[cfg(not(windows))]
        { Ok(()) }
    }
}
