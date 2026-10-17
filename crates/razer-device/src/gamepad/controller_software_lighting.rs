//! Kitsune software effects from the current lighting-engine 6394, 8905 and
//! 2578 owners, with the 4115 edition manifests as the native device geometry.
use anyhow::ensure;
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
    pub led_input_map: [(); 0],
}

pub fn led_configuration(edition: u8) -> anyhow::Result<LedConfiguration> {
    ensure!(
        matches!(edition, 0 | 128..=133),
        "Kitsune edition has no current original lighting manifest"
    );
    let mut led_matrix = Vec::with_capacity(32);
    for row in 0..8 {
        for col in 0..10 {
            let index = match (row, col) {
                (0, col) => col,
                (7, col) => 25 - col,
                (row, 0) => 32 - row,
                (row, 9) => 9 + row,
                _ => continue,
            };
            led_matrix.push(LedMapping {
                matrix_pos: [row, col],
                device_pos: [0, index],
            });
        }
    }
    Ok(LedConfiguration {
        version: 2,
        vendor_id: 5426,
        product_id: 4115,
        edition,
        layout: 0,
        matrix_max_row: 8,
        matrix_max_col: 10,
        device_max_row: 1,
        device_max_col: 32,
        led_width: 40,
        led_height: 40,
        led_width_offset: 0,
        led_height_offset: 0,
        max_region: 0,
        region_max_row: 0,
        region_max_col: 0,
        led_matrix,
        led_input_map: [],
    })
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
impl EngineFrame {
    pub fn validate_kitsune(&self, device_handle: u32) -> anyhow::Result<()> {
        ensure!(
            self.device_handle == device_handle,
            "Lighting frame belongs to another native device"
        );
        ensure!(
            self.region_id == 0
                && self.num_row == 1
                && self.num_col == 32
                && self.frame.len() == 96,
            "Lighting frame differs from the current Kitsune device geometry"
        );
        Ok(())
    }
}

/// Encode actual native device-order RGB bytes. Current SagePC splits its
/// single row into 25-LED packets and sends without a Feature IN transaction.
pub fn frame_reports(
    frame: &EngineFrame,
    device_handle: u32,
    transaction: &mut u8,
) -> anyhow::Result<Vec<[u8; 91]>> {
    frame.validate_kitsune(device_handle)?;
    let mut reports = Vec::with_capacity(2);
    for start in [0usize, 25] {
        let end = (start + 25).min(32);
        let mut data = vec![0, 0, 0, start as u8, (end - 1) as u8];
        data.extend_from_slice(&frame.frame[start * 3..end * 3]);
        let report =
            super::controller_feature::report(4, *transaction, [data.len() as u8, 15, 3], &data)?;
        *transaction = (*transaction + 1) % 31;
        reports.push(report);
    }
    Ok(reports)
}

#[derive(Debug, Default)]
pub struct FrameSubmission {
    pub reports_submitted: u8,
    pub complete: bool,
    pub errors: Vec<String>,
    pub interruption: Option<anyhow::Error>,
}

pub struct SoftwareHid<'a, T: crate::backend::FeatureTransport + ?Sized> {
    transport: &'a T,
    transaction: u8,
}
impl<'a, T: crate::backend::FeatureTransport + ?Sized> SoftwareHid<'a, T> {
    pub fn new(transport: &'a T) -> Self {
        Self {
            transport,
            transaction: 0,
        }
    }
    pub fn select_custom(
        &mut self,
        wait: &mut impl FnMut(std::time::Duration) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        self.effect(0, 8, wait)
    }
    pub fn restore_on_exit(
        &mut self,
        wait: &mut impl FnMut(std::time::Duration) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        self.effect(1, 3, wait)
    }
    fn effect(
        &mut self,
        profile: u8,
        effect: u8,
        wait: &mut impl FnMut(std::time::Duration) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let request = super::controller_feature::report(
            4,
            self.transaction,
            [6, 15, 2],
            &[profile, 0, effect, 0, 0, 0],
        )?;
        self.transaction = (self.transaction + 1) % 31;
        super::controller_feature::exchange_with_in_limit(
            self.transport,
            &request,
            std::time::Duration::from_millis(5),
            wait,
            30,
        )?;
        Ok(())
    }
    pub fn send_frame(
        &mut self,
        frame: &EngineFrame,
        device_handle: u32,
        wait: &mut impl FnMut(std::time::Duration) -> anyhow::Result<()>,
    ) -> anyhow::Result<FrameSubmission> {
        let reports = frame_reports(frame, device_handle, &mut self.transaction)?;
        let mut result = FrameSubmission::default();
        for report in reports {
            let mut submitted = false;
            let mut error = String::new();
            for _ in 0..20 {
                if let Err(e) = wait(std::time::Duration::from_millis(5)) {
                    result.interruption = Some(e);
                    return Ok(result);
                }
                match self.transport.send_feature_at_collection_max(&report) {
                    Ok(()) => {
                        submitted = true;
                        result.reports_submitted += 1;
                        result.complete = result.reports_submitted == 2;
                        break;
                    }
                    Err(e) => error = format!("{e:#}"),
                }
            }
            if !submitted {
                result.errors.push(error);
            }
            if let Err(e) = wait(std::time::Duration::from_millis(1)) {
                result.interruption = Some(e);
                return Ok(result);
            }
        }
        result.complete = result.reports_submitted == 2;
        Ok(result)
    }
}

#[cfg(test)]
#[path = "controller_software_lighting_tests.rs"]
mod tests;
