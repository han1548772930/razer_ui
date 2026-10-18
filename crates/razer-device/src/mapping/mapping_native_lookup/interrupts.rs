//! Retained interrupt registrations and physical-key suppression.
//! 8059c calls digital 8dd0e before analog 8e394 after notification.
use super::interrupt_registry::Registry;
use super::{Input, InputKind, Lookup, Phase, Resolved};
use std::collections::BTreeSet;

struct Registration {
    key_id: u8,
    input: Input,
}

#[derive(Default)]
pub struct AnalogInterrupts {
    registrations: Registry<Registration>,
    blocked: BTreeSet<u8>,
}

impl AnalogInterrupts {
    /// Register once at physical key discovery, after lookup selects the input.
    /// An empty list is interruptible; an absent list is not (input byte +48).
    pub fn register(&mut self, key_id: u8, input: &Input) {
        if input.interruptible {
            self.registrations.insert(
                input.stateless_id(),
                Registration {
                    key_id,
                    input: input.clone(),
                },
            );
        }
    }

    /// Returns key IDs for synthetic zero/release handling. The
    /// owner supplies each key's actual previous level and original resolution.
    /// These are release requests, not confirmation that output was delivered.
    /// Call `block` only when that key still has a physical previous sample.
    pub fn notify(&self, input: &Input) -> Vec<u8> {
        if input.phase() == Phase::Release {
            return Vec::new();
        }
        let identity = input.stateless_id();
        let mut releases = Vec::new();
        for (registered_id, registered) in self.registrations.iter() {
            if registered_id == &identity {
                continue;
            }
            let matches = if registered.input.interrupts.is_empty() {
                // 8e394 excludes the keyboard hook event corresponding to this
                // analog key only in the implicit "any other input" branch.
                !matches!((&registered.input.kind, &input.kind),
                    (InputKind::AnalogKey { scancode: analog_code, flag: analog_flag },
                     InputKind::Keyboard { scancode: code, flag })
                    if analog_code == code && (analog_flag ^ flag) < 2)
            } else {
                registered
                    .input
                    .interrupts
                    .iter()
                    .any(|interrupt| interrupt.stateless_id() == identity)
            };
            if matches {
                releases.push(registered.key_id);
            }
        }
        releases
    }

    pub fn blocked_keys(&self) -> &BTreeSet<u8> {
        &self.blocked
    }

    /// 6ae30 uses the same physical suppression set when a mapped analog key
    /// is released by a hypershift transition.
    pub fn block(&mut self, key_id: u8) {
        self.blocked.insert(key_id);
    }

    /// 79e9e clears suppression only when the key disappears from the physical
    /// report. Sparse zero-valued entries remain present and stay suppressed.
    pub fn remove_missing(&mut self, present: &BTreeSet<u8>) {
        self.blocked.retain(|key| present.contains(key));
    }

    /// AA588 clears registrations on data replacement, retaining suppression.
    pub fn clear_registrations(&mut self) {
        self.registrations.clear();
    }

    pub fn clear(&mut self) {
        self.registrations.clear();
        self.blocked.clear();
    }
}

/// The registered make remains available for stopping a macro after physical
/// release; only a currently held input has a synthetic release (8dd0e).
#[derive(Clone, Debug)]
pub struct DigitalInterrupt<O> {
    pub make: Resolved<O>,
    pub release: Option<Resolved<O>>,
}

impl<O: Clone> Lookup<O> {
    /// 8dd0e snapshots registered makes in native hash-chain order. The caller
    /// prepares and dispatches one request at a time: SnapTap replay may notify
    /// more inputs before the next request's physical hold is inspected.
    pub fn select_digital_interrupts(&self, input: &Input) -> Vec<Resolved<O>> {
        if input.phase() == Phase::Release {
            return Vec::new();
        }
        let identity = input.stateless_id();
        self.digital_interrupts
            .iter()
            .filter(|(registered_id, registered)| {
                **registered_id != identity
                    && (registered.input.interrupts.is_empty()
                        || registered
                            .input
                            .interrupts
                            .iter()
                            .any(|interrupt| interrupt.stateless_id() == identity))
            })
            .map(|(_, registered)| registered.clone())
            .collect()
    }

    /// Prepare just this request immediately before dispatching its retained
    /// release through SnapTap/composites, then stopping the matching macro.
    pub fn prepare_digital_interrupt(&mut self, make: Resolved<O>) -> DigitalInterrupt<O> {
        let identity = make.input.stateless_id();
        let release = self.held.iter().find_map(|(key, held)| {
            (held.make.input.stateless_id() == identity).then(|| {
                // 8dd0e inserts +960, retaining +1096 until the actual break
                // reaches 677ba. Repeated notifications may release again.
                self.suppressed.insert(key.clone());
                held.release.clone()
            })
        });
        DigitalInterrupt { make, release }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        mapping_native_action::{Output, matches_interrupted_macro},
        mapping_native_composite,
        mapping_native_lookup::{CompiledMapping, Modes, Selectors, Tables},
    };
    use std::time::Duration;

    // These tests have no reentrant dispatch between selected requests.
    fn interrupt(lookup: &mut Lookup<Output>, input: &Input) -> Vec<DigitalInterrupt<Output>> {
        lookup
            .select_digital_interrupts(input)
            .into_iter()
            .map(|make| lookup.prepare_digital_interrupt(make))
            .collect()
    }

    fn key(scancode: u16) -> Input {
        Input::new(InputKind::Keyboard { scancode, flag: 0 })
    }

    fn modes() -> Modes {
        Modes {
            mapping_enabled: true,
            hypershift: false,
            gaming: false,
            otfs: false,
            global_shortcuts_enabled: true,
        }
    }

    fn lookup(input: Input, make: Output, release: Output) -> Lookup<Output> {
        Lookup::new(
            Tables::new(
                vec![],
                vec![
                    CompiledMapping {
                        input: input.clone(),
                        selectors: Selectors::default(),
                        output: make,
                    },
                    CompiledMapping {
                        input: input.released(),
                        selectors: Selectors::default(),
                        output: release,
                    },
                ],
                vec![],
            )
            .unwrap(),
        )
    }

    fn macro_lookup(mut input: Input) -> Lookup<Output> {
        input.interruptible = true;
        lookup(
            input,
            Output::Macro {
                guid: "macro-a".into(),
                flag: 0,
                repeat: 2,
            },
            Output::Macro {
                guid: "macro-a".into(),
                flag: 1,
                repeat: 2,
            },
        )
    }

    #[test]
    fn digital_interrupt_keeps_physical_hold_and_registration_after_break() {
        let input = key(30);
        let mut lookup = macro_lookup(input.clone());
        let make = lookup.resolve(input.clone(), modes()).unwrap();
        let interruptions = interrupt(&mut lookup, &key(31));
        assert_eq!(interruptions.len(), 1);
        assert!(matches_interrupted_macro(&interruptions[0].make, &make));
        assert_eq!(
            interruptions[0].release.as_ref().unwrap().input.phase(),
            Phase::Release
        );
        assert_eq!(lookup.held_count(), 1);
        assert!(lookup.held_by_stateless(&input.stateless_id()).is_some());
        assert!(lookup.resolve(input.clone(), modes()).is_none());
        assert!(interrupt(&mut lookup, &key(32))[0].release.is_some());
        assert!(lookup.resolve(input.released(), modes()).is_none());
        assert_eq!(lookup.held_count(), 0);
        let after_break = interrupt(&mut lookup, &key(31));
        assert_eq!(after_break.len(), 1);
        assert!(after_break[0].release.is_none());
        assert!(matches_interrupted_macro(&after_break[0].make, &make));
        assert!(lookup.resolve(input, modes()).is_some());
    }

    #[test]
    fn digital_interrupt_registration_outlives_an_uninterrupted_physical_release() {
        let input = key(30);
        let mut lookup = macro_lookup(input.clone());
        lookup.resolve(input.clone(), modes()).unwrap();
        assert!(lookup.resolve(input.released(), modes()).is_some());
        let interruptions = interrupt(&mut lookup, &key(31));
        assert_eq!(interruptions.len(), 1);
        assert!(interruptions[0].release.is_none());
        lookup.drain_releases();
        assert!(interrupt(&mut lookup, &key(31)).is_empty());
    }

    #[test]
    fn digital_selection_retains_make_but_defers_suppression_and_held_lookup() {
        let input = key(30);
        let mut lookup = macro_lookup(input.clone());
        let make = lookup.resolve(input.clone(), modes()).unwrap();
        let selected = lookup.select_digital_interrupts(&key(31));
        assert_eq!(selected.len(), 1);
        // A replay callback can release the physical hold before preparation.
        // Selecting the request must not suppress that real break.
        assert!(lookup.resolve(input.released(), modes()).is_some());
        lookup.replace_tables(lookup.tables().clone());
        assert!(lookup.select_digital_interrupts(&key(31)).is_empty());
        let interruption = lookup.prepare_digital_interrupt(selected[0].clone());
        assert!(interruption.release.is_none());
        assert!(matches_interrupted_macro(&interruption.make, &make));
    }

    #[test]
    fn analog_interrupt_selection_uses_registration_chain_order() {
        let mut interrupts = AnalogInterrupts::default();
        for id in 30..=36 {
            let mut input = key(id);
            input.interruptible = true;
            interrupts.register(id as u8, &input);
        }
        assert_eq!(interrupts.notify(&key(40)), [36, 33, 35, 34, 32, 31, 30]);
        assert!(interrupts.blocked_keys().is_empty());
    }

    #[test]
    fn digital_interrupt_registration_obeys_list_presence_identity_and_notification_edge() {
        let input = key(30);
        let mut absent = lookup(input.clone(), Output::Disabled, Output::Disabled);
        absent.resolve(input.clone(), modes()).unwrap();
        assert!(interrupt(&mut absent, &key(31)).is_empty());

        let mut empty = macro_lookup(input.clone());
        empty.resolve(input.clone(), modes()).unwrap();
        assert!(interrupt(&mut empty, &input).is_empty());
        assert!(interrupt(&mut empty, &key(31).released()).is_empty());
        assert_eq!(
            interrupt(
                &mut empty,
                &Input::new(InputKind::ControlPodDial { clockwise: true })
            )
            .len(),
            1
        );

        let mut explicit_input = input.clone();
        let mut target = key(31);
        target.modifiers = 9;
        target.unit_id = Some(2);
        explicit_input.interrupts.push(target.clone());
        let mut explicit = macro_lookup(explicit_input);
        explicit.resolve(input, modes()).unwrap();
        assert!(interrupt(&mut explicit, &key(31)).is_empty());
        target.modifiers = 0;
        assert_eq!(interrupt(&mut explicit, &target).len(), 1);
    }

    #[test]
    fn digital_interrupt_has_no_analog_own_keyboard_exception() {
        let input = Input::new(InputKind::AnalogKey {
            scancode: 30,
            flag: 0,
        });
        let mut lookup = macro_lookup(input.clone());
        lookup.resolve(input, modes()).unwrap();
        assert_eq!(interrupt(&mut lookup, &key(30)).len(), 1);
    }

    #[test]
    fn replacing_tables_clears_registrations_without_clearing_physical_suppression() {
        let input = key(30);
        let mut lookup = macro_lookup(input.clone());
        lookup.resolve(input.clone(), modes()).unwrap();
        assert_eq!(interrupt(&mut lookup, &key(31)).len(), 1);
        lookup.replace_tables(lookup.tables().clone());
        assert!(interrupt(&mut lookup, &key(31)).is_empty());
        assert_eq!(lookup.held_count(), 1);
        assert!(lookup.resolve(input.clone(), modes()).is_none());
        assert!(lookup.resolve(input.released(), modes()).is_none());
        lookup.resolve(input, modes()).unwrap();
        assert_eq!(interrupt(&mut lookup, &key(31)).len(), 1);
    }

    #[test]
    fn digital_interrupt_release_consumes_pending_modtap_before_timer_can_fire() {
        let mut input = key(30);
        input.interruptible = true;
        let output = Output::ModTap {
            milliseconds: 100,
            tap: Box::new(Output::Macro {
                guid: "tap".into(),
                flag: 0,
                repeat: 1,
            }),
            hold: Box::new(Output::Macro {
                guid: "hold".into(),
                flag: 0,
                repeat: 1,
            }),
        };
        let mut lookup = lookup(input.clone(), output.clone(), output);
        let mut composite = mapping_native_composite::State::default();
        let make = lookup.resolve(input.clone(), modes()).unwrap();
        assert!(
            composite
                .apply_at(make, |id| lookup.held_by_stateless(id), Duration::ZERO)
                .unwrap()
                .is_empty()
        );
        let release = interrupt(&mut lookup, &key(31)).remove(0).release.unwrap();
        let dispatched = composite
            .apply_at(
                release,
                |id| lookup.held_by_stateless(id),
                Duration::from_millis(20),
            )
            .unwrap();
        assert_eq!(dispatched.len(), 2);
        assert_eq!(dispatched[0].resolved.input.phase(), Phase::Press);
        assert_eq!(dispatched[1].resolved.input.phase(), Phase::Release);
        assert!(composite.poll(Duration::from_millis(100)).is_empty());
        assert!(lookup.resolve(input.released(), modes()).is_none());
    }
}
