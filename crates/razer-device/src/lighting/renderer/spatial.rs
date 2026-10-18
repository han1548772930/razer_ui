use super::{ColorStop, FRAME_MILLIS, Pixel, Random, TRANSPARENT, gradient, pixel};
use crate::lighting_engine::EffectParameters;
use anyhow::{Context as _, ensure};

#[derive(Clone, Debug)]
pub(super) struct Wave {
    values: Vec<Pixel>,
    width: usize,
    active_period: usize,
    cursor: usize,
    remaining: i32,
    cycles: i32,
    speed: i32,
    mode: u32,
    sine: f32,
    cosine: f32,
    travel: i32,
    step: f32,
    short_extent: bool,
}
impl Wave {
    pub(super) fn new(p: &EffectParameters, rows: usize, cols: usize) -> anyhow::Result<Self> {
        let stops = p
            .color_stops
            .as_deref()
            .context("Wave omitted color stops")?;
        let mode = p.mode.unwrap_or(0);
        let angle = (f64::from(p.angle.unwrap_or(0) + 270) * std::f64::consts::TAU / 360.) as f32;
        let sine = angle.sin();
        let cosine = angle.cos();
        let mut extent =
            ((sine * rows as f32) as i32).abs() + ((cosine * cols as f32) as i32).abs();
        extent = extent.max(1);
        if mode & 0xc00 != 0 {
            extent = (2 * (rows + cols - 2)) as i32;
        }
        let speed = p.speed.unwrap_or(0.) as i32;
        ensure!(
            speed >= 0 && extent > 0,
            "Wave speed or perimeter is invalid"
        );
        let step = if speed == 0 {
            1. / 25.
        } else {
            (4 * speed) as f32 / 100. * extent as f32 / 25.
        };
        let travel = (extent as f32 / step) as i32;
        let width = ((f64::from(travel) * f64::from(p.width.unwrap_or(0).max(10)) / 100.) as usize)
            .max(stops.len());
        let mut values = gradient(stops, width)?;
        let period = width + (p.pause.unwrap_or(0) / FRAME_MILLIS) as usize;
        ensure!(period <= 1_000_000, "Wave period is too long");
        values.resize(period, pixel(TRANSPARENT));
        Ok(Self {
            values,
            width,
            active_period: period,
            cursor: 0,
            remaining: p.cycles.unwrap_or(0),
            cycles: p.cycles.unwrap_or(0),
            speed,
            mode,
            sine,
            cosine,
            travel,
            step,
            short_extent: extent < stops.len() as i32,
        })
    }
    pub(super) fn reset(&mut self) {
        self.cursor = 0;
        self.remaining = self.cycles;
        self.active_period = self.values.len();
    }
    pub(super) fn render(&mut self, output: &mut [Pixel], rows: usize, cols: usize) {
        if self.remaining == 0 {
            return;
        }
        if self.remaining == 1 && self.cursor == 0 && !self.short_extent {
            self.active_period = self.width + self.values.len();
        }
        let period = self.active_period;
        if self.mode & 0xc00 != 0 {
            self.perimeter(output, rows, cols, period);
        } else {
            let sine = if self.sine.abs() < 0.0001 { 0. } else { self.sine };
            let cosine = if self.cosine.abs() < 0.0001 {
                0.
            } else {
                self.cosine
            };
            let base = period as i32 - self.cursor as i32;
            let mut phase = base;
            if self.remaining == 1 && ((sine <= 0. && cosine <= 0.) || (sine > 0. && cosine < 0.)) {
                phase -= self.width as i32;
            }
            if self.mode & 2 != 0 {
                let mut a = phase;
                if cosine >= 0. {
                    a = base;
                }
                if sine < 0. {
                    a = phase;
                }
                let mut b = a;
                if cosine <= 0. {
                    b = self.cursor as i32;
                }
                if sine > 0. {
                    b = a;
                }
                let mut c = self.cursor as i32;
                if cosine >= 0. || sine <= 0. {
                    c = b;
                }
                phase = if cosine <= 0. || sine >= 0. { c } else { base };
            }
            let center = ((((cols >> 1) as f32 * cosine / cols as f32)
                + ((rows >> 1) as f32 * sine / rows as f32))
                * self.travel as f32) as i32;
            for row in 0..rows {
                for col in 0..cols {
                    let offset = ((col as f32 * cosine / cols as f32
                        + row as f32 * sine / rows as f32)
                        * self.travel as f32) as i32;
                    let offset = if self.mode & 2 != 0 {
                        (offset - center).abs()
                    } else {
                        offset
                    };
                    let index = (phase + offset + 2 * period as i32) as u32 as usize % period;
                    output[row * cols + col] = self
                        .values
                        .get(index)
                        .copied()
                        .unwrap_or(pixel(TRANSPARENT));
                }
            }
        }
        if self.speed != 0 {
            self.cursor = (self.cursor + 1) % self.values.len();
            if self.cursor == 0 && self.remaining != -1 {
                self.remaining -= 1;
            }
        }
    }
    fn perimeter(&self, output: &mut [Pixel], rows: usize, cols: usize, period: usize) {
        let mut positions = Vec::with_capacity(2 * (rows + cols));
        if self.mode & 0x400 != 0 {
            positions.extend((0..rows).map(|r| r * cols + cols - 1));
            positions.extend((0..cols - 1).rev().map(|c| (rows - 1) * cols + c));
            positions.extend((0..rows - 1).rev().map(|r| r * cols));
            positions.extend(1..cols - 1);
        } else {
            positions.extend((0..rows).map(|r| r * cols));
            positions.extend((1..cols).map(|c| (rows - 1) * cols + c));
            positions.extend((0..rows - 1).rev().map(|r| r * cols + cols - 1));
            positions.extend((1..cols - 1).rev());
        }
        for (position, index) in positions.into_iter().enumerate() {
            let phase =
                (position as f32 / self.step + (period - self.cursor) as f32) as usize % period;
            output[index] = self
                .values
                .get(phase)
                .copied()
                .unwrap_or(pixel(TRANSPARENT));
        }
        // 0x66160 copies horizontal strips inward, then the vertical strips.
        let mut depth = 1;
        while depth < rows.saturating_sub(1)
            && depth < cols.saturating_sub(1 + depth)
            && depth <= rows - 1 - depth
        {
            for col in depth..cols - depth {
                output[depth * cols + col] = output[col];
                output[(rows - 1 - depth) * cols + col] = output[(rows - 1) * cols + col];
            }
            depth += 1;
        }
        depth = 1;
        while depth < cols.saturating_sub(1) && depth < rows.saturating_sub(1 + depth) {
            for row in depth..rows - 1 - depth {
                output[row * cols + depth] = output[row * cols];
                output[row * cols + cols - 1 - depth] = output[row * cols + cols - 1];
            }
            depth += 1;
        }
    }
}

#[derive(Clone, Debug)]
struct TidalWave {
    offset: f64,
    values: Vec<Pixel>,
}
#[derive(Clone, Debug)]
pub(super) struct Tidal {
    waves: Vec<TidalWave>,
    stops: Vec<ColorStop>,
    random: Random,
    random_colors: bool,
    center_row: i32,
    center_col: i32,
    row_factor: f64,
    col_factor: f64,
    width: usize,
    travel: i32,
    reverse: bool,
    remaining: i32,
    cycles: i32,
    cursor: usize,
    pause: usize,
    speed: i32,
}
impl Tidal {
    pub(super) fn new(
        p: &EffectParameters,
        rows: usize,
        cols: usize,
        _random: &mut Random,
    ) -> anyhow::Result<Self> {
        let mut stops = p.color_stops.clone().context("Tidal omitted color stops")?;
        let center_row = i32::from(p.center_row.unwrap_or((rows / 2) as u32) as u16);
        let center_col = i32::from(p.center_col.unwrap_or((cols / 2) as u32) as u16);
        let extent_rows = center_row.abs().max((rows as i32 - center_row).abs());
        let extent_cols = center_col.abs().max((cols as i32 - center_col).abs());
        let angle = (f64::from(p.angle.unwrap_or(0)) * std::f64::consts::PI / 180.) as f32;
        let sine = angle.sin();
        let cosine = angle.cos();
        let extent = (((sine * extent_rows as f32) as i32).abs()
            + ((cosine * extent_cols as f32) as i32).abs())
        .max(1);
        let speed = p.speed.unwrap_or(0.) as i32;
        ensure!(speed >= 0, "Tidal speed is invalid");
        let step = if speed == 0 {
            1. / 25.
        } else {
            (2 * speed) as f32 / 100. * extent as f32 / 25.
        };
        let travel = (extent as f32 / step) as i32;
        let width = ((f64::from(travel) * f64::from(p.width.unwrap_or(0).max(10)) / 100.) as usize)
            .max(stops.len());
        let mut random = Random(0);
        let random_colors = p.mode.unwrap_or(0) & 1 != 0;
        let mut waves = Vec::with_capacity(3);
        for index in 0..3 {
            if random_colors {
                randomize(&mut stops, &mut random);
            }
            waves.push(TidalWave {
                offset: -(index as f64) * width as f64,
                values: gradient(&stops, width)?,
            });
        }
        Ok(Self {
            waves,
            stops,
            random,
            random_colors,
            center_row,
            center_col,
            row_factor: f64::from(travel as f32 * sine) / f64::from(extent_rows.max(rows as i32)),
            col_factor: f64::from(travel as f32 * cosine) / f64::from(extent_cols.max(cols as i32)),
            width,
            travel,
            reverse: p.direction.unwrap_or(0) == 0,
            remaining: p.cycles.unwrap_or(0),
            cycles: p.cycles.unwrap_or(0),
            cursor: 0,
            pause: (p.pause.unwrap_or(0) / FRAME_MILLIS) as usize,
            speed,
        })
    }
    pub(super) fn render(
        &mut self,
        output: &mut [Pixel],
        rows: usize,
        cols: usize,
    ) -> anyhow::Result<()> {
        if self.remaining == 0 {
            return Ok(());
        }
        for index in 0..self.waves.len() {
            self.waves[index].offset += 1.;
            if self.waves[index].offset - self.width as f64 > f64::from(self.travel) {
                self.waves[index].offset = self
                    .waves
                    .iter()
                    .map(|w| w.offset)
                    .fold(f64::INFINITY, f64::min)
                    - self.width as f64;
                if self.random_colors {
                    randomize(&mut self.stops, &mut self.random);
                    self.waves[index].values = gradient(&self.stops, self.width)?;
                }
            }
        }
        for row in 0..rows {
            for col in 0..cols {
                let distance = (f64::from(col as i32 - self.center_col) * self.col_factor
                    + f64::from(row as i32 - self.center_row) * self.row_factor)
                    .abs();
                let distance = if self.reverse {
                    f64::from(self.travel) - distance
                } else {
                    distance
                };
                output[row * cols + col] = [0; 4];
                if let Some(wave) = self
                    .waves
                    .iter()
                    .find(|w| w.offset > distance && distance >= w.offset - self.width as f64)
                {
                    let index = if self.reverse {
                        distance - (wave.offset - self.width as f64)
                    } else {
                        wave.offset - distance - 1.
                    };
                    if index >= 0. {
                        if let Some(color) = wave.values.get(index as usize) {
                            output[row * cols + col] = *color;
                        }
                    }
                }
            }
        }
        if self.speed != 0 {
            self.cursor = (self.cursor + 1)
                % (self.width + self.pause + if self.remaining == 1 { self.width } else { 0 });
            if self.cursor == 0 && self.remaining != -1 {
                self.remaining -= 1;
            }
        }
        Ok(())
    }
    pub(super) fn reset(&mut self) -> anyhow::Result<()> {
        // 0x68740 resets the cycle counters; the waves and their random
        // colors survive disable/enable. 0x67f40 is the separate input path.
        self.cursor = 0;
        self.remaining = self.cycles;
        Ok(())
    }
}
fn randomize(stops: &mut [ColorStop], random: &mut Random) {
    for stop in stops {
        if stop.color & 0xff_ffff != 0 {
            stop.color = random.color();
        }
    }
}
