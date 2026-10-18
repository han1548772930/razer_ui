//! Source engine 0x4c8f0/0x4cfd0 and container 0x521e0/0x52600/0x52df0.
//! One clock-driven owner holds devices and effect instances; its output stays
//! a software frame until the caller's HID transport acknowledgement.
use super::{FRAME_MILLIS, Renderer};
use crate::lighting_engine::{EngineEffect, EngineFrame, LedConfiguration, LedInputType};
use anyhow::{Context as _, ensure};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fade {
    In,
    Stable,
    Out,
    Remove,
    Paused,
}
struct Effect {
    id: u32,
    renderer: Renderer,
    fade: Fade,
    elapsed: u32,
    opacity: f32,
}
impl Effect {
    fn advance(&mut self) {
        match self.fade {
            Fade::In => {
                if self.elapsed != 0 {
                    self.opacity = (self.opacity + FRAME_MILLIS as f32 / 3000.).min(1.);
                }
                self.elapsed = (self.elapsed + FRAME_MILLIS) % 3000;
                if self.elapsed == 0 {
                    self.fade = Fade::Stable;
                    self.opacity = 1.;
                }
            }
            Fade::Stable => self.opacity = 1.,
            Fade::Out => {
                self.opacity = (self.opacity - FRAME_MILLIS as f32 / 3000.).max(0.);
                self.elapsed = (self.elapsed + FRAME_MILLIS) % 3000;
                if self.elapsed == 0 {
                    self.fade = Fade::Remove;
                    self.opacity = 0.;
                }
            }
            Fade::Paused => self.opacity = 0.,
            Fade::Remove => {}
        }
    }
}
struct Device {
    config: LedConfiguration,
}
struct Instance {
    device: Option<u32>,
    effects: Vec<Effect>,
    next_tick: Option<u64>,
    previous: Vec<[u8; 3]>,
    keep_alive: u32,
    ticks: u64,
}

/// The caller supplies monotonic milliseconds and actual system meter
/// observations. Handles are never reused within this owner.
pub struct Engine {
    devices: BTreeMap<u32, Device>,
    engines: BTreeMap<u32, Instance>,
    next_handle: u32,
    random_seed: u32,
    epoch_seconds: u64,
    stopped: bool,
}
impl Engine {
    pub fn new(epoch_seconds: u64) -> Self {
        Self {
            devices: BTreeMap::new(),
            engines: BTreeMap::new(),
            next_handle: 1,
            random_seed: 1,
            epoch_seconds,
            stopped: false,
        }
    }
    fn handle(&mut self) -> anyhow::Result<u32> {
        ensure!(!self.stopped, "Lighting runtime has terminated");
        let id = self.next_handle;
        self.next_handle = id
            .checked_add(1)
            .context("Lighting handle space exhausted")?;
        Ok(id)
    }
    pub fn create_device(&mut self, config: &LedConfiguration) -> anyhow::Result<u32> {
        ensure!(
            config.matrix_max_row > 0
                && config.matrix_max_col > 0
                && config.device_max_row > 0
                && config.device_max_col > 0,
            "Lighting device dimensions are empty"
        );
        ensure!(
            !config.led_matrix.is_empty(),
            "Lighting device has no LED mapping"
        );
        for map in &config.led_matrix {
            ensure!(
                map.matrix_pos[0] < config.matrix_max_row
                    && map.matrix_pos[1] < config.matrix_max_col
                    && map.device_pos[0] < config.device_max_row
                    && map.device_pos[1] < config.device_max_col,
                "Lighting LED mapping is outside its matrix"
            );
        }
        let id = self.handle()?;
        self.devices.insert(
            id,
            Device {
                config: config.clone(),
            },
        );
        Ok(id)
    }
    pub fn create_engine(&mut self) -> anyhow::Result<u32> {
        let id = self.handle()?;
        self.engines.insert(
            id,
            Instance {
                device: None,
                effects: Vec::new(),
                next_tick: None,
                previous: Vec::new(),
                keep_alive: 25,
                ticks: 0,
            },
        );
        Ok(id)
    }
    pub fn add_device(&mut self, engine: u32, device: u32) -> anyhow::Result<()> {
        let config = &self
            .devices
            .get(&device)
            .context("Unknown lighting device")?
            .config;
        let instance = self
            .engines
            .get_mut(&engine)
            .context("Unknown lighting engine")?;
        ensure!(
            instance.next_tick.is_none() && instance.device.is_none(),
            "Lighting engine already has a device"
        );
        instance.previous =
            vec![[0; 3]; usize::from(config.matrix_max_row) * usize::from(config.matrix_max_col)];
        instance.device = Some(device);
        Ok(())
    }
    pub fn add_effect(&mut self, engine: u32, effect: &EngineEffect) -> anyhow::Result<u32> {
        self.add_effect_at(engine, effect, self.epoch_seconds)
    }
    pub fn add_effect_at(
        &mut self,
        engine: u32,
        effect: &EngineEffect,
        epoch_seconds: u64,
    ) -> anyhow::Result<u32> {
        let instance = self
            .engines
            .get(&engine)
            .context("Unknown lighting engine")?;
        let device = instance.device.context("Lighting engine has no device")?;
        let config = &self
            .devices
            .get(&device)
            .context("Lighting device was destroyed")?
            .config;
        let mut source = effect.clone();
        // 0x67830 converts the source's 40-unit center into the engine grid.
        if source.effect == 19 {
            let width = if config.led_width == 0 {
                40
            } else {
                u32::from(config.led_width)
            };
            let height = if config.led_height == 0 {
                40
            } else {
                u32::from(config.led_height)
            };
            if let Some(value) = source.parameters.center_col.as_mut() {
                *value = 40 * *value / width;
            }
            if let Some(value) = source.parameters.center_row.as_mut() {
                *value = 40 * *value / height;
            }
        }
        let renderer = Renderer::new(
            &source,
            usize::from(config.matrix_max_row),
            usize::from(config.matrix_max_col),
            epoch_seconds,
            &mut self.random_seed,
        )?;
        let id = self.handle()?;
        self.engines.get_mut(&engine).unwrap().effects.push(Effect {
            id,
            renderer,
            fade: Fade::In,
            elapsed: 0,
            opacity: 0.,
        });
        Ok(id)
    }
    pub fn remove_effect(&mut self, engine: u32, effect: u32) -> anyhow::Result<()> {
        let instance = self
            .engines
            .get_mut(&engine)
            .context("Unknown lighting engine")?;
        let value = instance
            .effects
            .iter_mut()
            .find(|v| v.id == effect)
            .context("Unknown lighting effect")?;
        if instance.next_tick.is_some() {
            value.fade = Fade::Out;
            value.elapsed = 0;
        } else {
            instance.effects.retain(|v| v.id != effect);
        }
        Ok(())
    }
    pub fn enable(&mut self, engine: u32, enabled: bool, now: u64) -> anyhow::Result<()> {
        let instance = self
            .engines
            .get_mut(&engine)
            .context("Unknown lighting engine")?;
        ensure!(instance.device.is_some(), "Lighting engine has no device");
        if instance.next_tick.is_some() == enabled {
            return Ok(());
        }
        for effect in &mut instance.effects {
            if !matches!(effect.fade, Fade::Out | Fade::Remove) {
                effect.fade = if enabled { Fade::In } else { Fade::Paused };
                effect.elapsed = 0;
                if enabled {
                    effect.opacity = 0.;
                }
            }
            effect.renderer.reset()?;
            effect.advance();
        }
        instance.effects.retain(|v| v.fade != Fade::Remove);
        instance.next_tick = enabled.then_some(now);
        instance.previous.fill([0; 3]);
        instance.keep_alive = 25;
        instance.ticks = 0;
        Ok(())
    }
    /// JSON mouse id=1, data=0 maps to region 0 when InputMap is empty
    /// (0x7710 -> 0x4f940 -> 0x4c170). An explicit map restricts the LEDs.
    pub fn input(&mut self, device: u32) -> anyhow::Result<()> {
        let config = &self
            .devices
            .get(&device)
            .context("Unknown lighting input device")?
            .config;
        let map = config
            .led_input_map
            .iter()
            .find(|m| m.input_type == LedInputType::Mouse && m.input_data == [1, 0]);
        let positions = if config.led_input_map.is_empty()
            || map.is_some_and(|m| m.matrix_pos.contains(&u16::MAX))
        {
            config
                .led_matrix
                .iter()
                .map(|m| {
                    usize::from(m.matrix_pos[0]) * usize::from(config.matrix_max_col)
                        + usize::from(m.matrix_pos[1])
                })
                .collect::<Vec<_>>()
        } else if let Some(map) = map {
            vec![
                usize::from(map.matrix_pos[0]) * usize::from(config.matrix_max_col)
                    + usize::from(map.matrix_pos[1]),
            ]
        } else {
            return Ok(());
        };
        for instance in self
            .engines
            .values_mut()
            .filter(|v| v.device == Some(device) && v.next_tick.is_some())
        {
            for effect in &mut instance.effects {
                effect.renderer.input(&positions);
            }
        }
        Ok(())
    }
    pub fn audio_flows(&self) -> [bool; 2] {
        let mut flows = [false; 2];
        for instance in self.engines.values().filter(|v| v.next_tick.is_some()) {
            for effect in &instance.effects {
                if let Some(flow) = effect.renderer.audio_flow() {
                    flows[usize::from(flow)] = true;
                }
            }
        }
        flows
    }
    pub fn frames(
        &mut self,
        now: u64,
        audio: [Option<f32>; 2],
    ) -> anyhow::Result<Vec<EngineFrame>> {
        let mut frames = Vec::new();
        for instance in self.engines.values_mut() {
            if !instance.next_tick.is_some_and(|at| now >= at) {
                continue;
            }
            instance.next_tick = Some(now.saturating_add(u64::from(FRAME_MILLIS)));
            instance.effects.retain(|v| v.fade != Fade::Remove);
            let device = instance
                .device
                .context("Running lighting engine has no device")?;
            let config = &self
                .devices
                .get(&device)
                .context("Running lighting device was destroyed")?
                .config;
            let mut matrix = vec![[0; 3]; instance.previous.len()];
            for effect in &mut instance.effects {
                effect.advance();
                let peak = effect
                    .renderer
                    .audio_flow()
                    .and_then(|flow| audio[usize::from(flow)]);
                effect.renderer.render(&mut matrix, effect.opacity, peak)?;
            }
            let changed = matrix != instance.previous;
            let emit = if changed {
                instance.keep_alive = 25;
                true
            } else if instance.keep_alive > 0 {
                instance.keep_alive -= 1;
                true
            } else {
                instance.ticks.is_multiple_of(125)
            };
            instance.ticks = instance.ticks.wrapping_add(1);
            if !emit {
                continue;
            }
            instance.previous = matrix;
            let mut frame =
                vec![
                    0;
                    usize::from(config.device_max_row) * usize::from(config.device_max_col) * 3
                ];
            for map in &config.led_matrix {
                let source = usize::from(map.matrix_pos[0]) * usize::from(config.matrix_max_col)
                    + usize::from(map.matrix_pos[1]);
                let target = usize::from(map.device_pos[0]) * usize::from(config.device_max_col)
                    + usize::from(map.device_pos[1]);
                frame[target * 3..target * 3 + 3].copy_from_slice(&instance.previous[source]);
            }
            frames.push(EngineFrame {
                device_handle: device,
                region_id: 0,
                num_row: config.device_max_row,
                num_col: config.device_max_col,
                frame,
            });
        }
        Ok(frames)
    }
    pub fn destroy_engine(&mut self, engine: u32) -> anyhow::Result<()> {
        self.engines
            .remove(&engine)
            .context("Unknown lighting engine")?;
        Ok(())
    }
    pub fn destroy_device(&mut self, device: u32) -> anyhow::Result<()> {
        ensure!(
            !self.engines.values().any(|v| v.device == Some(device)),
            "Lighting device still belongs to an engine"
        );
        self.devices
            .remove(&device)
            .context("Unknown lighting device")?;
        Ok(())
    }
    pub fn terminate(&mut self) {
        self.engines.clear();
        self.devices.clear();
        self.stopped = true;
    }
}
