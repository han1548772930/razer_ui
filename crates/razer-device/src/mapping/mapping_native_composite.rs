//! Cascade, D-pad and ModTap ownership (8397a/84508/861ca/87b6e/8b460/8ba50).
//! Plans preserve the press-time lookup pair; only the transport owner can
//! confirm output delivery or report a failed release.
use crate::{
    mapping_native_action::{Branch, Output},
    mapping_native_lookup::{HeldMapping, Phase, Resolved, ResolvedOutput},
};
use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

#[path = "mapping_native_composite/analog.rs"]
mod analog;
#[path = "mapping_native_composite/analog_cascade.rs"]
mod analog_cascade;
#[path = "mapping_native_composite/analog_dispatch.rs"]
mod analog_dispatch;
#[path = "mapping_native_composite/modtap.rs"]
mod modtap;
pub use analog::{AnalogSample, RapidParameters};
pub use analog_dispatch::{
    AnalogDispatch, AnalogOperation, AnalogParameters, AnalogStep, AnalogTransition,
};

#[derive(Clone, Debug)]
pub struct Dispatch {
    pub resolved: Resolved<Output>,
    pub contribution_id: Option<String>,
}

#[derive(Default)]
pub struct State {
    cascades: BTreeMap<(String, String), Resolved<Output>>,
    dpad: VecDeque<(String, String)>,
    diagnostics: Vec<String>,
    omitted_diagnostics: u64,
    modtaps: modtap::State,
    analog: analog::State,
    analog_dispatch: analog_dispatch::State,
}

impl State {
    pub fn apply_at(
        &mut self,
        resolved: Resolved<Output>,
        held: impl Fn(&str) -> Option<HeldMapping<Output>>,
        now: Duration,
    ) -> Result<Vec<Dispatch>> {
        match &resolved.output {
            ResolvedOutput::Mapped(Output::Cascade(branches)) => {
                self.cascade(&resolved, branches, &held)
            }
            ResolvedOutput::Mapped(Output::Dpad(branches)) => self.dpad(&resolved, branches, &held),
            ResolvedOutput::Mapped(Output::ModTap { .. }) => self.modtaps.apply(resolved, now),
            _ => Ok(vec![Dispatch {
                resolved,
                contribution_id: None,
            }]),
        }
    }
    pub fn clear(&mut self) {
        self.cascades.clear();
        self.dpad.clear();
        self.modtaps.clear();
        self.analog.clear();
        self.analog_dispatch.clear();
    }

    pub fn poll(&mut self, now: Duration) -> Vec<Dispatch> {
        self.modtaps.poll(now)
    }

    pub fn rapid_transition(&mut self, sample: AnalogSample, parameters: RapidParameters) -> bool {
        self.analog.rapid_transition(sample, parameters)
    }

    pub fn trigger_transition(
        &mut self,
        sample: AnalogSample,
        trigger_id: u16,
        actuation: u16,
        has_retained_mapping: bool,
    ) -> bool {
        self.analog
            .trigger_transition(sample, trigger_id, actuation, has_retained_mapping)
    }

    pub fn take_diagnostics(&mut self) -> Vec<String> {
        let mut errors = std::mem::take(&mut self.diagnostics);
        if self.omitted_diagnostics != 0 {
            errors.push(format!(
                "{} additional composite errors",
                self.omitted_diagnostics
            ));
            self.omitted_diagnostics = 0;
        }
        errors
    }

    fn diagnostic(&mut self, error: String) {
        if self.diagnostics.len() < 64 {
            self.diagnostics.push(error);
        } else {
            self.omitted_diagnostics = self.omitted_diagnostics.saturating_add(1);
        }
    }

    fn cascade(
        &mut self,
        resolved: &Resolved<Output>,
        branches: &[Branch],
        held: &impl Fn(&str) -> Option<HeldMapping<Output>>,
    ) -> Result<Vec<Dispatch>> {
        let input_id = resolved.input.stateless_id();
        let phase = resolved.input.phase();
        let mut dispatches = Vec::new();
        for branch in branches {
            let condition = branch
                .input
                .as_ref()
                .map(|input| input.stateless_id())
                .unwrap_or_default();
            if !condition.is_empty() && held(&condition).is_none() {
                continue;
            }
            let pair = if condition.is_empty() {
                (input_id.clone(), condition)
            } else {
                (condition, input_id.clone())
            };
            match phase {
                Phase::Press => {
                    if self.cascades.contains_key(&pair) {
                        continue;
                    }
                    let release = held(&input_id)
                        .context("Cascade input has no retained press-time mapping")?
                        .release;
                    self.cascades.insert(pair, release);
                }
                Phase::Release if self.cascades.remove(&pair).is_none() => continue,
                _ => (),
            }
            dispatches.push(dispatch(resolved, branch.output.clone(), None));
            break;
        }
        if phase == Phase::Release {
            let stale: Vec<_> = self
                .cascades
                .keys()
                .filter(|(a, b)| {
                    (!a.is_empty() && held(a).is_none()) || (!b.is_empty() && held(b).is_none())
                })
                .cloned()
                .collect();
            for pair in stale {
                let release = self
                    .cascades
                    .remove(&pair)
                    .expect("Selected cascade is retained");
                let ResolvedOutput::Mapped(Output::Cascade(branches)) = &release.output else {
                    bail!("Cascade retained release is not a cascade mapping");
                };
                if let Some(branch) = branches.iter().find(|branch| {
                    branch.input.as_ref().is_none_or(|input| {
                        let id = input.stateless_id();
                        id == pair.0 || id == pair.1
                    })
                }) {
                    dispatches.push(dispatch(&release, branch.output.clone(), None));
                }
            }
        }
        Ok(dispatches)
    }

    fn dpad(
        &mut self,
        resolved: &Resolved<Output>,
        branches: &[Branch],
        held: &impl Fn(&str) -> Option<HeldMapping<Output>>,
    ) -> Result<Vec<Dispatch>> {
        let Some(branch) = branches.iter().find(|branch| {
            branch
                .input
                .as_ref()
                .is_none_or(|input| held(&input.stateless_id()).is_some())
        }) else {
            return Ok(Vec::new());
        };
        let input_id = resolved.input.stateless_id();
        let condition = branch
            .input
            .as_ref()
            .map(|input| input.stateless_id())
            .unwrap_or_default();
        let mut dispatches = Vec::new();
        if resolved.input.phase() == Phase::Press {
            self.dpad.push_back((input_id.clone(), condition.clone()));
            if self.dpad.len() >= 2 {
                let old = self.dpad.pop_front().expect("D-pad previous pair exists");
                match held(&old.0) {
                    Some(previous) => match dpad_named(&previous.release, &old.0, &old.1) {
                        Ok(Some(dispatch)) => dispatches.push(dispatch),
                        Ok(None) => (),
                        Err(error) => self.diagnostic(format!("{error:#}")),
                    },
                    None => self.diagnostic("D-pad previous input has no retained mapping".into()),
                }
            }
        }
        dispatches.push(dispatch(
            resolved,
            branch.output.clone(),
            digital_id(&branch.output, &input_id, &condition),
        ));
        if resolved.input.phase() == Phase::Release
            && let Some(old) = self.dpad.pop_front()
        {
            let other = if old.0 == input_id {
                old.1
            } else if old.1 == input_id {
                old.0
            } else {
                String::new()
            };
            if !other.is_empty()
                && let Some(remaining) = held(&other)
            {
                match dpad_named(&remaining.make, &other, "") {
                    Ok(Some(dispatch)) => dispatches.push(dispatch),
                    Ok(None) => (),
                    Err(error) => self.diagnostic(format!("{error:#}")),
                }
                self.dpad.push_back((other, String::new()));
            }
        }
        Ok(dispatches)
    }
}

fn dispatch(
    resolved: &Resolved<Output>,
    output: Output,
    contribution_id: Option<String>,
) -> Dispatch {
    let mut resolved = resolved.clone();
    resolved.output = ResolvedOutput::Mapped(output);
    Dispatch {
        resolved,
        contribution_id,
    }
}
fn digital_id(output: &Output, a: &str, b: &str) -> Option<String> {
    matches!(output, Output::Digital(_)).then(|| {
        if a < b {
            format!("{a}{b}")
        } else {
            format!("{b}{a}")
        }
    })
}
fn dpad_named(resolved: &Resolved<Output>, a: &str, b: &str) -> Result<Option<Dispatch>> {
    let ResolvedOutput::Mapped(Output::Dpad(branches)) = &resolved.output else {
        bail!("D-pad retained mapping has another output kind");
    };
    // ff5ae indexes the exact conditional input, including the empty default.
    Ok(branches
        .iter()
        .find(|branch| {
            branch
                .input
                .as_ref()
                .map(|input| input.stateless_id())
                .unwrap_or_default()
                == b
        })
        .map(|branch| {
            dispatch(
                resolved,
                branch.output.clone(),
                digital_id(&branch.output, a, b),
            )
        }))
}

#[cfg(test)]
#[path = "mapping_native_composite/tests.rs"]
mod tests;
