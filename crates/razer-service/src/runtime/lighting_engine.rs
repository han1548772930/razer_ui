//! The lighting worker owns the Rust effect engine, Ambient screen capture
//! and Audio VU endpoint meter. Device writes belong to the caller's HID owner.
use anyhow::Context as _;
use razer_device::lighting_engine::{EngineEffect, EngineFrame, LedConfiguration, renderer};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[path = "lighting_audio_meter.rs"]
mod audio;
#[path = "windows/ambient_capture.rs"]
mod screen;

pub(in crate::runtime) struct Engine {
    renderer: renderer::Engine,
    clock: Instant,
    audio: Option<audio::Meter>,
    screen: Option<screen::Capture>,
    final_screen_status: Option<renderer::ScreenCaptureStatus>,
}
impl Engine {
    pub(in crate::runtime) fn open() -> anyhow::Result<Self> {
        Ok(Self {
            renderer: renderer::Engine::new(
                SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            ),
            clock: Instant::now(),
            audio: None,
            screen: None,
            final_screen_status: None,
        })
    }
    pub(in crate::runtime) fn create_device(
        &mut self,
        config: &LedConfiguration,
    ) -> anyhow::Result<u32> {
        self.renderer.create_device(config)
    }
    pub(in crate::runtime) fn create_engine(&mut self) -> anyhow::Result<u32> {
        self.renderer.create_engine()
    }
    pub(in crate::runtime) fn add_device(
        &mut self,
        engine: u32,
        device: u32,
    ) -> anyhow::Result<()> {
        self.renderer.add_device(engine, device)
    }
    pub(in crate::runtime) fn add_effect(
        &mut self,
        engine: u32,
        effect: &EngineEffect,
    ) -> anyhow::Result<u32> {
        if effect.effect == 12 {
            if self.audio.is_none() {
                self.audio = Some(audio::Meter::open()?);
            }
            let mut flows = self.renderer.audio_flows();
            flows[usize::from(effect.parameters.flow == Some(1))] = true;
            self.audio.as_mut().unwrap().peaks(flows)?;
        }
        if effect.effect == 11 && self.screen.is_none() {
            self.screen = Some(screen::Capture::open()?);
            self.final_screen_status = None;
        }
        let result = self.renderer.add_effect_at(
            engine,
            effect,
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        );
        if let Err(error) = self.release_unused_screen() {
            return match result {
                Ok(_) => Err(error),
                Err(add) => Err(add).context(format!(
                    "Release unused Ambient capture also failed: {error:#}"
                )),
            };
        }
        result
    }
    pub(in crate::runtime) fn remove_effect(
        &mut self,
        engine: u32,
        effect: u32,
    ) -> anyhow::Result<()> {
        self.renderer.remove_effect(engine, effect)?;
        self.release_unused_screen()
    }
    pub(in crate::runtime) fn enable(&mut self, engine: u32, enabled: bool) -> anyhow::Result<()> {
        self.renderer.enable(
            engine,
            enabled,
            self.clock.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        )?;
        self.release_unused_screen()
    }
    pub(in crate::runtime) fn input(&mut self, device: u32) -> anyhow::Result<()> {
        self.renderer.input(device)
    }
    pub(in crate::runtime) fn frames(&mut self) -> anyhow::Result<Vec<EngineFrame>> {
        let now = self.clock.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        let flows = self.renderer.audio_flows();
        let peaks = if flows.iter().any(|flow| *flow) {
            if self.audio.is_none() {
                self.audio = Some(audio::Meter::open()?);
            }
            self.audio.as_mut().unwrap().peaks(flows)?
        } else {
            if let Some(mut audio) = self.audio.take() {
                audio.close()?;
            }
            [None, None]
        };
        let requests = self.renderer.screen_requests(now);
        let observations = if let Some(screen) = self.screen.as_mut() {
            screen.retain_effects(&self.renderer.ambient_effect_handles());
            screen.observe(&requests, now)
        } else {
            std::collections::BTreeMap::new()
        };
        let frames = self
            .renderer
            .frames_with_screen(now, peaks, &observations)?;
        self.release_unused_screen()?;
        Ok(frames)
    }
    pub(in crate::runtime) fn screen_status(&self) -> Option<renderer::ScreenCaptureStatus> {
        self.screen
            .as_ref()
            .map(screen::Capture::status)
            .or_else(|| self.final_screen_status.clone())
    }
    fn release_unused_screen(&mut self) -> anyhow::Result<()> {
        if let Some(screen) = self.screen.as_mut() {
            screen.retain_effects(&self.renderer.ambient_effect_handles());
        }
        if !self.renderer.has_ambient() {
            if let Some(screen) = self.screen.as_mut() {
                screen.close()?;
                self.final_screen_status = Some(screen.status());
                self.screen = None;
            }
        }
        Ok(())
    }
    pub(in crate::runtime) fn destroy_engine(&mut self, engine: u32) -> anyhow::Result<()> {
        self.renderer.destroy_engine(engine)?;
        self.release_unused_screen()
    }
    pub(in crate::runtime) fn destroy_device(&mut self, device: u32) -> anyhow::Result<()> {
        self.renderer.destroy_device(device)
    }
    pub(in crate::runtime) fn terminate(&mut self) -> anyhow::Result<()> {
        self.renderer.terminate();
        let mut errors = Vec::new();
        if let Some(mut audio) = self.audio.take() {
            if let Err(error) = audio.close() {
                errors.push(format!("Release lighting audio: {error:#}"));
            }
        }
        if let Err(error) = self.release_unused_screen() {
            errors.push(format!("Release Ambient capture: {error:#}"));
        }
        anyhow::ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}
