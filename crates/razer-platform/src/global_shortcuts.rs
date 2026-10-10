//! Source-defined mapping-engine hotkey identity, isolated from OS registration.
//! Native evidence: registerGlobalShortcut → 0x143e0 → 0xc97c0 RegisterHotKey.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shortcut {
    pub virtual_key: u32,
    pub modifiers: u32,
    pub argument: String,
}

/// Current JS turnModifiersToNumber and the native 0x143e0 modifier groups.
pub fn os_modifiers(source: u32) -> anyhow::Result<u32> {
    anyhow::ensure!(
        source & !0xfff == 0,
        "Unknown source shortcut modifier bits"
    );
    Ok(u32::from(source & 0x111 != 0)
        | (u32::from(source & 0x222 != 0) << 1)
        | (u32::from(source & 0x444 != 0) << 2)
        | (u32::from(source & 0x888 != 0) << 3))
}
pub fn capture_modifier_sides() -> u32 {
    #[cfg(windows)]
    return crate::platform::windows::global_shortcuts::side_modifiers();
    #[cfg(not(windows))]
    0
}

#[derive(Default)]
pub struct GlobalShortcuts {
    #[cfg(windows)]
    listener: Option<crate::platform::windows::global_shortcuts::Listener>,
}
impl GlobalShortcuts {
    pub fn register(&mut self, shortcut: Shortcut) -> anyhow::Result<()> {
        anyhow::ensure!(
            (1..=255).contains(&shortcut.virtual_key),
            "Invalid shortcut virtual key"
        );
        os_modifiers(shortcut.modifiers)?;
        #[cfg(windows)]
        {
            if self.listener.is_none() {
                self.listener =
                    Some(crate::platform::windows::global_shortcuts::Listener::start()?);
            }
            self.listener.as_ref().unwrap().register(shortcut)
        }
        #[cfg(not(windows))]
        anyhow::bail!("Global shortcut registration is not implemented on this platform");
    }
    pub fn unregister(&mut self, virtual_key: u32, modifiers: u32) -> anyhow::Result<()> {
        #[cfg(windows)]
        return self
            .listener
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Shortcut not found"))?
            .unregister(virtual_key, modifiers);
        #[cfg(not(windows))]
        {
            let _ = (virtual_key, modifiers);
            anyhow::bail!("Global shortcut registration is not implemented on this platform");
        }
    }
    pub fn enable(&mut self, enable: bool) -> anyhow::Result<()> {
        #[cfg(windows)]
        return self
            .listener
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No shortcut registration owner"))?
            .enable(enable);
        #[cfg(not(windows))]
        {
            let _ = enable;
            anyhow::bail!("Global shortcut registration is not implemented on this platform");
        }
    }
    pub fn registered(&self) -> anyhow::Result<Vec<Shortcut>> {
        #[cfg(windows)]
        return self
            .listener
            .as_ref()
            .map_or(Ok(Vec::new()), |listener| listener.registered());
        #[cfg(not(windows))]
        anyhow::bail!("Global shortcut registration is not implemented on this platform");
    }
    pub fn drain(&self) -> anyhow::Result<Vec<Shortcut>> {
        #[cfg(windows)]
        return self
            .listener
            .as_ref()
            .map_or(Ok(Vec::new()), |listener| listener.drain());
        #[cfg(not(windows))]
        anyhow::bail!("Global shortcut registration is not implemented on this platform");
    }
}
