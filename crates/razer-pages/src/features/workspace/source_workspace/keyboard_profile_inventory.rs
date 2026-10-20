//! Physical OBM observations belong to one request and connection generation.
//! Local profile documents and pending edits never supply device slot evidence.
use super::*;
use razer_device::keyboard_onboard_profiles::{CompleteReading, ReadOptions};
use razer_device::keyboard_profile_inventory::{ProfileInventory, ProfileSlot};
use razer_ipc::{keyboard_onboard_profiles as complete, keyboard_profile_inventory as inventory};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

struct Request {
    id: uuid::Uuid,
    generation: u64,
    product_id: u32,
    options: Option<ReadOptions>,
    canceled: Arc<AtomicBool>,
}

enum Observation {
    Unobserved,
    Loading,
    Ready {
        inventory: Option<inventory::Observation>,
        slots: Vec<ProfileSlot>,
        partial_target: Option<inventory::Target>,
        complete: Option<Box<CompleteReading>>,
    },
    Failed(String),
    Disconnected,
}

pub(super) struct Owner {
    request: Option<Request>,
    observation: Observation,
}

impl Default for Owner {
    fn default() -> Self {
        Self {
            request: None,
            observation: Observation::Unobserved,
        }
    }
}

impl Owner {
    fn invalidate(&mut self, connected: bool) {
        if let Some(request) = self.request.take() {
            request.canceled.store(true, Ordering::Release);
        }
        self.observation = if connected {
            Observation::Unobserved
        } else {
            Observation::Disconnected
        };
    }

    fn begin(&mut self, request: Request) {
        self.invalidate(true);
        self.request = Some(request);
        self.observation = Observation::Loading;
    }

    fn current(&self, id: uuid::Uuid, generation: u64) -> bool {
        self.request.as_ref().is_some_and(|request| {
            request.id == id
                && request.generation == generation
                && !request.canceled.load(Ordering::Acquire)
        })
    }

    fn finish(
        &mut self,
        id: uuid::Uuid,
        generation: u64,
        result: Result<(inventory::Observation, Option<CompleteReading>), String>,
    ) -> bool {
        if !self.current(id, generation) || !matches!(self.observation, Observation::Loading) {
            return false;
        }
        let Some(request) = self.request.as_ref() else {
            return false;
        };
        let result = result.and_then(|(observed, full)| {
            let validate = || -> anyhow::Result<()> {
                anyhow::ensure!(
                    observed.target.product_id() == request.product_id,
                    "Keyboard inventory belongs to another product"
                );
                observed.validate_for(&observed.target)?;
                match (&request.options, &full) {
                    (Some(options), Some(reading)) => {
                        complete::validate_target(&observed.target)?;
                        reading.validate_for(options)?;
                        anyhow::ensure!(
                            reading.snapshot.inventory == observed.inventory,
                            "Complete keyboard read and inventory differ"
                        );
                    }
                    (None, None) => {}
                    _ => anyhow::bail!("Keyboard inventory read mode differs from its request"),
                }
                Ok(())
            };
            validate().map_err(|error| format!("{error:#}"))?;
            Ok((observed, full))
        });
        self.observation = match result {
            Ok((inventory, complete)) => Observation::Ready {
                slots: inventory.inventory.slots.clone(),
                inventory: Some(inventory),
                partial_target: None,
                complete: complete.map(Box::new),
            },
            Err(error) => Observation::Failed(error),
        };
        true
    }

    fn inventory(&self) -> Option<&ProfileInventory> {
        if self
            .request
            .as_ref()
            .is_none_or(|request| request.canceled.load(Ordering::Acquire))
        {
            return None;
        }
        match &self.observation {
            Observation::Ready { inventory, .. } => {
                inventory.as_ref().map(|reading| &reading.inventory)
            }
            _ => None,
        }
    }

    fn slots(&self) -> Option<&[ProfileSlot]> {
        if self
            .request
            .as_ref()
            .is_none_or(|request| request.canceled.load(Ordering::Acquire))
        {
            return None;
        }
        match &self.observation {
            Observation::Ready { slots, .. } => Some(slots),
            _ => None,
        }
    }

    fn finish_slots(
        &mut self,
        id: uuid::Uuid,
        generation: u64,
        result: Result<inventory::SlotsObservation, String>,
    ) -> bool {
        if !self.current(id, generation) || !matches!(self.observation, Observation::Loading) {
            return false;
        }
        let Some(request) = self.request.as_ref() else {
            return false;
        };
        let result = result.and_then(|reading| {
            if request.product_id != 756 || request.options.is_some() {
                return Err("Partial slot observation differs from its source request".into());
            }
            reading
                .validate_for(&reading.target)
                .map_err(|error| format!("{error:#}"))?;
            Ok(reading)
        });
        self.observation = match result {
            Ok(reading) => Observation::Ready {
                inventory: None,
                slots: reading.slots,
                partial_target: Some(reading.target),
                complete: None,
            },
            Err(error) => Observation::Failed(error),
        };
        true
    }

    fn error(&self) -> Option<&str> {
        match &self.observation {
            Observation::Failed(message) => Some(message),
            _ => None,
        }
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.invalidate(false);
    }
}

impl SourceProductWorkspace {
    pub fn keyboard_profile_inventory(&self) -> Option<&ProfileInventory> {
        self.keyboard_profile_inventory.inventory()
    }

    pub fn keyboard_profile_slots(&self) -> Option<&[ProfileSlot]> {
        self.keyboard_profile_inventory.slots()
    }

    pub fn keyboard_profile_inventory_target(&self) -> Option<&inventory::Target> {
        self.keyboard_profile_inventory.slots()?;
        match &self.keyboard_profile_inventory.observation {
            Observation::Ready {
                inventory: Some(inventory),
                ..
            } => Some(&inventory.target),
            Observation::Ready { partial_target, .. } => partial_target.as_ref(),
            _ => None,
        }
    }

    pub fn keyboard_onboard_profiles(&self) -> Option<&CompleteReading> {
        self.keyboard_profile_inventory.inventory()?;
        match &self.keyboard_profile_inventory.observation {
            Observation::Ready { complete, .. } => complete.as_deref(),
            _ => None,
        }
    }

    pub fn begin_keyboard_profile_inventory(
        &mut self,
        id: uuid::Uuid,
        canceled: Arc<AtomicBool>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Option<ReadOptions>, String> {
        if self.exiting
            || !matches!(self.body, FamilyBody::Keyboard(_))
            || !razer_device::keyboard_profile_inventory::supports(self.device.product_id)
        {
            return Err("Keyboard inventory owner is unavailable".into());
        }
        let options =
            if self.device.product_id == razer_device::keyboard_onboard_profiles::PRODUCT_ID {
                if self.device.dashboard.readonly_keyboard_metadata.is_none() {
                    return Err("Keyboard inventory requires the current layout reading".into());
                }
                let known = self
                    .local_mapping_macros
                    .iter()
                    .map(|row| {
                        row.get("guid")
                            .and_then(Value::as_str)
                            .filter(|guid| !guid.is_empty())
                            .map(str::to_owned)
                            .ok_or_else(|| "Local keyboard macro identity is absent".to_owned())
                    })
                    .collect::<Result<_, _>>()?;
                let options = ReadOptions {
                    layout_id: self.device.layout_id,
                    locally_known_macro_guids: known,
                };
                options.validate().map_err(|error| format!("{error:#}"))?;
                Some(options)
            } else {
                None
            };
        self.keyboard_profile_inventory.begin(Request {
            id,
            generation: self.mapping_connection_generation,
            product_id: self.device.product_id,
            options: options.clone(),
            canceled,
        });
        self.refresh_profile_choices(window, cx);
        cx.notify();
        Ok(options)
    }

    pub fn finish_keyboard_profile_inventory(
        &mut self,
        id: uuid::Uuid,
        result: Result<inventory::Observation, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.publish_keyboard_profile_inventory(
            id,
            result.map(|inventory| (inventory, None)),
            window,
            cx,
        );
    }

    pub fn finish_keyboard_profile_slots(
        &mut self,
        id: uuid::Uuid,
        result: Result<inventory::SlotsObservation, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.exiting
            && self.keyboard_profile_inventory.finish_slots(
                id,
                self.mapping_connection_generation,
                result,
            )
        {
            self.refresh_profile_choices(window, cx);
            if let Some(error) = self.keyboard_profile_inventory.error() {
                cx.emit(WorkspaceEvent::KeyboardLegacyRuntimeStatus(format!(
                    "Keyboard OBM slots: {error}"
                )));
            }
            cx.notify();
        }
    }

    pub fn finish_keyboard_onboard_profiles(
        &mut self,
        id: uuid::Uuid,
        result: Result<complete::Observation, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = result.and_then(|observation| {
            let options = self
                .keyboard_profile_inventory
                .request
                .as_ref()
                .and_then(|request| request.options.as_ref())
                .ok_or("Keyboard complete request is absent")?;
            observation
                .validate_for(&observation.target, options)
                .map_err(|error| format!("{error:#}"))?;
            Ok((
                inventory::Observation {
                    target: observation.target,
                    inventory: observation.reading.snapshot.inventory.clone(),
                },
                Some(observation.reading),
            ))
        });
        self.publish_keyboard_profile_inventory(id, result, window, cx);
    }

    fn publish_keyboard_profile_inventory(
        &mut self,
        id: uuid::Uuid,
        result: Result<(inventory::Observation, Option<CompleteReading>), String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.exiting
            && self.keyboard_profile_inventory.finish(
                id,
                self.mapping_connection_generation,
                result,
            )
        {
            self.refresh_profile_choices(window, cx);
            if let Some(error) = self.keyboard_profile_inventory.error() {
                cx.emit(WorkspaceEvent::KeyboardLegacyRuntimeStatus(format!(
                    "Keyboard OBM inventory: {error}"
                )));
            }
            cx.notify();
        }
    }

    pub fn clear_keyboard_profile_inventory(
        &mut self,
        connected: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.invalidate_keyboard_profile_inventory(connected);
        self.refresh_profile_choices(window, cx);
        cx.notify();
    }

    pub(super) fn invalidate_keyboard_profile_inventory(&mut self, connected: bool) {
        self.keyboard_profile_inventory.invalidate(connected);
        // An invalidated physical observation cannot leave a slot image behind
        // while the window owner schedules the retained SelectState refresh.
        self.profile_decorations.clear();
    }
}

#[cfg(test)]
#[path = "keyboard_profile_inventory_tests.rs"]
mod tests;
