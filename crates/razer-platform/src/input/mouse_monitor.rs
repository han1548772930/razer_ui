//! Raw mouse observations, independent of redirected mapping-driver input.
//! Current mapping_engine D2EE0/D3060/D32B0 register usage 1/2 and retain
//! each RAWINPUT device path, movement, buttons and source clock sample.

#[derive(Clone, Debug)]
pub struct Sample {
    pub path: String,
    pub x: i32,
    pub y: i32,
    pub button_flags: u16,
    pub button_data: u16,
    pub modifiers: u32,
    pub time_tick: u64,
}

#[derive(Clone, Debug)]
pub enum Event {
    Input(Sample),
    Removed(String),
}

#[derive(Default)]
pub struct MouseMonitor {
    #[cfg(windows)]
    owner: Option<crate::platform::windows::mouse_monitor::Owner>,
}
impl MouseMonitor {
    pub fn start(&mut self) -> anyhow::Result<()> {
        #[cfg(windows)]
        {
            anyhow::ensure!(self.owner.is_none(), "Mouse input monitoring is already owned");
            self.owner=Some(crate::platform::windows::mouse_monitor::Owner::spawn()?);
            Ok(())
        }
        #[cfg(not(windows))]
        anyhow::bail!("Mouse input monitoring requires Windows RawInput")
    }
    pub fn events(&self) -> anyhow::Result<Vec<Event>> {
        #[cfg(windows)]
        {
            self.owner.as_ref().ok_or_else(||anyhow::anyhow!("Mouse monitor is inactive"))?.events()
        }
        #[cfg(not(windows))]
        anyhow::bail!("Mouse input monitoring requires Windows RawInput")
    }
    pub fn stop(&mut self) -> anyhow::Result<Vec<Event>> {
        #[cfg(windows)]
        if let Some(owner)=self.owner.as_mut() {
            owner.shutdown()?;
            let final_events=owner.final_events()?;
            self.owner=None;
            return Ok(final_events);
        }
        Ok(Vec::new())
    }
}
