//! Accessory UI conversion, preset expansion and the native effect boundary.
use super::QuickEffect;
use crate::lighting_engine::{
    ColorStop, EffectParameters, EngineEffect, LedConfiguration, SoftwareEffect,
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, de::DeserializeOwned};

const NO_COLOR: u32 = 0x6400_0000;

#[derive(Clone, Copy, Deserialize)]
#[serde(try_from = "String")]
enum SourceColor {
    Rgb(u32),
    None,
}
impl TryFrom<String> for SourceColor {
    type Error = String;
    fn try_from(source: String) -> Result<Self, Self::Error> {
        if source == "no-color" {
            return Ok(Self::None);
        }
        let digits = source
            .strip_prefix('#')
            .filter(|digits| {
                digits.len() == 6 && digits.bytes().all(|digit| digit.is_ascii_hexdigit())
            })
            .ok_or("Dock quick-effect color must be a six-digit source RGB color or no-color")?;
        Ok(Self::Rgb(
            u32::from_str_radix(digits, 16).expect("validated RGB digits"),
        ))
    }
}
impl SourceColor {
    fn rgb(self) -> Option<u32> {
        match self {
            Self::Rgb(value) => Some(value),
            Self::None => None,
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(try_from = "u8")]
struct Level(u8);
impl TryFrom<u8> for Level {
    type Error = String;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1..=3 => Ok(Self(value)),
            _ => Err("Dock effect level must be 1, 2 or 3".into()),
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Colors {
    color1: Option<SourceColor>,
    color2: Option<SourceColor>,
    #[serde(default)]
    is_random: bool,
}
#[derive(Default, Deserialize)]
struct TimedColors {
    #[serde(flatten)]
    colors: Colors,
    duration: Option<Level>,
}
#[derive(Default, Deserialize)]
struct Wave {
    direction: Option<u8>,
    speed: Option<Level>,
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Audio {
    color_boost: Option<f64>,
    flow: Option<u8>,
}
#[derive(Default, Deserialize)]
struct Tidal {
    #[serde(flatten)]
    colors: Colors,
    direction: Option<u8>,
}

fn swap_rgb(color: u32) -> u32 {
    if color == NO_COLOR {
        color
    } else {
        (color & 255) << 16 | color & 65280 | (color & 16711680) >> 16
    }
}
fn stops<const N: usize>(source: [(u8, u32); N]) -> Vec<ColorStop> {
    source
        .into_iter()
        .map(|(stop, color)| ColorStop {
            stop,
            color: swap_rgb(color),
        })
        .collect()
}
fn color(source: Option<SourceColor>) -> u32 {
    source.and_then(SourceColor::rgb).unwrap_or(NO_COLOR)
}

impl QuickEffect {
    fn settings<T: DeserializeOwned + Default>(&self) -> anyhow::Result<T> {
        self.selected_effect_setting
            .as_ref()
            .map(|settings| {
                serde_json::from_value(serde_json::Value::Object(settings.clone()))
                    .context("Dock quick-effect settings differ from the current source types")
            })
            .unwrap_or_else(|| Ok(T::default()))
    }

    /// Follow 6394 ACCESSORY conversion, 8905 presets and 2578/2198 native
    /// expansion. An absent source setting stays absent; UI defaults apply
    /// only when the source UI explicitly chooses a new effect.
    pub fn engine_effect(&self) -> anyhow::Result<EngineEffect> {
        self.engine_effect_with_center(1, 2)
    }

    pub fn engine_effect_for_geometry(
        &self,
        geometry: &LedConfiguration,
    ) -> anyhow::Result<EngineEffect> {
        ensure!(
            geometry.matrix_max_col > 0 && geometry.matrix_max_row > 0,
            "Quick effect requires observed native matrix geometry"
        );
        self.engine_effect_with_center(
            u32::from(geometry.matrix_max_col / 2),
            u32::from(geometry.matrix_max_row / 2),
        )
    }

    fn engine_effect_with_center(
        &self,
        center_col: u32,
        center_row: u32,
    ) -> anyhow::Result<EngineEffect> {
        let mut parameters = EffectParameters::default();
        let effect = match self.selected_effect_id {
            1 => {
                let settings: Colors = self.settings()?;
                parameters.color = Some(swap_rgb(color(settings.color1)));
                6
            }
            2 => {
                let settings: Colors = self.settings()?;
                let (first, second) = if matches!(settings.color1, Some(SourceColor::None)) {
                    (settings.color2, None)
                } else {
                    (settings.color1, settings.color2)
                };
                let first = color(first);
                let second = second.and_then(SourceColor::rgb);
                parameters.mode = Some(u32::from(settings.is_random));
                let mut duration = 7000;
                parameters.color_stops = Some(if settings.is_random {
                    Vec::new()
                } else if let Some(second) = second {
                    duration *= 2;
                    stops([
                        (0, 0),
                        (12, 0),
                        (25, first),
                        (38, first),
                        (50, 0),
                        (62, 0),
                        (75, second),
                        (88, second),
                        (100, 0),
                    ])
                } else {
                    stops([(0, 0), (25, 0), (50, first), (75, first), (100, 0)])
                });
                parameters.duration = Some(duration);
                parameters.cycles = Some(-1);
                parameters.pause = Some(0);
                parameters.static_color = Some(NO_COLOR);
                5
            }
            3 => return Ok(SoftwareEffect::Spectrum { duration: None }.engine_effect()),
            4 => {
                let settings: Wave = self.settings()?;
                let (mode, angle) = match settings.direction {
                    Some(1) => (0, 270),
                    Some(2) => (0, 90),
                    Some(3) => (0, 0),
                    Some(4) | None => (0, 180),
                    Some(5) => (2, 90),
                    Some(6) => (2, 270),
                    Some(11) => (1024, 0),
                    Some(12) => (2048, 0),
                    Some(_) => {
                        anyhow::bail!("Dock wave direction is outside the source accessory choices")
                    }
                };
                parameters.mode = Some(mode);
                parameters.angle = Some(angle);
                parameters.color_stops = Some(stops([
                    (0, 16711680),
                    (33, 65280),
                    (67, 255),
                    (100, 16711680),
                ]));
                parameters.duration = Some(0);
                parameters.width = Some(100);
                parameters.speed = Some(match settings.speed.map(|level| level.0) {
                    Some(1) => 15.,
                    Some(3) => 35.,
                    _ => 25.,
                });
                parameters.cycles = Some(-1);
                parameters.pause = Some(0);
                2
            }
            5 => {
                let settings: TimedColors = self.settings()?;
                parameters.mode = Some(32 + 4096);
                parameters.color_stops =
                    Some(stops([(0, color(settings.colors.color1)), (100, NO_COLOR)]));
                parameters.duration = Some(match settings.duration.map(|level| level.0) {
                    Some(1) => 500,
                    Some(3) => 2000,
                    _ => 1500,
                });
                parameters.cycles = Some(1);
                parameters.pause = Some(0);
                3
            }
            7 => {
                let settings: TimedColors = self.settings()?;
                let first = color(settings.colors.color1);
                parameters.mode = Some(u32::from(settings.colors.is_random));
                parameters.color_stops = Some(if settings.colors.is_random {
                    Vec::new()
                } else if let Some(second) = settings
                    .colors
                    .color2
                    .and_then(SourceColor::rgb)
                    .filter(|value| *value > 0)
                {
                    stops([(0, 0), (33, first), (67, second), (100, 0)])
                } else {
                    stops([(0, 0), (33, first), (67, 0), (100, 0)])
                });
                parameters.duration = Some(match settings.duration.map(|level| level.0) {
                    Some(1) => 500,
                    Some(3) => 2000,
                    _ => 1700,
                });
                parameters.max_star = Some(20.);
                parameters.regen = Some(10);
                parameters.cycles = Some(1);
                parameters.pause = Some(0);
                7
            }
            8 => {
                parameters.mode = Some(0);
                parameters.color_stops = Some(stops([(0, 16744192), (50, 16711680), (100, 0)]));
                parameters.rate = Some(160);
                parameters.seed = Some(100);
                parameters.cycles = Some(-1);
                8
            }
            12 => {
                let settings: Audio = self.settings()?;
                if let Some(boost) = settings.color_boost {
                    ensure!(
                        boost.is_finite() && (0.25..=4.).contains(&boost),
                        "Dock audio color boost exceeds the mounted source range"
                    );
                }
                ensure!(
                    settings.flow.is_none_or(|flow| flow <= 1),
                    "Dock audio flow exceeds source choices"
                );
                parameters.mode = Some(0);
                parameters.color_stops = Some(stops([(0, 65280), (50, 16776960), (100, 16711680)]));
                parameters.boost_level = settings.color_boost;
                parameters.auto_boost = Some(0);
                parameters.decay = Some(1000);
                parameters.automatic = Some(0);
                parameters.flow = Some(settings.flow.unwrap_or(0));
                12
            }
            19 => {
                let settings: Tidal = self.settings()?;
                ensure!(
                    settings.direction.is_none_or(|direction| direction <= 1),
                    "Dock tidal direction exceeds the mounted source choices"
                );
                let first = settings
                    .colors
                    .color1
                    .map(|color| color.rgb().context("Tidal has no no-color source choice"))
                    .transpose()?
                    .unwrap_or(0x00ff00);
                let second = settings
                    .colors
                    .color2
                    .map(|color| color.rgb().context("Tidal has no no-color source choice"))
                    .transpose()?
                    .unwrap_or(0x0000ff);
                parameters.mode = Some(if settings.colors.is_random { 3 } else { 2 });
                parameters.color_stops = Some(if settings.colors.is_random {
                    stops([
                        (0, 0),
                        (15, 0),
                        (30, 1),
                        (40, 0),
                        (65, 0),
                        (70, 2),
                        (90, 0),
                        (100, 0),
                    ])
                } else {
                    stops([
                        (0, 0),
                        (10, 0),
                        (25, first),
                        (40, 0),
                        (60, 0),
                        (75, second),
                        (90, 0),
                        (100, 0),
                    ])
                });
                parameters.duration = Some(0);
                parameters.width = Some(100);
                parameters.speed = Some(10.);
                parameters.pause = Some(0);
                parameters.angle = Some(160);
                parameters.direction = Some(u8::from(settings.direction != Some(0)));
                parameters.cycles = Some(-1);
                // createDeviceEffectHandle replaces UI centers with the
                // manifest matrix center before passing native parameters.
                parameters.center_col = Some(center_col);
                parameters.center_row = Some(center_row);
                19
            }
            14 => anyhow::bail!("Dock battery effect belongs to the retained battery monitor"),
            _ => anyhow::bail!(
                "Dock quick effect is absent from the current mounted product choices"
            ),
        };
        Ok(EngineEffect { effect, parameters })
    }
}

#[cfg(test)]
#[path = "receiver_quick_effect_tests.rs"]
mod tests;
