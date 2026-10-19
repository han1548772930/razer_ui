//! Shared mapping actor: publication, connected driver registrations and capture.
use super::{
    State, capture,
    connections::{self, Lease},
    device, driver, hardware, identity, philips,
};
use anyhow::{Context, Result, ensure};
use razer_device::{
    mapping_native_action::device_tables,
    mapping_native_hid::NativeHidDeviceConfig,
    mapping_native_mode::{DeviceInfo, ModeState},
};
use razer_ipc::{ServiceRequest, macro_recording::ActionReply, mapping_control};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{Arc, Mutex, mpsc},
};
use uuid::Uuid;

#[path = "backend/gamepad_observation.rs"]
mod gamepad_observation;
#[path = "backend/analog_observation.rs"]
mod analog_observation;
#[path = "backend/observation.rs"]
mod observation;
#[path = "backend/rotation.rs"]
mod rotation;

pub(super) struct ExecutionContext {
    pub caller: Uuid,
    pub info: identity::DeviceInfo,
    pub route: driver::Route,
    pub cancellation: Arc<super::Cancellation>,
    pub macros: Arc<Mutex<State>>,
    pub modes: Arc<Mutex<ModeState>>,
    pub notifications: mpsc::SyncSender<Notification>,
}
pub(super) struct Notification {
    pub caller: Uuid,
    pub input: String,
    pub output: String,
    pub tick: u64,
    pub global: bool,
}
/// Each queue is bound to its connection generation. Overflow makes a drain
/// fail without presenting a partial stream as a complete observation.
struct Inbox<T> {
    generation: Uuid,
    pending: VecDeque<T>,
    overflow: bool,
}
impl<T> Inbox<T> {
    fn new(generation: Uuid) -> Self {
        Self {
            generation,
            pending: VecDeque::new(),
            overflow: false,
        }
    }
    fn push(&mut self, generation: Uuid, value: T) {
        if generation != self.generation {
            return;
        }
        if self.pending.len() == 512 {
            self.overflow = true;
        } else {
            self.pending.push_back(value);
        }
    }
    fn drain(&mut self) -> Result<Vec<T>> {
        ensure!(
            !self.overflow,
            "Mapping event consumer fell behind; stream is incomplete"
        );
        Ok(self.pending.drain(..).collect())
    }
}
struct Registration {
    scope: Uuid,
    caller: Uuid,
    target: identity::Target,
    hardware: Option<hardware::Owner>,
    worker: device::Owner,
    document: Option<Value>,
    pending: Inbox<Value>,
    failure: Option<String>,
    revision: Option<u64>,
    input_registered: bool,
    physical_retired: bool,
    cleanup_result: Option<Result<(), String>>,
}
struct Global {
    scope: Uuid,
    owner: String,
    revision: Option<u64>,
    execution_revision: Option<u64>,
    engine: Option<Value>,
    pending: Inbox<razer_ipc::shortcuts::UnsupportedMapping>,
    failure: Option<String>,
}
pub(super) struct Core<E: device::Executor> {
    factory: fn(ExecutionContext) -> Result<E>,
    macros: Arc<Mutex<State>>,
    modes: Arc<Mutex<ModeState>>,
    devices: BTreeMap<String, Registration>,
    rotations: BTreeMap<String, rotation::Registration>,
    connections: connections::Registry,
    notification_sender: mpsc::SyncSender<Notification>,
    notifications: mpsc::Receiver<Notification>,
    global: Option<Global>,
    mapping_suppression: BTreeSet<Uuid>,
    shortcut_suppression: BTreeSet<Uuid>,
    mode_subscribers: BTreeSet<Uuid>,
    hotkeys: razer_platform::global_shortcuts::GlobalShortcuts,
    hotkey_scopes: BTreeMap<(u32, u32), BTreeSet<Uuid>>,
    hotkey_events: BTreeMap<Uuid, VecDeque<Value>>,
    hotkey_failures: BTreeMap<Uuid, String>,
    scopes: BTreeSet<Uuid>,
    observers: BTreeMap<String, observation::Observer>,
    input_observers:
        BTreeMap<String, observation::Observer<razer_ipc::input_observation::InputEvent>>,
    analog_observers: BTreeMap<String, analog_observation::Observer>,
    gamepad_observers: BTreeMap<String, gamepad_observation::Observer>,
}
pub(super) struct Backend<E: device::Executor> {
    core: Core<E>,
    capture: capture::State,
    philips: philips::State,
}
impl<E: device::Executor> Backend<E> {
    pub fn new(factory: fn(ExecutionContext) -> Result<E>) -> Self {
        let (notification_sender, notifications) = mpsc::sync_channel(512);
        Self {
            core: Core {
                factory,
                macros: Arc::new(Mutex::new(State::new())),
                modes: Arc::new(Mutex::new(ModeState::default())),
                devices: BTreeMap::new(),
                rotations: BTreeMap::new(),
                connections: connections::Registry::default(),
                notification_sender,
                notifications,
                global: None,
                mapping_suppression: BTreeSet::new(),
                shortcut_suppression: BTreeSet::new(),
                mode_subscribers: BTreeSet::new(),
                hotkeys: Default::default(),
                hotkey_scopes: BTreeMap::new(),
                hotkey_events: BTreeMap::new(),
                hotkey_failures: BTreeMap::new(),
                scopes: BTreeSet::new(),
                observers: BTreeMap::new(),
                input_observers: BTreeMap::new(),
                analog_observers: BTreeMap::new(),
                gamepad_observers: BTreeMap::new(),
            },
            capture: Default::default(),
            philips: Default::default(),
        }
    }
}
impl<E: device::Executor> Core<E> {
    fn start_device(
        &mut self,
        scope: Uuid,
        product: u32,
        container: &str,
        owner: String,
    ) -> Result<Value> {
        ensure!(
            !self.devices.contains_key(&owner),
            "Mapping device owner is already registered"
        );
        let target = identity::Target::observe(product, container, &owner)?;
        let caller = Uuid::new_v4();
        let route = self.connections.device(scope, caller, &target)?;
        let result = (|| -> Result<(device::Owner, hardware::Owner)> {
            let info = target.info.clone();
            let mode_info = DeviceInfo {
                guid: info.guid.clone(),
                vendor_id: info.vendor_id.try_into()?,
                product_id: info.product_id.try_into()?,
                container_id: info.container_id.clone(),
                edition_id: 0,
                manufacturer_name: String::new(),
                product_name: String::new(),
                serial_number: String::new(),
            };
            // DB558/DB630 retain empty optional descriptor strings and edition0
            // when addUsbDevice supplied only physical registration identity.
            self.modes
                .lock()
                .map_err(|_| anyhow::anyhow!("Mapping modes poisoned"))?
                .register_device(caller, mode_info)?;
            let macros = self.macros.clone();
            let modes = self.modes.clone();
            let worker_modes = modes.clone();
            let notifications = self.notification_sender.clone();
            let factory = self.factory;
            let mut worker =
                device::Owner::open(route, caller, worker_modes, move |route, cancellation| {
                    factory(ExecutionContext {
                        caller,
                        info,
                        route,
                        cancellation,
                        macros,
                        modes,
                        notifications,
                    })
                })?;
            // 277D0 addUsbDevice supplies {type:usb}; DAFC2 initializes
            // connectFilterDriver/connectHid to true before optional overrides.
            let mut hardware = match hardware::Owner::open(&target.info) {
                Ok(hardware) => hardware,
                Err(error) => {
                    let stopped = worker.stop();
                    anyhow::bail!(
                        "Hardware initialization: {error:#}; device cleanup: {stopped:?}"
                    );
                }
            };
            let configured = worker
                .mapping(self.mapping_suppression.is_empty())
                .and_then(|_| worker.shortcuts(self.shortcut_suppression.is_empty()));
            if let Err(error) = configured {
                let hardware = hardware.shutdown();
                let worker = worker.stop();
                anyhow::bail!(
                    "Device execution gates: {error:#}; hardware cleanup: {hardware:?}; device cleanup: {worker:?}"
                );
            }
            Ok((worker, hardware))
        })();
        let (worker, hardware) = match result {
            Ok(owners) => owners,
            Err(error) => {
                let release = self.connections.release(&Lease::Device { scope, caller });
                let modes = (|| -> Result<()> {
                    let tick = razer_platform::event_clock::tick()?;
                    self.modes
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Mapping modes poisoned"))?
                        .remove_device(caller, tick)?;
                    Ok(())
                })();
                anyhow::bail!(
                    "Device execution initialization: {error:#}; driver cleanup: {release:?}; modes: {modes:?}"
                );
            }
        };
        self.devices.insert(
            owner.clone(),
            Registration {
                scope,
                caller,
                target,
                hardware: Some(hardware),
                worker,
                document: None,
                pending: Inbox::new(caller),
                failure: None,
                revision: None,
                input_registered: false,
                physical_retired: false,
                cleanup_result: None,
            },
        );
        Ok(json!({"registered":true,"driver_ready":true,"owner_guid":owner}))
    }
    fn registration(&self, scope: Uuid, owner: &str) -> Result<&Registration> {
        let row = self
            .devices
            .get(owner)
            .context("Mapping device owner absent")?;
        ensure!(
            row.scope == scope,
            "Mapping registration belongs to another client"
        );
        Ok(row)
    }
    fn publish(&mut self, scope: Uuid, owner: &str, document: Value) -> Result<Value> {
        let registration = self.registration(scope, owner)?;
        registration.ensure_active()?;
        registration.target.revalidate()?;
        let expected_product = if registration.target.logical_product.product_id() == 158
            && document.get("dualLinked") == Some(&Value::Bool(true)) {
            registration.target.info.product_id
        } else { registration.target.logical_product.product_id() };
        ensure!(
            document.get("productId").and_then(Value::as_u64)
                == Some(u64::from(expected_product)),
            "Mapping document belongs to another product"
        );
        ensure!(
            document.get("_macro_compilation_error").is_none(),
            "Local macro draft has compilation errors"
        );
        let tables = device_tables(
            &document,
            self.global
                .as_ref()
                .and_then(|global| global.engine.as_ref()),
        )?;
        let hardware_config = NativeHidDeviceConfig::from_document(&document)?;
        for (field, key) in [
            ("defaultTurbos", "synapseDefaultTurbos"),
            ("synapseTurbos", "synapseTurbos"),
            ("macros", "synapseMacros"),
            ("phasedMacros", "synapsePhasedMacros"),
        ] {
            if let Some(value) = document.get(field) {
                self.publish_storage_value(key, value)?;
            } else if matches!(field, "defaultTurbos" | "synapseTurbos") {
                anyhow::bail!("Missing compiled {field} list");
            }
        }
        let row = self.devices.get_mut(owner).unwrap();
        row.worker.publish(tables, hardware_config)?;
        let active = document
            .get("activeProfile")
            .and_then(Value::as_str)
            .context("Mapping active profile missing")?
            .to_owned();
        row.document = Some(document);
        row.target.revalidate()?;
        Ok(json!({"published":true,"owner_guid":owner,"active_profile":active}))
    }
    fn publish_storage_value(&mut self, key: &str, value: &Value) -> Result<()> {
        let mut macros = self
            .macros
            .lock()
            .map_err(|_| anyhow::anyhow!("Mapping macro owner poisoned"))?;
        macros.publish_storage(key, value)
    }
    fn stop_device(&mut self, scope: Uuid, owner: &str) -> Result<Value> {
        self.registration(scope, owner)?;
        let row = self.devices.get_mut(owner).unwrap();
        row.stop(&self.macros, &self.modes, &mut self.connections)?;
        self.devices.remove(owner);
        Ok(json!({"removed":true,"owner_guid":owner}))
    }
    fn mapping(&mut self, scope: Uuid, enabled: bool) -> Result<ActionReply> {
        if enabled {
            self.mapping_suppression.remove(&scope);
        } else {
            self.mapping_suppression.insert(scope);
        }
        let effective = self.mapping_suppression.is_empty();
        let mut errors = Vec::new();
        for row in self.devices.values_mut() {
            if row.physical_retired {
                continue;
            }
            if let Err(error) = row.worker.mapping(effective) {
                errors.push(format!("{}: {error:#}", row.target.info.guid));
            }
            if !effective {
                if let Err(error) = self
                    .macros
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Mapping macro owner poisoned"))?
                    .drain_caller(row.caller)
                {
                    errors.push(format!("{error:#}"));
                }
            }
        }
        Ok(ActionReply::new(errors.is_empty(), errors.join("; ")))
    }
    fn shortcuts(&mut self, scope: Uuid, enabled: bool) -> Result<ActionReply> {
        if enabled {
            self.shortcut_suppression.remove(&scope);
        } else {
            self.shortcut_suppression.insert(scope);
        }
        let effective = self.shortcut_suppression.is_empty();
        let mut errors = Vec::new();
        if let Err(error) = self.hotkeys.enable(effective) {
            errors.push(format!("{error:#}"));
        }
        for row in self.devices.values_mut() {
            if row.physical_retired {
                continue;
            }
            if let Err(error) = row.worker.shortcuts(effective) {
                errors.push(format!("{error:#}"));
            }
        }
        Ok(ActionReply::new(errors.is_empty(), errors.join("; ")))
    }
    fn global_publish(
        &mut self,
        scope: Uuid,
        owner: &str,
        revision: u64,
        document: razer_model::shortcuts::CompiledDocument,
    ) -> Result<Value> {
        let current = self
            .global
            .as_mut()
            .context("Global shortcut owner absent")?;
        ensure!(
            current.scope == scope && current.owner == owner,
            "Global shortcut owner changed"
        );
        ensure!(
            current.revision.is_none_or(|old| revision > old),
            "Stale global shortcut publication"
        );
        document.validate().map_err(anyhow::Error::msg)?;
        current.revision = Some(revision);
        publish_dependencies(&document, |key, value| {
            self.publish_storage_value(key, value)
        })?;
        let engine = document.app_engine().clone();
        self.replace_global_engine(Some(engine))?;
        Ok(serde_json::to_value(
            razer_ipc::shortcuts::PublicationReceipt::new(owner.into(), revision, true),
        )?)
    }
    fn replace_global_engine(&mut self, engine: Option<Value>) -> Result<()> {
        let updates = self
            .devices
            .iter()
            .filter(|(_, row)| !row.physical_retired)
            .filter_map(|(owner, row)| {
                row.document.as_ref().map(|document| {
                    Ok((
                        owner.clone(),
                        device_tables(document, engine.as_ref())?,
                        NativeHidDeviceConfig::from_document(document)?,
                    ))
                })
            })
            .collect::<Result<Vec<_>>>()?;
        self.global
            .as_mut()
            .context("Global shortcut owner absent")?
            .engine = engine;
        let mut errors = Vec::new();
        for (owner, tables, config) in updates {
            if let Err(error) = self.devices[&owner].worker.publish(tables, config) {
                errors.push(format!("{owner}: {error:#}"));
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
    fn unregister_hotkey(&mut self, scope: Uuid, key: (u32, u32)) -> Result<()> {
        let scopes = self
            .hotkey_scopes
            .get_mut(&key)
            .context("Shortcut is not registered")?;
        ensure!(
            scopes.contains(&scope),
            "Shortcut belongs to another client"
        );
        if scopes.len() == 1 {
            self.hotkeys.unregister(key.0, key.1)?;
        }
        scopes.remove(&scope);
        if scopes.is_empty() {
            self.hotkey_scopes.remove(&key);
        }
        Ok(())
    }
    fn poll_hotkeys(&mut self) -> Result<()> {
        for event in self.hotkeys.drain()? {
            let shortcut = event.shortcut();
            let Some(scopes) = self
                .hotkey_scopes
                .get(&(shortcut.virtual_key, shortcut.modifiers))
            else {
                continue;
            };
            let value = json!({"eventType":0,"event":json!({"virtualKey":shortcut.virtual_key,"modifiers":shortcut.modifiers,"argument":shortcut.argument}).to_string(),"timeTick":event.time_tick()});
            for scope in scopes {
                let queue = self.hotkey_events.entry(*scope).or_default();
                if queue.len() < 512 {
                    queue.push_back(value.clone());
                } else {
                    self.hotkey_failures
                        .insert(*scope, "Shortcut consumer fell behind".into());
                }
            }
        }
        Ok(())
    }
}
impl<E: device::Executor> capture::Host for Core<E> {
    fn set_mappings(&mut self, scope: Uuid, enabled: bool) -> Result<ActionReply> {
        self.mapping(scope, enabled)
    }
    fn set_shortcuts(&mut self, scope: Uuid, enabled: bool) -> Result<ActionReply> {
        self.shortcuts(scope, enabled)
    }
    fn add_target(&mut self, scope: Uuid, target: &capture::Target) -> Result<()> {
        self.connections.capture(scope, target).map(|_| ())
    }
    fn redirect(
        &mut self,
        scope: Uuid,
        target: &capture::Target,
        kind: razer_device::mapping_native_session::hooks::RedirectKind,
        enabled: bool,
    ) -> Result<()> {
        self.connections.redirect(
            &Lease::Capture {
                scope,
                guid: target.guid,
            },
            kind,
            enabled,
        )?;
        if enabled
            && !target.keyboard_only
            && kind == razer_device::mapping_native_session::hooks::RedirectKind::Keyboard
        {
            for row in self.devices.values_mut().filter(|row| {
                !row.physical_retired
                    && row
                        .target
                        .info
                        .container_id
                        .eq_ignore_ascii_case(&target.container_id)
                    && row.target.info.product_id == target.product_id
            }) {
                row.worker.redirect_activated()?;
                self.macros
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Mapping macro owner poisoned"))?
                    .drain_caller(row.caller)?;
            }
        }
        Ok(())
    }
    fn remove_target(&mut self, scope: Uuid, target: &capture::Target) -> Result<()> {
        self.connections.release(&Lease::Capture {
            scope,
            guid: target.guid,
        })
    }
    fn subscribe_modes(&mut self, scope: Uuid, enabled: bool) -> Result<()> {
        if enabled {
            self.mode_subscribers.insert(scope);
        } else {
            self.mode_subscribers.remove(&scope);
        }
        Ok(())
    }
    fn global_mode(&mut self) -> Result<Value> {
        Ok(self
            .modes
            .lock()
            .map_err(|_| anyhow::anyhow!("Mapping modes poisoned"))?
            .global_mode_json())
    }
    fn publish_storage(&mut self, _scope: Uuid, key: &str, value: &Value) -> Result<()> {
        self.publish_storage_value(key, value)
    }
}

impl<E: device::Executor> philips::Host for Core<E> {
    fn acquire(&mut self, scope: Uuid, target: &capture::Target) -> Result<()> {
        self.connections.philips(scope, target).map(|_| ())
    }
    fn remove(&mut self, scope: Uuid, target: &capture::Target) -> Result<()> {
        self.connections.release(&Lease::Philips {
            scope,
            guid: target.guid,
        })
    }
    fn redirect(
        &mut self,
        scope: Uuid,
        target: &capture::Target,
        kind: razer_device::mapping_native_session::hooks::RedirectKind,
        enabled: bool,
    ) -> Result<()> {
        self.connections.redirect(
            &Lease::Philips {
                scope,
                guid: target.guid,
            },
            kind,
            enabled,
        )
    }
    fn activity(&mut self, _scope: Uuid, target: &capture::Target) -> Result<(usize, usize)> {
        let macros = self
            .macros
            .lock()
            .map_err(|_| anyhow::anyhow!("Mapping macro owner poisoned"))?
            .scheduler
            .activity_count();
        let mut keys = 0u8;
        for row in self.devices.values().filter(|row| {
            row.target
                .info
                .container_id
                .eq_ignore_ascii_case(&target.container_id)
                && row.target.info.product_id == target.product_id
        }) {
            keys = keys.wrapping_add(row.worker.held_count()?);
        }
        Ok((usize::from(macros), usize::from(keys)))
    }
    fn mapping(&mut self, scope: Uuid, enabled: bool) -> Result<()> {
        let reply = Core::mapping(self, scope, enabled)?;
        ensure!(reply.result(), "{}", reply.reason());
        Ok(())
    }
    fn escape(&mut self, scope: Uuid, owner: &str, enabled: bool) -> Result<()> {
        if enabled {
            self.hotkeys
                .register(razer_platform::global_shortcuts::Shortcut {
                    virtual_key: 27,
                    modifiers: 0,
                    argument: owner.into(),
                })?;
            self.hotkey_scopes.entry((27, 0)).or_default().insert(scope);
            Ok(())
        } else {
            self.unregister_hotkey(scope, (27, 0))
        }
    }
    fn take_escape(&mut self, scope: Uuid, owner: &str) -> Result<bool> {
        self.poll_hotkeys()?;
        if let Some(error) = self.hotkey_failures.get(&scope) {
            anyhow::bail!("{error}");
        }
        let events = self.hotkey_events.entry(scope).or_default();
        let mut observed = false;
        events.retain(|event| {
            let matched = event
                .get("event")
                .and_then(Value::as_str)
                .and_then(|event| serde_json::from_str::<Value>(event).ok())
                .is_some_and(|event| {
                    event.get("argument").and_then(Value::as_str) == Some(owner)
                        && event.get("virtualKey").and_then(Value::as_u64) == Some(27)
                });
            observed |= matched;
            !matched
        });
        Ok(observed)
    }
}

impl Registration {
    fn ensure_active(&self) -> Result<()> {
        ensure!(
            !self.physical_retired,
            "Mapping device registration is retired"
        );
        Ok(())
    }
    fn stop(
        &mut self,
        macros: &Mutex<State>,
        modes: &Mutex<ModeState>,
        connections: &mut connections::Registry,
    ) -> Result<()> {
        self.physical_retired = true;
        let Self {
            hardware,
            worker,
            caller,
            scope,
            cleanup_result,
            ..
        } = self;
        cleanup_once(cleanup_result, || {
            stop_registration_owners(
                || {
                    stop_input_owners(
                        || hardware.as_mut().map_or(Ok(()), hardware::Owner::shutdown),
                        || worker.stop(),
                    )
                },
                || match macros.lock() {
                    Ok(mut macros) => macros.drain_caller(*caller),
                    Err(error) => {
                        let cleanup = error.into_inner().drain_caller(*caller);
                        anyhow::bail!("Mapping macro owner poisoned; cleanup: {cleanup:?}")
                    }
                },
                || {
                    let tick = razer_platform::event_clock::tick()?;
                    match modes.lock() {
                        Ok(mut modes) => modes.remove_device(*caller, tick).map(|_| ()),
                        Err(error) => {
                            let cleanup = error.into_inner().remove_device(*caller, tick);
                            anyhow::bail!("Mapping modes poisoned; cleanup: {cleanup:?}")
                        }
                    }
                },
                || {
                    connections.release(&Lease::Device {
                        scope: *scope,
                        caller: *caller,
                    })
                },
            )
        })
    }
    fn push(&mut self, value: Value) {
        self.pending.push(self.caller, value);
    }
}
impl<E: device::Executor> crate::mapping::owner::Backend for Backend<E> {
    fn acquire(&mut self, scope: Uuid) -> Result<()> {
        self.core.scopes.insert(scope);
        Ok(())
    }
    fn request(&mut self, scope: Uuid, request: ServiceRequest) -> Result<Value> {
        if let Some(result) = self.capture.handle(scope, &request, &mut self.core) {
            return result;
        }
        if let Some(result) = self.philips.handle(scope, &request, &mut self.core) {
            return result;
        }
        use ServiceRequest::*;
        match request {
            MappingRotationStart{owner_guid,target}=>self.core.start_rotation(scope,owner_guid,target),
            MappingRotationRequest{owner_guid,request}=>self.core.request_rotation(scope,&owner_guid,request),
            MappingRotationPoll{owner_guid}=>self.core.poll_rotation(scope,&owner_guid),
            MappingRotationStop{owner_guid}=>self.core.stop_rotation(scope,&owner_guid),
            MappingGamepadObservationStart{target,observer_guid,registration}=>self.core.start_gamepad_observation(scope,target,observer_guid,registration),
            MappingGamepadObservationPoll{observer_guid}=>{
                self.poll()?;
                self.core.poll_gamepad_observation(scope,&observer_guid)
            },
            MappingGamepadObservationStop{observer_guid}=>self.core.stop_gamepad_observation(scope,&observer_guid),
            MappingAnalogObservationStart{target,observer_guid,registration}=>self.core.start_analog_observation(scope,target,observer_guid,registration),
            MappingAnalogObservationPoll{observer_guid}=>{
                self.poll()?;
                self.core.poll_analog_observation(scope,&observer_guid)
            },
            MappingAnalogObservationStop{observer_guid}=>self.core.stop_analog_observation(scope,&observer_guid),
            MappingInputObservationStart{target,observer_guid}=>self.core.start_input_observation(scope,target,observer_guid),
            MappingInputObservationPoll{observer_guid}=>{
                self.poll()?;
                self.core.poll_input_observation(scope,&observer_guid)
            },
            MappingInputObservationStop{observer_guid}=>self.core.stop_input_observation(scope,&observer_guid),
            MappingObservationStart{target,observer_guid}=>self.core.start_observation(scope,target,observer_guid),
            MappingObservationPoll{observer_guid}=>{
                self.poll()?;
                self.core.poll_observation(scope,&observer_guid)
            },
            MappingObservationStop{observer_guid}=>self.core.stop_observation(scope,&observer_guid),
            MappingDeviceStart{product_id,device_container_id,owner_guid}=>self.core.start_device(scope,product_id,&device_container_id,owner_guid),
            MappingDevicePublish{owner_guid,document}=>self.core.publish(scope,&owner_guid,document),
            MappingDeviceStop{owner_guid}=>self.core.stop_device(scope,&owner_guid),
            MappingDeviceEvents{owner_guid}=>{
                self.poll()?;self.core.registration(scope,&owner_guid)?;
                let row=self.core.devices.get_mut(&owner_guid).unwrap();
                if let Some(error)=&row.failure {anyhow::bail!("{error}");}
                Ok(json!({"owner_guid":owner_guid,"events":row.pending.drain()?}))
            }
            MappingDeviceHypershiftActive{owner_guid}=>{
                let row=self.core.registration(scope,&owner_guid)?;row.ensure_active()?;row.target.revalidate()?;
                let enabled=self.core.modes.lock().map_err(|_|anyhow::anyhow!("Mapping modes poisoned"))?.hypershift();
                Ok(json!({"owner_guid":owner_guid,"isEnabled":enabled}))
            }
            MappingKeyboardGamingIndicator{owner_guid,enabled}=>{let row=self.core.registration(scope,&owner_guid)?;row.ensure_active()?;identity::gaming_indicator(&row.target,enabled)},
            MappingOtfmInputStart{owner_guid}=>{
                let row=self.core.registration(scope,&owner_guid)?;row.ensure_active()?;let caller=row.caller;
                ensure!(!row.input_registered,"OTFM input callback already registered");
                self.core.connections.notifications(&Lease::Device{scope,caller},true)?;
                self.core.devices.get(&owner_guid).unwrap().worker.input_observation(true)?;
                self.core.devices.get_mut(&owner_guid).unwrap().input_registered=true;
                Ok(json!({"registered":true,"owner_guid":owner_guid}))
            }
            GlobalMode=>capture::Host::global_mode(&mut self.core),
            MacroRecorderStorageWrite{key,value}=>{self.core.publish_storage_value(&key,&value)?;Ok(json!({"key":key,"written":true}))}
            GlobalShortcutStart{owner_guid}=>{
                Uuid::parse_str(&owner_guid)?;ensure!(self.core.global.is_none(),"Global shortcut owner already active");
                self.core.global=Some(Global{scope,owner:owner_guid.clone(),revision:None,execution_revision:None,engine:None,pending:Inbox::new(scope),failure:None});
                Ok(serde_json::to_value(razer_ipc::shortcuts::RegistrationReceipt::new(owner_guid,true))?)
            }
            GlobalShortcutPublish{owner_guid,revision,document}=>self.core.global_publish(scope,&owner_guid,revision,document),
            GlobalShortcutPoll{owner_guid}=>{
                self.poll()?;let current=self.core.global.as_mut().context("Global shortcut owner absent")?;
                ensure!(current.scope==scope && current.owner==owner_guid,"Global shortcut owner changed");
                if let Some(error)=&current.failure {anyhow::bail!("{error}");}
                Ok(serde_json::to_value(razer_ipc::shortcuts::EventBatch::new(owner_guid,current.pending.drain()?))?)
            }
            GlobalShortcutStop{owner_guid}=>{
                let current=self.core.global.as_ref().context("Global shortcut owner absent")?;
                ensure!(current.scope==scope && current.owner==owner_guid,"Global shortcut owner changed");
                self.core.replace_global_engine(None)?;self.core.global=None;
                Ok(serde_json::to_value(razer_ipc::shortcuts::StopReceipt::new(owner_guid,true))?)
            }
            MappingExecution{owner_guid,revision,target}=>{
                if self.core.global.as_ref().is_some_and(|current|current.scope==scope&&current.owner==owner_guid) {
                    let current=self.core.global.as_mut().unwrap();
                    ensure!(current.execution_revision.is_none_or(|old|revision>old),"Stale global execution control");
                    current.execution_revision=Some(revision);
                } else {
                    let current=self.core.registration(scope,&owner_guid)?;
                    current.ensure_active()?;
                    ensure!(current.revision.is_none_or(|old|revision>old),"Stale mapping execution control");
                    self.core.devices.get_mut(&owner_guid).unwrap().revision=Some(revision);
                }
                let observations=apply_execution(target,|operation| {
                    let reply=match operation {
                        mapping_control::Operation::EnableMapping=>self.core.mapping(scope,true),
                        mapping_control::Operation::DisableMapping=>self.core.mapping(scope,false),
                        mapping_control::Operation::EnableGlobalShortcuts=>self.core.shortcuts(scope,true),
                        mapping_control::Operation::DisableGlobalShortcuts=>self.core.shortcuts(scope,false),
                    }?;
                    Ok(mapping_control::Observation::new(operation,reply.result(),reply.reason().into()))
                })?;
                Ok(serde_json::to_value(mapping_control::Receipt::with_observations(owner_guid,revision,target,observations))?)
            }
            EnableGlobalShortcuts{enable}=>{let reply=self.core.shortcuts(scope,enable)?;ensure!(reply.result(),"{}",reply.reason());Ok(json!({"enabled":enable}))}
            RegisterShortcut{vkey_code,modifiers,argument}=>{
                let key = (vkey_code, modifiers);
                let registered = self.core.hotkeys.registered()?;
                let existing = registered.iter().find(|shortcut| (shortcut.virtual_key, shortcut.modifiers) == key);
                ensure_hotkey_owner(scope, self.core.hotkey_scopes.get(&key), existing.map(|shortcut| shortcut.argument.as_str()), &argument)?;
                self.core.hotkeys.register(razer_platform::global_shortcuts::Shortcut{virtual_key:vkey_code,modifiers,argument})?;
                self.core.hotkey_scopes.entry((vkey_code,modifiers)).or_default().insert(scope);
                Ok(json!({"registered":true,"vkey_code":vkey_code,"modifiers":modifiers}))
            }
            UnregisterShortcut{vkey_code,modifiers}=>{self.core.unregister_hotkey(scope,(vkey_code,modifiers))?;Ok(json!({"registered":false,"vkey_code":vkey_code,"modifiers":modifiers}))}
            GlobalShortcuts=>Ok(Value::Array(self.core.hotkeys.registered()?.into_iter().filter(|s|self.core.hotkey_scopes.get(&(s.virtual_key,s.modifiers)).is_some_and(|scopes|scopes.contains(&scope))).map(|s|json!({"virtualKey":s.virtual_key,"modifiers":s.modifiers,"argument":s.argument})).collect())),
            ShortcutEvents=>{
                self.core.poll_hotkeys()?;
                if let Some(error)=self.core.hotkey_failures.get(&scope){anyhow::bail!("{error}");}
                Ok(Value::Array(self.core.hotkey_events.entry(scope).or_default().drain(..).collect()))
            }
            Shutdown=>{self.release(scope)?;Ok(json!({"stopped":true}))}
            _=>anyhow::bail!("Request does not belong to the mapping owner"),
        }
    }
    fn poll(&mut self) -> Result<()> {
        self.core.poll_hotkeys()?;
        for row in self.core.devices.values_mut() {
            if row.physical_retired {
                continue;
            }
            if let Err(error) = row.target.revalidate() {
                let cleanup = row.stop(
                    &self.core.macros,
                    &self.core.modes,
                    &mut self.core.connections,
                );
                row.failure = Some(format!(
                    "Mapping physical identity: {error:#}; registration cleanup: {cleanup:?}"
                ));
                continue;
            }
            if let Some(hardware) = row.hardware.as_mut() {
                match hardware.inputs() {
                    Ok(events) => {
                        for event in events {
                            match event {
                        Ok(packet)=>if let Err(error)=row.worker.hardware_input(packet){row.failure=Some(format!("{error:#}"));},
                        Err(error)=>row.push(json!({"deviceInfo":row.target.info,"deviceEventType":0,"jsonEvent":{"type":"error","error":error},"timeTick":razer_platform::event_clock::tick()?})),
                    }
                        }
                    }
                    Err(error) => {
                        let cleanup = row.stop(
                            &self.core.macros,
                            &self.core.modes,
                            &mut self.core.connections,
                        );
                        row.failure = Some(format!(
                            "Mapping hardware input owner: {error:#}; registration cleanup: {cleanup:?}"
                        ));
                    }
                }
            }
        }
        use razer_device::mapping_native_events::{NativeStreamKind, decode_native_stream_packet};
        for input in self.core.connections.inputs() {
            for lease in input.leases {
                match lease {
                    Lease::Rotation { scope, guid } => {
                        if let Err(error) = &input.result {
                            for row in self
                                .core
                                .rotations
                                .values_mut()
                                .filter(|row| row.scope == scope && row.guid == guid)
                            {
                                row.failure = Some(error.clone());
                            }
                        }
                    }
                    Lease::Device { caller, .. } => {
                        if let Some(row) = self
                            .core
                            .devices
                            .values_mut()
                            .find(|row| row.caller == caller)
                        {
                            if row.physical_retired {
                                continue;
                            }
                            match &input.result {
                                Ok(packet) => {
                                    if let Err(error) = row.worker.input(packet.clone()) {
                                        row.failure = Some(format!("{error:#}"));
                                    }
                                }
                                Err(error) => {
                                    row.failure = Some(error.clone());
                                }
                            }
                        }
                    }
                    Lease::Philips { scope, guid } => match &input.result {
                        Ok(packet) => match decode_native_stream_packet(&packet.bytes) {
                            Ok(decoded) => {
                                use razer_device::mapping_native_events::NativeStreamInput;
                                if let NativeStreamInput::Mouse(mouse) = decoded.input {
                                    if decoded.kind == NativeStreamKind::Notify
                                        && mouse.button_flags == 1
                                    {
                                        self.philips.input(
                                            scope,
                                            guid,
                                            philips::DriverEvent::LeftDown,
                                        );
                                    }
                                    if decoded.kind == NativeStreamKind::Redirect
                                        && mouse.packed_buttons() != 0
                                    {
                                        self.philips.input(
                                            scope,
                                            guid,
                                            philips::DriverEvent::Redirect,
                                        );
                                    }
                                    if mouse.movement != [0, 0] {
                                        self.philips.input(
                                            scope,
                                            guid,
                                            philips::DriverEvent::MouseMove,
                                        );
                                    }
                                }
                            }
                            Err(error) => self.philips.input_failed(scope, guid, error.to_string()),
                        },
                        Err(error) => self.philips.input_failed(scope, guid, error.clone()),
                    },
                    Lease::Capture { scope, guid } => match &input.result {
                        Ok(packet) => match decode_native_stream_packet(&packet.bytes) {
                            Ok(decoded) if decoded.kind == NativeStreamKind::Redirect => {
                                if matches!(decoded.input,razer_device::mapping_native_events::NativeStreamInput::Mouse(mouse) if mouse.packed_buttons()==0)
                                {
                                    continue;
                                }
                                if let Some(input) = decoded
                                    .input
                                    .normalized(false, packet.modifiers)
                                    .and_then(device::native_lookup)
                                {
                                    self.capture.input(
                                        scope,
                                        guid,
                                        input.to_native_json(),
                                        packet.time_tick,
                                    );
                                }
                            }
                            Ok(_) => {}
                            Err(error) => self.capture.input_failed(scope, guid, error.to_string()),
                        },
                        Err(error) => self.capture.input_failed(scope, guid, error.clone()),
                    },
                }
            }
        }
        self.philips.poll(&mut self.core)?;
        for row in self.core.devices.values_mut() {
            if row.physical_retired {
                continue;
            }
            match row.worker.events() {
                Ok(events) => {
                    for event in events {
                        match event {
                    device::Event::Gamepad{observer,delivery}=>{
                        gamepad_observation::notify(&mut self.core.gamepad_observers,row.caller,observer,delivery);
                    },
                    device::Event::Analog{observer,delivery}=>{
                        for subscription in self.core.analog_observers.values_mut() {
                            subscription.push(row.caller,observer,delivery.clone());
                        }
                    },
                    device::Event::SnapTap{observer,observation}=>{
                        for subscription in self.core.observers.values_mut() {
                            subscription.push(row.caller,observer,observation.clone());
                        }
                    },
                    device::Event::Input{input,tick,redirected:false,observer}=>{
                        if let Some(epoch)=observer {
                            let event=razer_ipc::input_observation::InputEvent::new(input.clone(),tick);
                            for subscription in self.core.input_observers.values_mut() { subscription.push(row.caller,epoch,event.clone()); }
                        }
                        if row.input_registered { row.push(json!({"deviceInfo":row.target.info,"deviceEventType":1,"jsonEvent":{"type":"inputnotified","input":input.to_native_string()},"timeTick":tick})); }
                    },
                    device::Event::Failure(error)=>row.push(json!({"deviceInfo":row.target.info,"deviceEventType":0,"jsonEvent":{"type":"error","error":error},"timeTick":razer_platform::event_clock::tick()?})),
                    _=>{},
                }
                    }
                }
                Err(error) => row.failure = Some(format!("{error:#}")),
            }
        }
        for event in self.core.notifications.try_iter() {
            if let Some(row) = self
                .core
                .devices
                .values_mut()
                .find(|row| row.caller == event.caller && !row.physical_retired)
            {
                if event.global {
                    if let Some(global) = self.core.global.as_mut() {
                        global.pending.push(
                            global.scope,
                            razer_ipc::shortcuts::UnsupportedMapping::new(
                                serde_json::to_string(&row.target.info)?,
                                event.input,
                                event.output,
                                event.tick,
                            ),
                        );
                    }
                } else {
                    row.push(json!({"deviceInfo":row.target.info,"deviceEventType":2,"jsonEvent":{"type":"unsupportedmapping","input":event.input,"output":event.output},"timeTick":event.tick}));
                }
            }
        }
        let events = self
            .core
            .modes
            .lock()
            .map_err(|_| anyhow::anyhow!("Mapping modes poisoned"))?
            .take_events();
        for event in events {
            let value = json!({"source":"devicemodechanged","eventType":event.kind(),"event":event.to_native_json().to_string(),"timeTick":event.time_tick()});
            self.capture.mode(value.clone());
            for row in self.core.devices.values_mut() {
                if row.physical_retired {
                    continue;
                }
                row.push(value.clone());
                if matches!(
                    event,
                    razer_device::mapping_native_mode::Event::Hypershift { enabled: false, .. }
                ) {
                    row.worker.hypershift_released()?;
                }
            }
        }
        let completions = self
            .core
            .macros
            .lock()
            .map_err(|_| anyhow::anyhow!("Mapping macro owner poisoned"))?
            .observed_completions();
        for (id, caller, result) in completions {
            let errors = match result {
                Ok(finished) => finished
                    .completion
                    .execution_error
                    .into_iter()
                    .chain(finished.completion.operation_errors)
                    .chain(finished.completion.cleanup_errors)
                    .collect::<Vec<_>>(),
                Err(error) => vec![format!("{error:#}")],
            };
            if !errors.is_empty() {
                if let Some(row) = self
                    .core
                    .devices
                    .values_mut()
                    .find(|row| Some(row.caller) == caller)
                {
                    row.push(json!({"deviceInfo":row.target.info,"deviceEventType":0,"jsonEvent":{"type":"error","error":errors.join("; "),"execution":id},"timeTick":razer_platform::event_clock::tick()?}));
                } else {
                    eprintln!("Mapping execution {id}: {}", errors.join("; "));
                }
            }
        }
        Ok(())
    }
    fn release(&mut self, scope: Uuid) -> Result<()> {
        let mut errors = Vec::new();
        for owner in self
            .core
            .rotations
            .iter()
            .filter_map(|(owner, row)| (row.scope == scope).then_some(owner.clone()))
            .collect::<Vec<_>>()
        {
            if let Err(error) = self.core.stop_rotation(scope, &owner) {
                errors.push(format!("{error:#}"));
            }
        }

        let input_observers = self
            .core
            .input_observers
            .iter()
            .filter_map(|(id, observer)| (observer.scope == scope).then_some(id.clone()))
            .collect::<Vec<_>>();
        for observer in input_observers {
            if let Err(error) = self.core.stop_input_observation(scope, &observer) {
                errors.push(format!("Input observation cleanup: {error:#}"));
            }
        }
        let gamepad_observers = self
            .core
            .gamepad_observers
            .iter()
            .filter_map(|(id, observer)| (observer.scope == scope).then_some(id.clone()))
            .collect::<Vec<_>>();
        for observer in gamepad_observers {
            if let Err(error) = self.core.stop_gamepad_observation(scope, &observer) {
                errors.push(format!("Gamepad observation cleanup: {error:#}"));
            }
        }
        let analog_observers = self
            .core
            .analog_observers
            .iter()
            .filter_map(|(id, observer)| (observer.scope == scope).then_some(id.clone()))
            .collect::<Vec<_>>();
        for observer in analog_observers {
            if let Err(error) = self.core.stop_analog_observation(scope, &observer) {
                errors.push(format!("Analog observation cleanup: {error:#}"));
            }
        }
        let observers = self
            .core
            .observers
            .iter()
            .filter_map(|(id, observer)| (observer.scope == scope).then_some(id.clone()))
            .collect::<Vec<_>>();
        for observer in observers {
            if let Err(error) = self.core.stop_observation(scope, &observer) {
                errors.push(format!("Mapping observation cleanup: {error:#}"));
            }
        }
        if let Err(error) = self.philips.release(scope, &mut self.core) {
            errors.push(format!("Philips cleanup: {error:#}"));
        }
        if let Err(error) = self.capture.release(scope, &mut self.core) {
            errors.push(format!("Capture cleanup: {error:#}"));
        }
        let owners = self
            .core
            .devices
            .iter()
            .filter_map(|(owner, row)| (row.scope == scope).then_some(owner.clone()))
            .collect::<Vec<_>>();
        for owner in owners {
            if let Err(error) = self.core.stop_device(scope, &owner) {
                errors.push(format!("{error:#}"));
            }
        }
        if self
            .core
            .global
            .as_ref()
            .is_some_and(|global| global.scope == scope)
        {
            if let Err(error) = self.core.replace_global_engine(None) {
                errors.push(format!("{error:#}"));
            } else {
                self.core.global = None;
            }
        }
        for lease in self.core.connections.leases_for(scope) {
            if let Lease::Device { caller, .. } = &lease {
                if self.core.devices.values().any(|row| row.caller == *caller) {
                    continue;
                }
            }
            if let Err(error) = self.core.connections.release(&lease) {
                errors.push(format!("{error:#}"));
            }
        }
        let keys = self
            .core
            .hotkey_scopes
            .iter()
            .filter_map(|(key, scopes)| scopes.contains(&scope).then_some(*key))
            .collect::<Vec<_>>();
        for key in keys {
            if let Err(error) = self.core.unregister_hotkey(scope, key) {
                errors.push(format!("{error:#}"));
            }
        }
        if self.core.mapping_suppression.contains(&scope) {
            match self.core.mapping(scope, true) {
                Ok(reply) if !reply.result() => errors.push(reply.reason().into()),
                Err(error) => errors.push(format!("{error:#}")),
                _ => {}
            }
        }
        if self.core.shortcut_suppression.contains(&scope) {
            match self.core.shortcuts(scope, true) {
                Ok(reply) if !reply.result() => errors.push(reply.reason().into()),
                Err(error) => errors.push(format!("{error:#}")),
                _ => {}
            }
        }
        if self.core.scopes.len() == 1 && self.core.scopes.contains(&scope) {
            if let Err(error) = self.shutdown() {
                errors.push(format!("Mapping final scope cleanup: {error:#}"));
            }
            if errors.is_empty() {
                *self
                    .core
                    .macros
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Mapping macro owner poisoned"))? = State::new();
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        self.core.hotkey_events.remove(&scope);
        self.core.hotkey_failures.remove(&scope);
        self.core.scopes.remove(&scope);
        Ok(())
    }
    fn shutdown(&mut self) -> Result<()> {
        let mut errors = Vec::new();
        for (owner, scope) in self
            .core
            .rotations
            .iter()
            .map(|(owner, row)| (owner.clone(), row.scope))
            .collect::<Vec<_>>()
        {
            if let Err(error) = self.core.stop_rotation(scope, &owner) {
                errors.push(format!("{error:#}"));
            }
        }

        for row in self.core.devices.values_mut() {
            if let Err(error) = row.stop(
                &self.core.macros,
                &self.core.modes,
                &mut self.core.connections,
            ) {
                errors.push(format!("{error:#}"));
            }
        }
        let result = match self.core.macros.lock() {
            Ok(mut macros) => macros.shutdown(),
            Err(error) => {
                errors.push("Mapping macro owner poisoned during shutdown".into());
                error.into_inner().shutdown()
            }
        };
        errors.extend(result.cleanup_errors);
        for (_, result) in result.executions {
            match result {
                Ok(finished) => errors.extend(finished.completion.cleanup_errors),
                Err(error) => errors.push(format!("{error:#}")),
            }
        }
        if let Err(error) = self.core.connections.shutdown() {
            errors.push(format!("{error:#}"));
        }
        if let Err(error) = self.core.hotkeys.shutdown() {
            errors.push(format!("{error:#}"));
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}

fn cleanup_once(
    previous: &mut Option<Result<(), String>>,
    cleanup: impl FnOnce() -> Result<()>,
) -> Result<()> {
    previous
        .get_or_insert_with(|| cleanup().map_err(|error| format!("{error:#}")))
        .clone()
        .map_err(anyhow::Error::msg)
}

fn stop_registration_owners(
    inputs: impl FnOnce() -> Result<()>,
    macros: impl FnOnce() -> Result<()>,
    modes: impl FnOnce() -> Result<()>,
    connection: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let inputs = inputs();
    let macros = macros();
    let modes = modes();
    let connection = connection();
    ensure!(
        inputs.is_ok() && macros.is_ok() && modes.is_ok() && connection.is_ok(),
        "Mapping registration cleanup: inputs={inputs:?}; macros={macros:?}; modes={modes:?}; connection={connection:?}"
    );
    Ok(())
}

fn stop_input_owners(
    hardware: impl FnOnce() -> Result<()>,
    device: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let hardware = hardware();
    let device = device();
    ensure!(
        hardware.is_ok() && device.is_ok(),
        "Mapping input cleanup: hardware={hardware:?}; device={device:?}"
    );
    Ok(())
}

fn publish_dependencies(
    document: &razer_model::shortcuts::CompiledDocument,
    mut publish: impl FnMut(&str, &Value) -> Result<()>,
) -> Result<()> {
    for (key, values) in [
        ("synapseDefaultTurbos", document.default_turbos()),
        ("synapseTurbos", document.synapse_turbos()),
        ("synapseMacros", document.macros()),
        ("synapsePhasedMacros", document.phased_macros()),
    ] {
        publish(key, &Value::Array(values.to_vec()))?;
    }
    Ok(())
}
fn execution_confirmed(observation: &mapping_control::Observation) -> bool {
    use mapping_control::Operation::*;
    observation.result()
        || matches!(
            (observation.operation(), observation.reason()),
            (EnableMapping, "already enabled")
                | (DisableMapping, "already disabled")
                | (EnableGlobalShortcuts, "hotkey was not disabled")
        )
}
fn apply_execution(
    target: mapping_control::Target,
    mut apply: impl FnMut(mapping_control::Operation) -> Result<mapping_control::Observation>,
) -> Result<Vec<mapping_control::Observation>> {
    use mapping_control::Operation::*;
    let mut operations = vec![if target.mapping_enabled() {
        EnableMapping
    } else {
        DisableMapping
    }];
    if let Some(enabled) = target.global_shortcuts_enabled() {
        operations.push(if enabled {
            EnableGlobalShortcuts
        } else {
            DisableGlobalShortcuts
        });
    }
    let mut observations = Vec::new();
    let mut errors = Vec::new();
    for operation in operations {
        match apply(operation) {
            Ok(observation) => {
                if !execution_confirmed(&observation) {
                    errors.push(observation.reason().to_owned());
                }
                observations.push(observation);
            }
            Err(error) => errors.push(format!("{error:#}")),
        }
    }
    ensure!(errors.is_empty(), "{}", errors.join("; "));
    Ok(observations)
}
#[cfg(test)]
#[path = "backend/tests.rs"]
mod tests;

// A shared chord must retain one argument for every live consumer. The source
// macro editor checks global registrations before publishing a recording key.
fn ensure_hotkey_owner(
    scope: Uuid,
    owners: Option<&BTreeSet<Uuid>>,
    existing: Option<&str>,
    requested: &str,
) -> Result<()> {
    if owners.is_some_and(|owners| owners.iter().any(|owner| *owner != scope)) {
        ensure!(
            existing == Some(requested),
            "Shortcut is already assigned to another owner"
        );
    }
    Ok(())
}
