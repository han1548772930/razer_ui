//! Renderer handles and effect replacement have a single owner. Cleanup
//! continues after individual failures so no failed call skips releases.
use razer_device::lighting_engine::{EngineEffect, EngineFrame, LedConfiguration, SoftwareEffect};
use std::{cell::RefCell, rc::Rc};

pub(in crate::runtime) trait EngineAdapter {
    fn create_device(&mut self, config: &LedConfiguration) -> anyhow::Result<u32>;
    fn create_engine(&mut self) -> anyhow::Result<u32>;
    fn add_device(&mut self, engine: u32, device: u32) -> anyhow::Result<()>;
    fn add_effect(&mut self, engine: u32, effect: &EngineEffect) -> anyhow::Result<u32>;
    fn remove_effect(&mut self, engine: u32, effect: u32) -> anyhow::Result<()>;
    fn enable(&mut self, engine: u32, enable: bool) -> anyhow::Result<()>;
    fn input(&mut self, device: u32) -> anyhow::Result<()>;
    fn frames(&mut self) -> anyhow::Result<Vec<EngineFrame>>;
    fn screen_status(
        &self,
    ) -> Option<razer_device::lighting_engine::renderer::ScreenCaptureStatus> {
        None
    }
    fn discard_frames_for(&mut self, _device: u32) -> anyhow::Result<()> {
        self.frames().map(|_| ())
    }
    fn destroy_engine(&mut self, engine: u32) -> anyhow::Result<()>;
    fn destroy_device(&mut self, device: u32) -> anyhow::Result<()>;
    fn terminate(&mut self) -> anyhow::Result<()>;
}

struct SharedRuntime<E: EngineAdapter> {
    engine: E,
    frames: Vec<EngineFrame>,
    termination: Option<Result<(), String>>,
}
impl<E: EngineAdapter> Drop for SharedRuntime<E> {
    fn drop(&mut self) {
        if self.termination.is_none() {
            let _ = self.engine.terminate();
        }
    }
}

/// Normal, charging and attached-device engines share one Rust renderer owner.
/// Their sessions release individual handles; the runtime owner terminates last.
pub(in crate::runtime) struct SharedEngine<E: EngineAdapter>(Rc<RefCell<SharedRuntime<E>>>);
impl<E: EngineAdapter> Clone for SharedEngine<E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<E: EngineAdapter> SharedEngine<E> {
    pub fn new(engine: E) -> Self {
        Self(Rc::new(RefCell::new(SharedRuntime {
            engine,
            frames: Vec::new(),
            termination: None,
        })))
    }
}
impl<E: EngineAdapter> EngineAdapter for SharedEngine<E> {
    fn create_device(&mut self, config: &LedConfiguration) -> anyhow::Result<u32> {
        self.0.borrow_mut().engine.create_device(config)
    }
    fn create_engine(&mut self) -> anyhow::Result<u32> {
        self.0.borrow_mut().engine.create_engine()
    }
    fn add_device(&mut self, engine: u32, device: u32) -> anyhow::Result<()> {
        self.0.borrow_mut().engine.add_device(engine, device)
    }
    fn add_effect(&mut self, engine: u32, effect: &EngineEffect) -> anyhow::Result<u32> {
        self.0.borrow_mut().engine.add_effect(engine, effect)
    }
    fn remove_effect(&mut self, engine: u32, effect: u32) -> anyhow::Result<()> {
        self.0.borrow_mut().engine.remove_effect(engine, effect)
    }
    fn enable(&mut self, engine: u32, enable: bool) -> anyhow::Result<()> {
        self.0.borrow_mut().engine.enable(engine, enable)
    }
    fn input(&mut self, device: u32) -> anyhow::Result<()> {
        self.0.borrow_mut().engine.input(device)
    }
    fn frames(&mut self) -> anyhow::Result<Vec<EngineFrame>> {
        let mut runtime = self.0.borrow_mut();
        let frames = runtime.engine.frames()?;
        runtime.frames.extend(frames);
        Ok(std::mem::take(&mut runtime.frames))
    }
    fn screen_status(
        &self,
    ) -> Option<razer_device::lighting_engine::renderer::ScreenCaptureStatus> {
        self.0.borrow().engine.screen_status()
    }
    fn discard_frames_for(&mut self, device: u32) -> anyhow::Result<()> {
        let mut runtime = self.0.borrow_mut();
        let frames = runtime.engine.frames()?;
        runtime.frames.extend(frames);
        runtime.frames.retain(|frame| frame.device_handle != device);
        Ok(())
    }
    fn destroy_engine(&mut self, engine: u32) -> anyhow::Result<()> {
        self.0.borrow_mut().engine.destroy_engine(engine)
    }
    fn destroy_device(&mut self, device: u32) -> anyhow::Result<()> {
        self.0.borrow_mut().engine.destroy_device(device)
    }
    fn terminate(&mut self) -> anyhow::Result<()> {
        let mut runtime = self.0.borrow_mut();
        if runtime.termination.is_none() {
            runtime.termination = Some(
                runtime
                    .engine
                    .terminate()
                    .map_err(|error| format!("{error:#}")),
            );
        }
        runtime
            .termination
            .as_ref()
            .unwrap()
            .clone()
            .map_err(anyhow::Error::msg)
    }
}

/// One physical device has independent normal and charging engines sharing
/// the device handle. The parent owns runtime termination.
pub(in crate::runtime) struct DeviceEngines<E: EngineAdapter> {
    runtime: SharedEngine<E>,
    config: LedConfiguration,
    pub device_handle: Option<u32>,
    normal: Option<Session<SharedEngine<E>>>,
    special: Option<Session<SharedEngine<E>>>,
    special_effect: Option<EngineEffect>,
    cleanup_receipt: Option<Vec<String>>,
}
impl<E: EngineAdapter> DeviceEngines<E> {
    pub fn new(runtime: SharedEngine<E>, config: LedConfiguration) -> Self {
        Self {
            runtime,
            config,
            device_handle: None,
            normal: None,
            special: None,
            special_effect: None,
            cleanup_receipt: None,
        }
    }
    fn device(&mut self) -> anyhow::Result<u32> {
        if let Some(device) = self.device_handle {
            return Ok(device);
        }
        if let Some(errors) = &self.cleanup_receipt {
            anyhow::ensure!(
                errors.is_empty(),
                "Previous device cleanup failed: {}",
                errors.join("; ")
            );
        }
        self.cleanup_receipt = None;
        let device = self.runtime.create_device(&self.config)?;
        anyhow::ensure!(device != 0, "Lighting device handle is null");
        self.device_handle = Some(device);
        Ok(device)
    }
    pub fn apply_normal(&mut self, effect: &EngineEffect) -> anyhow::Result<()> {
        let device = self.device()?;
        if let Some(normal) = self.normal.as_mut() {
            normal.update_effect(effect)?;
        } else {
            self.normal = Some(Session::with_shared_device(self.runtime.clone(), device)?);
            self.normal.as_mut().unwrap().initialize_shared(effect)?;
        }
        if self.special.is_some() {
            self.normal.as_mut().unwrap().suspend(true)?;
        }
        Ok(())
    }
    pub fn apply_special(&mut self, effect: &EngineEffect) -> anyhow::Result<()> {
        if self.special_effect.as_ref() == Some(effect) {
            return Ok(());
        }
        let device = self.device()?;
        if let Some(normal) = self.normal.as_mut() {
            normal.suspend(true)?;
        }
        if let Some(special) = self.special.as_mut() {
            special.update_effect(effect)?;
        } else {
            self.special = Some(Session::with_shared_device(self.runtime.clone(), device)?);
            self.special.as_mut().unwrap().initialize_shared(effect)?;
        }
        self.special_effect = Some(effect.clone());
        Ok(())
    }
    pub fn is_special(&self) -> bool {
        self.special.is_some()
    }
    pub fn remove_special(&mut self, normal_effect: Option<&EngineEffect>) -> anyhow::Result<()> {
        if self.special.is_none() {
            return Ok(());
        }
        let errors = self.cleanup();
        anyhow::ensure!(
            errors.is_empty(),
            "Charging engine restoration failed: {}",
            errors.join("; ")
        );
        if let Some(effect) = normal_effect {
            self.apply_normal(effect)?;
        }
        Ok(())
    }
    pub fn cleanup(&mut self) -> Vec<String> {
        if let Some(errors) = &self.cleanup_receipt {
            return errors.clone();
        }
        let mut errors = Vec::new();
        if let Some(mut special) = self.special.take() {
            errors.extend(special.cleanup());
        }
        if let Some(mut normal) = self.normal.take() {
            errors.extend(normal.cleanup());
        }
        self.special_effect = None;
        if let Some(device) = self.device_handle.take() {
            if let Err(error) = self.runtime.discard_frames_for(device) {
                errors.push(format!("Discard device frames: {error:#}"));
            }
            if let Err(error) = self.runtime.destroy_device(device) {
                errors.push(format!("Destroy device: {error:#}"));
            }
        }
        self.cleanup_receipt = Some(errors.clone());
        errors
    }
}
impl<E: EngineAdapter> Drop for DeviceEngines<E> {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

pub(in crate::runtime) struct Session<E: EngineAdapter> {
    pub engine: E,
    pub device_handle: Option<u32>,
    engine_handle: Option<u32>,
    effect_handle: Option<u32>,
    pub suspended: bool,
    terminated: bool,
    owns_device: bool,
    owns_runtime: bool,
    cleanup_receipt: Option<Vec<String>>,
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
            owns_device: true,
            owns_runtime: true,
            cleanup_receipt: None,
        }
    }
    pub fn with_shared_device(engine: E, device: u32) -> anyhow::Result<Self> {
        anyhow::ensure!(device != 0, "Shared lighting device handle is null");
        Ok(Self {
            engine,
            device_handle: Some(device),
            engine_handle: None,
            effect_handle: None,
            suspended: false,
            terminated: false,
            owns_device: false,
            owns_runtime: false,
            cleanup_receipt: None,
        })
    }

    pub fn initialize_shared(&mut self, effect: &EngineEffect) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.owns_device && self.engine_handle.is_none(),
            "Shared engine is already initialized or owns its device"
        );
        let device = self
            .device_handle
            .ok_or_else(|| anyhow::anyhow!("Shared lighting device handle is absent"))?;
        let engine = self.engine.create_engine()?;
        anyhow::ensure!(engine != 0, "Lighting engine returned a null handle");
        self.engine_handle = Some(engine);
        self.engine.add_device(engine, device)?;
        self.update_effect(effect)
    }
    pub fn initialize(
        &mut self,
        config: &LedConfiguration,
        effect: &SoftwareEffect,
    ) -> anyhow::Result<()> {
        self.initialize_effect(config, &effect.engine_effect())
    }
    pub fn initialize_effect(
        &mut self,
        config: &LedConfiguration,
        effect: &EngineEffect,
    ) -> anyhow::Result<()> {
        let engine = self.engine.create_engine()?;
        anyhow::ensure!(engine != 0, "Lighting engine returned a null handle");
        self.engine_handle = Some(engine);
        let device = self.engine.create_device(config)?;
        anyhow::ensure!(device != 0, "Lighting device returned a null handle");
        self.device_handle = Some(device);
        self.engine
            .add_device(self.engine_handle.unwrap(), device)?;
        self.update_effect(effect)
    }
    pub fn update(&mut self, effect: &SoftwareEffect) -> anyhow::Result<()> {
        self.update_effect(&effect.engine_effect())
    }
    pub fn update_effect(&mut self, effect: &EngineEffect) -> anyhow::Result<()> {
        let engine = self
            .engine_handle
            .ok_or_else(|| anyhow::anyhow!("Lighting engine has not been created"))?;
        if let Some(effect) = self.effect_handle.take() {
            self.engine.remove_effect(engine, effect)?;
        }
        // Remove queued frames from the previous effect before the generation
        // becomes active. They are observations of a superseded effect.
        if let Some(device) = self.device_handle {
            self.engine.discard_frames_for(device)?;
        }
        let effect = self.engine.add_effect(engine, effect)?;
        anyhow::ensure!(effect != 0, "Lighting effect returned a null handle");
        self.effect_handle = Some(effect);
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
        if let Some(device) = self.device_handle {
            self.engine.discard_frames_for(device)?;
        }
        Ok(())
    }
    pub fn input(&mut self) -> anyhow::Result<()> {
        let device = self
            .device_handle
            .ok_or_else(|| anyhow::anyhow!("Lighting device has not been created"))?;
        self.engine.input(device)
    }
    pub fn cleanup(&mut self) -> Vec<String> {
        if let Some(errors) = &self.cleanup_receipt {
            return errors.clone();
        }
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
        if self.owns_device {
            if let Some(device) = self.device_handle.take() {
                if let Err(e) = self.engine.destroy_device(device) {
                    errors.push(format!("Destroy device: {e:#}"));
                }
            }
        } else {
            self.device_handle = None;
        }
        if self.owns_runtime && !self.terminated {
            self.terminated = true;
            if let Err(e) = self.engine.terminate() {
                errors.push(format!("Terminate: {e:#}"));
            }
        }
        self.cleanup_receipt = Some(errors.clone());
        errors
    }
}
impl<E: EngineAdapter> Drop for Session<E> {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

#[cfg(windows)]
impl EngineAdapter for super::lighting_engine::Engine {
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
    fn screen_status(
        &self,
    ) -> Option<razer_device::lighting_engine::renderer::ScreenCaptureStatus> {
        Self::screen_status(self)
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
