//! CAmbienceEffect 0x5cf80/0x5d0b0 and its order-zero Deriche 0x5d9d0.
//! Capture observations enter explicitly; cached output never becomes a new
//! screen observation when capture fails or reports an unchanged frame.
use super::Pixel;
use crate::lighting_engine::{EffectParameters, ScreenRect};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenRequest {
    pub rect: ScreenRect,
    pub rows: usize,
    pub columns: usize,
}
impl ScreenRequest {
    /// 0x75260/0x81d40/0x82290 convert through f32 then truncate; an all-zero
    /// rectangle selects the full screen. Empty crop rectangles fail.
    pub fn pixel_rect(&self, width: usize, height: usize) -> anyhow::Result<[usize; 4]> {
        ensure!(
            width > 0 && height > 0,
            "Ambient desktop dimensions are empty"
        );
        let rect = if self.rect
            == (ScreenRect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            }) {
            ScreenRect::FULL
        } else {
            self.rect
        };
        let scale =
            |v: u16, dimension: usize| ((dimension as f32 * f32::from(v)) / 65535.) as usize;
        let right = scale(rect.right, width).min(width);
        let bottom = scale(rect.bottom, height).min(height);
        let left = scale(rect.left, width).min(right);
        let top = scale(rect.top, height).min(bottom);
        if [left, top, right, bottom] == [0; 4] {
            return Ok([0, 0, width, height]);
        }
        ensure!(left < right && top < bottom, "Ambient screen crop is empty");
        Ok([left, top, right, bottom])
    }

    /// The Direct3D capture paths use CImg resize interpolation 1 (0x75850):
    /// source positions are floor(output * source_size / output_size).
    pub fn sample_rgb(
        &self,
        width: usize,
        height: usize,
        rgb: &[[u8; 3]],
    ) -> anyhow::Result<Vec<[u8; 3]>> {
        ensure!(
            width.checked_mul(height) == Some(rgb.len()),
            "Ambient desktop buffer dimensions differ"
        );
        let [left, top, right, bottom] = self.pixel_rect(width, height)?;
        ensure!(
            self.rows > 0 && self.columns > 0,
            "Ambient capture request is empty"
        );
        Ok((0..self.rows)
            .flat_map(|row| {
                (0..self.columns).map(move |column| {
                    let y = top + row * (bottom - top) / self.rows;
                    let x = left + column * (right - left) / self.columns;
                    rgb[y * width + x]
                })
            })
            .collect())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScreenObservation {
    Captured {
        frame_id: u32,
        pixels: Vec<[u8; 3]>,
    },
    Unchanged,
    Failed {
        frame_id: Option<u32>,
        error: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScreenBackend {
    DesktopDuplication,
    Direct3D9,
    Gdi,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ScreenCaptureStatus {
    pub backend: ScreenBackend,
    pub frame_id: Option<u32>,
    pub refreshed_at_ms: Option<u64>,
    pub last_error: Option<String>,
    pub released: bool,
}

#[derive(Clone, Debug)]
pub(super) struct Ambient {
    request: ScreenRequest,
    blur: f32,
    last_frame_id: Option<u32>,
    cached: Vec<Pixel>,
}
impl Ambient {
    pub(super) fn new(p: &EffectParameters, rows: usize, columns: usize) -> anyhow::Result<Self> {
        let rect = p
            .screen_rect
            .context("Ambient omitted its source screen rectangle")?;
        let blur = p.blur.unwrap_or(0.);
        ensure!(blur.is_finite(), "Ambient blur is not finite");
        Ok(Self {
            request: ScreenRequest {
                rect,
                rows: rows.max(3),
                columns: columns.max(3),
            },
            blur: blur.clamp(0., 10.),
            last_frame_id: None,
            cached: vec![[0; 4]; rows * columns],
        })
    }
    pub(super) fn request(&self) -> ScreenRequest {
        self.request
    }
    pub(super) fn render(
        &mut self,
        output: &mut [Pixel],
        rows: usize,
        columns: usize,
        observation: Option<&ScreenObservation>,
    ) -> anyhow::Result<()> {
        let observation =
            observation.context("Ambient has no current screen-capture observation")?;
        if let ScreenObservation::Failed {
            frame_id: Some(frame_id),
            ..
        } = observation
        {
            self.last_frame_id = Some(*frame_id);
        }
        if let ScreenObservation::Captured { frame_id, pixels } = observation {
            ensure!(
                pixels.len() == self.request.rows * self.request.columns,
                "Ambient capture buffer dimensions differ"
            );
            if self.last_frame_id != Some(*frame_id) {
                // The native owner commits the frame identity before sampling;
                // it does not repeatedly process a failed sample of that ID.
                self.last_frame_id = Some(*frame_id);
                let mut buffer = pixels
                    .iter()
                    .map(|[r, g, b]| [*r, *g, *b, 0])
                    .collect::<Vec<_>>();
                if self.blur != 0. {
                    for channel in 0..3 {
                        for row in 0..self.request.rows {
                            let mut line = (0..self.request.columns)
                                .map(|column| buffer[row * self.request.columns + column][channel])
                                .collect::<Vec<_>>();
                            deriche(&mut line, self.blur);
                            for (column, value) in line.into_iter().enumerate() {
                                buffer[row * self.request.columns + column][channel] = value;
                            }
                        }
                        for column in 0..self.request.columns {
                            let mut line = (0..self.request.rows)
                                .map(|row| buffer[row * self.request.columns + column][channel])
                                .collect::<Vec<_>>();
                            deriche(&mut line, self.blur);
                            for (row, value) in line.into_iter().enumerate() {
                                buffer[row * self.request.columns + column][channel] = value;
                            }
                        }
                    }
                    // 0x5d0b0 writes the planar blur back with the actual matrix
                    // column stride, before copying with the padded stride.
                    let planar = buffer.clone();
                    for row in 0..self.request.rows {
                        for column in 0..self.request.columns {
                            buffer[row * columns + column] =
                                planar[row * self.request.columns + column];
                        }
                    }
                }
                let offset = if rows == 1 && columns == 1 { 4 } else { 0 };
                for row in 0..rows {
                    for column in 0..columns {
                        self.cached[row * columns + column] =
                            buffer[offset + row * self.request.columns + column];
                    }
                }
            }
        }
        output.copy_from_slice(&self.cached);
        Ok(())
    }
}

/// Boundary replication is enabled by the actual CAmbienceEffect calls.
/// The native path stores a truncated byte after each axis, not once after
/// the complete two-dimensional filter.
fn deriche(values: &mut [u8], sigma: f32) {
    if values.len() < 2 || sigma < 0.1 {
        return;
    }
    let alpha = 1.695_f32 / sigma.max(0.1);
    let ema = (-alpha).exp();
    let ema2 = (alpha * -2.).exp();
    let k = ((1. - ema) * (1. - ema)) / (((alpha + alpha) * ema + 1.) - ema2);
    let a1 = ((alpha - 1.) * k) * ema;
    let a2 = ((alpha + 1.) * k) * ema;
    let a3 = -k * ema2;
    let b1 = ema * 2.;
    let b2 = -ema2;
    let denominator = (1. - b1) + ema2;
    let forward_boundary = (a1 + k) / denominator;
    let reverse_boundary = (a3 + a2) / denominator;
    let mut previous_input = f32::from(values[0]);
    let mut previous = previous_input * forward_boundary;
    let mut previous2 = previous;
    let mut forward = Vec::with_capacity(values.len());
    for value in values.iter() {
        let input = f32::from(*value);
        let result = previous2 * b2 + (b1 * previous + (k * input + previous_input * a1));
        forward.push(result);
        previous_input = input;
        previous2 = previous;
        previous = result;
    }
    let mut next_input = f32::from(*values.last().unwrap());
    let mut next_input2 = next_input;
    let mut next = next_input * reverse_boundary;
    let mut next2 = next;
    for index in (0..values.len()).rev() {
        let input = f32::from(values[index]);
        let result = next2 * b2 + (b1 * next + (next_input * a2 + next_input2 * a3));
        values[index] = (forward[index] + result) as i32 as u8;
        next_input2 = next_input;
        next_input = input;
        next2 = next;
        next = result;
    }
}

#[cfg(test)]
#[path = "ambient_tests.rs"]
mod tests;
