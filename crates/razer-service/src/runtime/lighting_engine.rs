//! The lighting worker owns the Rust effect engine and, only for Audio VU,
//! the system endpoint peak meter. Device writes belong to the caller's HID owner.
use razer_device::lighting_engine::{EngineEffect, EngineFrame, LedConfiguration, renderer};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[path = "lighting_audio_meter.rs"]
mod audio;

pub(in crate::runtime) struct Engine {
    renderer: renderer::Engine,
    clock: Instant,
    audio: Option<audio::Meter>,
}
impl Engine {
    pub(in crate::runtime) fn open() -> anyhow::Result<Self> {
        Ok(Self {
            renderer: renderer::Engine::new(
                SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            ),
            clock: Instant::now(),
            audio: None,
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
        self.renderer.add_effect_at(
            engine,
            effect,
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        )
    }
    pub(in crate::runtime) fn remove_effect(
        &mut self,
        engine: u32,
        effect: u32,
    ) -> anyhow::Result<()> {
        self.renderer.remove_effect(engine, effect)
    }
    pub(in crate::runtime) fn enable(&mut self, engine: u32, enabled: bool) -> anyhow::Result<()> {
        self.renderer.enable(
            engine,
            enabled,
            self.clock.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        )
    }
    pub(in crate::runtime) fn input(&mut self, device: u32) -> anyhow::Result<()> {
        self.renderer.input(device)
    }
    pub(in crate::runtime) fn frames(&mut self) -> anyhow::Result<Vec<EngineFrame>> {
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
        self.renderer.frames(
            self.clock.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            peaks,
        )
    }
    pub(in crate::runtime) fn destroy_engine(&mut self, engine: u32) -> anyhow::Result<()> {
        self.renderer.destroy_engine(engine)
    }
    pub(in crate::runtime) fn destroy_device(&mut self, device: u32) -> anyhow::Result<()> {
        self.renderer.destroy_device(device)
    }
    pub(in crate::runtime) fn terminate(&mut self) -> anyhow::Result<()> {
        self.renderer.terminate();
        if let Some(mut audio) = self.audio.take() {
            audio.close()?;
        }
        Ok(())
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}
