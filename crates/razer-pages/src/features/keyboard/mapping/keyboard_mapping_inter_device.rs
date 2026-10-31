//! Current MapInterDevice target observations, forms and original mapping payload.
use super::*;
use crate::features::controls::Choice;
use gpui_kit::component::{
    IndexPath,
    input::{Input, InputEvent, InputState},
    radio::Radio,
    select::{SelectEvent, SelectState},
};
use razer_i18n as i18n;
use razer_widgets::theme::KeyboardInterDeviceColors as Colors;

#[derive(Clone)]
pub struct KeyboardMappingInterDeviceObservation {
    product_id: u32,
    profile_guid: String,
    /// Actual commonReducer.validDevices rows, including target profiles/DPI.
    devices: Vec<Value>,
    /// Actual dictionary lookup for an existing disconnected target. Absence
    /// preserves the original unknown-device branch without guessed resources.
    disconnected_descriptor: Option<(u32, Value)>,
}
impl KeyboardMappingInterDeviceObservation {
    pub fn new(
        product_id: u32,
        profile_guid: impl Into<String>,
        devices: Vec<Value>,
        disconnected_descriptor: Option<(u32, Value)>,
    ) -> Self {
        Self {
            product_id,
            profile_guid: profile_guid.into(),
            devices,
            disconnected_descriptor,
        }
    }
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn profile_guid(&self) -> &str {
        &self.profile_guid
    }
    pub fn devices(&self) -> &[Value] {
        &self.devices
    }
    pub fn disconnected_descriptor(&self) -> Option<(u32, &Value)> {
        self.disconnected_descriptor
            .as_ref()
            .map(|(pid, value)| (*pid, value))
    }
}
#[derive(Clone)]
pub struct KeyboardMappingInterDeviceConfigureRequested {
    source_product_id: u32,
    profile_guid: String,
    owner: EntityId,
    target_product_id: u32,
    ui_window_name: String,
    mw_window_name: String,
    source_message: Value,
}
impl KeyboardMappingInterDeviceConfigureRequested {
    pub fn source_product_id(&self) -> u32 {
        self.source_product_id
    }
    pub fn profile_guid(&self) -> &str {
        &self.profile_guid
    }
    pub fn owner(&self) -> EntityId {
        self.owner
    }
    pub fn target_product_id(&self) -> u32 {
        self.target_product_id
    }
    pub fn ui_window_name(&self) -> &str {
        &self.ui_window_name
    }
    pub fn mw_window_name(&self) -> &str {
        &self.mw_window_name
    }
    pub fn source_message(&self) -> &Value {
        &self.source_message
    }
}
impl EventEmitter<KeyboardMappingInterDeviceConfigureRequested> for KeyboardProductWorkspace {}
#[derive(Default)]
pub(super) struct State {
    owner: Option<EntityId>,
    observation: Option<KeyboardMappingInterDeviceObservation>,
    selectors: [Option<Entity<SelectState<Vec<Choice>>>>; 10],
    subscriptions: [Option<Subscription>; 10],
    dpi_inputs: [[Option<Entity<InputState>>; 2]; 2],
    dpi_sliders: [[Option<Entity<SliderState>>; 2]; 2],
    dpi_subscriptions: [Vec<Subscription>; 2],
    syncing: bool,
}
impl State {
    pub(super) fn dismiss(&mut self) {
        self.selectors = Default::default();
        self.subscriptions = Default::default();
        self.dpi_inputs = Default::default();
        self.dpi_sliders = Default::default();
        self.dpi_subscriptions = Default::default();
    }
}
fn eligible(pid: u32, device: &Value) -> bool {
    device["productId"] != pid
        && device["isSingleProfile"] != true
        && device["interDeviceMappingConfig"]["isSupported"] != false
        && device["category"] != "GAMEPAD"
        && !(device["isMouseConnected"] == true && device["productId"] == device["dongleId"])
}
fn message(workspace: &KeyboardProductWorkspace, alias: &str) -> String {
    app_specific::data(workspace.spec.product_id)
        .messages
        .get(alias)
        .map(|key| t(key))
        .unwrap_or_default()
}
fn functions(device: &Value) -> Vec<(&'static str, &'static str)> {
    let mut values = vec![("profileNavigationGroup", "SWITCH_PROFILE")];
    if matches!(device["category"].as_str(), Some("MOUSE" | "MOUSEPLUSMAT")) {
        values.push(("sensitivityGroup", "SENSITIVITY"));
    }
    if device["category"] == "KEYPAD" {
        values.push(("keymapGroup", "SWITCH_KEYMAP"));
    }
    values
}
fn reload_source_mapping(slot: &mut Slot) -> bool {
    if slot.category != "INTERDEVICE" {
        return false;
    }
    // Every current keyboard MapInterDevice componentDidUpdate(lang) clears
    // isLoaded, then loadDevices/assignDefaultValuesToGroup restore activeButton.
    // Keep actuation and the other slot's editors with their own owners.
    slot.staged["interDeviceGroup"] = if slot.original["outputType"] == "interDeviceGroup" {
        slot.original["interDeviceGroup"].clone()
    } else {
        json!({})
    };
    true
}
fn update_selected_name(group: &mut Value, devices: &[Value]) {
    if let Some(device) = devices
        .iter()
        .find(|device| device["productId"] == group["interPID"])
    {
        group["name"] = device["name"].clone();
    }
}
pub(super) fn prepare_save(workspace: &KeyboardProductWorkspace, edit: &mut Edit) {
    let active_slots = if edit.secondary_enabled { 2 } else { 1 };
    for slot in edit.slots.iter_mut().take(active_slots) {
        if slot.category != "INTERDEVICE" {
            continue;
        }
        // Current getInterDeviceGroup reads the selected validDevices row's
        // name when saving, not the name cached in activeButton.assignmentGroup.
        let devices = workspace.mapping_inter_rows(slot);
        update_selected_name(&mut slot.staged["interDeviceGroup"], &devices);
    }
}

#[cfg(test)]
mod locale_tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn locale_reload_restores_each_source_function_without_touching_actuation() {
        for (function, settings) in [
            (
                "profileNavigationGroup",
                json!({"profileNavigationAssignment":"Specific","guid":"profile-1","name":"CANCEL","lightpacId":"lighting-1"}),
            ),
            (
                "sensitivityGroup",
                json!({"sensitivityAssignment":"DPI_Clutch","x":1200,"y":2400,"independent":true}),
            ),
            ("keymapGroup", json!({"type":"nextKeymap"})),
        ] {
            let mut group = json!({"interPID":182,"name":"Saved name","interDeviceType":function});
            group[function] = settings;
            let original = json!({"outputType":"interDeviceGroup","interDeviceGroup":group,"actuationPoint":1.0});
            let mut slot = Slot::new(original.clone());
            slot.staged["interDeviceGroup"] = json!({"interPID":653,"name":"Pending selection","interDeviceType":"profileNavigationGroup","profileNavigationGroup":{"profileNavigationAssignment":"NextProfile"}});
            slot.staged["actuationPoint"] = json!(1.5);

            assert!(reload_source_mapping(&mut slot));
            assert_eq!(slot.staged["interDeviceGroup"], group);
            assert_eq!(slot.staged["actuationPoint"], 1.5);
            assert_eq!(slot.original, original);
        }
    }

    #[::core::prelude::v1::test]
    fn locale_reload_keeps_other_mapping_categories_and_unmapped_source_defaults() {
        let original =
            json!({"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_A","modifiers":[]}});
        let mut slot = Slot::new(original.clone());
        slot.staged["keyboardGroup"]["key"] = json!("KEY_B");
        let pending = slot.staged.clone();
        assert!(!reload_source_mapping(&mut slot));
        assert_eq!(slot.staged, pending);

        slot.category = "INTERDEVICE".into();
        slot.staged = json!({"outputType":"interDeviceGroup","interDeviceGroup":{"interPID":653}});
        assert!(reload_source_mapping(&mut slot));
        assert_eq!(slot.staged["interDeviceGroup"], json!({}));
        assert_eq!(slot.original, original);
    }

    #[::core::prelude::v1::test]
    fn inter_device_save_uses_the_selected_current_name_and_preserves_profile_names() {
        let mut group = json!({"interPID":182,"name":"Saved English name","interDeviceType":"profileNavigationGroup","profileNavigationGroup":{"profileNavigationAssignment":"Specific","guid":"profile-1","name":"CANCEL"}});
        let profile = group["profileNavigationGroup"].clone();
        let devices = vec![
            json!({"productId":653,"name":"Other device"}),
            json!({"productId":182,"name":"当前设备名称"}),
        ];
        update_selected_name(&mut group, &devices);
        assert_eq!(group["name"], "当前设备名称");
        assert_eq!(group["interPID"], 182);
        assert_eq!(group["profileNavigationGroup"], profile);

        let saved = group.clone();
        update_selected_name(&mut group, &devices[..1]);
        assert_eq!(group, saved);
    }
}
pub(super) fn valid(workspace: &KeyboardProductWorkspace, slot: &Slot) -> bool {
    let group = &slot.staged["interDeviceGroup"];
    let Some(device) = workspace
        .mapping_inter_rows(slot)
        .into_iter()
        .find(|device| device["productId"] == group["interPID"])
    else {
        return false;
    };
    if device["isNotConnected"] == true {
        return false;
    }
    let data = app_specific::data(workspace.spec.product_id);
    match group["interDeviceType"].as_str() {
        Some("profileNavigationGroup") => {
            let profiles = device["profiles"].as_array();
            let value = &group["profileNavigationGroup"];
            profiles.is_some_and(|profiles| !profiles.is_empty())
                && data
                    .profile_choices
                    .iter()
                    .any(|choice| choice["id"] == value["profileNavigationAssignment"])
                && (value["profileNavigationAssignment"] != "Specific"
                    || profiles.is_some_and(|profiles| {
                        profiles
                            .iter()
                            .any(|profile| profile["guid"] == value["guid"])
                    }))
        }
        Some("keymapGroup") => {
            device["profiles"]
                .as_array()
                .is_some_and(|profiles| !profiles.is_empty())
                && data
                    .keymap_choices
                    .iter()
                    .any(|choice| choice["id"] == group["keymapGroup"]["type"])
        }
        Some("sensitivityGroup") => {
            workspace
                .mapping_inter_sensitivity_choices()
                .iter()
                .any(|choice| choice["id"] == group["sensitivityGroup"]["sensitivityAssignment"])
                && workspace
                    .mapping_inter_stages(&device)
                    .is_some_and(|stages| !stages.is_empty())
        }
        _ => false,
    }
}
impl KeyboardProductWorkspace {
    pub fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reload_mapping_inter_device(window, cx);
    }

    fn reload_mapping_inter_device(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut reload = [false; 2];
        if let Some(edit) = &mut self.analog_gamepad.mapping.edit {
            let active_slots = if edit.secondary_enabled { 2 } else { 1 };
            for (slot, requested) in edit.slots.iter_mut().zip(&mut reload).take(active_slots) {
                *requested = reload_source_mapping(slot);
            }
        }
        for (slot, requested) in reload.into_iter().enumerate() {
            if requested {
                self.init_mapping_inter_device(slot, window, cx);
            }
        }
        // Both lang and validDevices reload the source UI without a mutation.
        cx.notify();
    }

    /// ActiveButton uses the original complete validDevices observation;
    /// MapInterDevice's eligible selection filter is a separate source branch.
    pub(super) fn mapping_inter_observed_devices(&self) -> Option<&[Value]> {
        self.analog_gamepad
            .mapping
            .inter_device
            .observation
            .as_ref()
            .filter(|observation| {
                observation.product_id == self.spec.product_id
                    && self.draft["guid"] == observation.profile_guid
            })
            .map(|observation| observation.devices.as_slice())
    }
    pub fn accepts_mapping_inter_device_configure(
        &self,
        request: &KeyboardMappingInterDeviceConfigureRequested,
    ) -> bool {
        self.analog_gamepad.mapping.inter_device.owner == Some(request.owner)
            && self.spec.product_id == request.source_product_id
            && self.draft["guid"] == request.profile_guid
    }
    pub fn observe_mapping_inter_device(
        &mut self,
        observation: KeyboardMappingInterDeviceObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if observation.product_id != self.spec.product_id
            || self.draft["guid"] != observation.profile_guid
        {
            return;
        }
        if self
            .analog_gamepad
            .mapping
            .inter_device
            .observation
            .as_ref()
            .is_some_and(|old| {
                old.product_id == observation.product_id
                    && old.profile_guid == observation.profile_guid
                    && old.devices == observation.devices
                    && old.disconnected_descriptor == observation.disconnected_descriptor
            })
        {
            return;
        }
        self.analog_gamepad.mapping.inter_device.observation = Some(observation);
        self.reload_mapping_inter_device(window, cx);
    }
    fn mapping_inter_rows(&self, slot: &Slot) -> Vec<Value> {
        let locale = i18n::locale().to_ascii_lowercase();
        let observation = self
            .analog_gamepad
            .mapping
            .inter_device
            .observation
            .as_ref()
            .filter(|observation| self.draft["guid"] == observation.profile_guid);
        let mut devices = observation
            .into_iter()
            .flat_map(|observation| observation.devices.iter())
            .filter(|device| eligible(self.spec.product_id, device))
            .map(|device| {
                let mut row = device.clone();
                row["name"] = device["productName"][&locale]
                    .as_str()
                    .filter(|name| !name.is_empty())
                    .or_else(|| device["productName"]["en"].as_str())
                    .map(|name| json!(name))
                    .unwrap_or(json!(""));
                row["isNotConnected"] = json!(false);
                row
            })
            .collect::<Vec<_>>();
        let group = &slot.staged["interDeviceGroup"];
        if group["interPID"].is_number()
            && !devices
                .iter()
                .any(|device| device["productId"] == group["interPID"])
        {
            let descriptor = observation
                .and_then(|observation| observation.disconnected_descriptor.as_ref())
                .filter(|(pid, _)| group["interPID"] == *pid)
                .map(|(_, descriptor)| descriptor);
            let mut row = json!({"productId":group["interPID"],"name":group["name"],"isNotConnected":true,"isUnknownMappedDevice":descriptor.is_none(),"category":"unknown","profiles":[]});
            if descriptor.is_none() {
                row["name"] = json!(message(self, "f_P"));
            } else if let Some(descriptor) = descriptor {
                row["name"] = json!(format!(
                    "{} {}",
                    text(&descriptor["name"][i18n::locale()]),
                    text(&descriptor["edition"][i18n::locale()])
                ));
            }
            row["activeProfile"] = group["guid"].clone();
            row["profiles"] = json!([{"name":group["profileNavigationGroup"]["name"],"guid":group["profileNavigationGroup"]["guid"]}]);
            if group["interDeviceType"] == "sensitivityGroup" {
                row["category"] = json!("MOUSE");
            }
            if group["interDeviceType"] == "keymapGroup" {
                row["category"] = json!("KEYPAD");
            }
            devices.insert(0, row);
        }
        devices
    }
    fn mapping_inter_device(&self, slot: usize) -> Option<Value> {
        let edit = self.analog_gamepad.mapping.edit.as_ref()?;
        let value = &edit.slots[slot];
        self.mapping_inter_rows(value)
            .into_iter()
            .find(|device| device["productId"] == value.staged["interDeviceGroup"]["interPID"])
    }
    fn mapping_inter_sensitivity_choices(&self) -> Vec<&'static Value> {
        let automation = self.spec.config["DeviceInfo"]["automationSupported"] == true;
        let shift = self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .is_some_and(|edit| edit.shift);
        app_specific::data(self.spec.product_id)
            .sensitivity_choices
            .iter()
            .filter(|choice| {
                !(automation
                    && matches!(choice["id"].as_str(), Some("DPI_Clutch" | "DPI_OnTheFly"))
                    || shift && choice["id"] == "DPI_OnTheFly")
            })
            .collect()
    }
    fn mapping_inter_stages<'a>(&self, device: &'a Value) -> Option<&'a Vec<Value>> {
        device["profiles"]
            .as_array()?
            .iter()
            .find(|profile| profile["guid"] == device["activeProfile"])?["dpiStages"]["stages"]
            .as_array()
    }
    fn stage_mapping_inter_function(
        &mut self,
        slot: usize,
        function: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let data = app_specific::data(self.spec.product_id);
        let defaults = match function {
            "sensitivityGroup" => {
                json!({"sensitivityAssignment":self.mapping_inter_sensitivity_choices().first().map(|choice|choice["id"].clone()).unwrap_or(Value::Null),"x":800,"y":800})
            }
            "keymapGroup" => {
                json!({"type":data.keymap_choices.first().map(|choice|choice["id"].clone()).unwrap_or(Value::Null)})
            }
            _ => {
                json!({"profileNavigationAssignment":data.profile_choices.first().map(|choice|choice["id"].clone()).unwrap_or(Value::Null)})
            }
        };
        if let Some(edit) = &mut self.analog_gamepad.mapping.edit {
            let group = &mut edit.slots[slot].staged["interDeviceGroup"];
            for name in ["profileNavigationGroup", "sensitivityGroup", "keymapGroup"] {
                if let Some(group) = group.as_object_mut() {
                    group.remove(name);
                }
            }
            group["interDeviceType"] = json!(function);
            group[function] = defaults;
        }
        self.init_mapping_inter_device(slot, window, cx);
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
    pub(super) fn init_mapping_inter_device(
        &mut self,
        slot: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.analog_gamepad.mapping.inter_device.owner = Some(cx.entity_id());
        if !self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .is_some_and(|edit| edit.slots[slot].category == "INTERDEVICE")
        {
            return;
        }
        let value = self.analog_gamepad.mapping.edit.as_ref().unwrap().slots[slot].clone();
        let devices = self.mapping_inter_rows(&value);
        if value.staged["interDeviceGroup"]["interPID"].is_null() {
            if let Some(device) = devices
                .first()
                .filter(|device| device["isNotConnected"] != true)
            {
                if let Some(edit) = &mut self.analog_gamepad.mapping.edit {
                    edit.slots[slot].staged["interDeviceGroup"] = json!({"interPID":device["productId"],"name":device["name"],"interDeviceType":"profileNavigationGroup","profileNavigationGroup":{"profileNavigationAssignment":"NextProfile"}});
                }
            }
        }
        let value = self.analog_gamepad.mapping.edit.as_ref().unwrap().slots[slot].clone();
        let device = self.mapping_inter_device(slot);
        let function = value.staged["interDeviceGroup"]["interDeviceType"]
            .as_str()
            .unwrap_or("profileNavigationGroup");
        let mut menus = vec![(
            devices
                .iter()
                .map(|device| Choice::new(text(&device["productId"]), text(&device["name"])))
                .collect::<Vec<_>>(),
            text(&value.staged["interDeviceGroup"]["interPID"]),
        )];
        menus.push((
            device
                .as_ref()
                .map(functions)
                .unwrap_or_default()
                .into_iter()
                .map(|(id, key)| Choice::localized(id, key))
                .collect(),
            function.to_owned(),
        ));
        menus.push((
            self.mapping_inter_sensitivity_choices()
                .iter()
                .map(|choice| Choice::localized(text(&choice["id"]), text(&choice["content"])))
                .collect(),
            text(&value.staged["interDeviceGroup"]["sensitivityGroup"]["sensitivityAssignment"]),
        ));
        let profiles = device
            .as_ref()
            .and_then(|device| device["profiles"].as_array())
            .cloned()
            .unwrap_or_default();
        menus.push((
            profiles
                .iter()
                .map(|profile| Choice::new(text(&profile["guid"]), text(&profile["name"])))
                .collect(),
            text(&value.staged["interDeviceGroup"]["profileNavigationGroup"]["guid"]),
        ));
        let chroma = self
            .lighting_observation
            .as_ref()
            .map(|observation| observation.chroma_profiles.clone())
            .unwrap_or_default();
        menus.push((
            chroma
                .iter()
                .map(|profile| Choice::new(text(&profile["id"]), text(&profile["name"])))
                .collect(),
            text(&value.staged["interDeviceGroup"]["profileNavigationGroup"]["lightpacId"]),
        ));
        for (kind, (choices, selected)) in menus.into_iter().enumerate() {
            let selected = choices.iter().position(|choice| choice.id() == selected);
            let selector = cx.new(|cx| {
                SelectState::new(choices, selected.map(IndexPath::new), window, cx)
                    .searchable(false)
            });
            let subscription =
                cx.subscribe_in(&selector, window, move |this, _, event, window, cx| {
                    let SelectEvent::Confirm(Some(id)) = event else {
                        return;
                    };
                    if kind == 0 {
                        let Some(edit) = this.analog_gamepad.mapping.edit.as_ref() else {
                            return;
                        };
                        let device = this
                            .mapping_inter_rows(&edit.slots[slot])
                            .into_iter()
                            .find(|device| text(&device["productId"]) == *id);
                        if let Some(device) = device {
                            if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                edit.slots[slot].staged["interDeviceGroup"] =
                                    json!({"interPID":device["productId"],"name":device["name"]});
                            }
                            this.stage_mapping_inter_function(
                                slot,
                                "profileNavigationGroup",
                                window,
                                cx,
                            );
                        }
                        return;
                    }
                    if kind == 1 {
                        this.stage_mapping_inter_function(slot, id, window, cx);
                        return;
                    }
                    let profiles = this
                        .mapping_inter_device(slot)
                        .and_then(|device| device["profiles"].as_array().cloned())
                        .unwrap_or_default();
                    if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                        let group = &mut edit.slots[slot].staged["interDeviceGroup"];
                        if kind == 2 {
                            group["sensitivityGroup"]["sensitivityAssignment"] = json!(id);
                        }
                        if kind == 3 {
                            if let Some(profile) =
                                profiles.iter().find(|profile| profile["guid"] == *id)
                            {
                                group["profileNavigationGroup"]["guid"] = profile["guid"].clone();
                                group["profileNavigationGroup"]["name"] = profile["name"].clone();
                            }
                        }
                        if kind == 4 {
                            group["profileNavigationGroup"]["lightpacId"] = json!(id);
                        }
                    }
                    this.init_mapping_inter_device(slot, window, cx);
                    cx.emit(KeyboardProductChanged);
                    cx.notify();
                });
            let index = slot * 5 + kind;
            self.analog_gamepad.mapping.inter_device.selectors[index] = Some(selector);
            self.analog_gamepad.mapping.inter_device.subscriptions[index] = Some(subscription);
        }
        self.init_mapping_inter_dpi(slot, window, cx);
    }
    fn init_mapping_inter_dpi(&mut self, slot: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.analog_gamepad.mapping.inter_device.dpi_subscriptions[slot].clear();
        let Some(device) = self.mapping_inter_device(slot) else {
            return;
        };
        let defaults = &app_specific::data(self.spec.product_id).dpi_defaults;
        let min = device["minDPI"].as_f64().unwrap_or(defaults["JhY"]);
        let max = device["maxDPI"].as_f64().unwrap_or(defaults["jDZ"]);
        let step = device["dpiStep"].as_f64().unwrap_or(defaults["RpH"]);
        for axis in 0..2 {
            let key = if axis == 0 { "x" } else { "y" };
            let value=self.analog_gamepad.mapping.edit.as_ref().unwrap().slots[slot].staged["interDeviceGroup"]["sensitivityGroup"][key].as_f64().unwrap_or(800.);
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value((value as u32).to_string())
                    .validate(|value, _| {
                        value.len() <= 5 && (value.is_empty() || value.parse::<i32>().is_ok())
                    })
            });
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(min as f32)
                    .max(max as f32)
                    .step(step as f32)
                    .default_value(value as f32)
            });
            let input_sub = cx.subscribe_in(
                &input,
                window,
                move |this, input, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        window.blur(cx);
                    }
                    if matches!(event, InputEvent::Blur) {
                        let raw = input.read(cx).value();
                        let raw = if raw.is_empty() {
                            this.analog_gamepad
                                .mapping
                                .edit
                                .as_ref()
                                .and_then(|edit| {
                                    edit.slots[slot].staged["interDeviceGroup"]["sensitivityGroup"]
                                        [key]
                                        .as_f64()
                                })
                                .unwrap_or(value)
                        } else {
                            raw.parse::<f64>().unwrap_or(1800.)
                        };
                        let snapped = if step > 0. && raw % step != 0. {
                            raw - raw % step + step
                        } else {
                            raw
                        };
                        this.stage_mapping_inter_dpi(
                            slot,
                            axis,
                            snapped.clamp(min, max),
                            window,
                            cx,
                        );
                    }
                },
            );
            let slider_sub = cx.subscribe_in(&slider, window, move |this, _, event, window, cx| {
                if let SliderEvent::Change(value) = event {
                    this.stage_mapping_inter_dpi(slot, axis, value.start() as f64, window, cx);
                }
            });
            self.analog_gamepad.mapping.inter_device.dpi_inputs[slot][axis] = Some(input);
            self.analog_gamepad.mapping.inter_device.dpi_sliders[slot][axis] = Some(slider);
            self.analog_gamepad.mapping.inter_device.dpi_subscriptions[slot]
                .extend([input_sub, slider_sub]);
        }
    }
    fn stage_mapping_inter_dpi(
        &mut self,
        slot: usize,
        axis: usize,
        value: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.analog_gamepad.mapping.inter_device.syncing {
            return;
        }
        if let Some(edit) = &mut self.analog_gamepad.mapping.edit {
            let group = &mut edit.slots[slot].staged["interDeviceGroup"]["sensitivityGroup"];
            group[if axis == 0 { "x" } else { "y" }] = json!(value as u32);
            if axis == 0 && group["independent"] != true {
                group["y"] = json!(value as u32);
            }
        }
        self.analog_gamepad.mapping.inter_device.syncing = true;
        for index in 0..2 {
            let key = if index == 0 { "x" } else { "y" };
            let value=self.analog_gamepad.mapping.edit.as_ref().unwrap().slots[slot].staged["interDeviceGroup"]["sensitivityGroup"][key].as_f64().unwrap_or(800.);
            if let Some(input) =
                self.analog_gamepad.mapping.inter_device.dpi_inputs[slot][index].clone()
            {
                input.update(cx, |input, cx| {
                    input.set_value((value as u32).to_string(), window, cx)
                });
            }
            if let Some(slider) =
                self.analog_gamepad.mapping.inter_device.dpi_sliders[slot][index].clone()
            {
                slider.update(cx, |slider, cx| slider.set_value(value as f32, window, cx));
            }
        }
        self.analog_gamepad.mapping.inter_device.syncing = false;
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
    fn inter_selector(&self, slot: usize, kind: usize) -> AnyElement {
        self.analog_gamepad.mapping.inter_device.selectors[slot * 5 + kind]
            .as_ref()
            .map(|selector| {
                gpui_kit::component::select::Select::new(selector)
                    .w_full()
                    .disabled(
                        kind != 0
                            && self.mapping_inter_device(slot).is_some_and(|device| {
                                device["isNotConnected"] == true
                                    || kind == 2
                                        && self
                                            .mapping_inter_stages(&device)
                                            .is_none_or(Vec::is_empty)
                                    || kind >= 3
                                        && device["profiles"].as_array().is_none_or(Vec::is_empty)
                            }),
                    )
                    .into_any_element()
            })
            .unwrap_or_else(|| div().into_any_element())
    }
    fn inter_dpi_input(&self, slot: usize, axis: usize, cx: &Context<Self>) -> AnyElement {
        let disabled = self.mapping_inter_device(slot).is_none_or(|device| {
            device["isNotConnected"] == true
                || self.mapping_inter_stages(&device).is_none_or(Vec::is_empty)
        });
        self.analog_gamepad.mapping.inter_device.dpi_inputs[slot][axis]
            .as_ref()
            .map(|input| {
                div()
                    .child(razer_widgets::keyboard_navigation::focus_input(
                        Input::new(input)
                            .w(surface::css(60.))
                            .h(surface::css(26.))
                            .text_size(surface::css(14.))
                            .line_height(surface::css(19.))
                            .text_center()
                            .bg(cx.theme().group_box)
                            .border_1()
                            .border_color(cx.theme().border)
                            .disabled(disabled),
                        input,
                        cx,
                    ))
                    .on_key_down(|event, window, cx| {
                        if event.keystroke.key == "escape" {
                            window.blur(cx);
                        }
                        if event.keystroke.key == "tab" {
                            cx.stop_propagation();
                            window.prevent_default();
                        }
                    })
                    .into_any_element()
            })
            .unwrap_or_else(|| div().into_any_element())
    }
    pub(super) fn mapping_inter_device_element(
        &self,
        slot: usize,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(edit) = &self.analog_gamepad.mapping.edit else {
            return div().into_any_element();
        };
        let rows = self.mapping_inter_rows(&edit.slots[slot]);
        let mut panel = v_flex()
            .mx(surface::css(20.))
            .gap(surface::css(10.))
            .text_size(surface::css(14.))
            .line_height(surface::css(17.));
        if rows.is_empty() {
            return panel.child(message(self, "shE")).into_any_element();
        }
        panel = panel.child(self.inter_selector(slot, 0));
        let Some(device) = self.mapping_inter_device(slot) else {
            return panel.into_any_element();
        };
        if device["subDevices"]
            .as_array()
            .is_some_and(|devices| devices.len() > 1)
        {
            panel = panel.child(
                v_flex()
                    .w(surface::css(210.))
                    .my(surface::css(15.))
                    .p(surface::css(10.))
                    .items_center()
                    .border_1()
                    .border_color(Colors::warning_border())
                    .rounded(surface::css(3.))
                    .child(img("synapse/automation-icon_warning.svg").size(surface::css(20.)))
                    .child(div().text_center().child(message(self, "tCz"))),
            );
        }
        if device["isNotConnected"] == true {
            panel = panel.child(
                div()
                    .text_color(Colors::disconnected_text())
                    .child(message(self, "f_P")),
            );
        }
        if device["isUnknownMappedDevice"] == true {
            return panel.into_any_element();
        }
        let group = &edit.slots[slot].staged["interDeviceGroup"];
        let disconnected = device["isNotConnected"] == true;
        let profiles_disabled =
            disconnected || device["profiles"].as_array().is_none_or(Vec::is_empty);
        let sensitivity_disabled =
            disconnected || self.mapping_inter_stages(&device).is_none_or(Vec::is_empty);
        let mut body = v_flex()
            .gap(surface::css(10.))
            .child(self.inter_selector(slot, 1));
        match group["interDeviceType"].as_str() {
            Some("sensitivityGroup") => {
                body = body.child(self.inter_selector(slot, 2));
                let index = self
                    .mapping_inter_sensitivity_choices()
                    .iter()
                    .position(|choice| {
                        choice["id"] == group["sensitivityGroup"]["sensitivityAssignment"]
                    })
                    .unwrap_or_default();
                if index == 0 {
                    let independent = group["sensitivityGroup"]["independent"] == true;
                    body = body.child(
                        h_flex()
                            .gap(surface::css(10.))
                            .child("DPI")
                            .child(self.inter_dpi_input(slot, 0, cx))
                            .when(independent, |row| {
                                row.child(self.inter_dpi_input(slot, 1, cx))
                            }),
                    );
                    if device["supportXYDPI"] == true {
                        body=body.child(Checkbox::new(SharedString::from(format!("mapping-inter-xy-{slot}"))).label(message(self,"mQ7")).checked(independent).disabled(sensitivity_disabled).on_click(cx.listener(move|this,checked:&bool,_,cx|{if let Some(edit)=&mut this.analog_gamepad.mapping.edit{edit.slots[slot].staged["interDeviceGroup"]["sensitivityGroup"]["independent"]=json!(checked);}cx.emit(KeyboardProductChanged);cx.notify();})));
                    }
                    body = body
                        .children(
                            self.analog_gamepad.mapping.inter_device.dpi_sliders[slot][0]
                                .as_ref()
                                .map(|slider| Slider::new(slider).disabled(sensitivity_disabled)),
                        )
                        .when(independent, |body| {
                            body.children(
                                self.analog_gamepad.mapping.inter_device.dpi_sliders[slot][1]
                                    .as_ref()
                                    .map(|slider| {
                                        Slider::new(slider).disabled(sensitivity_disabled)
                                    }),
                            )
                        });
                } else if index == 3 && !edit.shift {
                    body = body.child(message(self, "qMP"));
                } else {
                    body=body.children(self.mapping_inter_stages(&device).into_iter().flatten().filter(|stage|stage["visible"] == true).enumerate().map(|(index,stage)| {
                        let dpi=if stage["x"] == stage["y"]{text(&stage["x"])}else{format!("X:{} Y:{}",text(&stage["x"]),text(&stage["y"]))};div().child(format!("{} {} - {dpi}",message(self,"Ldn"),index+1))
                    })).child(BaseButton::new(SharedString::from(format!("mapping-inter-configure-{slot}"))).child(div().underline().child(message(self,"ZEP"))).disabled(sensitivity_disabled).on_click(cx.listener(move|this,_,_,cx|{
                        let Some(device)=this.mapping_inter_device(slot)else{return;};let(Some(ui),Some(mw))=(device["UIWindowName"].as_str().filter(|name| !name.is_empty()),device["MWWindowName"].as_str().filter(|name| !name.is_empty()))else{return;};
                        cx.emit(KeyboardMappingInterDeviceConfigureRequested{source_product_id:this.spec.product_id,profile_guid:text(&this.draft["guid"]),owner:cx.entity_id(),target_product_id:device["productId"].as_u64().unwrap_or_default() as u32,ui_window_name:ui.to_owned(),mw_window_name:mw.to_owned(),source_message:json!({"type":app_specific::data(this.spec.product_id).configure_action,"payload":"TAB_PERFORMANCE"})});
                    })));
                }
            }
            Some("keymapGroup") => {
                body =
                    body.children(
                        app_specific::data(self.spec.product_id)
                            .keymap_choices
                            .iter()
                            .map(|choice| {
                                let id = text(&choice["id"]);
                                Radio::new(SharedString::from(format!(
                                    "mapping-inter-keymap-{slot}-{id}"
                                )))
                                .label(t(&text(&choice["content"])))
                                .checked(group["keymapGroup"]["type"] == id)
                                .disabled(profiles_disabled)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                        edit.slots[slot].staged["interDeviceGroup"]["keymapGroup"]
                                            ["type"] = json!(id);
                                    }
                                    cx.emit(KeyboardProductChanged);
                                    cx.notify();
                                }))
                            }),
                    );
            }
            _ => {
                body =
                    body.children(
                        app_specific::data(self.spec.product_id)
                            .profile_choices
                            .iter()
                            .map(|choice| {
                                let id = text(&choice["id"]);
                                Radio::new(SharedString::from(format!(
                                    "mapping-inter-profile-{slot}-{id}"
                                )))
                                .label(t(&text(&choice["content"])))
                                .checked(
                                    group["profileNavigationGroup"]["profileNavigationAssignment"]
                                        == id,
                                )
                                .disabled(profiles_disabled)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    let profile =
                                        this.mapping_inter_device(slot).and_then(|device| {
                                            device["profiles"]
                                                .as_array()
                                                .and_then(|profiles| profiles.first())
                                                .cloned()
                                        });
                                    if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                        let value = &mut edit.slots[slot].staged["interDeviceGroup"]
                                            ["profileNavigationGroup"];
                                        *value = json!({"profileNavigationAssignment":id});
                                        if id == "Specific" {
                                            if let Some(profile) = profile {
                                                value["guid"] = profile["guid"].clone();
                                                value["name"] = profile["name"].clone();
                                            }
                                        }
                                    }
                                    this.init_mapping_inter_device(slot, window, cx);
                                    cx.emit(KeyboardProductChanged);
                                    cx.notify();
                                }))
                            }),
                    );
                if group["profileNavigationGroup"]["profileNavigationAssignment"] == "Specific" {
                    body = body.child(self.inter_selector(slot, 3));
                    let enabled = !group["profileNavigationGroup"]["lightpacId"].is_null();
                    let chroma = self
                        .lighting_observation
                        .as_ref()
                        .map(|observation| observation.chroma_profiles.as_slice())
                        .unwrap_or_default();
                    let lighting_disabled =
                        !enabled && (self.source_is_basic_lighting_device() || chroma.is_empty());
                    body = body.child(
                        Checkbox::new(SharedString::from(format!("mapping-inter-lighting-{slot}")))
                            .label(message(self, "aeo"))
                            .checked(enabled)
                            .disabled(profiles_disabled || lighting_disabled)
                            .on_click(
                                cx.listener(move |this, checked: &bool, window, cx| {
                                    let first = this
                                        .lighting_observation
                                        .as_ref()
                                        .and_then(|observation| observation.chroma_profiles.first())
                                        .map(|profile| profile["id"].clone());
                                    if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                        let group = &mut edit.slots[slot].staged["interDeviceGroup"]
                                            ["profileNavigationGroup"];
                                        if *checked {
                                            if let Some(first) = first {
                                                group["lightpacId"] = first;
                                            }
                                        } else if let Some(group) = group.as_object_mut() {
                                            group.remove("lightpacId");
                                        }
                                    }
                                    this.init_mapping_inter_device(slot, window, cx);
                                    cx.emit(KeyboardProductChanged);
                                    cx.notify();
                                }),
                            ),
                    );
                    if enabled && !lighting_disabled {
                        body = body.child(self.inter_selector(slot, 4));
                    }
                }
            }
        }
        panel
            .child(body.when(device["isNotConnected"] == true, |body| body.opacity(0.3)))
            .into_any_element()
    }
}
