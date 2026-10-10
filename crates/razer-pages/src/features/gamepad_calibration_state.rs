//! 2636 current XM/xM and controllerCalibrationReducer contract.
//! Local intentions are never device acknowledgements. No hardware writes live here.
use std::f64::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalibrationAction {
    Start,
    Stop,
    RotateComplete,
}

#[derive(Clone, Debug)]
pub struct CalibrationIntent {
    generation: u64,
    part: u8,
    action: CalibrationAction,
}

impl CalibrationIntent {
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn part(&self) -> u8 {
        self.part
    }
    pub fn action(&self) -> CalibrationAction {
        self.action
    }
}

#[derive(Clone, Debug)]
enum ObservationValue {
    Progress { step: i8, valid: bool },
    Position { x: f64, y: f64 },
    Unavailable,
}

/// Supply only fresh, genuine service observations, in the source tester's
/// -1000..1000 axis units. `sequence` must increase within this generation.
#[derive(Clone, Debug)]
pub struct CalibrationObservation {
    generation: u64,
    sequence: u64,
    part: u8,
    value: ObservationValue,
}

impl CalibrationObservation {
    pub fn progress(
        generation: u64,
        sequence: u64,
        part: u8,
        step: i8,
        valid: bool,
    ) -> Option<Self> {
        if !(1..=2).contains(&part) || !(-1..=6).contains(&step) {
            return None;
        }
        Some(Self {
            generation,
            sequence,
            part,
            value: ObservationValue::Progress { step, valid },
        })
    }
    pub fn position(generation: u64, sequence: u64, part: u8, x: f64, y: f64) -> Option<Self> {
        if !(1..=2).contains(&part)
            || !x.is_finite()
            || !y.is_finite()
            || x.abs() > 1000.
            || y.abs() > 1000.
        {
            return None;
        }
        Some(Self {
            generation,
            sequence,
            part,
            value: ObservationValue::Position { x, y },
        })
    }
    /// 2676/2684 CALIBRATION_USER_MOVEMENT storage payload: percentages,
    /// clamped by the source popup and projected as x*10, -y*10.
    pub fn user_movement(generation: u64, sequence: u64, part: u8, x: f64, y: f64) -> Option<Self> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        Self::position(
            generation,
            sequence,
            part,
            x.clamp(-100., 100.) * 10.,
            -y.clamp(-100., 100.) * 10.,
        )
    }
    pub fn unavailable(generation: u64, sequence: u64) -> Self {
        Self {
            generation,
            sequence,
            part: 0,
            value: ObservationValue::Unavailable,
        }
    }
}

#[derive(Default)]
pub struct CalibrationState {
    generation: u64,
    sequence: Option<u64>,
    intent: Option<CalibrationIntent>,
    pub(super) step: i8,
    pub(super) part: u8,
    pub(super) valid: bool,
    pub(super) positions: [Option<(f64, f64)>; 2],
    pub(super) rotations: u8,
    last_angle: Option<f64>,
    accumulated_angle: f64,
    pub(super) observed: bool,
    rotate_complete_requested: bool,
}

impl CalibrationState {
    pub(super) fn generation(&self) -> u64 {
        self.generation
    }
    pub(super) fn intent(&self) -> Option<&CalibrationIntent> {
        self.intent.as_ref()
    }
    pub(super) fn request(
        &mut self,
        action: CalibrationAction,
        part: u8,
    ) -> Option<CalibrationIntent> {
        if !(1..=2).contains(&part) {
            return None;
        }
        if action == CalibrationAction::RotateComplete {
            if self.part != part
                || self.step != 5
                || !self.valid
                || self.rotations != 3
                || self.rotate_complete_requested
            {
                return None;
            }
            self.rotate_complete_requested = true;
            let intent = CalibrationIntent {
                generation: self.generation,
                part,
                action,
            };
            self.intent = Some(intent.clone());
            return Some(intent);
        }
        if action == CalibrationAction::Start && !matches!(self.step, 0 | -1)
            || action == CalibrationAction::Stop && self.step == 0
        {
            return None;
        }
        self.generation = self.generation.wrapping_add(1);
        self.sequence = None;
        self.step = if action == CalibrationAction::Start {
            1
        } else {
            0
        };
        self.part = part;
        self.valid = false;
        self.positions = [None, None];
        self.observed = false;
        self.reset_rotation();
        let intent = CalibrationIntent {
            generation: self.generation,
            part,
            action,
        };
        self.intent = Some(intent.clone());
        Some(intent)
    }
    fn reset_rotation(&mut self) {
        self.rotations = 0;
        self.last_angle = None;
        self.accumulated_angle = 0.;
        self.rotate_complete_requested = false;
    }
    pub(super) fn observe(&mut self, observation: CalibrationObservation) -> bool {
        if observation.generation != self.generation
            || self
                .sequence
                .is_some_and(|last| observation.sequence <= last)
        {
            return false;
        }
        match observation.value {
            ObservationValue::Unavailable => {
                // Disconnect invalidates all pending observations without turning
                // missing data into the source's explicit calibration error.
                self.generation = self.generation.wrapping_add(1);
                self.positions = [None, None];
                self.valid = false;
                self.observed = false;
                self.step = 0;
                self.intent = None;
                self.sequence = None;
                self.reset_rotation();
                return true;
            }
            ObservationValue::Progress { step, valid } => {
                if self.step == 0 || observation.part != self.part {
                    return false;
                }
                if self.step != step {
                    self.reset_rotation();
                }
                self.step = step;
                self.valid = valid;
                self.observed = true;
            }
            ObservationValue::Position { x, y } => {
                if self.step != 0 && observation.part != self.part {
                    return false;
                }
                self.positions[usize::from(observation.part - 1)] = Some((x, y));
                if self.step == 5 && self.rotations < 3 {
                    // Current xM: m >= 62.05 on its 150px canvas, not a made-up
                    // timer. Preserve signed angular accumulation and wrap.
                    let dx = x * 141. / 2000.;
                    let dy = y * 141. / 2000.;
                    if dx.hypot(dy) >= 62.05 {
                        let angle = dy.atan2(dx);
                        if let Some(last) = self.last_angle {
                            let mut delta = angle - last;
                            if delta > PI {
                                delta -= TAU;
                            }
                            if delta < -PI {
                                delta += TAU;
                            }
                            self.accumulated_angle += delta;
                            if self.accumulated_angle.abs() >= TAU {
                                self.rotations = (self.rotations + 1).min(3);
                                self.accumulated_angle = 0.;
                            }
                        }
                        self.last_angle = Some(angle);
                    }
                }
            }
        }
        self.sequence = Some(observation.sequence);
        true
    }
    pub(super) fn is_complete(&self) -> bool {
        self.observed && (self.step == 6 || self.step == 5 && self.valid && self.rotations == 3)
    }
}
