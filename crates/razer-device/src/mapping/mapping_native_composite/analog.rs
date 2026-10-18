//! Per-device analog transition gates from 978cc, 94a0e, 9429b and 976fa.
use super::Phase;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug)]
pub struct AnalogSample {
    pub key_id: u8,
    pub phase: Phase,
    pub previous: u16,
    pub value: u16,
    /// The descriptor's explicit maximum, or the source-defined report-width maximum.
    pub maximum: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct RapidParameters {
    pub actuation: u16,
    pub make_sensitivity: u16,
    pub break_sensitivity: u16,
    /// Resolved by the mapping/descriptor owner, including the optional mapping override.
    pub continuous: bool,
}

struct Extreme {
    value: u16,
    pressed: bool,
}

#[derive(Default)]
pub(super) struct State {
    rapid: BTreeMap<u8, Extreme>,
    triggers: BTreeSet<(u8, u16)>,
}

impl State {
    pub(super) fn clear(&mut self) {
        self.rapid.clear();
        self.triggers.clear();
    }

    pub(super) fn rapid_transition(
        &mut self,
        sample: AnalogSample,
        parameters: RapidParameters,
    ) -> bool {
        if sample.previous == sample.value || sample.phase == Phase::Instant {
            return false;
        }
        let Some(extreme) = self.rapid.get_mut(&sample.key_id) else {
            if sample.phase == Phase::Press && sample.value >= parameters.actuation {
                self.rapid.insert(
                    sample.key_id,
                    Extreme {
                        value: sample.value,
                        pressed: true,
                    },
                );
                return true;
            }
            return false;
        };
        if sample.phase == Phase::Press {
            if extreme.pressed {
                extreme.value = extreme.value.max(sample.value);
                return false;
            }
            let makes = if parameters.continuous || extreme.value >= parameters.actuation {
                i32::from(sample.value) - i32::from(extreme.value)
                    >= i32::from(parameters.make_sensitivity)
            } else {
                sample.value >= parameters.actuation
            };
            if makes {
                extreme.value = sample.value;
                extreme.pressed = true;
            }
            makes
        } else if extreme.pressed {
            if i32::from(extreme.value) - i32::from(sample.value)
                >= i32::from(parameters.break_sensitivity)
            {
                extreme.value = sample.value;
                extreme.pressed = false;
                true
            } else if sample.value == 0 {
                self.rapid.remove(&sample.key_id);
                true
            } else {
                false
            }
        } else {
            if sample.value == 0 {
                self.rapid.remove(&sample.key_id);
            } else {
                extreme.value = extreme.value.min(sample.value);
            }
            false
        }
    }

    pub(super) fn trigger_transition(
        &mut self,
        sample: AnalogSample,
        trigger_id: u16,
        actuation: u16,
        has_retained_mapping: bool,
    ) -> bool {
        if !trigger_crosses(sample, actuation) {
            return false;
        }
        match sample.phase {
            Phase::Press => {
                if !has_retained_mapping {
                    return false;
                }
                self.triggers.insert((sample.key_id, trigger_id))
            }
            Phase::Release => self.triggers.remove(&(sample.key_id, trigger_id)),
            Phase::Instant => false,
        }
    }
}

pub(super) fn trigger_crosses(sample: AnalogSample, actuation: u16) -> bool {
    if sample.previous == sample.value {
        return false;
    }
    match sample.phase {
        Phase::Press => sample.previous <= actuation && actuation <= sample.value,
        Phase::Release => {
            sample.value == 0
                || (sample.value <= actuation && actuation <= sample.previous)
                || (actuation == sample.maximum && sample.previous == sample.maximum)
        }
        Phase::Instant => false,
    }
}
