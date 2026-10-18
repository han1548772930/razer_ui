//! Analog-only cascade ownership (94b58 / 97b60).
use super::*;

impl State {
    pub(super) fn analog_cascade(
        &mut self,
        resolved: &Resolved<Output>,
        release: &Resolved<Output>,
    ) -> Result<Vec<Dispatch>> {
        let ResolvedOutput::Mapped(Output::Cascade(branches)) = &resolved.output else {
            bail!("Analog cascade received another output kind");
        };
        let input_id = resolved.input.stateless_id();
        let active: std::collections::BTreeSet<_> =
            self.analog_dispatch.active.values().cloned().collect();
        let phase = resolved.input.phase();
        let mut dispatches = Vec::new();
        for branch in branches {
            let condition = branch
                .input
                .as_ref()
                .map(|input| input.stateless_id())
                .unwrap_or_default();
            if !condition.is_empty() && !active.contains(&condition) {
                continue;
            }
            let pair = if condition.is_empty() {
                (input_id.clone(), condition)
            } else {
                (condition, input_id.clone())
            };
            if phase == Phase::Press {
                if self.analog_dispatch.cascades.contains_key(&pair) {
                    continue;
                }
                self.analog_dispatch.cascades.insert(pair, release.clone());
            } else if phase == Phase::Release
                && self.analog_dispatch.cascades.remove(&pair).is_none()
            {
                continue;
            }
            dispatches.push(dispatch(resolved, branch.output.clone(), None));
            break;
        }
        if phase == Phase::Release {
            let stale: Vec<_> = self
                .analog_dispatch
                .cascades
                .keys()
                .filter(|(a, b)| {
                    (!a.is_empty() && !active.contains(a)) || (!b.is_empty() && !active.contains(b))
                })
                .cloned()
                .collect();
            for pair in stale {
                let release = self
                    .analog_dispatch
                    .cascades
                    .remove(&pair)
                    .expect("Selected analog cascade is retained");
                let ResolvedOutput::Mapped(Output::Cascade(branches)) = &release.output else {
                    bail!("Analog cascade retained release has another output kind");
                };
                // 97b60 walks the retained release's own branch order, accepting
                // either pair endpoint or its unconditional branch.
                if let Some(branch) = branches.iter().find(|branch| {
                    branch.input.as_ref().is_none_or(|input| {
                        let identity = input.stateless_id();
                        identity == pair.0 || identity == pair.1
                    })
                }) {
                    dispatches.push(dispatch(&release, branch.output.clone(), None));
                }
            }
        }
        Ok(dispatches)
    }
}
