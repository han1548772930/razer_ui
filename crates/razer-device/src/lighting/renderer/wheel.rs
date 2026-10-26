use super::{FRAME_MILLIS, Pixel, gradient};
use crate::lighting_engine::EffectParameters;
use anyhow::Context as _;

/// Current 4.0.55.0 CColorWheelEffect, 0x66b40 / 0x66d60. The small
/// 0x66f20 adapter converts the row's f32 and column's i32 to CRT atan2 f64.
#[derive(Clone, Debug)]
pub(super) struct Wheel {
    values: Vec<Pixel>,
    cursor: u32,
    steps: u32,
    remaining: i32,
    reverse: bool,
    center_row: i32,
    center_col: i32,
}
impl Wheel {
    pub(super) fn new(p: &EffectParameters) -> anyhow::Result<Self> {
        let speed = p.speed.unwrap_or(0.) as u32;
        let steps = if speed == 0 {
            0
        } else {
            ((360000.0_f32 / speed as f32) as u32) / FRAME_MILLIS
        };
        Ok(Self {
            values: gradient(
                p.color_stops
                    .as_deref()
                    .context("Wheel omitted color stops")?,
                1024,
            )?,
            cursor: 0,
            steps,
            remaining: p.cycles.unwrap_or(0),
            reverse: p.mode.unwrap_or(0) & 1024 != 0,
            center_row: p.center_row.unwrap_or(0) as i32,
            center_col: p.center_col.unwrap_or(0) as i32,
        })
    }
    pub(super) fn reset(&mut self) {
        self.cursor = 0;
    }
    pub(super) fn render(&mut self, output: &mut [Pixel], columns: usize) {
        if self.remaining == 0 {
            return;
        }
        let mut phase = 0;
        if self.steps != 0 {
            let position = self.cursor as f32 * 1024. / self.steps as f32;
            phase = if self.reverse { 1024. - position } else { position } as u32;
            self.cursor = (self.cursor + 1) % self.steps;
            if self.cursor == 0 && self.remaining != -1 {
                self.remaining -= 1;
            }
        }
        for (index, target) in output.iter_mut().enumerate() {
            let row = (index / columns) as i32 - self.center_row;
            let col = (index % columns) as i32 - self.center_col;
            let angle = f64::from(row as f32).atan2(f64::from(col));
            let position = (1024. * (angle + std::f64::consts::PI) / std::f64::consts::TAU) as u32;
            *target = self.values[((position + phase) % 1024) as usize];
        }
    }
}
