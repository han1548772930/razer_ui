//! Current native disableMapping/enableMapping control this DLL instance's
//! device manager, including the global appEngine lookup. The shortcut switch
//! separately controls this instance's HotkeyService lease.
use super::{Callback2, NativeRuntime, callback2};
use anyhow::ensure;
use razer_ipc::mapping_control::{Observation, Operation as Action, Receipt, Target};
use serde_json::Value;

type Operation = unsafe extern "C" fn(Callback2);

#[derive(Default)]
pub(super) struct State {
    revision: Option<u64>,
}

fn operations(target: Target) -> Vec<Action> {
    let mut operations = vec![if target.mapping_enabled() {
        Action::EnableMapping
    } else {
        Action::DisableMapping
    }];
    if let Some(enabled) = target.global_shortcuts_enabled() {
        operations.push(if enabled {
            Action::EnableGlobalShortcuts
        } else {
            Action::DisableGlobalShortcuts
        });
    }
    operations
}

fn symbol(action: &Action) -> &'static str {
    match action {
        Action::EnableMapping => "enableMapping",
        Action::DisableMapping => "disableMapping",
        Action::EnableGlobalShortcuts => "enableGlobalShortcut",
        Action::DisableGlobalShortcuts => "disableGlobalShortcut",
    }
}

fn confirmed(observation: &Observation) -> bool {
    observation.result()
        || matches!(
            (observation.operation(), observation.reason()),
            (Action::EnableMapping, "already enabled")
                | (Action::DisableMapping, "already disabled")
                | (Action::EnableGlobalShortcuts, "hotkey was not disabled")
        )
}

fn execute(
    target: Target,
    mut operation: impl FnMut(&Action) -> anyhow::Result<Observation>,
) -> anyhow::Result<Vec<Observation>> {
    let mut observations = Vec::new();
    let mut errors = Vec::new();
    for action in operations(target) {
        match operation(&action) {
            Ok(observation) => {
                if observation.operation() != &action || !confirmed(&observation) {
                    errors.push(format!(
                        "{} rejected: {}",
                        symbol(&action),
                        observation.reason()
                    ));
                }
                observations.push(observation);
            }
            Err(error) => errors.push(format!("{}: {error:#}", symbol(&action))),
        }
    }
    ensure!(errors.is_empty(), "{}", errors.join("; "));
    Ok(observations)
}

impl NativeRuntime {
    pub(super) fn set_mapping_execution(
        &mut self,
        owner: &str,
        revision: u64,
        target: Target,
    ) -> anyhow::Result<Value> {
        ensure!(
            self.global_shortcuts.owner().as_deref() == Some(owner)
                || self.mapping_device.owner_guid().as_deref() == Some(owner),
            "Mapping execution owner changed"
        );
        ensure!(
            self.mapping_execution
                .revision
                .is_none_or(|previous| revision > previous),
            "Mapping execution control revision is stale"
        );
        self.mapping_execution.revision = Some(revision);
        // Current restore uses Promise.all: submit both independent source
        // operations even if one rejects. A timed-out ABI cannot be reused.
        let observations = execute(target, |action| -> anyhow::Result<Observation> {
            ensure!(!self.poisoned, "Mapping execution callback owner timed out");
            let operation: Operation = unsafe { self.mapping_symbol(symbol(action))? };
            let reply = self.call_source(|| unsafe { operation(callback2) })?;
            Ok(Observation::new(
                action.clone(),
                reply.success,
                reply.reason,
            ))
        })?;
        Ok(serde_json::to_value(Receipt::with_observations(
            owner.into(),
            revision,
            target,
            observations,
        ))?)
    }
}

#[cfg(test)]
#[path = "mapping_execution/tests.rs"]
mod tests;
