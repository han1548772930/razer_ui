//! Retained current Seiren stream-mixer local storage, AudioUtil and Marie owner.
use super::*;
use razer_device::{
    audio_util::AudioFlow,
    seiren_stream_mixer::{self as mixer, Action, Operation},
};
use razer_pages::features::SeirenStreamMixerObservation as Observation;
use razer_service::mapping::hotkeys::{Session as Hotkeys, Shortcut};
use razer_storage::host::{HostStorageCall, HostStorageReply, HostStoreKind};

pub(super) struct MixerOwner {
    product_id: u32,
    container_id: String,
    device_name: String,
    view_id: u64,
    view_name: String,
    active: Option<String>,
    status: Sender<Update>,
    playback: Vec<String>,
    record: Vec<String>,
    shortcuts: BTreeMap<(u32, u32), String>,
    hotkeys: Hotkeys,
}
impl MixerOwner {
    pub(super) fn new(
        product_id: u32,
        container_id: String,
        device_name: String,
        view_id: u64,
        view_name: String,
        status: Sender<Update>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            product_id,
            container_id,
            device_name,
            view_id,
            view_name,
            active: None,
            status,
            playback: Vec::new(),
            record: Vec::new(),
            shortcuts: BTreeMap::new(),
            hotkeys: Hotkeys::start()?,
        })
    }
    pub(super) fn enable_shortcuts(&mut self, enabled: bool) -> anyhow::Result<()> {
        self.hotkeys.enable(enabled)
    }
    fn host(&self, action: &str, payload: Value) -> anyhow::Result<HostStorageReply> {
        let (reply, receiver) = mpsc::channel();
        self.status
            .send(Update::Host(
                HostStorageCall {
                    store: HostStoreKind::Window,
                    sender_id: self.view_id,
                    action: action.into(),
                    payload,
                    target_url_array: vec![],
                },
                reply,
            ))
            .context("Audio host owner closed")?;
        receiver
            .recv_timeout(Duration::from_secs(10))
            .context("Audio host storage reply unavailable")?
            .map_err(anyhow::Error::msg)
    }
    pub(super) fn initialize_host(&mut self) -> anyhow::Result<()> {
        let reply = self.host(
            "getWindowStorageItem",
            serde_json::json!({"key":mixer::ACTIVE_KEY}),
        )?;
        if reply.defined {
            let rows: Value = serde_json::from_str(
                reply
                    .value
                    .as_str()
                    .context("Active mixer aggregate is not a JSON string")?,
            )?;
            self.active = rows
                .as_array()
                .and_then(|rows| rows.first())
                .and_then(|row| row["value"].as_str())
                .map(str::to_owned);
        }
        self.host("registerWindowStorageEvent", serde_json::json!({}))?;
        Ok(())
    }
    pub(super) fn other_active(&self) -> bool {
        self.active
            .as_deref()
            .is_some_and(|name| !name.is_empty() && name != self.view_name)
    }
    fn load(&self) -> anyhow::Result<Value> {
        let path = razer_storage::source_local::path();
        let guid = uuid::Uuid::new_v4().to_string();
        let mut snapshot = None;
        razer_storage::source_local::update(&path, mixer::STORAGE_KEY, |old| {
            let (cache, changed) = mixer::prepare_cache(old, self.product_id, &guid)?;
            snapshot = Some(cache.clone());
            Ok(changed.then_some(cache))
        })?;
        snapshot.context("Mixer cache read did not produce data")
    }
    fn edit(&self, action: &Action) -> anyhow::Result<(Value, bool)> {
        let path = razer_storage::source_local::path();
        let guid = uuid::Uuid::new_v4().to_string();
        let mut snapshot = None;
        let mut applied = false;
        razer_storage::source_local::update(&path, mixer::STORAGE_KEY, |old| {
            let (mut cache, created) = mixer::prepare_cache(old, self.product_id, &guid)?;
            applied = mixer::apply_cache(&mut cache, self.product_id, action, &guid)?;
            snapshot = Some(cache.clone());
            Ok((created || applied).then_some(cache))
        })?;
        Ok((
            snapshot.context("Mixer save did not produce data")?,
            applied,
        ))
    }
    fn names(client: &mut ServiceClient, flow: AudioFlow) -> anyhow::Result<Vec<String>> {
        let reply = client.request(ServiceRequest::AudioEndpoints { flow })?;
        serde_json::from_value(reply["source_response"]["devices"].clone())
            .context("Current audio source device names are absent")
    }
    fn refresh_names(&mut self, client: &mut ServiceClient) -> anyhow::Result<()> {
        // Current getStreamMixerDeviceName enumerates both flows and matches
        // their first exact-case substring, preserving OS collection order.
        self.playback = Self::names(client, AudioFlow::Playback)?;
        self.record = Self::names(client, AudioFlow::Record)?;
        Ok(())
    }
    fn name(&self, role: &str) -> String {
        mixer::endpoint_name(&self.device_name, role, &self.playback, &self.record).to_owned()
    }
    pub(super) fn publish(&mut self, client: &mut ServiceClient) -> anyhow::Result<()> {
        self.refresh_names(client)?;
        let cache = self.load()?;
        let _ = self
            .status
            .send(Update::Mixer(Observation::Cache(mixer::ui_payload(
                &cache,
                self.product_id,
                &self.name("Headphones"),
            )?)));
        let _ = self
            .status
            .send(Update::Mixer(Observation::ActiveMixer(self.other_active())));
        let playback = self
            .playback
            .iter()
            .filter(|name| !name.contains(&self.device_name) || name.contains("Headphones"))
            .cloned()
            .collect();
        let record = self
            .record
            .iter()
            .filter(|name| !name.contains(&self.device_name))
            .cloned()
            .collect();
        let _ = self
            .status
            .send(Update::Mixer(Observation::Devices { playback, record }));
        Ok(())
    }
    fn route_external(
        &self,
        client: &mut ServiceClient,
        primary: String,
        routed: String,
    ) -> anyhow::Result<bool> {
        let reply = client.request(ServiceRequest::AudioRouteDevice {
            primary_device: primary,
            routed_device: routed,
            primary_device_id: 0,
        })?;
        // AudioUtil.dispatch checks the native top-level status, not whether
        // the command returned a response field or IPC itself completed.
        Ok(reply["source_status"].as_str() == Some("success"))
    }
    fn operation(
        &mut self,
        client: &mut ServiceClient,
        route: &DirectRoute,
        operation: Operation,
    ) -> anyhow::Result<bool> {
        match operation {
            Operation::Native(operation) => match native_call(client, route, operation)? {
                NativeReply::Boolean(result) => Ok(result),
                reply => anyhow::bail!("Mixer native setter returned {reply:?}"),
            },
            Operation::External { channel, device } => {
                self.refresh_names(client)?;
                self.route_external(client, self.name(&channel), device)
            }
            Operation::Playback(device) => {
                self.refresh_names(client)?;
                let primary = self.name("PlaybackMix");
                let headphones = self.name("Headphones");
                let external = !device.is_empty() && device != headphones;
                let succeeded = self.route_external(
                    client,
                    primary,
                    if external { device } else { String::new() },
                )?;
                if succeeded {
                    let _ = native_call(
                        client,
                        route,
                        NativeOperation::SetPlaybackExternal { enabled: external },
                    )?;
                }
                Ok(succeeded)
            }
        }
    }
    pub(super) fn init_preset(
        &mut self,
        client: &mut ServiceClient,
        route: &DirectRoute,
    ) -> anyhow::Result<()> {
        let cache = self.load()?;
        let operations = mixer::preset_operations(self.product_id, mixer::active_preset(&cache)?)?;
        for operation in operations {
            // Original initDevice awaits each call without gating later writes
            // on false native results. A thrown route helper is caught inside
            // changePlaybackMixDevice, so it does not stop following inputs.
            let helper = matches!(&operation, Operation::Playback(_));
            match self.operation(client, route, operation) {
                Ok(false) => report(
                    &self.status,
                    "Mixer preset native setter returned false".into(),
                ),
                Err(error) if helper => {
                    report(&self.status, format!("Mixer playback route: {error:#}"))
                }
                Err(error) => return Err(error),
                _ => {}
            }
        }
        Ok(())
    }
    pub(super) fn set_active(
        &mut self,
        client: &mut ServiceClient,
        route: &DirectRoute,
        active: Option<String>,
    ) -> anyhow::Result<()> {
        if self.active == active {
            return Ok(());
        }
        self.active = active;
        if self.active.is_none() {
            self.host(
                "removeWindowStorageItem",
                serde_json::json!({"key":mixer::ACTIVE_KEY}),
            )?;
            return Ok(());
        }
        let cache = self.load()?;
        let current = cache["devices"]
            .as_array()
            .and_then(|devices| {
                devices
                    .iter()
                    .find(|device| device["productId"] == self.product_id)
            })
            .context("Mixer current device is absent")?;
        if self.other_active() && current["isStreamMixerEnabled"] == true {
            // setupActiveStreamMixer's direct cache edit does not increment.
            let _ = set_mode(client, route, false, &self.status)?;
            let path = razer_storage::source_local::path();
            razer_storage::source_local::update(&path, mixer::STORAGE_KEY, |old| {
                let mut cache = old.context("Mixer cache vanished during active-owner change")?;
                for device in cache["devices"]
                    .as_array_mut()
                    .context("Mixer devices is not an array")?
                {
                    if device["productId"] == self.product_id {
                        device["isStreamMixerEnabled"] = serde_json::json!(false);
                    }
                }
                Ok(Some(cache))
            })?;
            self.publish(client)?;
            return Ok(());
        }
        self.host(
            "setWindowStorageItem",
            serde_json::json!({"key":mixer::ACTIVE_KEY,"value":self.active}),
        )?;
        self.init_preset(client, route)
    }
    pub(super) fn claim(
        &mut self,
        client: &mut ServiceClient,
        route: &DirectRoute,
    ) -> anyhow::Result<()> {
        self.set_active(client, route, Some(self.view_name.clone()))
    }
    pub(super) fn event(
        &mut self,
        client: &mut ServiceClient,
        route: &DirectRoute,
        event: Value,
    ) -> anyhow::Result<()> {
        if event["key"] != mixer::ACTIVE_KEY {
            return Ok(());
        }
        self.set_active(client, route, event["newValue"].as_str().map(str::to_owned))?;
        let _ = self
            .status
            .send(Update::Mixer(Observation::ActiveMixer(self.other_active())));
        Ok(())
    }
    pub(super) fn execute(
        &mut self,
        client: &mut ServiceClient,
        route: &DirectRoute,
        request: &SeirenStreamMixerRequested,
        stream_enabled: &mut bool,
    ) -> anyhow::Result<()> {
        let action = request.action();
        let before = self.load()?;
        let (_cache, applied) = self.edit(action)?;
        // updateStorage's UI cache broadcast precedes native execution.
        self.publish(client)?;
        save_device_version(&self.status, request)?;
        if !applied {
            return Ok(());
        }
        if let Some(enabled) = action.enabled()? {
            *stream_enabled = enabled;
            let result = set_mode(client, route, enabled, &self.status);
            // The descriptor always awaits setupActiveStreamMixer even when
            // its setter returns false. Preserve that failure and owner change.
            let setup = self.set_active(client, route, enabled.then(|| self.view_name.clone()));
            let confirmed = result?;
            setup?;
            ensure!(
                confirmed,
                "Mixer mode setter returned false; local setting remains saved"
            );
        } else if action.applies_active_preset() {
            if *stream_enabled {
                self.init_preset(client, route)?;
            }
        } else if action.event() == "ON_CHANGE_STREAM_MIXER_PRESET_MAPPING" {
            let argument = action.argument()?;
            let guid = argument["changePresetGUID"]
                .as_str()
                .context("Mixer mapping preset GUID is absent")?;
            if let Some(old) = before["presets"]
                .as_array()
                .and_then(|presets| presets.iter().find(|preset| preset["guid"] == guid))
            {
                if let Err(error) = self.change_mapping(
                    &old["shortcutMapping"],
                    &argument["newShortcutMapping"],
                    guid,
                ) {
                    report(&self.status, format!("Mixer preset shortcut: {error:#}"));
                }
            }
        } else {
            let operations = mixer::action_operations(self.product_id, action)?;
            let mut failures = Vec::new();
            let mut results = Vec::new();
            let mut threw = false;
            let is_add = action.event() == "ON_MODIFY_INPUT_CHANNEL"
                && action.argument()?["action"] == "add";
            for operation in operations {
                if is_add && threw && matches!(&operation, Operation::External { .. }) {
                    break;
                }
                match self.operation(client, route, operation.clone()) {
                    Ok(result) => {
                        if !result {
                            failures.push(format!("{operation:?} returned false"));
                        }
                        results.push((operation, result));
                    }
                    Err(error) => {
                        threw = true;
                        failures.push(format!("{operation:?}: {error:#}"));
                        if !is_add {
                            break;
                        }
                    }
                }
            }
            // Earlier false returns are logged by current JS even when a later
            // assignment makes its final result true. Preserve both facts.
            for failure in &failures {
                report(&self.status, format!("Mixer operation: {failure}"));
            }
            let result = !threw && mixer::operation_result(action, &results)?;
            ensure!(
                result || results.is_empty() && !threw,
                "Mixer local cache saved; source operation returned false: {}",
                failures.join("; ")
            );
        }
        self.publish(client)?;
        Ok(())
    }
    fn register_mapping(
        &mut self,
        mapping: &Value,
        guid: &str,
    ) -> anyhow::Result<()> {
        let Some(binding) = mixer::shortcut_binding(self.product_id, mapping)? else {
            return Ok(());
        };
        if self.shortcuts.contains_key(&binding) {
            eprintln!("[Audio_StreamMixer] shortcut already linked: {binding:?}");
            return Ok(());
        }
        self.hotkeys.register(Shortcut {
            virtual_key: binding.0,
            modifiers: binding.1,
            argument: String::new(),
        })?;
        self.shortcuts.insert(binding, guid.into());
        Ok(())
    }
    fn unregister_mapping(
        &mut self,
        mapping: &Value,
    ) -> anyhow::Result<()> {
        let Some(binding) = mixer::shortcut_binding(self.product_id, mapping)? else {
            return Ok(());
        };
        self.hotkeys.unregister(binding.0, binding.1)?;
        self.shortcuts.remove(&binding);
        Ok(())
    }
    fn change_mapping(
        &mut self,
        old: &Value,
        new: &Value,
        guid: &str,
    ) -> anyhow::Result<()> {
        if !old.as_object().is_some_and(serde_json::Map::is_empty) {
            self.unregister_mapping(old)?;
        }
        if new.get("inputID").is_some() {
            self.register_mapping(new, guid)?;
        }
        Ok(())
    }
    pub(super) fn register_presets(&mut self) -> anyhow::Result<()> {
        let cache = self.load()?;
        for preset in cache["presets"]
            .as_array()
            .context("Mixer presets is not an array")?
        {
            if let Err(error) = self.register_mapping(
                &preset["shortcutMapping"],
                preset["guid"]
                    .as_str()
                    .context("Mixer preset GUID is absent")?,
            ) {
                report(&self.status, format!("Mixer initial shortcut: {error:#}"));
            }
        }
        Ok(())
    }
    pub(super) fn poll_shortcuts(
        &mut self,
        client: &mut ServiceClient,
        route: &DirectRoute,
        stream_enabled: bool,
        capturing: bool,
    ) -> anyhow::Result<()> {
        if self.shortcuts.is_empty() {
            return Ok(());
        }
        for event in self.hotkeys.drain()? {
            let key = event.shortcut.virtual_key;
            let modifiers = event.shortcut.modifiers;
            let Some(guid) = self
                .shortcuts
                .get(&(key, modifiers))
                .cloned()
                .filter(|_| !capturing)
            else {
                continue;
            };
            let action = Action::new(
                "ON_SELECTED_STREAM_MIXER_PRESET",
                serde_json::json!({"selectedPresetGuid":guid}),
            )?;
            let (cache, _) = self.edit(&action)?;
            self.publish(client)?;
            if stream_enabled {
                self.init_preset(client, route)?;
            }
            // presetChangedByDeviceHandler emits only after the awaited preset
            // initialization. Preserve its actual callback tick and preset
            // name; neither comes from the notification consumer's clock.
            let preset_name = mixer::active_preset(&cache)?["name"]
                .as_str()
                .context("Mixer preset name is absent")?
                .to_owned();
            self.status
                .send(Update::ProfileNotification {
                    sender_id: self.view_id,
                    product_id: self.product_id,
                    container_id: self.container_id.clone(),
                    preset_name,
                    timer_tick: event.time_tick.into(),
                })
                .context("Mixer notification owner closed")?;
        }
        Ok(())
    }
    pub(super) fn cleanup_shortcuts(&mut self) -> anyhow::Result<()> {
        let mut errors = Vec::new();
        for ((key, modifiers), _) in std::mem::take(&mut self.shortcuts) {
            if let Err(error) = self.hotkeys.unregister(key, modifiers) {
                errors.push(format!("Mixer shortcut cleanup: {error:#}"));
            }
        }
        if let Err(error) = self.hotkeys.close() {
            errors.push(format!("Mixer shortcut scope cleanup: {error:#}"));
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}
pub(super) fn set_mode(
    client: &mut ServiceClient,
    route: &DirectRoute,
    enabled: bool,
    status: &Sender<Update>,
) -> anyhow::Result<bool> {
    let _ = status.send(Update::Ready(false));
    routing(client, false)?;
    match native_call(client, route, NativeOperation::SetMode { enabled })? {
        NativeReply::Boolean(true) => {
            let _ = status.send(Update::Ready(true));
            if enabled {
                routing(client, true)?;
            }
            Ok(true)
        }
        reply => {
            report(status, format!("AudioMarie SetMode: {reply:?}"));
            Ok(false)
        }
    }
}
