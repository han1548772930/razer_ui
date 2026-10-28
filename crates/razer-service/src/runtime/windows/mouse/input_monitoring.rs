//! D32B0 emits movement before buttons; CE280 flushes per device on a button
//! or strictly more than 100000 source-clock microseconds. CC8C0/CE4D0 flush
//! pending movement before removing a device or stopping the monitor.
#[path = "input_monitoring/identity.rs"]
mod identity;
#[cfg(test)]
#[path = "input_monitoring/tests.rs"]
mod tests;
use super::NativeRuntime;
use anyhow::{Result, ensure};
use razer_ipc::mouse_input_monitoring::{
    InputEvent, MAX_BATCH_BYTES, MAX_BATCH_EVENTS, Poll, StartAcknowledgement, StopReceipt,
};
use razer_platform::mouse_monitor::{Event, MouseMonitor, Sample};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};

#[derive(Serialize)]
struct Row {
    data: String,
    #[serde(rename = "timeTick")]
    time_tick: String,
}
struct Device {
    identity: identity::Identity,
    rows: Vec<Row>,
    last_publication: Option<u64>,
}
impl Device {
    fn push(&mut self, data: Value, tick: u64, button: bool) -> Result<Option<InputEvent>> {
        ensure!(
            self.rows.len() < 16384,
            "Mouse monitor device batch overflowed"
        );
        self.rows.push(Row {
            data: data.to_string(),
            time_tick: tick.to_string(),
        });
        if button
            || self
                .last_publication
                .is_some_and(|previous| tick.wrapping_sub(previous) > 100000)
        {
            self.last_publication = Some(tick);
            return self.flush();
        }
        self.last_publication.get_or_insert(tick);
        Ok(None)
    }
    fn flush(&mut self) -> Result<Option<InputEvent>> {
        if self.rows.is_empty() {
            return Ok(None);
        }
        let event = InputEvent::from_native(
            &serde_json::to_string(&self.identity)?,
            1,
            serde_json::to_string(&self.rows)?,
        )?;
        self.rows.clear();
        Ok(Some(event))
    }
}
#[derive(Default)]
pub(super) struct State {
    owner: Option<String>,
    input: MouseMonitor,
    devices: BTreeMap<String, Device>,
    pending: VecDeque<InputEvent>,
    stopping: bool,
    stopped_events: Option<VecDeque<Event>>,
    failures: Vec<String>,
}
impl State {
    pub(super) fn owner(&self) -> Option<String> {
        self.owner.clone()
    }
    fn start_with(
        &mut self,
        owner: String,
        start: impl FnOnce(&mut MouseMonitor) -> Result<()>,
    ) -> Result<StartAcknowledgement> {
        ensure!(!owner.is_empty(), "Input monitor owner is empty");
        if self.owner.is_some() {
            self.check_owner(&owner)?;
            ensure!(
                !self.stopping,
                "Input monitor is draining its final callbacks"
            );
            // CB100 returns this negative acknowledgement without restarting
            // the native runner or clearing any pending device movement.
            return Ok(StartAcknowledgement::rejected(owner, "already started"));
        }
        start(&mut self.input)?;
        self.owner = Some(owner.clone());
        Ok(StartAcknowledgement::started(owner))
    }
    fn check_owner(&self, owner: &str) -> Result<()> {
        ensure!(
            !owner.is_empty() && self.owner.as_deref() == Some(owner),
            "Input monitor request belongs to another owner"
        );
        Ok(())
    }
    fn publish(&mut self, event: Option<InputEvent>) -> Result<()> {
        if let Some(event) = event {
            ensure!(
                self.pending.len() < 512,
                "Mouse input monitor publication queue overflowed"
            );
            self.pending.push_back(event);
        }
        Ok(())
    }
    fn sample(
        &mut self,
        sample: Sample,
        observe: impl FnOnce(&str) -> Result<identity::Identity>,
    ) -> Result<()> {
        let key = sample.path.to_ascii_lowercase();
        if !self.devices.contains_key(&key) {
            let identity = observe(&sample.path)?;
            self.devices.insert(
                key.clone(),
                Device {
                    identity,
                    rows: Vec::new(),
                    last_publication: None,
                },
            );
        }
        if sample.x != 0 || sample.y != 0 {
            let event = self.devices.get_mut(&key).unwrap().push(
                json!({"type":"mouseMove","x":sample.x,"y":sample.y}),
                sample.time_tick,
                false,
            )?;
            self.publish(event)?;
        }
        if sample.button_flags != 0 || sample.button_data != 0 {
            let mut input = razer_device::mapping_native_lookup::Input::new(
                razer_device::mapping_native_lookup::InputKind::Mouse {
                    id: sample.button_flags,
                    data: sample.button_data,
                },
            );
            input.modifiers = sample.modifiers;
            let event = self.devices.get_mut(&key).unwrap().push(
                input.to_native_json(),
                sample.time_tick,
                true,
            )?;
            self.publish(event)?;
        }
        Ok(())
    }
    fn removed(&mut self, path: &str) -> Result<()> {
        let key = path.to_ascii_lowercase();
        if let Some(device) = self.devices.get_mut(&key) {
            let event = device.flush()?;
            self.publish(event)?;
            self.devices.remove(&key);
        }
        Ok(())
    }
    fn accept(&mut self, events: Vec<Event>) -> Result<()> {
        for event in events {
            self.accept_one(event, identity::observe)?;
        }
        Ok(())
    }
    fn accept_one(
        &mut self,
        event: Event,
        observe: impl FnOnce(&str) -> Result<identity::Identity>,
    ) -> Result<()> {
        match event {
            Event::Input(sample) => self.sample(sample, observe),
            Event::Removed(path) => self.removed(&path),
        }
    }
    fn record_failure(&mut self, failure: String) {
        if !self.failures.contains(&failure) {
            self.failures.push(failure);
        }
    }
    fn failure(&self) -> Option<String> {
        (!self.failures.is_empty()).then(|| self.failures.join("; "))
    }
    fn collect(&mut self) -> Result<()> {
        let events = self.input.events()?;
        self.accept(events)
    }
    fn poll(&mut self, owner: &str) -> Result<Poll> {
        self.check_owner(owner)?;
        ensure!(
            !self.stopping,
            "Input monitor is draining its final callbacks"
        );
        if let Err(error) = self.collect() {
            self.record_failure(format!("{error:#}"));
        }
        let mut events = Vec::new();
        let mut bytes = 2;
        while events.len() < MAX_BATCH_EVENTS {
            let Some(event) = self.pending.front() else {
                break;
            };
            let encoded = serde_json::to_vec(event)?.len();
            ensure!(
                encoded + 2 <= MAX_BATCH_BYTES,
                "Input monitor event exceeds IPC capacity"
            );
            let added = encoded + usize::from(!events.is_empty());
            if bytes + added > MAX_BATCH_BYTES {
                break;
            }
            bytes += added;
            events.push(self.pending.pop_front().unwrap());
        }
        Ok(Poll::new(owner.into(), events, self.failure()))
    }
    fn stop_with(
        &mut self,
        owner: &str,
        stop: impl FnOnce(&mut MouseMonitor) -> Result<Vec<Event>>,
        mut observe: impl FnMut(&str) -> Result<identity::Identity>,
    ) -> Result<StopReceipt> {
        self.check_owner(owner)?;
        self.stopping = true;
        if self.stopped_events.is_none() {
            // The same owner remains reserved while final callbacks cross IPC.
            // A continuation must not stop or join the native thread again.
            self.stopped_events = Some(stop(&mut self.input)?.into());
        }
        let mut events = Vec::new();
        let mut bytes = 2;
        while events.len() < MAX_BATCH_EVENTS {
            if let Some(event) = self.pending.front() {
                let encoded = serde_json::to_vec(event)?.len();
                if encoded + 2 > MAX_BATCH_BYTES {
                    self.record_failure("Final input monitor event exceeds IPC capacity".into());
                    self.pending.pop_front();
                    continue;
                }
                let added = encoded + usize::from(!events.is_empty());
                if bytes + added > MAX_BATCH_BYTES {
                    break;
                }
                bytes += added;
                events.push(self.pending.pop_front().unwrap());
                continue;
            }
            if let Some(event) = self.stopped_events.as_mut().unwrap().pop_front() {
                if let Err(error) = self.accept_one(event, &mut observe) {
                    self.record_failure(format!("Drain monitor: {error:#}"));
                }
                continue;
            }
            if let Some(path) = self.devices.keys().next().cloned() {
                if let Err(error) = self.removed(&path) {
                    self.record_failure(format!("Flush monitor: {error:#}"));
                    self.devices.remove(&path);
                }
                continue;
            }
            break;
        }
        let more = !self.pending.is_empty()
            || !self.stopped_events.as_ref().unwrap().is_empty()
            || !self.devices.is_empty();
        let receipt = StopReceipt::new(owner.into(), events, more, self.failure());
        if !more {
            *self = Self::default();
        }
        Ok(receipt)
    }
    fn shutdown_with(
        &mut self,
        mut stop: impl FnMut(&mut MouseMonitor) -> Result<Vec<Event>>,
        mut observe: impl FnMut(&str) -> Result<identity::Identity>,
    ) -> Result<()> {
        let Some(owner) = self.owner() else {
            return Ok(());
        };
        let mut failures = Vec::new();
        while self.owner.is_some() {
            match self.stop_with(&owner, &mut stop, &mut observe) {
                Ok(receipt) => {
                    if let Some(failure) = receipt.failure() {
                        failures.push(failure.to_owned());
                    }
                }
                Err(error) => {
                    failures.push(format!("{error:#}"));
                    break;
                }
            }
        }
        ensure!(failures.is_empty(), "{}", failures.join("; "));
        Ok(())
    }
    pub(super) fn shutdown(&mut self) -> Result<()> {
        // Service shutdown detaches all observers, but still drains each page
        // and preserves final capture/flush errors before releasing the owner.
        self.shutdown_with(MouseMonitor::stop, identity::observe)
    }
}
impl NativeRuntime {
    pub(super) fn start_mouse_input_monitoring(&mut self, owner: String) -> Result<Value> {
        Ok(serde_json::to_value(
            self.mouse_input_monitoring
                .start_with(owner, MouseMonitor::start)?,
        )?)
    }
    pub(super) fn poll_mouse_input_monitoring(&mut self, owner: &str) -> Result<Value> {
        Ok(serde_json::to_value(
            self.mouse_input_monitoring.poll(owner)?,
        )?)
    }
    pub(super) fn stop_mouse_input_monitoring(&mut self, owner: &str) -> Result<Value> {
        Ok(serde_json::to_value(
            self.mouse_input_monitoring
                .stop_with(owner, MouseMonitor::stop, identity::observe)?,
        )?)
    }
}
