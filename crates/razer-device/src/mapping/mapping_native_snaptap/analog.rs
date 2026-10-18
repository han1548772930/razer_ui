//! Analog Snap Tap arbitration from 95f72 and 96450. Physical levels and
//! retained mappings remain owned by the device actor, outside this state.
use super::Policy;
use crate::mapping_native_lookup::Phase;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnalogKeyState {
    pub group: u32,
    pub active: bool,
    /// 9ac00 writes the second byte on synthetic break; an ordinary make
    /// changes only the active byte and retains this value.
    pub released_by_arbitration: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalogEffect {
    /// Invoke the current continuation when execute is true, then post the
    /// current Snap Tap event. A retained inactive release can notify only.
    Current {
        execute: bool,
    },
    Replay {
        key_id: u8,
        phase: Phase,
    },
    Pause,
}

#[derive(Default)]
struct Group {
    normal: Vec<u8>,
    priority: Vec<u8>,
}

#[derive(Default)]
pub struct AnalogState {
    keys: BTreeMap<u8, AnalogKeyState>,
    groups: BTreeMap<u32, Group>,
}

pub struct AnalogTransition {
    key_id: u8,
    group: u32,
    priority: bool,
    policy: Policy,
    phase: Phase,
    depth_only: bool,
    stage: u8,
    selected: Option<u8>,
    emit: bool,
    was_last: bool,
    level: u8,
}

impl AnalogState {
    /// The caller applies 95f72's mapped-output exclusions before entering.
    pub fn begin(
        &self,
        key_id: u8,
        group: u32,
        priority: bool,
        policy: Policy,
        phase: Phase,
    ) -> anyhow::Result<Option<AnalogTransition>> {
        if phase == Phase::Instant {
            return Ok(None);
        }
        anyhow::ensure!(
            !matches!(policy, Policy::Unknown(_)),
            "Unknown analog Snap Tap policy {policy:?}"
        );
        Ok(Some(AnalogTransition {
            key_id,
            group,
            priority,
            policy,
            phase,
            depth_only: false,
            stage: 0,
            selected: None,
            emit: true,
            was_last: false,
            level: 0,
        }))
    }

    /// 96450 runs when the threshold/rapid gate emits no current action. Its
    /// comparison applies only to policy 2, even between threshold crossings.
    pub fn begin_depth(
        &self,
        key_id: u8,
        group: u32,
        policy: Policy,
        phase: Phase,
    ) -> Option<AnalogTransition> {
        if policy != Policy::CompareLevel || phase == Phase::Instant {
            return None;
        }
        Some(AnalogTransition {
            key_id,
            group,
            priority: false,
            policy,
            phase,
            depth_only: true,
            stage: 0,
            selected: None,
            emit: true,
            was_last: false,
            level: 0,
        })
    }

    pub fn key_state(&self, key_id: u8) -> Option<AnalogKeyState> {
        self.keys.get(&key_id).copied()
    }

    /// Call only after 9ac00's tracked identity and retained make/break pair
    /// have both been found, immediately before notifying and dispatching it.
    pub fn replayed(&mut self, key_id: u8, phase: Phase) -> bool {
        let Some(key) = self.keys.get_mut(&key_id) else {
            return false;
        };
        match phase {
            Phase::Press => {
                key.active = true;
                key.released_by_arbitration = false;
            }
            Phase::Release => {
                key.active = false;
                key.released_by_arbitration = true;
            }
            Phase::Instant => return false,
        }
        true
    }

    /// 7eb2a runs after missing-key release. Priority lists at +1856 survive;
    /// only the key state and all normal lists at +1816 are cleaned here.
    pub fn physical_missing(&mut self, key_id: u8) {
        self.keys.remove(&key_id);
        for group in self.groups.values_mut() {
            group.normal.retain(|key| *key != key_id);
        }
    }

    pub fn clear(&mut self) {
        self.keys.clear();
        self.groups.clear();
    }

    /// The closure reads the actor's current physical map. Native comparisons
    /// read byte +18 of its WORD level, including V2 and missing-as-zero.
    pub fn next(
        &mut self,
        transition: &mut AnalogTransition,
        level: impl Fn(u8) -> Option<u16>,
    ) -> Option<AnalogEffect> {
        let level = |key| level(key).unwrap_or(0) as u8;
        if transition.depth_only {
            return self.next_depth(transition, level);
        }
        loop {
            match (transition.phase, transition.policy) {
                (Phase::Press, _) => {
                    if let Some(effect) = self.press(transition, &level) {
                        return Some(effect);
                    }
                }
                (Phase::Release, Policy::LastInput) => {
                    if let Some(effect) = self.release_last(transition) {
                        return Some(effect);
                    }
                }
                (Phase::Release, Policy::Priority) => {
                    if let Some(effect) = self.release_priority(transition) {
                        return Some(effect);
                    }
                }
                (Phase::Release, Policy::CompareLevel) => {
                    if let Some(effect) = self.release_compare(transition, &level) {
                        return Some(effect);
                    }
                }
                (Phase::Release, Policy::Neutral) => {
                    if let Some(effect) = self.release_neutral(transition) {
                        return Some(effect);
                    }
                }
                _ => return None,
            }
            if transition.stage == u8::MAX {
                return None;
            }
        }
    }

    fn make_state(&mut self, t: &AnalogTransition, active: bool) {
        self.keys
            .entry(t.key_id)
            .and_modify(|key| key.active = active)
            .or_insert(AnalogKeyState {
                group: t.group,
                active,
                released_by_arbitration: false,
            });
    }

    fn press(
        &mut self,
        t: &mut AnalogTransition,
        level: &impl Fn(u8) -> u8,
    ) -> Option<AnalogEffect> {
        match t.stage {
            0 => {
                t.stage = 1;
                let group = self.groups.entry(t.group).or_default();
                let previous = match t.policy {
                    Policy::Priority if !t.priority && !group.priority.is_empty() => {
                        t.emit = false;
                        None
                    }
                    Policy::Priority => group
                        .priority
                        .last()
                        .or_else(|| group.normal.last())
                        .copied(),
                    Policy::CompareLevel => {
                        t.level = level(t.key_id);
                        let active = group.normal.iter().copied().find(|key| {
                            *key != t.key_id && self.keys.get(key).is_some_and(|state| state.active)
                        });
                        if let Some(key) = active {
                            t.emit = t.level >= level(key);
                        }
                        active.filter(|_| t.emit)
                    }
                    _ => group.normal.last().copied(),
                };
                t.selected = previous.filter(|key| *key != t.key_id).filter(|key| {
                    !matches!(t.policy, Policy::Priority | Policy::CompareLevel) || *key != 0
                });
                if let Some(key_id) = t.selected {
                    return Some(AnalogEffect::Replay {
                        key_id,
                        phase: Phase::Release,
                    });
                }
            }
            1 => {
                t.stage = 2;
                // Analog neutral also waits after displacing a previous key.
                if t.selected.is_some() {
                    return Some(AnalogEffect::Pause);
                }
            }
            2 => {
                t.stage = 3;
                if t.policy == Policy::Neutral {
                    let empty = self.groups[&t.group].normal.is_empty();
                    self.keys.entry(t.key_id).or_insert(AnalogKeyState {
                        group: t.group,
                        active: false,
                        released_by_arbitration: false,
                    });
                    t.emit = empty;
                    if empty {
                        self.keys.get_mut(&t.key_id).unwrap().active = true;
                    }
                } else {
                    self.make_state(t, t.emit);
                }
                if t.policy == Policy::Priority {
                    self.append(t);
                }
                if t.emit {
                    return Some(AnalogEffect::Current { execute: true });
                }
            }
            3 => {
                t.stage = u8::MAX;
                if t.policy != Policy::Priority {
                    self.append(t);
                }
            }
            _ => {
                t.stage = u8::MAX;
            }
        }
        None
    }

    fn append(&mut self, t: &AnalogTransition) {
        let group = self.groups.get_mut(&t.group).unwrap();
        let list = if t.policy == Policy::Priority && t.priority {
            &mut group.priority
        } else {
            &mut group.normal
        };
        list.retain(|key| *key != t.key_id);
        list.push(t.key_id);
    }

    fn release_last(&mut self, t: &mut AnalogTransition) -> Option<AnalogEffect> {
        match t.stage {
            0 => {
                t.stage = 1;
                t.was_last =
                    self.groups.entry(t.group).or_default().normal.last() == Some(&t.key_id);
                if let Some(state) = self.keys.get_mut(&t.key_id) {
                    let execute = state.active;
                    if execute {
                        state.active = false;
                        state.released_by_arbitration = false;
                    }
                    return Some(AnalogEffect::Current { execute });
                }
            }
            1 => {
                t.stage = 2;
                let list = &mut self.groups.get_mut(&t.group).unwrap().normal;
                list.retain(|key| *key != t.key_id);
                if t.was_last && !list.is_empty() {
                    return Some(AnalogEffect::Pause);
                }
                t.stage = u8::MAX;
            }
            2 => {
                t.stage = u8::MAX;
                if let Some(&key_id) = self.groups[&t.group].normal.last() {
                    return Some(AnalogEffect::Replay {
                        key_id,
                        phase: Phase::Press,
                    });
                }
            }
            _ => t.stage = u8::MAX,
        }
        None
    }

    fn release_priority(&mut self, t: &mut AnalogTransition) -> Option<AnalogEffect> {
        match t.stage {
            0 => {
                t.stage = 1;
                if let Some(state) = self.keys.get_mut(&t.key_id) {
                    state.active = false;
                    state.released_by_arbitration = false;
                }
                let group = self.groups.entry(t.group).or_default();
                let priority_present = !group.priority.is_empty();
                let list = if t.priority {
                    &mut group.priority
                } else {
                    &mut group.normal
                };
                t.emit = (t.priority || !priority_present)
                    && (list.is_empty() || list.last() == Some(&t.key_id));
                list.retain(|key| *key != t.key_id);
                if t.emit {
                    return Some(AnalogEffect::Current { execute: true });
                }
                t.stage = u8::MAX;
            }
            1 => {
                t.stage = 2;
                let group = &self.groups[&t.group];
                t.selected = group
                    .priority
                    .last()
                    .or_else(|| group.normal.last())
                    .copied()
                    .filter(|key| *key != 0);
                if t.selected.is_some() {
                    return Some(AnalogEffect::Pause);
                }
            }
            2 => {
                t.stage = u8::MAX;
                if let Some(key_id) = t.selected {
                    return Some(AnalogEffect::Replay {
                        key_id,
                        phase: Phase::Press,
                    });
                }
            }
            _ => t.stage = u8::MAX,
        }
        None
    }

    fn release_compare(
        &mut self,
        t: &mut AnalogTransition,
        level: &impl Fn(u8) -> u8,
    ) -> Option<AnalogEffect> {
        match t.stage {
            0 => {
                t.stage = 1;
                self.groups.entry(t.group).or_default();
                t.level = level(t.key_id);
                t.emit = self.keys.get(&t.key_id).is_some_and(|state| state.active);
                if t.emit {
                    return Some(AnalogEffect::Current { execute: true });
                }
            }
            1 => {
                t.stage = 2;
                if t.emit {
                    let mut best = t.level;
                    for &key in self.groups[&t.group].normal.iter().rev() {
                        let value = level(key);
                        if key != t.key_id && value > best {
                            best = value;
                            t.selected = Some(key);
                        }
                    }
                    t.selected = t.selected.filter(|key| *key != 0);
                    if t.selected.is_some() {
                        return Some(AnalogEffect::Pause);
                    }
                }
            }
            2 => {
                t.stage = 3;
                if let Some(key_id) = t.selected {
                    return Some(AnalogEffect::Replay {
                        key_id,
                        phase: Phase::Press,
                    });
                }
            }
            3 => {
                t.stage = u8::MAX;
                self.groups
                    .get_mut(&t.group)
                    .unwrap()
                    .normal
                    .retain(|key| *key != t.key_id);
            }
            _ => t.stage = u8::MAX,
        }
        None
    }

    fn release_neutral(&mut self, t: &mut AnalogTransition) -> Option<AnalogEffect> {
        match t.stage {
            0 => {
                t.stage = 1;
                if self.groups.entry(t.group).or_default().normal.len() == 1 {
                    if let Some(state) = self.keys.get_mut(&t.key_id) {
                        state.active = false;
                        state.released_by_arbitration = false;
                        return Some(AnalogEffect::Current { execute: true });
                    }
                }
            }
            1 => {
                t.stage = u8::MAX;
                let list = &mut self.groups.get_mut(&t.group).unwrap().normal;
                list.retain(|key| *key != t.key_id);
                if list.len() == 1 {
                    return Some(AnalogEffect::Replay {
                        key_id: list[0],
                        phase: Phase::Press,
                    });
                }
            }
            _ => t.stage = u8::MAX,
        }
        None
    }

    fn next_depth(
        &mut self,
        t: &mut AnalogTransition,
        level: impl Fn(u8) -> u8,
    ) -> Option<AnalogEffect> {
        match t.stage {
            0 => {
                t.stage = 1;
                let group = self.groups.entry(t.group).or_default();
                let current = level(t.key_id);
                let active = self.keys.get(&t.key_id).is_some_and(|state| state.active);
                if !active && t.phase == Phase::Press {
                    t.selected = group
                        .normal
                        .iter()
                        .copied()
                        .find(|key| {
                            *key != t.key_id
                                && current >= level(*key)
                                && self.keys.get(key).is_some_and(|state| state.active)
                        })
                        .filter(|key| *key != 0);
                    if let Some(key_id) = t.selected {
                        return Some(AnalogEffect::Replay {
                            key_id,
                            phase: Phase::Release,
                        });
                    }
                } else if active && t.phase == Phase::Release {
                    let mut best = current;
                    for &key in group.normal.iter().rev() {
                        let value = level(key);
                        if key != t.key_id
                            && value > best
                            && !self.keys.get(&key).is_some_and(|state| state.active)
                        {
                            best = value;
                            t.selected = Some(key);
                        }
                    }
                    t.selected = t.selected.filter(|key| *key != 0);
                    if t.selected.is_some() {
                        return Some(AnalogEffect::Replay {
                            key_id: t.key_id,
                            phase: Phase::Release,
                        });
                    }
                }
                t.stage = u8::MAX;
            }
            1 => {
                t.stage = 2;
                return Some(AnalogEffect::Pause);
            }
            2 => {
                t.stage = u8::MAX;
                let key_id = if t.phase == Phase::Press {
                    Some(t.key_id)
                } else {
                    t.selected
                };
                return key_id.map(|key_id| AnalogEffect::Replay {
                    key_id,
                    phase: Phase::Press,
                });
            }
            _ => t.stage = u8::MAX,
        }
        None
    }
}

#[cfg(test)]
#[path = "analog_tests.rs"]
mod tests;
