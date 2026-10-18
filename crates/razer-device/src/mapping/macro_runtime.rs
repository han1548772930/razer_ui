//! MacroData publication and MappingExecutor state, independent of platform I/O.
//! Current native owners: f7106/f6bd0, ecee3 and 128692/12b5b0/12be6e/12a8ca.

#[path = "macro_runtime/advanced.rs"]
pub mod advanced;
#[path = "macro_runtime/events.rs"]
mod events;
#[path = "macro_runtime/execution.rs"]
mod execution;
#[path = "macro_runtime/held.rs"]
mod held;
#[path = "macro_runtime/keyboard.rs"]
mod keyboard;
#[path = "macro_runtime/scheduler.rs"]
pub mod scheduler;
#[path = "macro_runtime/sequence.rs"]
pub mod sequence;
pub use events::parse as parse_event;
pub use events::{AudioAction, AudioTarget, Event, Keyboard, Launch, Mouse, MovementMode};
pub use execution::{Action, Completion, Execution, Outcome, PendingAction, Poll, Ticket};
pub use keyboard::{KeyboardExecutor, KeyboardTiming};

use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug)]
pub struct Definition {
    guid: String,
    hash: String,
    movement: MovementMode,
    // The source accepts the event array at publication and rejects an invalid
    // event only when execution reaches it. Retain that timing with typed
    // events or a deferred parse failure, without JSON indexing in execution.
    events: Vec<Result<Event, String>>,
}

impl Definition {
    pub fn guid(&self) -> &str {
        &self.guid
    }
    pub fn hash(&self) -> &str {
        &self.hash
    }
    pub fn movement(&self) -> MovementMode {
        self.movement
    }
    pub fn event_count(&self) -> usize {
        self.events.len()
    }
}

#[derive(Default)]
pub struct Library {
    definitions: BTreeMap<String, Arc<Definition>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Publication {
    pub changed: Vec<String>,
    pub count: usize,
}

impl Library {
    /// Validate the complete publication before replacing the live map. Running
    /// executions retain their Arc; a nested call looks up the then-current map.
    pub fn publish(&mut self, document: &Value) -> Result<Publication> {
        let rows = document.as_array().context("Macro data must be an array")?;
        let mut next = BTreeMap::new();
        let mut changed = Vec::new();
        for row in rows {
            let row = row.as_object().context("Macro row must be an object")?;
            let guid = row
                .get("guid")
                .and_then(Value::as_str)
                .context("Macro GUID is missing")?;
            ensure!(!next.contains_key(guid), "Duplicate macro GUID {guid}");
            let engine = row
                .get("appEngine")
                .filter(|value| value.is_object())
                .context("Macro appEngine is missing")?;
            let hash = engine
                .get("hash")
                .and_then(Value::as_str)
                .context("Macro hash is missing")?;
            if let Some(existing) = self.definitions.get(guid).filter(|item| item.hash == hash) {
                next.insert(guid.to_owned(), existing.clone());
                continue;
            }
            ensure!(
                crate::mapping_engine_hash::source_hash(engine)? == hash,
                "Macro hash is invalid: {guid}"
            );
            let movement = events::movement_mode(engine.get("mouseMoveType"))?;
            let events = engine
                .get("events")
                .and_then(Value::as_array)
                .context("Macro events are missing")?;
            let definition = Definition {
                guid: guid.to_owned(),
                hash: hash.to_owned(),
                movement,
                events: events
                    .iter()
                    .map(|event| events::parse(event).map_err(|error| format!("{error:#}")))
                    .collect(),
            };
            next.insert(guid.to_owned(), Arc::new(definition));
            changed.push(guid.to_owned());
        }
        let count = next.len();
        self.definitions = next;
        Ok(Publication { changed, count })
    }

    pub fn get(&self, guid: &str) -> Option<Arc<Definition>> {
        self.definitions.get(guid).cloned()
    }

    pub fn clear(&mut self) {
        self.definitions.clear();
    }
}

#[cfg(test)]
#[path = "macro_runtime/tests.rs"]
mod tests;
