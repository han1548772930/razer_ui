//! Source-recovered local AudioRouter commands. No vendor module is loaded.
//! COM and audio streams remain inside their owning Windows threads.
use anyhow::Result;
use serde_json::Value;

#[derive(Default)]
pub struct AudioRouter {
    #[cfg(windows)]
    owner: Option<windows::Owner>,
}

impl AudioRouter {
    pub fn enable(&mut self, enable: i32) -> Result<Value> {
        #[cfg(windows)]
        return self.owner()?.call(windows::Command::Enable(enable));
        #[cfg(not(windows))]
        {
            let _ = enable;
            anyhow::bail!(
                "Source AudioRouter requires Windows Core Audio; this platform has no adapter"
            )
        }
    }

    pub fn route(&mut self, primary: String, routed: String, product_id: u32) -> Result<Value> {
        #[cfg(windows)]
        return self.owner()?.call(windows::Command::Route {
            primary,
            routed,
            product_id,
        });
        #[cfg(not(windows))]
        {
            let _ = (primary, routed, product_id);
            anyhow::bail!(
                "Source AudioRouter requires Windows Core Audio; this platform has no adapter"
            )
        }
    }

    pub fn drain(&mut self) -> Result<Value> {
        #[cfg(windows)]
        return self.owner()?.call(windows::Command::Drain);
        #[cfg(not(windows))]
        anyhow::bail!(
            "Source AudioRouter requires Windows Core Audio; this platform has no adapter"
        );
    }

    #[cfg(windows)]
    fn owner(&mut self) -> Result<&mut windows::Owner> {
        if self.owner.is_none() {
            self.owner = Some(windows::Owner::open()?);
        }
        Ok(self.owner.as_mut().unwrap())
    }
}

#[cfg(windows)]
#[path = "platform/windows/audio_router.rs"]
mod windows;
