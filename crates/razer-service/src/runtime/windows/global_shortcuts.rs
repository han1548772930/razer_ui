//! Current Dashboard 59007/69782 publishes native storage documents, while the
//! host mapping wrapper owns the type-3 unsupported-mapping callback until shutdown.
use super::{Callback2, NativeRuntime, StorageSetItem, callback2, copy_string};
use anyhow::{Context as _, ensure};
use razer_ipc::shortcuts::{
    EventBatch, PublicationReceipt, RegistrationReceipt, StopReceipt, UnsupportedMapping,
};
use razer_model::shortcuts::CompiledDocument;
use serde_json::{Value, json};
use std::{
    ffi::{CString, c_char},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
};

type UnsupportedCallback =
    unsafe extern "C" fn(i32, *const c_char, *const c_char, *const c_char, u64);
type InstallUnsupported = unsafe extern "C" fn(UnsupportedCallback, Callback2);
static EVENTS: Mutex<Option<SyncSender<UnsupportedMapping>>> = Mutex::new(None);
static OVERFLOW: AtomicBool = AtomicBool::new(false);

unsafe extern "C" fn unsupported(
    kind: i32,
    device: *const c_char,
    input: *const c_char,
    output: *const c_char,
    tick: u64,
) {
    // Current createCbGlobalShortcutUnsupportedMappingEvent forwards only 3.
    if kind != 3 {
        return;
    }
    let event = UnsupportedMapping::new(
        unsafe { copy_string(device) },
        unsafe { copy_string(input) },
        unsafe { copy_string(output) },
        tick,
    );
    match EVENTS.lock() {
        Ok(slot) => {
            if let Some(sender) = slot.as_ref() {
                if sender.try_send(event).is_err() {
                    OVERFLOW.store(true, Ordering::Release);
                }
            }
        }
        Err(_) => OVERFLOW.store(true, Ordering::Release),
    }
}

struct Registration {
    owner: String,
    callback_installed: bool,
    stopping: bool,
    mappings_published: bool,
    revision: Option<u64>,
}

pub(super) struct State {
    registration: Option<Registration>,
    sender: SyncSender<UnsupportedMapping>,
    events: Receiver<UnsupportedMapping>,
}
impl State {
    pub(super) fn new() -> Self {
        let (sender, events) = mpsc::sync_channel(512);
        Self {
            registration: None,
            sender,
            events,
        }
    }
    fn activate_events(&self) -> anyhow::Result<()> {
        let mut slot = EVENTS
            .lock()
            .map_err(|_| anyhow::anyhow!("Global shortcut callback lock poisoned"))?;
        ensure!(
            slot.is_none(),
            "Global shortcut callback already has an owner"
        );
        *slot = Some(self.sender.clone());
        OVERFLOW.store(false, Ordering::Release);
        Ok(())
    }
    fn registration(&self, owner: &str) -> anyhow::Result<&Registration> {
        let current = self
            .registration
            .as_ref()
            .context("Global shortcut owner is absent")?;
        ensure!(current.owner == owner, "Global shortcut owner changed");
        Ok(current)
    }
    pub(super) fn owner(&self) -> Option<String> {
        self.registration
            .as_ref()
            .map(|registration| registration.owner.clone())
    }
}

fn publication_steps(
    document: &CompiledDocument,
    mut write: impl FnMut(&str, &Value) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    document.validate().map_err(anyhow::Error::msg)?;
    // Dependencies must be acknowledged before a mapping may reference them.
    for (key, values) in [
        ("synapseDefaultTurbos", document.default_turbos()),
        ("synapseTurbos", document.synapse_turbos()),
        ("synapseMacros", document.macros()),
        ("synapsePhasedMacros", document.phased_macros()),
    ] {
        write(key, &Value::Array(values.to_vec()))?;
    }
    write(
        "synapseGlobalShortcuts",
        &json!({"appEngine":document.app_engine()}),
    )
}

fn empty_mappings() -> anyhow::Result<Value> {
    let mut engine = json!({"mappings":[]});
    engine["hash"] = razer_device::mapping_engine_hash::source_hash(&engine)?.into();
    Ok(json!({"appEngine":engine}))
}

impl NativeRuntime {
    pub(super) fn start_global_shortcut_owner(&mut self, owner: &str) -> anyhow::Result<Value> {
        ensure!(
            uuid::Uuid::parse_str(owner).is_ok(),
            "Invalid global shortcut owner UUID"
        );
        ensure!(
            self.global_shortcuts.registration.is_none(),
            "Global shortcut owner is already active"
        );
        // Retain startup ownership even if initialization or callback setup fails.
        self.global_shortcuts.registration = Some(Registration {
            owner: owner.into(),
            callback_installed: false,
            stopping: false,
            mappings_published: false,
            revision: None,
        });
        self.mapping()?;
        self.global_shortcuts.activate_events()?;
        let install: InstallUnsupported =
            unsafe { self.mapping_symbol("setGlobalShortcutUnsupportedMappingCallback")? };
        self.call(|| unsafe { install(unsupported, callback2) })?;
        self.global_shortcuts
            .registration
            .as_mut()
            .unwrap()
            .callback_installed = true;
        Ok(serde_json::to_value(RegistrationReceipt::new(
            owner.into(),
            true,
        ))?)
    }

    pub(super) fn publish_global_shortcuts(
        &mut self,
        owner: &str,
        revision: u64,
        document: &CompiledDocument,
    ) -> anyhow::Result<Value> {
        let current = self.global_shortcuts.registration(owner)?;
        ensure!(!current.stopping, "Global shortcut owner is stopping");
        ensure!(
            current.callback_installed,
            "Global shortcut callback is not registered"
        );
        ensure!(
            current.revision.is_none_or(|previous| revision > previous),
            "Stale global shortcut publication"
        );
        ensure!(
            !OVERFLOW.load(Ordering::Acquire),
            "Global shortcut event queue overflowed"
        );
        document.validate().map_err(anyhow::Error::msg)?;
        self.global_shortcuts
            .registration
            .as_mut()
            .unwrap()
            .revision = Some(revision);
        publication_steps(document, |key, value| {
            if key == "synapseGlobalShortcuts" {
                // A rejected callback may follow a native mutation. Cleanup owns
                // this publication from submission, not only after its receipt.
                self.global_shortcuts
                    .registration
                    .as_mut()
                    .unwrap()
                    .mappings_published = true;
            }
            self.write_global_shortcut_storage(key, value)
        })?;
        Ok(serde_json::to_value(PublicationReceipt::new(
            owner.into(),
            revision,
            true,
        ))?)
    }

    pub(super) fn global_shortcut_events(&mut self, owner: &str) -> anyhow::Result<Value> {
        ensure!(
            self.global_shortcuts
                .registration(owner)?
                .callback_installed,
            "Global shortcut callback is not registered"
        );
        ensure!(
            !OVERFLOW.load(Ordering::Acquire),
            "Global shortcut event queue overflowed"
        );
        let events = self.global_shortcuts.events.try_iter().collect();
        Ok(serde_json::to_value(EventBatch::new(owner.into(), events))?)
    }

    pub(super) fn stop_global_shortcut_owner(&mut self, owner: &str) -> anyhow::Result<Value> {
        self.global_shortcuts.registration(owner)?;
        self.global_shortcuts
            .registration
            .as_mut()
            .unwrap()
            .stopping = true;
        if self
            .global_shortcuts
            .registration(owner)?
            .mappings_published
        {
            self.write_global_shortcut_storage("synapseGlobalShortcuts", &empty_mappings()?)?;
            self.global_shortcuts
                .registration
                .as_mut()
                .unwrap()
                .mappings_published = false;
        }
        // No callback-unregister export exists in the current ABI. Keep the
        // owner, static callback and DLL alive through mappingEngineShutdown.
        Ok(serde_json::to_value(StopReceipt::new(owner.into(), true))?)
    }

    fn write_global_shortcut_storage(&mut self, key: &str, value: &Value) -> anyhow::Result<()> {
        let key = CString::new(key)?;
        let value = CString::new(value.to_string())?;
        let set: StorageSetItem = unsafe { self.mapping_symbol("localStorageSetItem")? };
        let result = self.call(|| unsafe { set(key.as_ptr(), value.as_ptr(), callback2) });
        if self.poisoned {
            // The native callback timed out; it may still borrow these buffers.
            std::mem::forget(key);
            std::mem::forget(value);
        }
        result?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "global_shortcuts/tests.rs"]
mod tests;
