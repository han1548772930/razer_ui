//! Native handles and effect replacement have a single owner. Cleanup continues
//! after individual native failures so no failed call skips remaining releases.
use razer_device::controller_lighting::{
    SoftwareEffect,
    software::{EngineEffect, EngineFrame, LedConfiguration},
};

pub(super) trait EngineAdapter {
    fn create_device(&mut self, config: &LedConfiguration) -> anyhow::Result<u32>;
    fn create_engine(&mut self) -> anyhow::Result<u32>;
    fn add_device(&mut self, engine: u32, device: u32) -> anyhow::Result<()>;
    fn add_effect(&mut self, engine: u32, effect: &EngineEffect) -> anyhow::Result<u32>;
    fn remove_effect(&mut self, engine: u32, effect: u32) -> anyhow::Result<()>;
    fn enable(&mut self, engine: u32, enable: bool) -> anyhow::Result<()>;
    fn input(&mut self, device: u32) -> anyhow::Result<()>;
    fn frames(&mut self) -> anyhow::Result<Vec<EngineFrame>>;
    fn destroy_engine(&mut self, engine: u32) -> anyhow::Result<()>;
    fn destroy_device(&mut self, device: u32) -> anyhow::Result<()>;
    fn terminate(&mut self) -> anyhow::Result<()>;
}

pub(super) struct Session<E: EngineAdapter> {
    pub engine: E,
    pub device_handle: Option<u32>,
    engine_handle: Option<u32>,
    effect_handle: Option<u32>,
    pub suspended: bool,
    terminated: bool,
}
impl<E: EngineAdapter> Session<E> {
    pub fn new(engine: E) -> Self {
        Self {
            engine,
            device_handle: None,
            engine_handle: None,
            effect_handle: None,
            suspended: false,
            terminated: false,
        }
    }
    pub fn initialize(
        &mut self,
        config: &LedConfiguration,
        effect: &SoftwareEffect,
    ) -> anyhow::Result<()> {
        self.engine_handle = Some(self.engine.create_engine()?);
        let device = self.engine.create_device(config)?;
        self.device_handle = Some(device);
        self.engine
            .add_device(self.engine_handle.unwrap(), device)?;
        self.update(effect)
    }
    pub fn update(&mut self, effect: &SoftwareEffect) -> anyhow::Result<()> {
        let engine = self
            .engine_handle
            .ok_or_else(|| anyhow::anyhow!("Lighting engine has not been created"))?;
        if let Some(effect) = self.effect_handle.take() {
            self.engine.remove_effect(engine, effect)?;
        }
        // Remove queued frames from the previous effect before the generation
        // becomes active. They are observations of a superseded effect.
        self.engine.frames()?;
        self.effect_handle = Some(self.engine.add_effect(engine, &effect.engine_effect())?);
        self.engine.enable(engine, true)?;
        self.suspended = false;
        Ok(())
    }
    pub fn suspend(&mut self, suspended: bool) -> anyhow::Result<()> {
        let engine = self
            .engine_handle
            .ok_or_else(|| anyhow::anyhow!("Lighting engine has not been created"))?;
        self.engine.enable(engine, !suspended)?;
        self.suspended = suspended;
        self.engine.frames()?;
        Ok(())
    }
    pub fn input(&mut self) -> anyhow::Result<()> {
        let device = self
            .device_handle
            .ok_or_else(|| anyhow::anyhow!("Lighting device has not been created"))?;
        self.engine.input(device)
    }
    pub fn cleanup(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        if let Some(engine) = self.engine_handle.take() {
            if let Err(e) = self.engine.enable(engine, false) {
                errors.push(format!("Disable: {e:#}"));
            }
            if let Err(e) = self.engine.destroy_engine(engine) {
                errors.push(format!("Destroy engine: {e:#}"));
            }
        }
        self.effect_handle = None;
        if let Some(device) = self.device_handle.take() {
            if let Err(e) = self.engine.destroy_device(device) {
                errors.push(format!("Destroy device: {e:#}"));
            }
        }
        if !self.terminated {
            self.terminated = true;
            if let Err(e) = self.engine.terminate() {
                errors.push(format!("Terminate: {e:#}"));
            }
        }
        errors
    }
}
impl<E: EngineAdapter> Drop for Session<E> {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

#[cfg(windows)]
impl EngineAdapter for super::native::Engine {
    fn create_device(&mut self, c: &LedConfiguration) -> anyhow::Result<u32> {
        Self::create_device(self, c)
    }
    fn create_engine(&mut self) -> anyhow::Result<u32> {
        Self::create_engine(self)
    }
    fn add_device(&mut self, e: u32, d: u32) -> anyhow::Result<()> {
        Self::add_device(self, e, d)
    }
    fn add_effect(&mut self, e: u32, f: &EngineEffect) -> anyhow::Result<u32> {
        Self::add_effect(self, e, f)
    }
    fn remove_effect(&mut self, e: u32, f: u32) -> anyhow::Result<()> {
        Self::remove_effect(self, e, f)
    }
    fn enable(&mut self, e: u32, enabled: bool) -> anyhow::Result<()> {
        Self::enable(self, e, enabled)
    }
    fn input(&mut self, d: u32) -> anyhow::Result<()> {
        Self::input(self, d)
    }
    fn frames(&mut self) -> anyhow::Result<Vec<EngineFrame>> {
        Self::frames(self)
    }
    fn destroy_engine(&mut self, e: u32) -> anyhow::Result<()> {
        Self::destroy_engine(self, e)
    }
    fn destroy_device(&mut self, d: u32) -> anyhow::Result<()> {
        Self::destroy_device(self, d)
    }
    fn terminate(&mut self) -> anyhow::Result<()> {
        Self::terminate(self)
    }
}

#[cfg(test)]
#[path = "controller_lighting_session_tests.rs"]
mod tests;
