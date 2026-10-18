//! DeviceModeService state (0xbf868–0xc1c10), manager leases (0x17d6c4–
//! 0x17d78a), and MappingEngine notifications (0x18370–0x18890).
use crate::{
    mapping_native_action::{ModeScope, Output},
    mapping_native_lookup::Input,
};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub guid: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub edition_id: u32,
    pub manufacturer_name: String,
    pub product_name: String,
    pub serial_number: String,
    pub container_id: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub vendor: Option<u16>,
    pub product: Option<u16>,
    pub edition: Option<u16>,
}
impl Filter {
    pub fn from_native_json(value: &Value) -> Result<Self> {
        ensure!(value.is_object(), "Invalid mode query filter");
        let field = |key: &str| -> Result<Option<u16>> {
            value
                .get(key)
                .map(|v| {
                    v.as_i64()
                        .and_then(|n| i32::try_from(n).ok())
                        .map(|n| n as u16)
                        .context("Invalid mode query filter")
                })
                .transpose()
        };
        Ok(Self {
            vendor: field("vid")?,
            product: field("pid")?,
            edition: field("eid")?,
        })
    }
    fn matches(self, info: &DeviceInfo) -> bool {
        self.vendor.is_none_or(|v| v == info.vendor_id)
            && self.product.is_none_or(|v| v == info.product_id)
            && self.edition.is_none_or(|v| u32::from(v) == info.edition_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModeAction {
    pub guid: String,
    pub scope: ModeScope,
    pub input: String,
    pub output: String,
    pub time_tick: u64,
}
impl ModeAction {
    pub fn to_native_json(&self) -> Value {
        json!({"guid":self.guid,"scope":match self.scope {ModeScope::Global=>"global",ModeScope::Interdevice=>"interdevice",ModeScope::Local=>"local"},"input":self.input,"output":self.output,"timeTick":self.time_tick.to_string()})
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Hypershift {
        enabled: bool,
        time_tick: u64,
    },
    Otfs {
        enabled: bool,
        time_tick: u64,
    },
    Gaming {
        enabled: bool,
        device: DeviceInfo,
        time_tick: u64,
    },
    Action(ModeAction),
}
impl Event {
    pub fn kind(&self) -> u32 {
        match self {
            Self::Hypershift { .. } => 3,
            Self::Otfs { .. } => 4,
            Self::Gaming { .. } => 1,
            Self::Action(action) => {
                if action.scope == ModeScope::Global {
                    2
                } else {
                    0
                }
            }
        }
    }
    pub fn time_tick(&self) -> u64 {
        match self {
            Self::Hypershift { time_tick, .. }
            | Self::Otfs { time_tick, .. }
            | Self::Gaming { time_tick, .. } => *time_tick,
            Self::Action(action) => action.time_tick,
        }
    }
    pub fn to_native_json(&self) -> Value {
        match self {
            Self::Hypershift { enabled, .. } | Self::Otfs { enabled, .. } => {
                json!({"isEnabled":enabled})
            }
            Self::Gaming {
                enabled, device, ..
            } => {
                json!({"isEnabled":enabled,"deviceInfo":serde_json::to_string(device).expect("DeviceInfo serialization is infallible")})
            }
            Self::Action(action) => action.to_native_json(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ActionKey {
    guid: String,
    input: String,
    hypershift: bool,
}
struct Device {
    caller: Uuid,
    info: DeviceInfo,
    gaming: bool,
    hypershift_leases: usize,
    otfs_leases: usize,
    actions: BTreeMap<ActionKey, ModeAction>,
}

/// The shared service actor owns this state. A caller UUID is scoped to one
/// registration, so a late operation cannot mutate a replacement connection.
#[derive(Default)]
pub struct ModeState {
    devices: BTreeMap<String, Device>,
    global_actions: BTreeMap<ActionKey, ModeAction>,
    hypershift: bool,
    otfs: bool,
    events: VecDeque<Event>,
}
impl ModeState {
    const EVENT_CAPACITY: usize = 1024;
    fn room(&self, count: usize) -> Result<()> {
        ensure!(
            self.events.len() + count <= Self::EVENT_CAPACITY,
            "Mode notification queue is full"
        );
        Ok(())
    }
    fn device(&self, caller: Uuid) -> Result<&Device> {
        self.devices
            .values()
            .find(|v| v.caller == caller)
            .context("Mapping mode registration changed")
    }
    fn device_mut(&mut self, caller: Uuid) -> Result<&mut Device> {
        self.devices
            .values_mut()
            .find(|v| v.caller == caller)
            .context("Mapping mode registration changed")
    }

    pub fn register_device(&mut self, caller: Uuid, info: DeviceInfo) -> Result<()> {
        ensure!(
            !self.devices.contains_key(&info.guid)
                && self.devices.values().all(|v| v.caller != caller),
            "Mapping mode device already exists"
        );
        self.devices.insert(
            info.guid.clone(),
            Device {
                caller,
                info,
                gaming: false,
                hypershift_leases: 0,
                otfs_leases: 0,
                actions: BTreeMap::new(),
            },
        );
        Ok(())
    }
    pub fn hypershift(&self) -> bool {
        self.hypershift
    }
    pub fn otfs(&self) -> bool {
        self.otfs
    }
    pub fn gaming(&self, caller: Uuid) -> Result<bool> {
        Ok(self.device(caller)?.gaming)
    }

    pub fn set_game_mode(&mut self, caller: Uuid, enabled: bool, time_tick: u64) -> Result<()> {
        self.room(1)?;
        let device = self.device_mut(caller)?;
        device.gaming = enabled;
        let device = device.info.clone();
        // 0xc0e76 publishes even if the enabled value is unchanged.
        self.events.push_back(Event::Gaming {
            enabled,
            device,
            time_tick,
        });
        Ok(())
    }

    pub fn apply_hypershift(&mut self, caller: Uuid, flag: u32, time_tick: u64) -> Result<bool> {
        self.apply_lease(caller, flag, time_tick, true)
    }
    pub fn apply_otfs(&mut self, caller: Uuid, flag: u32, time_tick: u64) -> Result<bool> {
        self.apply_lease(caller, flag, time_tick, false)
    }
    fn apply_lease(
        &mut self,
        caller: Uuid,
        flag: u32,
        time_tick: u64,
        hypershift: bool,
    ) -> Result<bool> {
        ensure!(flag < 2, "Invalid mapping mode lease flag");
        self.device(caller)?;
        let current = if hypershift { self.hypershift } else { self.otfs };
        let own = self.device(caller)?;
        let count = if hypershift {
            own.hypershift_leases
        } else {
            own.otfs_leases
        };
        let next = if flag == 0 {
            count.checked_add(1).context("Mode lease count overflow")?
        } else {
            count.saturating_sub(1)
        };
        let other_active = self.devices.values().any(|device| {
            device.caller != caller
                && if hypershift {
                    device.hypershift_leases != 0
                } else {
                    device.otfs_leases != 0
                }
        });
        let enabled = next != 0 || other_active;
        if enabled != current {
            self.room(1)?;
        }
        let device = self.device_mut(caller)?;
        if hypershift {
            device.hypershift_leases = next;
        } else {
            device.otfs_leases = next;
        }
        if enabled != current {
            if hypershift {
                self.hypershift = enabled;
                self.events
                    .push_back(Event::Hypershift { enabled, time_tick });
            } else {
                self.otfs = enabled;
                self.events.push_back(Event::Otfs { enabled, time_tick });
            }
        }
        Ok(enabled != current)
    }

    pub fn apply_mode_action(
        &mut self,
        caller: Uuid,
        input: &Input,
        output: &Output,
        time_tick: u64,
    ) -> Result<()> {
        let Output::Mode {
            scope,
            flag,
            payload,
        } = output
        else {
            anyhow::bail!("Expected compiled mode output")
        };
        ensure!(*flag < 2, "Invalid mode output flag");
        self.room(1)?;
        let guid = self.device(caller)?.info.guid.clone();
        let key = ActionKey {
            guid: guid.clone(),
            input: input.stateless_id(),
            hypershift: self.hypershift,
        };
        let action = ModeAction {
            guid,
            scope: *scope,
            input: input.to_native_string(),
            output: payload.as_json().into(),
            time_tick,
        };
        let map = if *scope == ModeScope::Global {
            &mut self.global_actions
        } else {
            &mut self.device_mut(caller)?.actions
        };
        if *flag == 0 {
            map.entry(key).or_insert_with(|| action.clone());
        } else {
            map.remove(&key);
        }
        // Native notification uses this operation's action even when insert
        // or erase did not change the retained map (0xc1602/0xc1c10).
        self.events.push_back(Event::Action(action));
        Ok(())
    }

    /// Called after held input releases have executed. The manager leases are
    /// released before the device registration is removed; global actions are
    /// owned by their actual mode release operations, as in 0xc05e2.
    pub fn remove_device(&mut self, caller: Uuid, time_tick: u64) -> Result<()> {
        let guid = self.device(caller)?.info.guid.clone();
        let next_hs = self
            .devices
            .values()
            .any(|v| v.caller != caller && v.hypershift_leases != 0);
        let next_otfs = self
            .devices
            .values()
            .any(|v| v.caller != caller && v.otfs_leases != 0);
        self.room(usize::from(self.hypershift != next_hs) + usize::from(self.otfs != next_otfs))?;
        self.devices.remove(&guid);
        if self.hypershift != next_hs {
            self.hypershift = next_hs;
            self.events.push_back(Event::Hypershift {
                enabled: next_hs,
                time_tick,
            });
        }
        if self.otfs != next_otfs {
            self.otfs = next_otfs;
            self.events.push_back(Event::Otfs {
                enabled: next_otfs,
                time_tick,
            });
        }
        Ok(())
    }

    pub fn take_events(&mut self) -> Vec<Event> {
        self.events.drain(..).collect()
    }
    pub fn global_mode_json(&self) -> Value {
        let mut value = json!({});
        if self.hypershift {
            value["hypershift"] = true.into();
        }
        if self.otfs {
            value["otfs"] = true.into();
        }
        self.write_global_actions(&mut value);
        value
    }
    fn write_global_actions(&self, value: &mut Value) {
        if !self.global_actions.is_empty() {
            value["globalmode"] = self
                .global_actions
                .values()
                .map(ModeAction::to_native_json)
                .collect();
        }
    }
    pub fn device_mode_json(&self, filter: Filter) -> Value {
        let mut value = json!({});
        self.write_global_actions(&mut value);
        let devices = self
            .devices
            .values()
            .filter(|v| filter.matches(&v.info))
            .map(|device| {
                let mut value = serde_json::to_value(&device.info)
                    .expect("DeviceInfo serialization is infallible");
                if device.gaming {
                    value["gamemode"] = true.into();
                }
                if !device.actions.is_empty() {
                    value["actions"] = device
                        .actions
                        .values()
                        .map(ModeAction::to_native_json)
                        .collect();
                }
                value
            })
            .collect::<Vec<_>>();
        if !devices.is_empty() {
            value["devicemode"] = devices.into();
        }
        value
    }
}

#[cfg(test)]
#[path = "mapping_native_mode/tests.rs"]
mod tests;
