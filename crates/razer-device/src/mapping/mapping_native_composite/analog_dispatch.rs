//! Analog mapping dispatch ordering (92a08/9333e/95660/96d2e/971c2).
use super::*;
use crate::mapping_native_lookup::InputIdentity;

#[derive(Clone, Copy, Debug)]
pub struct AnalogParameters {
    /// Make or break threshold selected by the observed direction (7e05c).
    pub actuation: u16,
    pub rapid: Option<RapidParameters>,
    /// 96d2e reads the descriptor continuous presence byte (+21), unlike the
    /// ordinary descriptor rapid path, which reads its value byte (+20).
    /// The descriptor owner supplies `continuous.is_some()` here.
    pub default_continuous: bool,
}

#[derive(Clone, Debug)]
pub enum AnalogDispatch {
    Action(Dispatch),
    Analog {
        resolved: Resolved<Output>,
        actuation: u16,
    },
}

/// A single source branch. Gate state changes only when the device actor
/// reaches this operation, after the previous branch's callbacks complete.
#[derive(Clone, Debug)]
pub struct AnalogOperation {
    sample: AnalogSample,
    resolved: Resolved<Output>,
    kind: OperationKind,
}

#[derive(Clone, Debug)]
enum OperationKind {
    Continuous,
    Gate {
        trigger: Option<u16>,
        actuation: u16,
        rapid: Option<RapidParameters>,
        release: Resolved<Output>,
    },
}

#[derive(Clone, Debug)]
pub struct AnalogTransition {
    pub key_id: u8,
    pub actuation: u16,
    pub resolved: Resolved<Output>,
    release: Resolved<Output>,
}

#[derive(Clone, Debug)]
pub enum AnalogStep {
    Continuous {
        key_id: u8,
        resolved: Resolved<Output>,
        actuation: u16,
        notify: bool,
    },
    Transition(AnalogTransition),
    Depth {
        key_id: u8,
        resolved: Resolved<Output>,
    },
}

#[derive(Default)]
pub(super) struct State {
    releases: BTreeMap<(u8, Option<u16>), HeldMapping<Output>>,
    continuous: BTreeMap<InputIdentity, Resolved<Output>>,
    pub(super) active: BTreeMap<u8, String>,
    pub(super) cascades: BTreeMap<(String, String), Resolved<Output>>,
}

impl State {
    pub(super) fn clear(&mut self) {
        self.releases.clear();
        self.continuous.clear();
        self.active.clear();
        self.cascades.clear();
    }
}

impl super::State {
    /// Native +1576 activity snapshot, separate from retained lookup mappings
    /// and from the individual trigger pairs used for release ownership.
    pub fn active_analog_keys(&self) -> Vec<(u8, String)> {
        self.analog_dispatch
            .active
            .iter()
            .map(|(&key, identity)| (key, identity.clone()))
            .collect()
    }
    pub fn active_analog_identity(&self, key: u8) -> Option<&str> {
        self.analog_dispatch.active.get(&key).map(String::as_str)
    }
    /// Expand wrappers without changing gate or composite state. The actor
    /// advances each branch and runs its notifications/output before the next.
    pub fn prepare_analog(
        sample: AnalogSample,
        parameters: AnalogParameters,
        pair: &HeldMapping<Output>,
    ) -> Vec<AnalogOperation> {
        if sample.previous == sample.value || sample.phase == Phase::Instant {
            return Vec::new();
        }
        let resolved = if sample.phase == Phase::Press {
            &pair.make
        } else {
            &pair.release
        };
        let mut dispatches = Vec::new();
        Self::analog_output(sample, parameters, resolved, &pair.release, &mut dispatches);
        dispatches
    }

    fn analog_output(
        sample: AnalogSample,
        parameters: AnalogParameters,
        resolved: &Resolved<Output>,
        release: &Resolved<Output>,
        dispatches: &mut Vec<AnalogOperation>,
    ) {
        match &resolved.output {
            ResolvedOutput::Mapped(Output::Analog(_)) => {
                dispatches.push(AnalogOperation {
                    sample,
                    resolved: resolved.clone(),
                    kind: OperationKind::Continuous,
                });
            }
            ResolvedOutput::Mapped(Output::DoubleMapping { analog, digital }) => {
                let analog = dispatch(resolved, Output::Analog(analog.clone()), None).resolved;
                Self::analog_output(sample, parameters, &analog, release, dispatches);
                let digital = dispatch(resolved, *digital.clone(), None).resolved;
                Self::analog_output(sample, parameters, &digital, release, dispatches);
            }
            ResolvedOutput::Mapped(Output::RapidTrigger {
                actuation,
                make_sensitivity,
                break_sensitivity,
                continuous,
                mapping,
            }) => {
                let parameters = AnalogParameters {
                    actuation: *actuation,
                    rapid: Some(RapidParameters {
                        actuation: *actuation,
                        make_sensitivity: *make_sensitivity,
                        break_sensitivity: *break_sensitivity,
                        continuous: continuous.unwrap_or(parameters.default_continuous),
                    }),
                    ..parameters
                };
                let mapping = dispatch(resolved, *mapping.clone(), None).resolved;
                Self::analog_output(sample, parameters, &mapping, release, dispatches);
            }
            ResolvedOutput::Mapped(Output::Triggers(triggers)) => {
                // 971c2 traverses IDs forward on make and backward on break.
                let ordered: Vec<_> = if sample.phase == Phase::Press {
                    triggers.iter().collect()
                } else {
                    triggers.iter().rev().collect()
                };
                for (id, trigger) in ordered {
                    let output = dispatch(resolved, trigger.output.clone(), None).resolved;
                    dispatches.push(AnalogOperation {
                        sample,
                        resolved: output,
                        kind: OperationKind::Gate {
                            trigger: Some(*id),
                            actuation: trigger.actuation,
                            rapid: None,
                            release: release.clone(),
                        },
                    });
                }
            }
            _ => dispatches.push(AnalogOperation {
                sample,
                resolved: resolved.clone(),
                kind: OperationKind::Gate {
                    trigger: Some(1),
                    actuation: parameters.actuation,
                    rapid: if matches!(
                        resolved.output,
                        ResolvedOutput::Mapped(Output::ModTap { .. })
                    ) {
                        None
                    } else {
                        parameters.rapid
                    },
                    release: release.clone(),
                },
            }),
        }
    }

    pub fn advance_analog(&mut self, operation: AnalogOperation) -> Result<Option<AnalogStep>> {
        let AnalogOperation {
            sample,
            resolved,
            kind,
        } = operation;
        let OperationKind::Gate {
            trigger,
            actuation,
            rapid,
            release,
        } = kind
        else {
            if sample.value == 0 {
                self.analog_dispatch
                    .continuous
                    .remove(&resolved.input.identity());
            } else {
                self.analog_dispatch
                    .continuous
                    .insert(resolved.input.identity(), resolved.clone());
            }
            return Ok(Some(AnalogStep::Continuous {
                key_id: sample.key_id,
                resolved,
                actuation: sample.value,
                // 9333e continuous branches notify on increasing depth and zero.
                notify: sample.value.wrapping_sub(1) >= sample.previous,
            }));
        };
        let gate = if rapid.is_some() { None } else { trigger };
        // 976fa erases key activity on a crossing break even when that trigger
        // pair was not inserted; other triggers for the key do not keep it active.
        if rapid.is_none()
            && sample.phase == Phase::Release
            && analog::trigger_crosses(sample, actuation)
        {
            self.analog_dispatch.active.remove(&sample.key_id);
        }
        let transitioned = match rapid {
            Some(parameters) => self.analog.rapid_transition(sample, parameters),
            None => self.analog.trigger_transition(
                sample,
                trigger.context("Missing analog trigger ID")?,
                actuation,
                true,
            ),
        };
        if !transitioned {
            if matches!(
                resolved.output,
                ResolvedOutput::Mapped(Output::Cascade(_) | Output::ModTap { .. })
            ) {
                return Ok(None);
            }
            // 9333e/92a08 invoke 96450 even without a threshold transition.
            return Ok(Some(AnalogStep::Depth {
                key_id: sample.key_id,
                resolved,
            }));
        }
        if sample.phase == Phase::Press {
            self.analog_dispatch
                .active
                .insert(sample.key_id, resolved.input.stateless_id());
        } else {
            self.analog_dispatch.active.remove(&sample.key_id);
        }
        let key = (sample.key_id, gate);
        let resolved = if sample.phase == Phase::Press {
            self.analog_dispatch.releases.insert(
                key,
                HeldMapping {
                    make: resolved.clone(),
                    release: release.clone(),
                },
            );
            resolved
        } else {
            let release = self
                .analog_dispatch
                .releases
                .remove(&key)
                .context("Analog gate has no retained release")?
                .release;
            let Some(release) = release_leaf(&release, trigger.unwrap_or(1)) else {
                return Ok(None);
            };
            release
        };
        let release = release_leaf(&release, trigger.unwrap_or(1)).unwrap_or(release);
        Ok(Some(AnalogStep::Transition(AnalogTransition {
            key_id: sample.key_id,
            actuation: sample.value,
            resolved,
            release,
        })))
    }

    /// The current-output callback runs only after Snap Tap arbitration accepts
    /// it. Suppressed callbacks must not mutate cascade or ModTap state.
    pub fn dispatch_analog(
        &mut self,
        transition: AnalogTransition,
        held: impl Fn(&str) -> Option<HeldMapping<Output>>,
        now: Duration,
    ) -> Result<Vec<Dispatch>> {
        let AnalogTransition {
            resolved, release, ..
        } = transition;
        if matches!(resolved.output, ResolvedOutput::Mapped(Output::Cascade(_))) {
            return self.analog_cascade(&resolved, &release);
        }
        let analog_held: Vec<_> = self
            .analog_dispatch
            .releases
            .iter()
            .filter_map(|((_, trigger), pair)| {
                Some(HeldMapping {
                    make: pair.make.clone(),
                    release: release_leaf(&pair.release, trigger.unwrap_or(1))?,
                })
            })
            .collect();
        self.apply_at(
            resolved,
            |id| {
                held(id).or_else(|| {
                    analog_held
                        .iter()
                        .find(|pair| pair.make.input.stateless_id() == id)
                        .cloned()
                })
            },
            now,
        )
    }

    /// Dispatch these before clearing lookup pairs and closing the transport.
    /// Only gates that actually made are released; continuous contributions
    /// receive their own zero update. Transport confirmation remains external.
    pub fn drain_analog_dispatches(
        &mut self,
        held: impl Fn(&str) -> Option<HeldMapping<Output>>,
        now: Duration,
    ) -> Result<Vec<AnalogDispatch>> {
        let mut dispatches = Vec::new();
        self.analog_dispatch.active.clear();
        for ((_, trigger), pair) in std::mem::take(&mut self.analog_dispatch.releases)
            .into_iter()
            .rev()
        {
            let Some(release) = release_leaf(&pair.release, trigger.unwrap_or(1)) else {
                continue;
            };
            let actions = if matches!(release.output, ResolvedOutput::Mapped(Output::Cascade(_))) {
                self.analog_cascade(&release, &release)?
            } else {
                self.apply_at(release, &held, now)?
            };
            dispatches.extend(actions.into_iter().map(AnalogDispatch::Action));
        }
        for resolved in std::mem::take(&mut self.analog_dispatch.continuous).into_values() {
            dispatches.push(AnalogDispatch::Analog {
                resolved,
                actuation: 0,
            });
        }
        self.analog.clear();
        self.analog_dispatch.active.clear();
        self.analog_dispatch.cascades.clear();
        Ok(dispatches)
    }
}

// Make and break mappings are selected independently by 909da. Preserve the
// complete release object as 976fa does, then follow its own wrapper structure.
// A trigger absent from the release dictionary has no release branch to emit.
fn release_leaf(release: &Resolved<Output>, trigger: u16) -> Option<Resolved<Output>> {
    let mut resolved = release.clone();
    loop {
        let output = match &resolved.output {
            ResolvedOutput::Mapped(Output::DoubleMapping { digital, .. }) => *digital.clone(),
            ResolvedOutput::Mapped(Output::RapidTrigger { mapping, .. }) => *mapping.clone(),
            ResolvedOutput::Mapped(Output::Triggers(triggers)) => {
                triggers.get(&trigger)?.output.clone()
            }
            _ => return Some(resolved),
        };
        resolved.output = ResolvedOutput::Mapped(output);
    }
}
