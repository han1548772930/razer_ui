//! Local effect rendering from the current 4.0.55.0 engine's effect vtables.
//! Time, randomness and audio observations enter explicitly; this module does
//! not load a native library or perform device, platform or filesystem I/O.
use super::{ColorStop, EngineEffect};
use anyhow::{Context as _, ensure};
use std::collections::BTreeMap;

mod engine;
mod spatial;
mod stochastic;
pub use engine::Engine;

#[cfg(test)]
mod tests;

pub const FRAME_MILLIS: u32 = 40;
const TRANSPARENT: u32 = 0x6400_0000;

/// Native COLORREF channels and the source transparency percentage.
type Pixel = [u8; 4];
fn pixel(color: u32) -> Pixel {
    color.to_le_bytes()
}
fn packed(color: Pixel) -> u32 {
    u32::from_le_bytes(color)
}

/// 0x692e0 adjusts colliding segment boundaries, includes both endpoints of
/// each segment, and accumulates f32 increments before truncating channels.
fn gradient(source: &[ColorStop], count: usize) -> anyhow::Result<Vec<Pixel>> {
    ensure!(
        count > 0 && count <= 1_000_000,
        "Lighting gradient length is invalid"
    );
    ensure!(
        source.len() >= 2 && source.len() <= 10,
        "Lighting color stop count is invalid"
    );
    ensure!(
        source[0].stop == 0
            && source.last().unwrap().stop == 100
            && source.windows(2).all(|pair| pair[0].stop < pair[1].stop),
        "Lighting color stops are unordered"
    );
    let mut stops = source
        .iter()
        .map(|s| usize::from(s.stop))
        .collect::<Vec<_>>();
    for index in 0..stops.len() - 1 {
        let start = count * stops[index] / 100;
        if stops[index] == 100 {
            break;
        }
        if start >= count * stops[index + 1] / 100 {
            stops[index + 1] = (((start + 1) as f64 * 100. / count as f64 + 1.) as usize).min(100);
        }
    }
    let mut output = vec![[0; 4]; count];
    for index in 0..stops.len() - 1 {
        if stops[index] == 100 {
            break;
        }
        let start = count * stops[index] / 100;
        let end = count * stops[index + 1] / 100;
        if start >= count {
            break;
        }
        output[start] = pixel(source[index].color);
        if start + 1 < end {
            output[end - 1] = pixel(source[index + 1].color);
            let first = output[start];
            let last = output[end - 1];
            let step: [f32; 4] = std::array::from_fn(|c| {
                (f32::from(last[c]) - f32::from(first[c])) / (end - 1 - start) as f32
            });
            let mut current = first.map(f32::from);
            for target in &mut output[start + 1..end - 1] {
                for channel in 0..4 {
                    current[channel] += step[channel];
                    target[channel] = current[channel] as u8;
                }
            }
            if stops[index + 1] == 100 {
                output[0] = pixel(source[0].color);
                output[count - 1] = pixel(source[index + 1].color);
                break;
            }
        }
    }
    Ok(output)
}

#[derive(Clone, Debug)]
struct Random(u32);
impl Random {
    fn new(epoch_seconds: u64) -> Self {
        Self(((epoch_seconds / 1_800_000 * 1_800_000) | 31) as u32)
    }
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(214013).wrapping_add(2531011);
        self.0 ^ (self.0 >> 16)
    }
    fn color(&mut self) -> u32 {
        loop {
            let channels: [u8; 3] = std::array::from_fn(|_| match self.next() % 255 {
                0..=85 => 0,
                86..=170 => 128,
                _ => 255,
            });
            if channels[0] != channels[1] || channels[1] != channels[2] {
                return packed([channels[0], channels[1], channels[2], 0]);
            }
        }
    }
}

#[derive(Clone, Debug)]
struct Sequence {
    values: Vec<Pixel>,
    cursor: usize,
    remaining: i32,
    active: bool,
}
impl Sequence {
    fn new(values: Vec<Pixel>, remaining: i32) -> Self {
        Self {
            values,
            cursor: 0,
            remaining,
            active: true,
        }
    }
    fn next(&mut self) -> Option<Pixel> {
        if self.remaining == 0 {
            self.active = false;
            return None;
        }
        let result = self.values[self.cursor];
        self.cursor = (self.cursor + 1) % self.values.len();
        if self.cursor == 0 && self.remaining != -1 {
            self.remaining -= 1;
        }
        Some(result)
    }
}

#[derive(Clone, Debug)]
enum Effect {
    Static(Pixel),
    Uniform {
        sequence: Sequence,
        random_stops: Option<Vec<ColorStop>>,
        random: Random,
    },
    Reactive {
        values: Vec<Pixel>,
        cycles: i32,
        active: BTreeMap<usize, Sequence>,
    },
    Wave(spatial::Wave),
    Stars(stochastic::Stars),
    Fire(stochastic::Fire),
    Audio {
        values: Vec<Pixel>,
        peak: u32,
        maximum: u32,
        decay: f32,
        boost: f32,
        flow: bool,
    },
    Tidal(spatial::Tidal),
}

/// One effect instance advances once per engine tick. RGB output is composited
/// separately so transparency and reactive's non-black blend mode stay intact.
#[derive(Clone, Debug)]
pub struct Renderer {
    rows: usize,
    columns: usize,
    mode: u32,
    effect: Effect,
}
impl Renderer {
    pub fn new(
        source: &EngineEffect,
        rows: usize,
        columns: usize,
        epoch_seconds: u64,
        random_seed: &mut u32,
    ) -> anyhow::Result<Self> {
        ensure!(
            rows > 0
                && columns > 0
                && rows
                    .checked_mul(columns)
                    .is_some_and(|count| count <= 65536),
            "Lighting matrix dimensions are invalid"
        );
        let p = &source.parameters;
        let mut mode = p.mode.unwrap_or(0);
        ensure!(
            match source.effect {
                2 => matches!(mode, 0 | 2 | 1024 | 2048),
                3 => mode == 4128,
                4 | 6 | 8 | 12 => mode == 0,
                5 | 7 => matches!(mode, 0 | 1),
                19 => matches!(mode, 2 | 3),
                _ => false,
            },
            "Lighting mode is outside the current mounted effect contract"
        );
        ensure!(
            p.speed
                .is_none_or(|v| v.is_finite() && (0.0..=100.0).contains(&v)),
            "Lighting speed is invalid"
        );
        ensure!(cycles_valid(p.cycles), "Lighting cycle count is invalid");
        let mut random = Random::new(epoch_seconds);
        let cycles = p.cycles.unwrap_or(0);
        let stops = || {
            p.color_stops
                .as_deref()
                .context("Lighting effect omitted its color stops")
        };
        let effect = match source.effect {
            6 => Effect::Static(pixel(p.color.context("Static lighting omitted its color")?)),
            4 | 5 => {
                let mut colors = stops()?.to_vec();
                let random_stops = if source.effect == 5 && mode & 1 != 0 {
                    colors = vec![
                        ColorStop { stop: 0, color: 0 },
                        ColorStop { stop: 25, color: 0 },
                        ColorStop {
                            stop: 50,
                            color: 0xffffff,
                        },
                        ColorStop { stop: 75, color: 0 },
                        ColorStop {
                            stop: 100,
                            color: 0,
                        },
                    ];
                    Some(colors.clone())
                } else {
                    None
                };
                let duration = p
                    .duration
                    .filter(|value| *value != 0)
                    .unwrap_or(if source.effect == 4 { 37740 } else { 14000 });
                let count = (duration / FRAME_MILLIS) as usize;
                let values = gradient(&colors, count.max(colors.len()))?;
                Effect::Uniform {
                    sequence: Sequence::new(
                        values,
                        if source.effect == 5 && cycles == 0 {
                            -1
                        } else {
                            cycles
                        },
                    ),
                    random_stops,
                    random,
                }
            }
            3 => {
                let colors = stops()?;
                let count =
                    (p.duration.filter(|v| *v != 0).unwrap_or(14000) / FRAME_MILLIS) as usize;
                Effect::Reactive {
                    values: gradient(colors, count.max(colors.len()))?,
                    cycles,
                    active: BTreeMap::new(),
                }
            }
            2 => Effect::Wave(spatial::Wave::new(p, rows, columns)?),
            7 => Effect::Stars(stochastic::Stars::new(p, rows * columns, epoch_seconds)?),
            8 => {
                mode = p.mode.unwrap_or(0);
                Effect::Fire(stochastic::Fire::new(p, rows, columns, random_seed)?)
            }
            12 => {
                ensure!(
                    p.auto_boost.is_none_or(|v| v == 0)
                        && p.automatic.is_none_or(|v| v == 0)
                        && p.flow.is_none_or(|v| v <= 1),
                    "Audio lighting options are outside the mounted source contract"
                );
                let boost = p.boost_level.unwrap_or(0.0) as f32;
                ensure!(
                    boost.is_finite() && boost >= 0.,
                    "Audio lighting boost is invalid"
                );
                let decay = p.decay.filter(|value| *value != 0).unwrap_or(1000) / FRAME_MILLIS;
                ensure!(decay > 0, "Audio lighting decay is too short");
                Effect::Audio {
                    values: gradient(stops()?, 1025)?,
                    peak: 0,
                    maximum: 1024,
                    decay: 1024. / decay as f32,
                    boost,
                    flow: p.flow == Some(1),
                }
            }
            19 => Effect::Tidal(spatial::Tidal::new(p, rows, columns, &mut random)?),
            _ => anyhow::bail!(
                "Lighting effect {} is outside the mounted local effect contract",
                source.effect
            ),
        };
        Ok(Self {
            rows,
            columns,
            mode,
            effect,
        })
    }
    pub fn audio_flow(&self) -> Option<bool> {
        match self.effect {
            Effect::Audio { flow, .. } => Some(flow),
            _ => None,
        }
    }
    pub fn reset(&mut self) -> anyhow::Result<()> {
        match &mut self.effect {
            Effect::Uniform { sequence, .. } => sequence.cursor = 0,
            Effect::Wave(wave) => wave.reset(),
            Effect::Tidal(tidal) => tidal.reset()?,
            _ => {}
        }
        Ok(())
    }
    pub fn input(&mut self, indexes: &[usize]) {
        if let Effect::Reactive {
            values,
            cycles,
            active,
        } = &mut self.effect
        {
            for &index in indexes {
                if index < self.rows * self.columns {
                    active.insert(index, Sequence::new(values.clone(), *cycles));
                }
            }
        }
    }
    pub fn render(
        &mut self,
        destination: &mut [[u8; 3]],
        opacity: f32,
        audio_peak: Option<f32>,
    ) -> anyhow::Result<()> {
        ensure!(
            destination.len() == self.rows * self.columns,
            "Lighting destination dimensions changed"
        );
        ensure!(
            opacity.is_finite() && (0.0..=1.0).contains(&opacity),
            "Lighting opacity is invalid"
        );
        let mut pixels = vec![pixel(TRANSPARENT); destination.len()];
        match &mut self.effect {
            Effect::Static(color) => pixels.fill(*color),
            Effect::Uniform {
                sequence,
                random_stops,
                random,
            } => {
                let first = sequence.cursor == 0;
                if let Some(color) = sequence.next() {
                    pixels.fill(color);
                }
                if first {
                    if let Some(stops) = random_stops {
                        let color = random.color();
                        stops[2].color = color;
                        stops[3].color = color;
                        sequence.values = gradient(stops, sequence.values.len())?;
                    }
                }
            }
            Effect::Reactive { active, .. } => {
                active.retain(|index, sequence| match sequence.next() {
                    Some(color) => {
                        pixels[*index] = color;
                        true
                    }
                    None => false,
                });
            }
            Effect::Wave(wave) => wave.render(&mut pixels, self.rows, self.columns),
            Effect::Stars(stars) => stars.render(&mut pixels)?,
            Effect::Fire(fire) => fire.render(&mut pixels),
            Effect::Tidal(tidal) => tidal.render(&mut pixels, self.rows, self.columns)?,
            Effect::Audio {
                values,
                peak,
                maximum,
                decay,
                boost,
                ..
            } => {
                let audio =
                    audio_peak.context("Audio lighting has no current system meter observation")?;
                ensure!(
                    audio.is_finite() && (0.0..=1.0).contains(&audio),
                    "Audio lighting peak is invalid"
                );
                let observed = ((audio * *boost) * 1024.) as u32;
                let observed = observed.min(1024);
                if observed > *maximum || observed > *peak {
                    *maximum = observed;
                    *peak = observed;
                }
                if *peak != 0 || observed != 0 {
                    pixels.fill(values[*peak as usize]);
                    *peak = (*peak as f32 - *decay).max(0.) as u32;
                }
            }
        }
        // 0x52df0: bit 0x20 replaces alpha blending with non-black keying.
        for (target, color) in destination.iter_mut().zip(pixels) {
            let alpha = if self.mode & 32 != 0 {
                if color[..3].iter().any(|v| *v != 0) {
                    opacity
                } else {
                    0.
                }
            } else {
                (100. - f32::from(color[3])) * opacity / 100.
            };
            for channel in 0..3 {
                target[channel] = (i32::from(target[channel])
                    + ((f32::from(color[channel]) - f32::from(target[channel])) * alpha) as i32)
                    as u8;
            }
        }
        Ok(())
    }
}
fn cycles_valid(cycles: Option<i32>) -> bool {
    cycles.is_none_or(|value| value >= -1)
}
