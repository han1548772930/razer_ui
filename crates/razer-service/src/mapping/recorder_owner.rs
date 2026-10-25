//! A recorder process has one mutable transport owner. Other mapping workers
//! borrow its query path without retaining the process or copying its mode.
use super::Client;
use super::Transport;
use anyhow::Context as _;
use razer_ipc::ServiceRequest;
use razer_platform::macro_recorder::Mode;
use serde::Deserialize;
use serde_json::Value;
use std::sync::{Arc, Mutex, OnceLock, Weak};

struct State<T> {
    transport: Option<T>,
    closing: bool,
}

struct Registry<T> {
    owner: Arc<Mutex<Weak<Mutex<State<T>>>>>,
}
impl<T> Default for Registry<T> {
    fn default() -> Self {
        Self {
            owner: Arc::new(Mutex::new(Weak::new())),
        }
    }
}
impl<T> Clone for Registry<T> {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner.clone(),
        }
    }
}

impl<T: Transport> Registry<T> {
    fn register(&self, transport: T) -> anyhow::Result<Owner<T>> {
        let mut slot = self
            .owner
            .lock()
            .map_err(|_| anyhow::anyhow!("Recorder owner registry is poisoned"))?;
        anyhow::ensure!(
            slot.upgrade().is_none(),
            "Recorder owner is already retained"
        );
        let state = Arc::new(Mutex::new(State {
            transport: Some(transport),
            closing: false,
        }));
        *slot = Arc::downgrade(&state);
        Ok(Owner {
            state,
            registry: self.clone(),
        })
    }

    fn mode(&self) -> anyhow::Result<Mode> {
        // Release the registry lock before pipe I/O or owner cleanup. Capture
        // suppression waits on mapping actors that may query this same owner.
        let state = self
            .owner
            .lock()
            .map_err(|_| anyhow::anyhow!("Recorder owner registry is poisoned"))?
            .upgrade()
            .context("Macro recorder owner is absent")?;
        let mut state = state
            .lock()
            .map_err(|_| anyhow::anyhow!("Recorder transport is poisoned"))?;
        anyhow::ensure!(!state.closing, "Macro recorder owner is closing");
        let transport = state
            .transport
            .as_mut()
            .context("Macro recorder owner was released")?;
        anyhow::ensure!(!transport.is_stopped(), "Macro recorder worker stopped");
        #[derive(Deserialize)]
        struct Observation {
            mode: Mode,
        }
        let observation: Observation =
            serde_json::from_value(transport.request(ServiceRequest::MacroRecordingMode)?)?;
        Ok(observation.mode)
    }
}

/// The recorder actor owns this value through restoration and child teardown.
/// Query borrowers retain only the synchronization cell; Drop takes the child
/// out of that cell before publishing the slot as available again.
pub(super) struct Owner<T: Transport> {
    state: Arc<Mutex<State<T>>>,
    registry: Registry<T>,
}
impl<T: Transport> Transport for Owner<T> {
    fn request(&mut self, request: ServiceRequest) -> anyhow::Result<Value> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Recorder transport is poisoned"))?;
        if matches!(request, ServiceRequest::Shutdown) {
            state.closing = true;
        }
        state
            .transport
            .as_mut()
            .context("Macro recorder owner was released")?
            .request(request)
    }

    fn is_stopped(&self) -> bool {
        match self.state.lock() {
            Ok(state) => state.transport.as_ref().is_none_or(Transport::is_stopped),
            Err(_) => true,
        }
    }
}
impl<T: Transport> Drop for Owner<T> {
    fn drop(&mut self) {
        let transport = {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            state.closing = true;
            state.transport.take()
        };
        // Destruction stays on the actor even if an earlier query retained the
        // cell. No registry lock spans child shutdown, join or pipe cleanup.
        drop(transport);
        let mut slot = self
            .registry
            .owner
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if slot.ptr_eq(&Arc::downgrade(&self.state)) {
            *slot = Weak::new();
        }
    }
}

fn global() -> &'static Registry<Client> {
    static REGISTRY: OnceLock<Registry<Client>> = OnceLock::new();
    REGISTRY.get_or_init(Registry::default)
}

pub(super) fn spawn() -> anyhow::Result<Owner<Client>> {
    global().register(Client::spawn()?)
}

pub(super) fn mode() -> anyhow::Result<Mode> {
    global().mode()
}

#[cfg(test)]
#[path = "recorder_owner_tests.rs"]
mod tests;
