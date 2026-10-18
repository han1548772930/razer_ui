//! One mode notification subscription fans out to its retained consumers.
use super::{Callback2, NativeRuntime, callback2};
use std::{collections::BTreeSet, ffi::c_char};

type Operation = unsafe extern "C" fn(Callback2);
type Callback = unsafe extern "C" fn(i32, *const c_char, u64);
type Install = unsafe extern "C" fn(Callback, Callback2);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Owner {
    Device(String),
    Capture,
}

#[derive(Default)]
pub(super) struct State {
    owners: BTreeSet<Owner>,
    installed: bool,
}

impl State {
    pub(super) fn owners(&self) -> Vec<Owner> {
        self.owners.iter().cloned().collect()
    }
}

unsafe extern "C" fn event(kind: i32, data: *const c_char, tick: u64) {
    // Each receiver copies the callback-owned data before returning.
    unsafe {
        super::mapping_device::mode_event(kind, data, tick);
        super::shortcut_capture::mode_event(kind, data, tick);
    }
}

impl NativeRuntime {
    pub(super) fn retain_mapping_mode_events(&mut self, owner: Owner) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.mapping_mode_events.owners.contains(&owner),
            "Mode event owner is already registered"
        );
        let first = self.mapping_mode_events.owners.is_empty();
        anyhow::ensure!(
            first || self.mapping_mode_events.installed,
            "Mode event registration has not completed"
        );
        self.mapping_mode_events.owners.insert(owner);
        if first {
            let register: Operation =
                unsafe { self.mapping_symbol("registerDeviceModeChangedEvent")? };
            self.call(|| unsafe { register(callback2) })?;
            let install: Install =
                unsafe { self.mapping_symbol("setDeviceModeChangedEventCallback")? };
            self.call(|| unsafe { install(event, callback2) })?;
            self.mapping_mode_events.installed = true;
        }
        Ok(())
    }

    pub(super) fn release_mapping_mode_events(&mut self, owner: Owner) -> anyhow::Result<()> {
        if !self.mapping_mode_events.owners.contains(&owner) {
            return Ok(());
        }
        if self.mapping_mode_events.owners.len() == 1 {
            let unregister: Operation =
                unsafe { self.mapping_symbol("unregisterDeviceModeChangedEvent")? };
            self.call(|| unsafe { unregister(callback2) })?;
            self.mapping_mode_events.installed = false;
        }
        self.mapping_mode_events.owners.remove(&owner);
        Ok(())
    }
}
