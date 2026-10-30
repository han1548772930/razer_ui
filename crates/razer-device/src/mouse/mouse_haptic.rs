//! Current 167/231 rzDevice25 haptic wheel execution. No vendor code is loaded.
//! Caller retains an observed collection/session and allocates transactions.
use crate::{
    backend::FeatureTransport, device_reads::DeviceReadCapability, feature_exchange::exchange,
};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};

pub const AUDITED_PRODUCTS: &[u32] = &[167, 231];

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum HapticPreset {
    Standard,
    Distinct,
    UltraFine,
    Adaptive,
    SmoothScroll,
    Custom,
}
impl HapticPreset {
    pub fn code(self) -> u8 {
        match self {
            Self::Standard => 129,
            Self::Distinct => 130,
            Self::UltraFine => 131,
            Self::Adaptive => 132,
            Self::SmoothScroll => 133,
            Self::Custom => 134,
        }
    }
    fn decode(code: u8) -> Result<Self> {
        Ok(match code {
            129 => Self::Standard,
            130 => Self::Distinct,
            131 => Self::UltraFine,
            132 => Self::Adaptive,
            133 => Self::SmoothScroll,
            134 => Self::Custom,
            _ => anyhow::bail!("Unsupported haptic preset code {code}"),
        })
    }
}

/// Firmware active stage is a one-based index into this ordered enabled list,
/// not the preset's 129..134 code and not a permanent UI row number.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct HapticStageLayout {
    active_index: u8,
    stages: Vec<HapticPreset>,
}
impl HapticStageLayout {
    pub fn new(active: HapticPreset, stages: Vec<HapticPreset>) -> Result<Self> {
        ensure!(
            (1..=6).contains(&stages.len()),
            "Haptic layout must contain 1..6 stages"
        );
        for (index, stage) in stages.iter().enumerate() {
            ensure!(
                !stages[..index].contains(stage),
                "Duplicate haptic preset in enabled stages"
            );
        }
        let index = stages
            .iter()
            .position(|s| *s == active)
            .context("Active haptic preset must be enabled")?;
        Ok(Self {
            active_index: (index + 1) as u8,
            stages,
        })
    }
    pub fn active_index(&self) -> u8 {
        self.active_index
    }
    pub fn stages(&self) -> &[HapticPreset] {
        &self.stages
    }
    pub fn active_preset(&self) -> HapticPreset {
        self.stages[usize::from(self.active_index) - 1]
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum HapticTableKind {
    Clockwise,
    CounterClockwise,
    VelocityFactor,
}
impl HapticTableKind {
    pub fn code(self) -> u8 {
        match self {
            Self::Clockwise => 1,
            Self::CounterClockwise => 2,
            Self::VelocityFactor => 3,
        }
    }
    fn decode(code: u8) -> Result<Self> {
        Ok(match code {
            1 => Self::Clockwise,
            2 => Self::CounterClockwise,
            3 => Self::VelocityFactor,
            _ => anyhow::bail!("Unsupported haptic table class {code}"),
        })
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum HapticRead {
    ActiveStage {
        profile: u8,
    },
    MaxStage {
        profile: u8,
    },
    StageInformation {
        profile: u8,
    },
    TickSetting {
        profile: u8,
        stage: u8,
    },
    GeneratorSetting {
        profile: u8,
        stage: u8,
    },
    GeneratorTable {
        profile: u8,
        stage: u8,
        kind: HapticTableKind,
        offset: u16,
        count: u8,
    },
    CustomMetadata {
        profile: u8,
    },
    TableCapacity,
    LibraryVersion,
    BrowserMode,
    Acceleration {
        profile: u8,
    },
}
impl HapticRead {
    fn profile(self) -> Option<u8> {
        match self {
            Self::ActiveStage { profile }
            | Self::MaxStage { profile }
            | Self::StageInformation { profile }
            | Self::TickSetting { profile, .. }
            | Self::GeneratorSetting { profile, .. }
            | Self::GeneratorTable { profile, .. }
            | Self::CustomMetadata { profile }
            | Self::Acceleration { profile } => Some(profile),
            _ => None,
        }
    }
    fn stage(self) -> Option<u8> {
        match self {
            Self::TickSetting { stage, .. }
            | Self::GeneratorSetting { stage, .. }
            | Self::GeneratorTable { stage, .. } => Some(stage),
            _ => None,
        }
    }
    fn validate(self) -> Result<()> {
        if let Some(profile) = self.profile() {
            ensure!((1..=5).contains(&profile), "Haptic profile ID must be 1..5");
        }
        if let Some(stage) = self.stage() {
            ensure!((1..=6).contains(&stage), "Haptic stage index must be 1..6");
        }
        if let Self::GeneratorTable { offset, count, .. } = self {
            ensure!(
                (1..=8).contains(&count),
                "One source table request must contain 1..8 points"
            );
            ensure!(
                usize::from(offset) + usize::from(count) <= usize::from(u16::MAX),
                "Haptic table offset/count overflow"
            );
        }
        Ok(())
    }
    fn command(self, write: bool) -> Result<[u8; 3]> {
        self.validate()?;
        let (bytes, class, id) = match self {
            Self::ActiveStage { .. } => (2, 21, 0),
            Self::MaxStage { .. } => (2, 21, 1),
            Self::TickSetting { .. } => (80, 21, 2),
            Self::GeneratorSetting { .. } => (80, 21, 3),
            Self::GeneratorTable { .. } => (80, 21, 4),
            Self::TableCapacity => (6, 21, 5),
            Self::LibraryVersion => (2, 21, 6),
            Self::StageInformation { .. } => (9, 21, 7),
            Self::BrowserMode => (1, 21, 8),
            Self::CustomMetadata { .. } => (80, 21, 9),
            Self::Acceleration { .. } => (2, 2, 0x16),
        };
        if write {
            ensure!(
                !matches!(
                    self,
                    Self::TableCapacity
                        | Self::LibraryVersion
                        | Self::TickSetting { .. }
                        | Self::MaxStage { .. }
                ),
                "No audited product action submits this low-level setter"
            );
        }
        Ok([bytes, class, id | if write { 0 } else { 0x80 }])
    }
    fn payload(self) -> Result<Vec<u8>> {
        self.validate()?;
        let mut bytes = Vec::new();
        if let Some(profile) = self.profile() {
            bytes.push(profile);
        }
        if let Some(stage) = self.stage() {
            bytes.push(stage);
        }
        if let Self::GeneratorTable {
            kind,
            offset,
            count,
            ..
        } = self
        {
            bytes.push(kind.code());
            bytes.extend_from_slice(&offset.to_be_bytes());
            bytes.extend_from_slice(&u16::from(count).to_be_bytes());
        }
        Ok(bytes)
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct HapticTickSetting {
    pub enabled: bool,
    pub mode: u8,
    pub tick_angle: f32,
    pub tick_current: f32,
    pub velocity_factor: f32,
    pub index_window_angle: f32,
    pub window_angle: f32,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct HapticGeneratorSetting {
    pub enabled: bool,
    pub alternate_current: u8,
    pub reduction_angle_cw: f32,
    pub index_count_angle_cw: f32,
    pub reduction_angle_ccw: f32,
    pub index_count_angle_ccw: f32,
    pub current_table_size_cw: u16,
    pub current_table_size_ccw: u16,
    pub velocity_factor_table_size: u16,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct HapticCustomMetadata {
    pub order: u8,
    pub active: bool,
    pub linked: bool,
    pub up_steps: u8,
    pub up_tension: u8,
    pub down_steps: u8,
    pub down_tension: u8,
    pub up_points: [[u8; 2]; 5],
    pub down_points: [[u8; 2]; 5],
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct HapticTableChunk {
    pub kind: HapticTableKind,
    pub offset: u16,
    pub points: Vec<[f32; 2]>,
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub struct HapticTableCapacity {
    pub clockwise: u16,
    pub counterclockwise: u16,
    pub velocity_factor: u16,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub enum HapticValue {
    ActiveStage(u8),
    MaxStage(u8),
    StageInformation(HapticStageLayout),
    TickSetting(HapticTickSetting),
    GeneratorSetting(HapticGeneratorSetting),
    GeneratorTable(HapticTableChunk),
    CustomMetadata(HapticCustomMetadata),
    TableCapacity(HapticTableCapacity),
    LibraryVersion { major: u8, minor: u8 },
    BrowserMode(bool),
    Acceleration(bool),
}

fn boolean(raw: u8) -> Result<bool> {
    ensure!(raw <= 1, "Haptic response contains a nonboolean byte");
    Ok(raw == 1)
}
fn float(bytes: &[u8], offset: usize) -> Result<f32> {
    let value = f32::from_le_bytes(bytes[offset..offset + 4].try_into()?);
    ensure!(
        value.is_finite(),
        "Haptic response contains a nonfinite float"
    );
    Ok(value)
}
fn word(bytes: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([bytes[offset], bytes[offset + 1]])
}

fn parse(query: HapticRead, data: &[u8]) -> Result<HapticValue> {
    query.validate()?;
    if let Some(profile) = query.profile() {
        ensure!(
            data.first() == Some(&profile),
            "Haptic response profile mismatch"
        );
    }
    if let Some(stage) = query.stage() {
        ensure!(
            data.get(1) == Some(&stage),
            "Haptic response stage mismatch"
        );
    }
    Ok(match query {
        HapticRead::ActiveStage { .. } => {
            ensure!(data.len() >= 2, "Truncated haptic active-stage response");
            ensure!(
                (1..=6).contains(&data[1]),
                "Unsupported active haptic stage index"
            );
            HapticValue::ActiveStage(data[1])
        }
        HapticRead::MaxStage { .. } => {
            // This source parser retains the raw byte without a range check.
            ensure!(data.len() >= 2, "Truncated haptic max-stage response");
            HapticValue::MaxStage(data[1])
        }
        HapticRead::StageInformation { .. } => {
            ensure!(data.len() >= 3, "Truncated haptic stage information");
            let count = usize::from(data[2]);
            ensure!(
                (1..=6).contains(&count) && data.len() >= 3 + count,
                "Truncated or invalid haptic stage list"
            );
            ensure!(
                (1..=data[2]).contains(&data[1]),
                "Active haptic index is outside the enabled list"
            );
            let stages = data[3..3 + count]
                .iter()
                .map(|&code| HapticPreset::decode(code))
                .collect::<Result<Vec<_>>>()?;
            HapticValue::StageInformation(HapticStageLayout::new(
                stages[usize::from(data[1]) - 1],
                stages,
            )?)
        }
        HapticRead::Acceleration { .. } => {
            ensure!(data.len() >= 2, "Truncated scroll acceleration");
            HapticValue::Acceleration(boolean(data[1])?)
        }
        HapticRead::BrowserMode => {
            ensure!(!data.is_empty(), "Truncated haptic browser state");
            HapticValue::BrowserMode(boolean(data[0])?)
        }
        HapticRead::TableCapacity => {
            ensure!(data.len() >= 6, "Truncated haptic table capacities");
            HapticValue::TableCapacity(HapticTableCapacity {
                clockwise: word(data, 0),
                counterclockwise: word(data, 2),
                velocity_factor: word(data, 4),
            })
        }
        HapticRead::LibraryVersion => {
            ensure!(data.len() >= 2, "Truncated haptic library version");
            HapticValue::LibraryVersion {
                major: data[0],
                minor: data[1],
            }
        }
        HapticRead::TickSetting { .. } => {
            ensure!(data.len() >= 24, "Truncated haptic tick settings");
            ensure!([1, 3].contains(&data[3]), "Unsupported haptic tick mode");
            HapticValue::TickSetting(HapticTickSetting {
                enabled: boolean(data[2])?,
                mode: data[3],
                tick_angle: float(data, 4)?,
                tick_current: float(data, 8)?,
                velocity_factor: float(data, 12)?,
                index_window_angle: float(data, 16)?,
                window_angle: float(data, 20)?,
            })
        }
        HapticRead::GeneratorSetting { .. } => {
            ensure!(data.len() >= 26, "Truncated haptic generator settings");
            HapticValue::GeneratorSetting(HapticGeneratorSetting {
                enabled: boolean(data[2])?,
                alternate_current: data[3],
                reduction_angle_cw: float(data, 4)?,
                index_count_angle_cw: float(data, 8)?,
                reduction_angle_ccw: float(data, 12)?,
                index_count_angle_ccw: float(data, 16)?,
                current_table_size_cw: word(data, 20),
                current_table_size_ccw: word(data, 22),
                velocity_factor_table_size: word(data, 24),
            })
        }
        HapticRead::GeneratorTable {
            kind,
            offset,
            count,
            ..
        } => {
            ensure!(data.len() >= 7, "Truncated haptic table header");
            let actual = word(data, 5);
            ensure!(
                HapticTableKind::decode(data[2])? == kind && word(data, 3) == offset,
                "Haptic table class/offset mismatch"
            );
            ensure!(
                actual == u16::from(count)
                    && actual <= 8
                    && data.len() >= 7 + usize::from(actual) * 8,
                "Haptic table count/data mismatch"
            );
            let points = (0..usize::from(actual))
                .map(|i| Ok([float(data, 7 + i * 8)?, float(data, 11 + i * 8)?]))
                .collect::<Result<Vec<_>>>()?;
            HapticValue::GeneratorTable(HapticTableChunk {
                kind,
                offset,
                points,
            })
        }
        HapticRead::CustomMetadata { .. } => {
            ensure!(
                data.len() >= 31 && data[1] == 40,
                "Truncated or unknown custom metadata layout"
            );
            let points =
                |start| std::array::from_fn(|i| [data[start + i * 2], data[start + i * 2 + 1]]);
            HapticValue::CustomMetadata(HapticCustomMetadata {
                order: data[2],
                active: boolean(data[3])?,
                linked: boolean(data[4])?,
                up_steps: data[5],
                up_tension: data[6],
                down_steps: data[7],
                down_tension: data[8],
                up_points: points(9),
                down_points: points(21),
            })
        }
    })
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct HapticChartPoint {
    pub x: f64,
    pub y: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HapticCustomDirection {
    pub scroll_steps: u8,
    pub scroll_tension: u8,
    /// Current source order: pLeft, pMidLeft, pMid, pMidRight, pRight.
    pub coordinates: [HapticChartPoint; 5],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HapticCustomStage {
    pub linked_curves: bool,
    pub up: HapticCustomDirection,
    pub down: HapticCustomDirection,
}

#[derive(Clone, Debug, Serialize)]
pub struct PreparedHapticCustom {
    pub metadata: HapticCustomMetadata,
    pub generator: HapticGeneratorSetting,
    pub clockwise: Vec<[f32; 2]>,
    pub counterclockwise: Vec<[f32; 2]>,
    pub velocity_factor: Vec<[f32; 2]>,
}

fn normalized(direction: &HapticCustomDirection) -> Result<[[u8; 2]; 5]> {
    ensure!(
        direction.scroll_steps > 0,
        "Custom scroll step count cannot be zero"
    );
    let mut result = [[0; 2]; 5];
    for (out, input) in result.iter_mut().zip(&direction.coordinates) {
        ensure!(
            input.x.is_finite()
                && input.y.is_finite()
                && (0.0..=520.0).contains(&input.x)
                && (0.0..=330.0).contains(&input.y),
            "Custom control point is outside the current 520 by 330 source chart"
        );
        *out = [
            (input.x / 5.2 + 0.5).floor() as u8,
            ((input.y / 3.3 - 100.0).abs() + 0.5).floor() as u8,
        ];
    }
    ensure!(
        result[0] == [0, 0] && result[4] == [100, 0],
        "Custom chart endpoints must retain their source fixed positions"
    );
    ensure!(
        result.windows(2).all(|w| w[0][0] <= w[1][0]) && result[2][0] > 0 && result[2][0] < 100,
        "Custom chart control points must preserve the two source curve segments"
    );
    Ok(result)
}

fn sampled(points: &[[u8; 2]; 5]) -> Vec<[f64; 2]> {
    let p = points.map(|p| [f64::from(p[0]), f64::from(p[1])]);
    let mut left = vec![p[0]];
    let mut right = vec![p[4]];
    for sample in 1..=50 {
        let t = f64::from(sample) / 50.0;
        let curve = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
            std::array::from_fn(|i| {
                (1.0 - t) * (1.0 - t) * a[i] + 2.0 * (1.0 - t) * t * b[i] + t * t * c[i]
            })
        };
        left.push(curve(p[0], p[1], p[2]));
        right.push(curve(p[4], p[3], p[2]));
    }
    right.reverse();
    right.remove(0);
    left.extend(right);
    left
}

fn discretized(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut output = vec![points[0]];
    let mut anchor = 0;
    let mut candidate = 1;
    while candidate < points.len() - 1 {
        let start = points[anchor];
        let end = points[candidate];
        let slope = (end[1] - start[1]) / (end[0] - start[0]);
        let intercept = start[1] - slope * start[0];
        let accepted = (anchor + 1..=candidate)
            .all(|i| (points[i][0] * slope + intercept - points[i][1]).abs() <= 0.2);
        if accepted {
            candidate += 1;
        } else {
            anchor = candidate - 1;
            output.push(points[anchor]);
            candidate = anchor + 2;
        }
    }
    output.push(points[points.len() - 1]);
    output
}

fn direction_table(
    direction: &HapticCustomDirection,
    points: &[[u8; 2]; 5],
) -> Result<(Vec<[f32; 2]>, f32)> {
    let reduced = discretized(&sampled(points));
    let angle = 360.0 / f64::from(direction.scroll_steps);
    let current = 0.2 * f64::from(direction.scroll_tension) / 100.0;
    let maximum = reduced.iter().map(|p| p[1]).fold(0.0, f64::max);
    let peak = reduced
        .iter()
        .find(|p| p[1] == maximum)
        .map_or(0.0, |p| angle * (p[0] / 100.0));
    let table = reduced
        .iter()
        .map(|p| {
            [
                (angle * (p[0] / 100.0)) as f32,
                (current * (p[1] / 100.0)) as f32,
            ]
        })
        .collect::<Vec<_>>();
    ensure!(
        table.iter().flatten().all(|f| f.is_finite()) && peak.is_finite(),
        "Custom generator produced nonfinite parameters"
    );
    Ok((table, peak as f32))
}

/// Current JS coordinate conversion, two 50-sample quadratic segments, forward
/// 0.2 discretization, angle/current scaling, and fixed velocity table.
pub fn prepare_custom(custom: &HapticCustomStage) -> Result<PreparedHapticCustom> {
    let up = normalized(&custom.up)?;
    let down = normalized(&custom.down)?;
    let (clockwise, index_cw) = direction_table(&custom.up, &up)?;
    let (counterclockwise, index_ccw) = direction_table(&custom.down, &down)?;
    let velocity_factor = vec![[-720.0, 1.0], [0.0, 1.0], [720.0, 1.0]];
    Ok(PreparedHapticCustom {
        metadata: HapticCustomMetadata {
            order: 6,
            active: true,
            linked: custom.linked_curves,
            up_steps: custom.up.scroll_steps,
            up_tension: custom.up.scroll_tension,
            down_steps: custom.down.scroll_steps,
            down_tension: custom.down.scroll_tension,
            up_points: up,
            down_points: down,
        },
        generator: HapticGeneratorSetting {
            enabled: true,
            alternate_current: 1,
            reduction_angle_cw: 0.0,
            index_count_angle_cw: index_cw,
            reduction_angle_ccw: 0.0,
            index_count_angle_ccw: index_ccw,
            current_table_size_cw: clockwise.len().try_into()?,
            current_table_size_ccw: counterclockwise.len().try_into()?,
            velocity_factor_table_size: 3,
        },
        clockwise,
        counterclockwise,
        velocity_factor,
    })
}

#[derive(Clone, Debug)]
pub enum HapticChange {
    ActiveStage(u8),
    StageInformation(HapticStageLayout),
    Custom(HapticCustomStage),
    Acceleration(bool),
    BrowserMode(bool),
}
#[derive(Clone, Debug, Default)]
pub struct HapticProfileObservation {
    /// Actual obmData.profiles.find(GUID).slotId, used by 167 stage/custom tasks.
    pub first_matching_cached_slot: Option<u8>,
    /// Actual selected profile first obmSlotId, used by acceleration tasks.
    pub selected_first_slot: Option<u8>,
}

fn profiles(
    product: u32,
    change: &HapticChange,
    observed: &HapticProfileObservation,
) -> Result<Vec<u8>> {
    if matches!(change, HapticChange::BrowserMode(_)) {
        return Ok(Vec::new());
    }
    let optional = if matches!(change, HapticChange::Acceleration(_)) {
        observed.selected_first_slot.filter(|s| *s > 0)
    } else if product == 167 {
        observed.first_matching_cached_slot
    } else {
        None
    };
    let mut profiles = vec![1];
    if let Some(id) = optional {
        ensure!(
            (1..=5).contains(&id),
            "Observed haptic profile is outside 1..5"
        );
        if id != 1 {
            profiles.push(id);
        }
    }
    Ok(profiles)
}

struct PreparedStep {
    query: HapticRead,
    requested: HapticValue,
    payload: Vec<u8>,
}
impl PreparedStep {
    fn new(query: HapticRead, requested: HapticValue) -> Result<Self> {
        query.command(true)?;
        let mut payload = Vec::new();
        if let Some(profile) = query.profile() {
            payload.push(profile);
        }
        match &requested {
            HapticValue::ActiveStage(index) => {
                ensure!((1..=6).contains(index), "Active haptic stage must be 1..6");
                payload.push(*index);
            }
            HapticValue::StageInformation(layout) => {
                payload.push(layout.active_index);
                payload.push(layout.stages.len() as u8);
                payload.extend(layout.stages.iter().map(|s| s.code()));
            }
            HapticValue::Acceleration(enabled) | HapticValue::BrowserMode(enabled) => {
                payload.push(u8::from(*enabled))
            }
            HapticValue::CustomMetadata(meta) => {
                payload.resize(40, 0);
                payload[1..9].copy_from_slice(&[
                    40,
                    meta.order,
                    u8::from(meta.active),
                    u8::from(meta.linked),
                    meta.up_steps,
                    meta.up_tension,
                    meta.down_steps,
                    meta.down_tension,
                ]);
                for (start, points) in [(9, meta.up_points), (21, meta.down_points)] {
                    for (index, point) in points.iter().enumerate() {
                        payload[start + index * 2..start + index * 2 + 2].copy_from_slice(point);
                    }
                    payload[start + 10..start + 12].fill(255);
                }
            }
            HapticValue::GeneratorSetting(generator) => {
                payload.push(query.stage().context("Generator stage required")?);
                payload.push(u8::from(generator.enabled));
                payload.push(generator.alternate_current);
                for value in [
                    generator.reduction_angle_cw,
                    generator.index_count_angle_cw,
                    generator.reduction_angle_ccw,
                    generator.index_count_angle_ccw,
                ] {
                    ensure!(value.is_finite(), "Nonfinite generator value");
                    payload.extend_from_slice(&value.to_le_bytes());
                }
                for count in [
                    generator.current_table_size_cw,
                    generator.current_table_size_ccw,
                    generator.velocity_factor_table_size,
                ] {
                    payload.extend_from_slice(&count.to_be_bytes());
                }
            }
            HapticValue::GeneratorTable(table) => {
                payload = query.payload()?;
                for point in &table.points {
                    for value in point {
                        ensure!(value.is_finite(), "Nonfinite table point");
                        payload.extend_from_slice(&value.to_le_bytes());
                    }
                }
            }
            _ => anyhow::bail!("This haptic value has no audited product write action"),
        }
        ensure!(
            payload.len() <= usize::from(query.command(true)?[0]),
            "Haptic setter exceeds its source payload size"
        );
        Ok(Self {
            query,
            requested,
            payload,
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct HapticObservation {
    pub query: HapticRead,
    pub value: HapticValue,
}

/// A valid haptic observation survives a failed ownership or release boundary
/// only as diagnostics; callers must still stop the operation with its error.
#[derive(Clone, Debug)]
pub struct CompletedHapticRead {
    observation: HapticObservation,
}

impl CompletedHapticRead {
    pub fn from_error(error: &anyhow::Error) -> Option<&Self> {
        error.downcast_ref()
    }

    pub fn observation(&self) -> &HapticObservation {
        &self.observation
    }
}

impl std::fmt::Display for CompletedHapticRead {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Haptic observation for {:?} retained as diagnostics",
            self.observation.query
        )
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct HapticWriteStep {
    pub query: HapticRead,
    pub requested: HapticValue,
    pub acknowledgment_received: bool,
    pub acknowledged: Option<HapticValue>,
    pub observed: Option<HapticValue>,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct HapticWriteReport {
    pub observations: Vec<HapticObservation>,
    pub steps: Vec<HapticWriteStep>,
    pub skipped_source_write: bool,
    verified: bool,
}
impl HapticWriteReport {
    pub fn verified(&self) -> bool {
        self.verified
    }
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum HapticFailurePhase {
    Prepare,
    Observe,
    Write,
    Readback,
}
#[derive(Debug)]
pub struct HapticWriteFailure {
    pub phase: HapticFailurePhase,
    pub progress: HapticWriteReport,
    pub failed_step: Option<usize>,
    error: anyhow::Error,
}
impl HapticWriteFailure {
    pub fn device_state_may_have_changed(&self) -> bool {
        !self.progress.steps.is_empty()
    }
}
impl std::fmt::Display for HapticWriteFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Haptic wheel {:?} failed; partial observations retained: {:#}",
            self.phase, self.error
        )
    }
}
impl std::error::Error for HapticWriteFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}

pub struct MouseHaptic<'a> {
    device: &'a dyn FeatureTransport,
    transport: &'a DeviceReadCapability,
}
impl<'a> MouseHaptic<'a> {
    pub fn new(
        device: &'a dyn FeatureTransport,
        transport: &'a DeviceReadCapability,
    ) -> Result<Self> {
        ensure!(
            AUDITED_PRODUCTS.contains(&transport.product_id)
                && transport.source_class == "rzDevice25DualLinkMouse",
            "No audited direct haptic wheel adapter for this product"
        );
        Ok(Self { device, transport })
    }
    pub fn read(
        &self,
        query: HapticRead,
        mut next_transaction: impl FnMut() -> Result<u8>,
        validate: impl Fn() -> Result<()>,
    ) -> Result<HapticValue> {
        let command = query.command(false)?;
        let payload = query.payload()?;
        validate()?;
        let response = exchange(
            self.device,
            self.transport,
            command,
            &payload,
            next_transaction()?,
            &validate,
        );
        match response {
            Ok(data) => parse(query, &data),
            Err(error) => {
                let value = crate::feature_exchange::acknowledged_error(&error)
                    .and_then(|data| parse(query, data).ok());
                Err(match value {
                    Some(value) => error.context(CompletedHapticRead {
                        observation: HapticObservation { query, value },
                    }),
                    None => error,
                })
            }
        }
    }

    fn read_observing(
        &self,
        query: HapticRead,
        next_transaction: impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
        observe: &mut impl FnMut(&HapticValue),
    ) -> Result<HapticValue> {
        let result = self.read(query, next_transaction, validate);
        match &result {
            Ok(value) => observe(value),
            Err(error) => {
                if let Some(completed) = CompletedHapticRead::from_error(error) {
                    observe(&completed.observation().value);
                }
            }
        }
        result
    }
    pub fn apply(
        &self,
        change: HapticChange,
        observed: &HapticProfileObservation,
        mut next_transaction: impl FnMut() -> Result<u8>,
        validate: impl Fn() -> Result<()>,
    ) -> std::result::Result<HapticWriteReport, HapticWriteFailure> {
        let mut progress = HapticWriteReport::default();
        let fail = |phase, progress, failed_step, error| HapticWriteFailure {
            phase,
            progress,
            failed_step,
            error,
        };
        let prepared = (|| -> Result<_> {
            let profiles = profiles(self.transport.product_id, &change, observed)?;
            let mut steps = Vec::new();
            let mut capacity = None;
            match &change {
                HapticChange::ActiveStage(index) => {
                    for &profile in &profiles {
                        steps.push(PreparedStep::new(
                            HapticRead::ActiveStage { profile },
                            HapticValue::ActiveStage(*index),
                        )?);
                    }
                }
                HapticChange::StageInformation(layout) => {
                    for &profile in &profiles {
                        steps.push(PreparedStep::new(
                            HapticRead::StageInformation { profile },
                            HapticValue::StageInformation(layout.clone()),
                        )?);
                    }
                }
                HapticChange::Acceleration(enabled) => {
                    for &profile in &profiles {
                        steps.push(PreparedStep::new(
                            HapticRead::Acceleration { profile },
                            HapticValue::Acceleration(*enabled),
                        )?);
                    }
                }
                HapticChange::BrowserMode(enabled) => steps.push(PreparedStep::new(
                    HapticRead::BrowserMode,
                    HapticValue::BrowserMode(*enabled),
                )?),
                HapticChange::Custom(custom) => {
                    let generated = prepare_custom(custom)?;
                    capacity = Some(HapticTableCapacity {
                        clockwise: generated.generator.current_table_size_cw,
                        counterclockwise: generated.generator.current_table_size_ccw,
                        velocity_factor: 3,
                    });
                    // Source completes metadata for all targets, then generator
                    // for all targets, then each CW/CCW/velocity chunk for all.
                    for &profile in &profiles {
                        steps.push(PreparedStep::new(
                            HapticRead::CustomMetadata { profile },
                            HapticValue::CustomMetadata(generated.metadata.clone()),
                        )?);
                    }
                    for &profile in &profiles {
                        steps.push(PreparedStep::new(
                            HapticRead::GeneratorSetting { profile, stage: 6 },
                            HapticValue::GeneratorSetting(generated.generator.clone()),
                        )?);
                    }
                    for (kind, table) in [
                        (HapticTableKind::Clockwise, generated.clockwise),
                        (
                            HapticTableKind::CounterClockwise,
                            generated.counterclockwise,
                        ),
                        (HapticTableKind::VelocityFactor, generated.velocity_factor),
                    ] {
                        for (chunk, points) in table.chunks(8).enumerate() {
                            for &profile in &profiles {
                                let offset = (chunk * 8).try_into()?;
                                let count = points.len().try_into()?;
                                steps.push(PreparedStep::new(
                                    HapticRead::GeneratorTable {
                                        profile,
                                        stage: 6,
                                        kind,
                                        offset,
                                        count,
                                    },
                                    HapticValue::GeneratorTable(HapticTableChunk {
                                        kind,
                                        offset,
                                        points: points.to_vec(),
                                    }),
                                )?);
                            }
                        }
                    }
                }
            }
            validate()?;
            Ok((steps, capacity, profiles))
        })()
        .map_err(|e| fail(HapticFailurePhase::Prepare, progress.clone(), None, e))?;
        let (steps, needed_capacity, profiles) = prepared;
        if let Some(needed) = needed_capacity {
            let result = (|| -> Result<()> {
                let query = HapticRead::TableCapacity;
                let value =
                    self.read_observing(query, &mut next_transaction, &validate, &mut |value| {
                        progress.observations.push(HapticObservation {
                            query,
                            value: value.clone(),
                        })
                    })?;
                let HapticValue::TableCapacity(actual) = value else {
                    unreachable!()
                };
                ensure!(
                    actual.clockwise >= needed.clockwise
                        && actual.counterclockwise >= needed.counterclockwise
                        && actual.velocity_factor >= needed.velocity_factor,
                    "Generated curve exceeds the device's actually observed table capacities"
                );
                Ok(())
            })();
            if let Err(e) = result {
                return Err(fail(HapticFailurePhase::Observe, progress, None, e));
            }
        }
        // Source active-stage task checks the first target and skips all writes
        // when that one already matches. Rust additionally observes other
        // targets before treating that no-op as fully verified.
        if let HapticChange::ActiveStage(requested) = change {
            let query = HapticRead::ActiveStage {
                profile: profiles[0],
            };
            let baseline = self
                .read_observing(query, &mut next_transaction, &validate, &mut |value| {
                    progress.observations.push(HapticObservation {
                        query,
                        value: value.clone(),
                    })
                })
                .map_err(|e| fail(HapticFailurePhase::Observe, progress.clone(), None, e))?;
            if baseline == HapticValue::ActiveStage(requested) {
                progress.skipped_source_write = true;
                for profile in profiles.into_iter().skip(1) {
                    let query = HapticRead::ActiveStage { profile };
                    let result = (|| -> Result<()> {
                        let value = self.read_observing(
                            query,
                            &mut next_transaction,
                            &validate,
                            &mut |value| {
                                progress.observations.push(HapticObservation {
                                    query,
                                    value: value.clone(),
                                })
                            },
                        )?;
                        ensure!(
                            value == HapticValue::ActiveStage(requested),
                            "Unwritten haptic target has a different active stage"
                        );
                        Ok(())
                    })();
                    if let Err(e) = result {
                        return Err(fail(HapticFailurePhase::Readback, progress, None, e));
                    }
                }
                progress.verified = true;
                return Ok(progress);
            }
        }
        for step in steps {
            if let Err(e) = validate() {
                return Err(fail(HapticFailurePhase::Write, progress, None, e));
            }
            let index = progress.steps.len();
            progress.steps.push(HapticWriteStep {
                query: step.query,
                requested: step.requested,
                acknowledgment_received: false,
                acknowledged: None,
                observed: None,
            });
            let result = (|| -> Result<()> {
                let response = exchange(
                    self.device,
                    self.transport,
                    step.query.command(true)?,
                    &step.payload,
                    next_transaction()?,
                    &validate,
                );
                if let Some(data) = crate::feature_exchange::acknowledged_reply(&response) {
                    progress.steps[index].acknowledgment_received = true;
                    if !data.is_empty() {
                        match parse(step.query, data) {
                            Ok(value) => progress.steps[index].acknowledged = Some(value),
                            Err(error) => return response.and(Err(error)),
                        }
                    }
                }
                response.map(|_| ())
            })();
            if let Err(e) = result {
                return Err(fail(HapticFailurePhase::Write, progress, Some(index), e));
            }
        }
        for index in 0..progress.steps.len() {
            let step = progress.steps[index].clone();
            let result = (|| -> Result<()> {
                let value = self.read_observing(
                    step.query,
                    &mut next_transaction,
                    &validate,
                    &mut |value| progress.steps[index].observed = Some(value.clone()),
                )?;
                ensure!(
                    value == step.requested,
                    "Haptic getter readback differs from the requested value"
                );
                Ok(())
            })();
            if let Err(e) = result {
                return Err(fail(HapticFailurePhase::Readback, progress, Some(index), e));
            }
        }
        progress.verified = !progress.steps.is_empty();
        Ok(progress)
    }
}

#[cfg(test)]
#[path = "mouse_haptic_tests.rs"]
mod tests;
