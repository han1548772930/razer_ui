//! Current 69728 dynamic-key arbitration, called after 677BA held tracking.
//! Each dkpId owns an insertion-ordered list of retained keyboard mappings.
use crate::{
    macro_runtime::Event,
    mapping_native_action::Output,
    mapping_native_lookup::{Phase, Resolved, ResolvedOutput},
};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct State {
    groups: BTreeMap<u32, Vec<Retained>>,
}
struct Retained {
    identity: String,
    mapping: Resolved<Output>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Current,
    /// 82443/11AA78 use the same inclusive 15..35 ms pause as Snap Tap.
    Pause,
    Replay(Resolved<Output>),
}
pub struct Transition {
    group: u32,
    identity: String,
    mapping: Resolved<Output>,
    phase: Phase,
    stage: Stage,
    matched: Option<usize>,
    replay: Option<Resolved<Output>>,
    handled: bool,
}
#[derive(Clone, Copy)]
enum Stage {
    Start,
    Commit,
    Replay,
    Done,
}
impl Transition {
    /// A missing release may replay a retained key but still fall through to
    /// ordinary dispatch. Consumption and emitted effects are independent.
    pub fn handled(&self) -> bool {
        self.handled
    }
}
impl State {
    pub fn begin(&self, resolved: &Resolved<Output>) -> Option<Transition> {
        let group = resolved.selectors.dynamic_key?;
        // Native output +0 returns type21 only for a direct keyboard output;
        // nested keyboard actions and non-keyboard mappings do not arbitrate.
        if !matches!(
            resolved.output,
            ResolvedOutput::Mapped(Output::Event(Event::Keyboard(_)))
        ) {
            return None;
        }
        Some(Transition {
            group,
            identity: resolved.input.stateless_id(),
            mapping: resolved.clone(),
            phase: resolved.input.phase(),
            stage: Stage::Start,
            matched: None,
            replay: None,
            handled: false,
        })
    }
    /// Suspend at each callback boundary: native inserts/erases only after
    /// the current-output callback, even if the actual output reports failure.
    pub fn next(&mut self, transition: &mut Transition) -> Option<Effect> {
        let group = self.groups.entry(transition.group).or_default();
        loop {
            match (transition.phase, transition.stage) {
                (Phase::Press, Stage::Start) => {
                    transition.stage = Stage::Commit;
                    if group.len() <= 1 {
                        return Some(Effect::Current);
                    }
                }
                (Phase::Press, Stage::Commit) => {
                    group.push(Retained {
                        identity: transition.identity.clone(),
                        mapping: transition.mapping.clone(),
                    });
                    transition.handled = true;
                    transition.stage = Stage::Done;
                }
                (Phase::Release, Stage::Start) => {
                    transition.matched = group
                        .iter()
                        .position(|key| key.identity == transition.identity);
                    transition.handled = transition.matched.is_some();
                    transition.stage = Stage::Commit;
                    if transition.matched.is_some_and(|index| index <= 1) {
                        return Some(Effect::Current);
                    }
                }
                (Phase::Release, Stage::Commit) => {
                    if let Some(index) = transition.matched {
                        group.remove(index);
                    }
                    transition.replay = group.get(1).map(|key| key.mapping.clone());
                    transition.stage = Stage::Replay;
                    if transition.replay.is_some() {
                        return Some(Effect::Pause);
                    }
                }
                (Phase::Release, Stage::Replay) => {
                    transition.stage = Stage::Done;
                    if let Some(mapping) = transition.replay.take() {
                        return Some(Effect::Replay(mapping));
                    }
                }
                _ => {
                    transition.stage = Stage::Done;
                    return None;
                }
            }
        }
    }
    /// 656CA destroys this queue after the device owner finishes releasing
    /// held outputs. Profile/mode/redirect operations do not clear it.
    pub fn clear(&mut self) {
        self.groups.clear();
    }
}

#[cfg(test)]
#[path = "mapping_native_dynamic_key_tests.rs"]
mod tests;
