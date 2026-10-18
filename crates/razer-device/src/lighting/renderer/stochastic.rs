use super::{ColorStop, FRAME_MILLIS, Pixel, Random, Sequence, gradient, packed, pixel};
use crate::lighting_engine::EffectParameters;
use anyhow::{Context as _, ensure};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(super) struct Stars {
    stars: BTreeMap<usize, Sequence>,
    stops: Vec<ColorStop>,
    random: Random,
    random_colors: bool,
    capacity: usize,
    frames: usize,
    regen: usize,
    cursor: usize,
    cycles: i32,
}
impl Stars {
    pub(super) fn new(p: &EffectParameters, pixels: usize, epoch: u64) -> anyhow::Result<Self> {
        let mut stops = p
            .color_stops
            .clone()
            .context("Starlight omitted color stops")?;
        let mut mode = p.mode.unwrap_or(0);
        if stops.is_empty() {
            stops = vec![
                ColorStop { stop: 0, color: 0 },
                ColorStop {
                    stop: 50,
                    color: 255,
                },
                ColorStop {
                    stop: 100,
                    color: 0,
                },
            ];
            mode = 1;
        }
        let percent = p.max_star.unwrap_or(0.) as u32;
        ensure!(percent <= 100, "Starlight density is invalid");
        let capacity = ((percent as f32 * pixels as f32 / 100.) as usize).max(1);
        let frames = ((p.duration.filter(|v| *v != 0).unwrap_or(14000) / FRAME_MILLIS) as usize)
            .max(stops.len());
        let regen = (frames >> 3)
            .max((p.regen.unwrap_or(0) / FRAME_MILLIS) as usize)
            .max(1);
        let sum = [
            p.pause.unwrap_or(0),
            mode,
            p.cycles.unwrap_or(0) as u32,
            p.regen.unwrap_or(0),
            p.duration.unwrap_or(0),
            percent,
        ]
        .into_iter()
        .chain(stops.iter().map(|s| s.color & 0xff_ffff))
        .fold(0u32, u32::wrapping_add);
        let random = Random((u64::from(sum) * 31 + epoch / 1_800_000 * 1_800_000) as u32);
        gradient(&stops, frames)?;
        Ok(Self {
            stars: BTreeMap::new(),
            stops,
            random,
            random_colors: mode & 1 != 0,
            capacity,
            frames,
            regen,
            cursor: 0,
            cycles: p.cycles.unwrap_or(0),
        })
    }
    pub(super) fn render(&mut self, output: &mut [Pixel]) -> anyhow::Result<()> {
        for (&position, sequence) in &mut self.stars {
            if let Some(color) = sequence.next() {
                output[position] = color;
            }
        }
        if self.cursor == 0 {
            self.spawn(output.len())?;
            self.stars.retain(|_, s| s.active);
        }
        self.cursor = (self.cursor + 1) % self.regen;
        Ok(())
    }
    fn spawn(&mut self, pixels: usize) -> anyhow::Result<()> {
        if self.stars.len() >= self.capacity {
            return Ok(());
        }
        let count = self.random.next() as usize % (self.capacity - self.stars.len() + 1);
        for _ in 0..count {
            let mut position = self.random.next() as usize % pixels;
            if self.stars.contains_key(&position) {
                position = self.random.next() as usize % pixels;
                if self.stars.contains_key(&position) {
                    let third = self.random.next() as usize % pixels;
                    if self.stars.contains_key(&third) {
                        self.random.next();
                    }
                    return Ok(());
                }
            }
            let frames = (self.random.next() as usize % self.frames + 5).max(self.stops.len());
            // 0x60950 performs a signed remainder, then 0x60de0 truncates
            // each scaled channel to the low byte (including negative values).
            let brightness = (self.random.next() as i32 % 80 + 20) as f32 / 100.;
            let random_color = if self.random_colors {
                Some(self.random.color())
            } else {
                None
            };
            let stops = self
                .stops
                .iter()
                .map(|s| {
                    let mut color = pixel(if s.color & 0xff_ffff != 0 {
                        random_color.unwrap_or(s.color)
                    } else {
                        s.color
                    });
                    for channel in &mut color[..3] {
                        *channel = (f32::from(*channel) * brightness) as i32 as u8;
                    }
                    ColorStop {
                        stop: s.stop,
                        color: packed(color),
                    }
                })
                .collect::<Vec<_>>();
            self.stars.insert(
                position,
                Sequence::new(gradient(&stops, frames)?, self.cycles),
            );
        }
        Ok(())
    }
}

// Current engine 4.0.55.0, 0x625d0: immutable 7 x 23 attenuation and
// color lookup tables at RVAs 0x1f52c0 and 0x1f5370 respectively.
const ATTENUATION: [u8; 161] = [
    128, 128, 96, 128, 128, 96, 64, 32, 64, 96, 128, 128, 96, 128, 128, 128, 96, 64, 64, 96, 128,
    128, 128, 128, 96, 64, 96, 96, 64, 32, 16, 32, 64, 96, 96, 64, 96, 128, 96, 64, 32, 32, 64, 96,
    128, 128, 96, 64, 32, 64, 64, 32, 16, 8, 16, 32, 64, 64, 32, 64, 96, 64, 32, 16, 16, 32, 64,
    96, 128, 64, 32, 16, 32, 32, 16, 8, 6, 8, 16, 32, 32, 16, 32, 64, 32, 16, 8, 8, 16, 32, 64, 96,
    32, 16, 8, 16, 16, 8, 6, 4, 6, 8, 16, 16, 8, 16, 32, 16, 8, 6, 6, 8, 16, 32, 64, 16, 8, 6, 8,
    8, 6, 4, 2, 4, 6, 8, 8, 6, 8, 16, 8, 6, 4, 4, 6, 8, 16, 32, 8, 6, 4, 6, 6, 4, 2, 2, 2, 4, 6, 6,
    4, 6, 8, 6, 4, 2, 2, 4, 6, 8, 16,
];
const COLOR_INDEX: [u8; 161] = [
    224, 224, 192, 224, 224, 192, 160, 128, 160, 192, 224, 224, 192, 224, 224, 224, 192, 160, 160,
    192, 224, 224, 224, 224, 192, 160, 192, 192, 160, 128, 96, 128, 160, 192, 192, 160, 192, 224,
    192, 160, 128, 128, 160, 192, 224, 224, 192, 160, 128, 160, 160, 128, 96, 64, 96, 128, 160,
    160, 128, 160, 192, 160, 128, 96, 96, 128, 160, 192, 224, 160, 128, 96, 128, 128, 96, 64, 32,
    64, 96, 128, 128, 96, 128, 160, 128, 96, 64, 64, 96, 128, 160, 192, 128, 96, 64, 96, 96, 64,
    32, 16, 32, 64, 96, 96, 64, 96, 128, 96, 64, 32, 32, 64, 96, 128, 160, 96, 64, 32, 64, 64, 32,
    16, 8, 16, 32, 64, 64, 32, 64, 96, 64, 32, 16, 16, 32, 64, 96, 128, 64, 32, 16, 32, 32, 16, 8,
    8, 8, 16, 32, 32, 16, 32, 64, 32, 16, 8, 8, 16, 32, 64, 96,
];

#[derive(Clone, Debug)]
pub(super) struct Fire {
    frames: Vec<Vec<Pixel>>,
    cursor: usize,
    remaining: i32,
}
impl Fire {
    pub(super) fn new(
        p: &EffectParameters,
        rows: usize,
        cols: usize,
        random: &mut u32,
    ) -> anyhow::Result<Self> {
        let colors = p
            .color_stops
            .as_deref()
            .context("Fire omitted color stops")?;
        ensure!(colors.len() >= 2, "Fire requires two colors");
        let steps = (p.rate.unwrap_or(0).max(40) / FRAME_MILLIS + 1) as usize;
        let seeds = p.seed.filter(|v| *v != 0).unwrap_or(50) as usize;
        let count = steps
            .checked_mul(seeds)
            .context("Fire frame count overflow")?;
        ensure!(
            count <= 100_000
                && count
                    .checked_mul(rows * cols)
                    .is_some_and(|v| v <= 16_000_000),
            "Fire animation is too large"
        );
        let mut base = vec![[0u8; 161]; seeds];
        for frame in &mut base {
            for col in 0..23 {
                *random = random.wrapping_mul(214013).wrapping_add(2531011);
                let noise = (*random >> 16 & 0x7fff) % 192 + 32;
                frame[138 + col] = (noise - noise * u32::from(ATTENUATION[138 + col]) / 255) as u8;
            }
        }
        for row in (1..7).rev() {
            for index in 0..seeds {
                for col in 0..23 {
                    let reference = u32::from(base[index][row * 23 + col]);
                    let attenuation = u32::from(ATTENUATION[(row - 1) * 23 + col]);
                    let next = (index + 1) % seeds;
                    base[next][(row - 1) * 23 + col] = if reference * attenuation / 255 >= reference
                    {
                        1
                    } else {
                        (reference - reference * attenuation / 255) as u8
                    };
                }
            }
        }
        let mut noise = vec![[0u8; 161]; count];
        for frame in 0..seeds {
            for index in 0..161 {
                let start = u32::from(base[frame][index]) * 0x010101;
                let end = u32::from(base[(frame + 1) % seeds][index]) * 0x010101;
                let colors = gradient(
                    &[
                        ColorStop {
                            stop: 0,
                            color: start,
                        },
                        ColorStop {
                            stop: 100,
                            color: end,
                        },
                    ],
                    steps,
                )?;
                for step in 0..steps {
                    noise[frame * steps + step][index] = colors[step][0];
                }
            }
        }
        let palette = gradient(
            &[
                ColorStop {
                    stop: 0,
                    color: colors[0].color,
                },
                ColorStop {
                    stop: 50,
                    color: colors[1].color,
                },
                ColorStop {
                    stop: 100,
                    color: colors.get(2).map_or(0, |s| s.color & 0xff00_0000),
                },
            ],
            256,
        )?;
        let frames = noise
            .into_iter()
            .map(|frame| {
                let mut output = vec![[0; 4]; rows * cols];
                for row in 0..rows {
                    for col in 0..cols {
                        let bottom = rows - 1 - row;
                        let source_row = if rows <= 6 && cols <= 22 {
                            6 - bottom
                        } else {
                            6 - (bottom as f32 * (7. / rows as f32)) as usize
                        };
                        let index = source_row * 23 + col % 23;
                        let color = palette[usize::from(COLOR_INDEX[index])];
                        let mut target = color;
                        for channel in 0..3 {
                            target[channel] =
                                (u32::from(frame[index]) * u32::from(color[channel]) / 255) as u8;
                        }
                        output[row * cols + col] = target;
                    }
                }
                output
            })
            .collect();
        Ok(Self {
            frames,
            cursor: 0,
            remaining: p.cycles.filter(|v| *v != 0).unwrap_or(-1),
        })
    }
    pub(super) fn render(&mut self, output: &mut [Pixel]) {
        if self.remaining == 0 {
            return;
        }
        output.copy_from_slice(&self.frames[self.cursor]);
        if self.cursor == 0 && self.remaining != -1 {
            self.remaining -= 1;
        }
        self.cursor = (self.cursor + 1) % self.frames.len();
    }
}
