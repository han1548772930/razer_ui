//! Digital Snap Tap arbitration (68d0c, 82482, 828c8, 82f94).
//! Effects suspend transitions so replayed input notifications observe the
//! state at the same point as the native callback, before subsequent changes.
use crate::{
    macro_runtime::Event,
    mapping_native_action::Output,
    mapping_native_lookup::{Phase, Resolved, ResolvedOutput},
};
use std::collections::BTreeMap;

#[path = "mapping_native_snaptap/analog.rs"]
mod analog;
pub use analog::{AnalogEffect, AnalogKeyState, AnalogState, AnalogTransition};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Policy {
    #[default]
    LastInput,
    Priority,
    CompareLevel,
    Neutral,
    Unknown(u32),
}

impl Policy {
    pub fn from_native(value: u32) -> Self {
        match value {
            0 => Self::LastInput,
            1 => Self::Priority,
            2 => Self::CompareLevel,
            3 => Self::Neutral,
            value => Self::Unknown(value),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Current,
    Replay {
        identity: String,
        phase: Phase,
    },
    /// Native 82443 selects an inclusive 15..35 millisecond interval.
    Pause,
}

#[derive(Default)]
struct Group {
    normal: Vec<String>,
    priority: Vec<String>,
}

#[derive(Default)]
pub struct DigitalState {
    active: BTreeMap<String, bool>,
    groups: BTreeMap<u32, Group>,
}

pub struct Transition {
    identity: String,
    group: u32,
    priority: bool,
    policy: Policy,
    phase: Phase,
    stage: u8,
    replaced: bool,
    emit: bool,
    resume: bool,
}

impl DigitalState {
    pub fn begin(
        &self,
        resolved: &Resolved<Output>,
        policy: Policy,
    ) -> anyhow::Result<Option<Transition>> {
        let Some(group) = resolved.selectors.snap_tap else {
            return Ok(None);
        };
        // 68d0c bypasses clipboard, dpad, hypershift and modtap outputs.
        let ResolvedOutput::Mapped(output) = &resolved.output else {
            return Ok(None);
        };
        if matches!(
            output,
            Output::Event(Event::Clipboard(_))
                | Output::Dpad(_)
                | Output::Hypershift { .. }
                | Output::ModTap { .. }
        ) || resolved.input.phase() == Phase::Instant
        {
            return Ok(None);
        }
        anyhow::ensure!(
            !matches!(policy, Policy::CompareLevel | Policy::Unknown(_)),
            "Snap Tap policy {policy:?} is not supported for digital input"
        );
        Ok(Some(Transition {
            identity: resolved.input.stateless_id(),
            group,
            priority: resolved.selectors.snap_tap_prioritize,
            policy,
            phase: resolved.input.phase(),
            stage: 0,
            replaced: false,
            emit: true,
            resume: false,
        }))
    }

    /// 677ba/68c96 remove this record on physical release. An inactive key
    /// still traverses arbitration with an empty current-output callback.
    pub fn physical_release(&mut self, identity: &str) -> bool {
        self.active.remove(identity) == Some(false)
    }

    /// 6a5e0/6ae30 consume inactive records and skip their direct release.
    /// Active records and arbitration lists survive a mode transition.
    pub fn remove_inactive(&mut self, identity: &str) -> bool {
        if self.active.get(identity) == Some(&false) {
            self.active.remove(identity);
            true
        } else {
            false
        }
    }

    /// 832e4 changes the active flag only after finding a retained lookup pair.
    pub fn replayed(&mut self, identity: &str, phase: Phase) -> bool {
        let Some(active) = self.active.get_mut(identity) else {
            return false;
        };
        *active = phase == Phase::Press;
        true
    }

    pub fn clear(&mut self) {
        self.active.clear();
        self.groups.clear();
    }

    pub fn next(&mut self, transition: &mut Transition) -> Option<Effect> {
        let t = transition;
        loop {
            let group = self.groups.entry(t.group).or_default();
            match (t.phase, t.stage) {
                (Phase::Press, 0) => {
                    t.stage = 1;
                    let previous = if t.policy == Policy::Priority {
                        if !t.priority && !group.priority.is_empty() {
                            t.emit = false;
                            None
                        } else {
                            group
                                .priority
                                .last()
                                .or_else(|| group.normal.last())
                                .cloned()
                        }
                    } else {
                        if t.policy == Policy::Neutral {
                            t.emit = group.normal.is_empty();
                        }
                        group.normal.last().cloned()
                    };
                    if let Some(identity) = previous.filter(|id| id != &t.identity) {
                        t.replaced = true;
                        return Some(Effect::Replay {
                            identity,
                            phase: Phase::Release,
                        });
                    }
                }
                (Phase::Press, 1) => {
                    t.stage = 2;
                    if t.replaced && t.policy != Policy::Neutral {
                        return Some(Effect::Pause);
                    }
                }
                (Phase::Press, 2) => {
                    t.stage = 3;
                    self.active.insert(t.identity.clone(), t.emit);
                    let list = if t.policy == Policy::Priority && t.priority {
                        &mut group.priority
                    } else {
                        &mut group.normal
                    };
                    list.retain(|id| id != &t.identity);
                    list.push(t.identity.clone());
                    if t.emit {
                        return Some(Effect::Current);
                    }
                }
                (Phase::Release, 0) => {
                    t.stage = 1;
                    let list = if t.policy == Policy::Priority && t.priority {
                        &mut group.priority
                    } else {
                        &mut group.normal
                    };
                    let was_last = list.last() == Some(&t.identity);
                    t.emit = match t.policy {
                        Policy::Priority => list.is_empty() || was_last,
                        Policy::Neutral => list.len() == 1,
                        _ => true,
                    };
                    if t.policy != Policy::Neutral || t.emit {
                        if let Some(active) = self.active.get_mut(&t.identity) {
                            *active = false;
                        }
                    }
                    if t.policy != Policy::Neutral {
                        list.retain(|id| id != &t.identity);
                    }
                    if t.policy == Policy::Priority && !t.priority && !group.priority.is_empty() {
                        t.emit = false;
                    }
                    t.resume = match t.policy {
                        Policy::LastInput => was_last,
                        Policy::Priority => t.emit,
                        Policy::Neutral => true,
                        _ => false,
                    };
                    if t.emit {
                        return Some(Effect::Current);
                    }
                }
                (Phase::Release, 1) => {
                    t.stage = 2;
                    if t.policy == Policy::Neutral {
                        group.normal.retain(|id| id != &t.identity);
                    }
                    let available = if t.policy == Policy::Priority {
                        !group.priority.is_empty() || !group.normal.is_empty()
                    } else if t.policy == Policy::Neutral {
                        group.normal.len() == 1
                    } else {
                        !group.normal.is_empty()
                    };
                    t.resume &= available;
                    if t.resume && t.policy != Policy::Neutral {
                        return Some(Effect::Pause);
                    }
                }
                (Phase::Release, 2) => {
                    t.stage = 3;
                    if t.resume {
                        let identity = if t.policy == Policy::Priority {
                            group.priority.last().or_else(|| group.normal.last())
                        } else {
                            group.normal.last()
                        };
                        if let Some(identity) = identity {
                            return Some(Effect::Replay {
                                identity: identity.clone(),
                                phase: Phase::Press,
                            });
                        }
                    }
                }
                _ => return None,
            }
        }
    }
}

#[cfg(test)]
#[path = "mapping_native_snaptap/tests.rs"]
mod tests;
