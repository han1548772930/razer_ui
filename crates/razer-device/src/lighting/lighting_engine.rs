//! Shared native lighting geometry, frames and quick-effect translation from
//! the current lighting-engine 6394, 8905 and 2578 owners.
use serde::{Deserialize, Serialize};

const NO_COLOR: u32 = 0x6400_0000;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum QuickEffectLevel {
    One,
    Two,
    Three,
}
impl TryFrom<u8> for QuickEffectLevel {
    type Error = anyhow::Error;
    fn try_from(value: u8) -> anyhow::Result<Self> {
        match value {
            1 => Ok(Self::One),
            2 => Ok(Self::Two),
            3 => Ok(Self::Three),
            _ => anyhow::bail!("Quick-effect level must be one of the three source choices"),
        }
    }
}
impl QuickEffectLevel {
    pub fn source_value(self) -> u8 {
        match self {
            Self::One => 1,
            Self::Two => 2,
            Self::Three => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WaveDirection {
    Left,
    Right,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SoftwareEffect {
    Breathing {
        color: Option<[u8; 3]>,
        color2: Option<[u8; 3]>,
        random: bool,
        duration: Option<QuickEffectLevel>,
    },
    Spectrum {
        duration: Option<QuickEffectLevel>,
    },
    Reactive {
        color: [u8; 3],
        duration: QuickEffectLevel,
    },
    Static {
        color: [u8; 3],
    },
    Wave {
        direction: WaveDirection,
        speed: Option<QuickEffectLevel>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SoftwarePhase {
    Starting,
    Running,
    Suspended,
    Stopped,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct SoftwareStatus {
    pub phase: SoftwarePhase,
    pub generation: u64,
    pub frames_submitted: u64,
    pub reports_submitted: u64,
    pub acknowledged_writes: u32,
    pub errors: Vec<String>,
    pub cleanup_errors: Vec<String>,
    pub restore_acknowledged: bool,
}
impl SoftwareStatus {
    pub fn starting(generation: u64) -> Self {
        Self {
            phase: SoftwarePhase::Starting,
            generation,
            frames_submitted: 0,
            reports_submitted: 0,
            acknowledged_writes: 0,
            errors: Vec::new(),
            cleanup_errors: Vec::new(),
            restore_acknowledged: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct LedMapping {
    pub matrix_pos: [u8; 2],
    pub device_pos: [u8; 2],
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
#[non_exhaustive]
pub struct RegionMatrix {
    pub matrix_pos: [u16; 2],
}
impl RegionMatrix {
    pub fn new(matrix_pos: [u16; 2]) -> Self {
        Self { matrix_pos }
    }
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
#[non_exhaustive]
pub struct LedRegion {
    pub region_id: u8,
    pub num_led: u16,
    pub max_row: u8,
    pub max_col: u8,
    pub region_matrix: Vec<RegionMatrix>,
}
impl LedRegion {
    pub fn new(
        region_id: u8,
        num_led: u16,
        max_row: u8,
        max_col: u8,
        region_matrix: Vec<RegionMatrix>,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            usize::from(num_led) == region_matrix.len(),
            "Native region LED count differs from its matrix length"
        );
        Ok(Self {
            region_id,
            num_led,
            max_row,
            max_col,
            region_matrix,
        })
    }
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
pub enum LedInputType {
    #[serde(rename = "dkm")]
    Dkm,
    #[serde(rename = "mou")]
    Mouse,
    #[serde(rename = "kbd")]
    Keyboard,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
#[non_exhaustive]
pub struct LedInputMap {
    pub input_type: LedInputType,
    pub input_data: [u16; 2],
    pub matrix_pos: [u16; 2],
}
impl LedInputMap {
    pub fn new(input_type: LedInputType, input_data: [u16; 2], matrix_pos: [u16; 2]) -> Self {
        Self {
            input_type,
            input_data,
            matrix_pos,
        }
    }
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
#[non_exhaustive]
pub struct LedConfiguration {
    pub version: u8,
    #[serde(rename = "VID")]
    pub vendor_id: u16,
    #[serde(rename = "PID")]
    pub product_id: u16,
    #[serde(rename = "EID")]
    pub edition: u8,
    pub layout: u8,
    pub matrix_max_row: u8,
    pub matrix_max_col: u8,
    pub device_max_row: u8,
    pub device_max_col: u8,
    pub led_width: u8,
    pub led_height: u8,
    pub led_width_offset: u8,
    pub led_height_offset: u8,
    pub max_region: u8,
    pub region_max_row: u8,
    pub region_max_col: u8,
    pub led_matrix: Vec<LedMapping>,
    #[serde(rename = "LedRegion", skip_serializing_if = "Vec::is_empty")]
    pub led_regions: Vec<LedRegion>,
    pub led_input_map: Vec<LedInputMap>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct ColorStop {
    pub stop: u8,
    pub color: u32,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct EffectParameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_stops: Option<Vec<ColorStop>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cycles: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pause: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub angle: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub static_color: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_star: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regen: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boost_level: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_boost: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decay: Option<u32>,
    #[serde(rename = "Auto", skip_serializing_if = "Option::is_none")]
    pub automatic: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center_col: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center_row: Option<u32>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct EngineEffect {
    pub effect: u8,
    #[serde(rename = "EffectParam")]
    pub parameters: EffectParameters,
}

fn bgr(color: [u8; 3]) -> u32 {
    u32::from(color[2]) << 16 | u32::from(color[1]) << 8 | u32::from(color[0])
}
fn stop(stop: u8, color: u32) -> ColorStop {
    ColorStop { stop, color }
}
impl SoftwareEffect {
    /// Translate the original UI levels, then the 8905 preset and 2578 effect
    /// owners. The JSON boundary receives BGR COLORREF values, including the
    /// original no-color sentinel; a missing UI color is never invented green.
    pub fn engine_effect(&self) -> EngineEffect {
        let mut p = EffectParameters::default();
        let effect = match *self {
            Self::Static { color } => {
                p.color = Some(bgr(color));
                6
            }
            Self::Reactive { color, duration } => {
                p.mode = Some(32 + 4096);
                p.color_stops = Some(vec![stop(0, bgr(color)), stop(100, NO_COLOR)]);
                p.duration = Some(match duration {
                    QuickEffectLevel::One => 500,
                    QuickEffectLevel::Two => 1500,
                    QuickEffectLevel::Three => 2000,
                });
                p.cycles = Some(1);
                p.pause = Some(0);
                3
            }
            Self::Spectrum { duration } => {
                p.mode = Some(0);
                p.color_stops = Some(
                    [
                        (0, 16718079),
                        (18, 16715792),
                        (28, 16744448),
                        (35, 15728384),
                        (42, 14679808),
                        (49, 65280),
                        (52, 65535),
                        (64, 1595647),
                        (77, 1595647),
                        (100, 16718079),
                    ]
                    .into_iter()
                    .map(|(s, c)| stop(s, swap_rgb(c)))
                    .collect(),
                );
                p.duration = Some(match duration.unwrap_or(QuickEffectLevel::Two) {
                    QuickEffectLevel::One => 18870,
                    QuickEffectLevel::Two => 37740,
                    QuickEffectLevel::Three => 56610,
                });
                p.cycles = Some(-1);
                p.pause = Some(0);
                4
            }
            Self::Wave { direction, speed } => {
                p.mode = Some(0);
                p.color_stops = Some(vec![
                    stop(0, 255),
                    stop(33, 65280),
                    stop(67, 16711680),
                    stop(100, 255),
                ]);
                p.duration = Some(0);
                p.width = Some(100);
                p.speed = Some(match speed.unwrap_or(QuickEffectLevel::Two) {
                    QuickEffectLevel::One => 12.5,
                    QuickEffectLevel::Two => 25.0,
                    QuickEffectLevel::Three => 43.75,
                });
                p.angle = Some(match direction {
                    WaveDirection::Left => 270,
                    WaveDirection::Right => 90,
                });
                p.cycles = Some(-1);
                p.pause = Some(0);
                2
            }
            Self::Breathing {
                mut color,
                mut color2,
                random,
                duration,
            } => {
                if color.is_none() {
                    color = color2;
                    color2 = None;
                }
                let color = color.map(bgr).unwrap_or(NO_COLOR);
                let color2 = color2.map(bgr);
                let mut duration = match duration.unwrap_or(QuickEffectLevel::Two) {
                    QuickEffectLevel::One => 3500,
                    QuickEffectLevel::Two => 7000,
                    QuickEffectLevel::Three => 11200,
                };
                p.mode = Some(u32::from(random));
                p.color_stops = Some(if random {
                    Vec::new()
                } else if let Some(color2) = color2 {
                    duration *= 2;
                    vec![
                        stop(0, 0),
                        stop(12, 0),
                        stop(25, color),
                        stop(38, color),
                        stop(50, 0),
                        stop(62, 0),
                        stop(75, color2),
                        stop(88, color2),
                        stop(100, 0),
                    ]
                } else {
                    vec![
                        stop(0, 0),
                        stop(25, 0),
                        stop(50, color),
                        stop(75, color),
                        stop(100, 0),
                    ]
                });
                p.duration = Some(duration);
                p.cycles = Some(-1);
                p.pause = Some(0);
                p.static_color = Some(NO_COLOR);
                5
            }
        };
        EngineEffect {
            effect,
            parameters: p,
        }
    }
}
fn swap_rgb(color: u32) -> u32 {
    (color & 255) << 16 | color & 65280 | (color & 16711680) >> 16
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct EngineFrame {
    pub device_handle: u32,
    #[serde(rename = "regionId")]
    pub region_id: u8,
    #[serde(rename = "numRow")]
    pub num_row: u8,
    #[serde(rename = "numCol")]
    pub num_col: u8,
    pub frame: Vec<u8>,
}
