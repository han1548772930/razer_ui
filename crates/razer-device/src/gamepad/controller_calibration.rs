//! Current 2676/2684 middleware V3 trigger calibration arithmetic and state.
//! Module 36840 methods and independently resolved module 69427 parameters.
//! No OS API, vendor execution, fabricated observations or completion timer.
use anyhow::{Context as _, ensure};
use serde::Deserialize;
use std::{sync::OnceLock, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriggerPart {
    Left,
    Right,
}
impl TriggerPart {
    pub fn from_ui_part(part: u8) -> Option<Self> {
        match part {
            3 => Some(Self::Left),
            4 => Some(Self::Right),
            _ => None,
        }
    }
    pub fn ui_part(self) -> u8 {
        if self == Self::Left { 3 } else { 4 }
    }
    /// Native AnalogId bitmask is distinct from the UI part enum.
    pub fn analog_mask(self) -> u8 {
        if self == Self::Left { 4 } else { 8 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriggerStep {
    None,
    Press,
    Release,
    Complete,
    Error,
}
impl TriggerStep {
    pub fn ui_step(self) -> i8 {
        match self {
            Self::None => 0,
            Self::Press => 10,
            Self::Release => 11,
            Self::Complete => 12,
            Self::Error => -1,
        }
    }
    pub fn hold_duration(self) -> Option<Duration> {
        match self {
            Self::Press => Some(Duration::from_secs(2)),
            Self::Release => Some(Duration::from_secs(3)),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawAnalogInput {
    pub lx: f64,
    pub ly: f64,
    pub rx: f64,
    pub ry: f64,
    pub lt: f64,
    pub rt: f64,
}
impl RawAnalogInput {
    fn trigger(self, part: TriggerPart) -> f64 {
        if part == TriggerPart::Left {
            self.lt
        } else {
            self.rt
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TriggerCalibrationRange {
    pub min: u16,
    pub max: u16,
}

/// Existing device data, queried through the actual adapter. A completed set
/// does not count as calibration success; the session must read back its range.
pub trait TriggerCalibrationTransport {
    fn raw_analog_input(&mut self) -> anyhow::Result<RawAnalogInput>;
    fn set_trigger_calibration(
        &mut self,
        part: TriggerPart,
        range: TriggerCalibrationRange,
    ) -> anyhow::Result<()>;
    fn trigger_calibration(&mut self, part: TriggerPart)
    -> anyhow::Result<TriggerCalibrationRange>;
}

#[derive(Deserialize)]
struct Product {
    product_id: u32,
    params: Params,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Params {
    version: u8,
    range: Range,
    trigger: TriggerParams,
}
#[derive(Deserialize)]
struct Range {
    min: f64,
    max: f64,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TriggerParams {
    threshold_percent: Range,
    trigger_max: f64,
    left: Limits,
    right: Limits,
    validation_range: f64,
    #[serde(rename = "PressValidityPercentUI")]
    press_validity_percent: f64,
    #[serde(rename = "ReleaseValidityPercentUI")]
    release_validity_percent: f64,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Limits {
    press_threshold: f64,
    release_threshold: f64,
}
fn product(pid: u32) -> Option<&'static Product> {
    static PRODUCTS: OnceLock<Vec<Product>> = OnceLock::new();
    PRODUCTS
        .get_or_init(|| make_controller_calibration_current_params_table())
        .iter()
        .find(|p| p.product_id == pid && p.params.version == 3)
}

/// Owns one native calibration operation. The adapter owns timers and fresh
/// device identity. Generation guards at that boundary are Rust cancellation
/// protection, not a claim that the original JS had those guards.
pub struct TriggerCalibrationSession {
    product: &'static Product,
    part: TriggerPart,
    step: TriggerStep,
    active: bool,
    valid: bool,
    movement: Option<TriggerMovement>,
    previous: Option<f64>,
    stable_polls: u64,
    travelled: bool,
    returned: bool,
    sample_min: Option<f64>,
    sample_max: Option<f64>,
    progress: TriggerCalibrationProgress,
}

#[derive(Clone, Copy)]
enum RestingTriggerValue {
    Observed(f64),
    SourceRangeMaximum,
}

#[derive(Clone, Copy)]
enum TriggerMovementValue {
    Observed(f64),
    ResetResting,
}

struct TriggerMovement {
    resting: RestingTriggerValue,
    value: TriggerMovementValue,
}
impl TriggerMovement {
    fn resting(&self, range_maximum: f64) -> f64 {
        match self.resting {
            RestingTriggerValue::Observed(value) => value,
            RestingTriggerValue::SourceRangeMaximum => range_maximum,
        }
    }

    fn value(&self, range_maximum: f64) -> f64 {
        match self.value {
            TriggerMovementValue::Observed(value) => value,
            TriggerMovementValue::ResetResting => self.resting(range_maximum),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TriggerCalibrationProgress {
    step: TriggerStep,
    valid: bool,
}
impl TriggerCalibrationProgress {
    pub fn step(self) -> TriggerStep {
        self.step
    }
    pub fn valid(self) -> bool {
        self.valid
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoldChange {
    Unchanged,
    Start(Duration),
    Cancel,
}

impl TriggerCalibrationSession {
    pub fn new(pid: u32, part: TriggerPart) -> Option<Self> {
        let product = product(pid)?;
        Some(Self {
            product,
            part,
            step: TriggerStep::None,
            active: false,
            valid: false,
            movement: None,
            previous: None,
            stable_polls: 0,
            travelled: false,
            returned: false,
            sample_min: None,
            sample_max: None,
            progress: TriggerCalibrationProgress {
                step: TriggerStep::None,
                valid: false,
            },
        })
    }
    pub fn part(&self) -> TriggerPart {
        self.part
    }
    pub fn step(&self) -> TriggerStep {
        self.progress.step
    }
    /// Source updateUI snapshot is separate from the handler's subsequent
    /// stop/reset. Complete is broadcast with its prior valid=true.
    pub fn progress(&self) -> TriggerCalibrationProgress {
        self.progress
    }
    pub fn valid(&self) -> bool {
        self.valid
    }
    pub fn poll_delay() -> Duration {
        Duration::from_millis(10)
    }
    pub fn movement_throttle() -> Duration {
        Duration::from_millis(33)
    }
    fn valid_value(&self, value: f64) -> bool {
        value.is_finite()
            && value >= self.product.params.range.min
            && value <= self.product.params.range.max
    }
    fn press_threshold(&self) -> f64 {
        self.product.params.trigger.press_validity_percent / 100.
            * self.product.params.trigger.trigger_max
    }
    fn headroom(&self, resting: f64) -> f64 {
        resting - self.product.params.range.min
    }
    fn release_threshold(&self, resting: f64) -> f64 {
        resting
            - (self.headroom(resting) * 0.01).min(
                self.product.params.trigger.release_validity_percent / 100.
                    * self.product.params.trigger.trigger_max,
            )
    }
    fn prepare(&mut self, step: TriggerStep, resting: RestingTriggerValue, value: Option<f64>) {
        self.step = step;
        self.valid = false;
        self.previous = None;
        self.stable_polls = 0;
        self.travelled = false;
        self.returned = false;
        self.movement = Some(TriggerMovement {
            resting,
            value: match value {
                Some(value) => TriggerMovementValue::Observed(value),
                None => TriggerMovementValue::ResetResting,
            },
        });
        self.progress = TriggerCalibrationProgress { step, valid: false };
    }
    /// Source start makes two distinct raw queries: first captures resting,
    /// second initializes Press movement. Its wrapper returns no value on raw
    /// query failure. Only this start baseline uses Range.max; prepare retains
    /// reset movement when its separate query is unavailable or out of range.
    pub fn start(&mut self, transport: &mut dyn TriggerCalibrationTransport) -> anyhow::Result<()> {
        self.stop();
        let resting = match self.read_value(transport.raw_analog_input()) {
            Some(value) => RestingTriggerValue::Observed(value),
            None => RestingTriggerValue::SourceRangeMaximum,
        };
        let value = self.read_value(transport.raw_analog_input());
        self.active = true;
        self.prepare(TriggerStep::Press, resting, value);
        Ok(())
    }
    fn read_value(&self, raw: anyhow::Result<RawAnalogInput>) -> Option<f64> {
        let value = raw.ok()?.trigger(self.part);
        self.valid_value(value).then_some(value)
    }
    fn fail(&mut self) {
        self.progress = TriggerCalibrationProgress {
            step: TriggerStep::Error,
            valid: false,
        };
        self.stop();
    }
    pub fn stop(&mut self) {
        self.active = false;
        self.step = TriggerStep::None;
        self.valid = false;
        self.movement = None;
        self.previous = None;
        self.stable_polls = 0;
        self.travelled = false;
        self.returned = false;
        self.sample_min = None;
        self.sample_max = None;
    }
    /// Source polling skips movement/stability assignment on an unavailable or
    /// invalid query, then still validates the retained movement. A previously
    /// valid hold therefore survives that skipped poll; hold sampling itself
    /// continues to require five valid actual reads.
    pub fn poll(&mut self, raw: anyhow::Result<RawAnalogInput>) -> anyhow::Result<HoldChange> {
        if !self.active || !matches!(self.step, TriggerStep::Press | TriggerStep::Release) {
            return Ok(HoldChange::Unchanged);
        }
        let resting = self
            .movement
            .as_ref()
            .context("Trigger movement state is missing")?
            .resting(self.product.params.range.max);
        if let Some(value) = self.read_value(raw) {
            if self
                .previous
                .is_some_and(|previous| (value - previous).abs() <= self.headroom(resting) * 0.01)
            {
                self.stable_polls = self.stable_polls.saturating_add(1);
            } else {
                self.stable_polls = 0;
            }
            self.previous = Some(value);
            self.movement.as_mut().unwrap().value = TriggerMovementValue::Observed(value);
        }
        let value = self
            .movement
            .as_ref()
            .unwrap()
            .value(self.product.params.range.max);
        let previous_valid = self.valid;
        self.valid = match self.step {
            TriggerStep::Press => {
                self.travelled |= value <= self.press_threshold();
                self.travelled && value <= self.press_threshold() && self.stable_polls >= 5
            }
            TriggerStep::Release => {
                self.returned |= value >= self.release_threshold(resting);
                self.returned && value >= self.release_threshold(resting) && self.stable_polls >= 5
            }
            _ => false,
        };
        self.progress = TriggerCalibrationProgress {
            step: self.step,
            valid: self.valid,
        };
        Ok(match (previous_valid, self.valid) {
            (false, true) => HoldChange::Start(self.step.hold_duration().unwrap()),
            (true, false) => HoldChange::Cancel,
            _ => HoldChange::Unchanged,
        })
    }
    /// Exact source {x:0,y:0,t} storage percentage. No elapsed-time value.
    pub fn movement_percentage(&self) -> Option<f64> {
        let movement = self.movement.as_ref()?;
        let resting = movement.resting(self.product.params.range.max);
        let value = (self.release_threshold(resting)
            - movement.value(self.product.params.range.max))
            / (self.release_threshold(resting) - self.press_threshold())
            * 100.;
        value
            .is_finite()
            .then(|| 100. - js_round(value).clamp(0., 100.))
    }
    pub fn validate_range(&self, range: TriggerCalibrationRange) -> bool {
        let limits = if self.part == TriggerPart::Left {
            &self.product.params.trigger.left
        } else {
            &self.product.params.trigger.right
        };
        let min = f64::from(range.min);
        let max = f64::from(range.max);
        min > self.product.params.range.min
            && max < self.product.params.range.max
            && min < limits.press_threshold
            && max > limits.release_threshold
            && max - min > self.product.params.trigger.validation_range
    }
    fn computed_range(&self) -> anyhow::Result<TriggerCalibrationRange> {
        let min = self
            .sample_min
            .context("Trigger press samples are missing")?;
        let max = self
            .sample_max
            .context("Trigger release samples are missing")?;
        let t = &self.product.params.trigger;
        // Source uses abs(Math.round(...)), not clamp or Rust's negative tie
        // rounding. Bound checks apply after this exact construction.
        let min = js_round(min - t.threshold_percent.min / 100. * t.trigger_max).abs();
        let max = js_round(max + t.threshold_percent.max / 100. * t.trigger_max).abs();
        ensure!(
            min.is_finite() && max.is_finite() && min <= 65535. && max <= 65535.,
            "Trigger range is unrepresentable"
        );
        Ok(TriggerCalibrationRange {
            min: min as u16,
            max: max as u16,
        })
    }
    /// Invoke only after the adapter's valid hold expires. It stops polling
    /// while collecting five actual samples, waiting 10ms between each.
    /// Success requires device write plus exact native calibration readback.
    pub fn hold_complete(
        &mut self,
        transport: &mut dyn TriggerCalibrationTransport,
        mut wait: impl FnMut(Duration) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        ensure!(
            self.active && self.valid && self.step.hold_duration().is_some(),
            "Trigger hold is no longer valid"
        );
        let result = self.sample_and_advance(transport, &mut wait);
        if result.is_err() {
            self.fail();
        }
        result
    }
    fn sample_and_advance(
        &mut self,
        transport: &mut dyn TriggerCalibrationTransport,
        wait: &mut impl FnMut(Duration) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let mut sum = 0.;
        for index in 0..5 {
            let value = transport
                .raw_analog_input()
                .context("Trigger sampling failed")?
                .trigger(self.part);
            ensure!(self.valid_value(value), "Trigger sampling failed");
            sum += value;
            if index < 4 {
                wait(Self::poll_delay())?;
            }
        }
        let sample = js_round(sum / 5.);
        if self.step == TriggerStep::Press {
            self.sample_min = Some(sample);
            let initial = self.read_value(transport.raw_analog_input());
            let resting = self
                .movement
                .as_ref()
                .context("Trigger movement state is missing")?
                .resting;
            self.prepare(TriggerStep::Release, resting, initial);
            return Ok(());
        }
        self.sample_max = Some(sample);
        let range = self.computed_range()?;
        ensure!(
            self.validate_range(range),
            "Trigger calculated value not within range"
        );
        transport
            .set_trigger_calibration(self.part, range)
            .context("Command Failed")?;
        let observed = transport
            .trigger_calibration(self.part)
            .context("Trigger calibration read back failed")?;
        ensure!(observed == range, "Trigger calibration read back mismatch");
        ensure!(
            self.validate_range(observed),
            "Trigger calculated value not within range"
        );
        self.progress = TriggerCalibrationProgress {
            step: TriggerStep::Complete,
            valid: self.valid,
        };
        self.stop();
        Ok(())
    }
}

fn js_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

#[cfg(test)]
#[path = "controller_calibration_tests.rs"]
mod tests;

// Generated typed table: razer-device/controller_calibration_current_params.
#[rustfmt::skip]
fn make_controller_calibration_current_params_table() -> Vec<Product> {
    vec![
        Product{
            product_id: 2676u32,
            params: Params{
                version: 3u8,
                range: Range{min:0.0f64,max:8192.0f64},
                trigger: TriggerParams{
                    threshold_percent: Range{min:5.0f64,max:2.0f64},
                    trigger_max: 2048.0f64,
                    left: Limits{press_threshold:500.0f64,release_threshold:1490.0f64},
                    right: Limits{press_threshold:600.0f64,release_threshold:1600.0f64},
                    validation_range: 300.0f64,
                    press_validity_percent: 18.0f64,
                    release_validity_percent: 11.0f64
                }
            }
        },
        Product{
            product_id: 2684u32,
            params: Params{
                version: 3u8,
                range: Range{min:0.0f64,max:8192.0f64},
                trigger: TriggerParams{
                    threshold_percent: Range{min:5.0f64,max:2.0f64},
                    trigger_max: 2048.0f64,
                    left: Limits{press_threshold:400.0f64,release_threshold:1400.0f64},
                    right: Limits{press_threshold:500.0f64,release_threshold:1500.0f64},
                    validation_range: 300.0f64,
                    press_validity_percent: 18.0f64,
                    release_validity_percent: 11.0f64
                }
            }
        }
    ]
}
