//! Process-local mapping owners share temporary source-defined suppression.
//! Commands remain typed messages; the registered actor owns all native I/O.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

const CONTROL_TIMEOUT: Duration = Duration::from_secs(60);
static GLOBAL: LazyLock<Registry> = LazyLock::new(Registry::default);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Suppression {
    Mapping,
    SoftwareRecording,
    MappingAndShortcuts,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    mapping_enabled: bool,
    global_shortcuts_enabled: Option<bool>,
}
impl Target {
    pub fn mapping_enabled(self) -> bool {
        self.mapping_enabled
    }
    pub fn global_shortcuts_enabled(self) -> Option<bool> {
        self.global_shortcuts_enabled
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    EnableMapping,
    DisableMapping,
    EnableGlobalShortcuts,
    DisableGlobalShortcuts,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    operation: Operation,
    result: bool,
    reason: String,
}
impl Observation {
    pub fn new(operation: Operation, result: bool, reason: String) -> Self {
        Self {
            operation,
            result,
            reason,
        }
    }
    pub fn operation(&self) -> &Operation {
        &self.operation
    }
    pub fn result(&self) -> bool {
        self.result
    }
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Confirms an in-process execution gate. Source callbacks that report an
/// already satisfied target remain false in the retained observations.
pub struct Receipt {
    owner_guid: String,
    revision: u64,
    target: Target,
    observations: Vec<Observation>,
}
impl Receipt {
    pub fn new(owner_guid: String, revision: u64, target: Target) -> Self {
        Self {
            owner_guid,
            revision,
            target,
            observations: Vec::new(),
        }
    }
    pub fn confirms(&self, owner: &str, revision: u64, target: Target) -> bool {
        self.owner_guid == owner && self.revision == revision && self.target == target
    }
    pub fn with_observations(
        owner_guid: String,
        revision: u64,
        target: Target,
        observations: Vec<Observation>,
    ) -> Self {
        Self {
            owner_guid,
            revision,
            target,
            observations,
        }
    }
    pub fn observations(&self) -> &[Observation] {
        &self.observations
    }
}

pub struct Control {
    owner_guid: String,
    revision: u64,
    target: Target,
    completion: Option<mpsc::Sender<Result<(), String>>>,
}
impl Control {
    pub fn owner_guid(&self) -> &str {
        &self.owner_guid
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn target(&self) -> Target {
        self.target
    }
    pub fn acknowledge(mut self, result: Result<(), String>) {
        if let Some(completion) = self.completion.take() {
            let _ = completion.send(result);
        }
    }
}
impl Drop for Control {
    fn drop(&mut self) {
        if let Some(completion) = self.completion.take() {
            let _ = completion.send(Err(
                "Mapping control was dropped without acknowledgement".into()
            ));
        }
    }
}

#[derive(Default)]
struct State {
    revision: u64,
    scopes: BTreeMap<String, Suppression>,
    owners: BTreeMap<String, mpsc::Sender<Control>>,
}

#[derive(Clone, Default)]
pub struct Registry(Arc<Mutex<State>>);
type Pending = Vec<(String, mpsc::Receiver<Result<(), String>>)>;

impl Registry {
    pub fn global() -> Self {
        GLOBAL.clone()
    }

    pub fn register(&self, owner_guid: String) -> Result<Owner, String> {
        uuid::Uuid::parse_str(&owner_guid).map_err(|_| "Invalid mapping owner UUID")?;
        let (sender, commands) = mpsc::channel();
        let mut state = self
            .0
            .lock()
            .map_err(|_| "Mapping control registry poisoned")?;
        if state.owners.contains_key(&owner_guid) {
            return Err("Mapping control owner already registered".into());
        }
        if !state.scopes.is_empty() {
            let (completion, _) = mpsc::channel();
            sender
                .send(Control {
                    owner_guid: owner_guid.clone(),
                    revision: state.revision,
                    target: target(&state, None),
                    completion: Some(completion),
                })
                .map_err(|_| "Mapping control initial delivery failed")?;
        }
        state.owners.insert(owner_guid.clone(), sender);
        Ok(Owner {
            registry: self.clone(),
            owner_guid,
            commands,
            finished: false,
        })
    }

    pub fn suspend(&self, kind: Suppression) -> Result<Suspension, String> {
        self.suspend_inner(kind, None)
    }

    pub fn suspend_cancelable(
        &self,
        kind: Suppression,
        canceled: &AtomicBool,
    ) -> Result<Suspension, String> {
        self.suspend_inner(kind, Some(&|| canceled.load(Ordering::Acquire)))
    }

    pub fn suspend_while(
        &self,
        kind: Suppression,
        valid: impl Fn() -> bool,
    ) -> Result<Suspension, String> {
        self.suspend_inner(kind, Some(&|| !valid()))
    }

    fn suspend_inner(
        &self,
        kind: Suppression,
        canceled: Option<&dyn Fn() -> bool>,
    ) -> Result<Suspension, String> {
        if canceled.is_some_and(|is_canceled| is_canceled()) {
            return Err("Mapping suppression canceled".into());
        }
        let (mut suspension, pending) = self.begin(kind)?;
        if let Err(error) = wait(pending, canceled) {
            let cleanup = suspension.release();
            return Err(match cleanup {
                Ok(()) => error,
                Err(cleanup) => format!("{error}; mapping suppression rollback failed: {cleanup}"),
            });
        }
        Ok(suspension)
    }

    fn begin(&self, kind: Suppression) -> Result<(Suspension, Pending), String> {
        let identity = uuid::Uuid::new_v4().to_string();
        let pending = {
            let mut state = self
                .0
                .lock()
                .map_err(|_| "Mapping control registry poisoned")?;
            state.revision = state
                .revision
                .checked_add(1)
                .ok_or("Mapping control revision exhausted")?;
            state.scopes.insert(identity.clone(), kind);
            broadcast(&state, target(&state, None))
        };
        Ok((
            Suspension {
                registry: self.clone(),
                identity: Some(identity),
            },
            pending,
        ))
    }

    fn remove(&self, identity: &str) -> Result<Pending, String> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| "Mapping control registry poisoned")?;
        let Some(kind) = state.scopes.get(identity).copied() else {
            return Err("Mapping suppression lease is absent".into());
        };
        state.revision = state
            .revision
            .checked_add(1)
            .ok_or("Mapping control revision exhausted")?;
        state.scopes.remove(identity);
        Ok(broadcast(&state, target(&state, Some(kind))))
    }
}

fn target(state: &State, released: Option<Suppression>) -> Target {
    let global_shortcuts_enabled = if state
        .scopes
        .values()
        .any(|kind| *kind == Suppression::MappingAndShortcuts)
    {
        Some(false)
    } else if matches!(
        released,
        Some(Suppression::SoftwareRecording | Suppression::MappingAndShortcuts)
    ) {
        Some(true)
    } else {
        None
    };
    Target {
        mapping_enabled: state.scopes.is_empty(),
        global_shortcuts_enabled,
    }
}

fn broadcast(state: &State, target: Target) -> Pending {
    state
        .owners
        .iter()
        .map(|(owner, sender)| {
            let (completion, received) = mpsc::channel();
            // A disconnected sender drops the command, which reports failure
            // through the retained completion sender rather than forging an ACK.
            let _ = sender.send(Control {
                owner_guid: owner.clone(),
                revision: state.revision,
                target,
                completion: Some(completion),
            });
            (owner.clone(), received)
        })
        .collect()
}

fn wait(pending: Pending, canceled: Option<&dyn Fn() -> bool>) -> Result<(), String> {
    let deadline = Instant::now() + CONTROL_TIMEOUT;
    let mut failures = Vec::new();
    for (owner, response) in pending {
        loop {
            if canceled.is_some_and(|is_canceled| is_canceled()) {
                failures.push("Mapping suppression canceled".into());
                break;
            }
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                failures.push(format!("Mapping owner {owner} control timed out"));
                break;
            };
            match response.recv_timeout(remaining.min(Duration::from_millis(50))) {
                Ok(Ok(())) => break,
                Ok(Err(error)) => {
                    failures.push(format!("Mapping owner {owner}: {error}"));
                    break;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    failures.push(format!("Mapping owner {owner} control disconnected"));
                    break;
                }
            }
        }
    }
    if canceled.is_some_and(|is_canceled| is_canceled())
        && !failures
            .iter()
            .any(|error| error == "Mapping suppression canceled")
    {
        failures.push("Mapping suppression canceled".into());
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

/// Retain until native shutdown and transport destruction have both completed.
pub struct Owner {
    registry: Registry,
    owner_guid: String,
    commands: mpsc::Receiver<Control>,
    finished: bool,
}
impl Owner {
    pub fn drain(&self) -> Vec<Control> {
        self.commands.try_iter().collect()
    }
    pub fn finish(&mut self, result: Result<(), String>) -> Result<(), String> {
        if self.finished {
            return Ok(());
        }
        let mut state = self
            .registry
            .0
            .lock()
            .map_err(|_| "Mapping control registry poisoned")?;
        if state.owners.remove(&self.owner_guid).is_none() {
            return Err("Mapping control owner was replaced".into());
        }
        self.finished = true;
        // A closing worker can satisfy suppression by completing its cleanup;
        // no native mapping remains live when the caller confirms this finish.
        for command in self.commands.try_iter() {
            command.acknowledge(result.clone());
        }
        Ok(())
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        let _ = self.finish(Err(
            "Mapping owner released without confirmed cleanup".into()
        ));
    }
}

pub struct Suspension {
    registry: Registry,
    identity: Option<String>,
}
impl Suspension {
    pub fn acquire(kind: Suppression) -> Result<Self, String> {
        Registry::global().suspend(kind)
    }
    pub fn acquire_cancelable(kind: Suppression, canceled: &AtomicBool) -> Result<Self, String> {
        Registry::global().suspend_cancelable(kind, canceled)
    }
    pub fn acquire_while(kind: Suppression, valid: impl Fn() -> bool) -> Result<Self, String> {
        Registry::global().suspend_while(kind, valid)
    }
    pub fn release(&mut self) -> Result<(), String> {
        let Some(identity) = self.identity.as_ref() else {
            return Ok(());
        };
        let pending = self.registry.remove(identity)?;
        self.identity = None;
        wait(pending, None)
    }
}
impl Drop for Suspension {
    fn drop(&mut self) {
        if let Some(identity) = self.identity.take() {
            let _ = self.registry.remove(&identity);
        }
    }
}

#[cfg(test)]
mod tests;
