//! Current PID 207 Customize: independent Redux observations and UI intent.
//! Evidence: docs/re/hyperflux-customize-current.json. No native result is inferred.
use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "hyperflux_207_artwork.rs"]
mod artwork;
#[path = "hyperflux_207_pairing.rs"]
mod pairing;
#[path = "hyperflux_207_ui.rs"]
mod ui;

pub(super) struct State {
    auto_reducer: Value,
    battery_reducer: Value,
    paired_info: Vec<Value>,
    valid_devices: Vec<Value>,
    is_dongle: bool,
    master_blocks_pairing: bool,
    feature: bool,
    edition: u32,
    serial: String,
    mouse_window: Option<String>,
    unpair_open: bool,
    slider_dragging: bool,
    slider: Option<Entity<SliderState>>,
    select: Option<Entity<SelectState<Vec<Choice>>>>,
    current_timer_tick: Option<u64>,
    generation: u64,
    mounted: bool,
    pairing: pairing::Pairing,
    connected_device_info: Vec<Value>,
    pairing_queries: Option<Task<()>>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            auto_reducer: serde_json::json!({"autoPairing":{"isEnabled":false,"value":0},"actionsFromUI":[],"actionsFromLocalStorage":[],"errorActions":[]}),
            battery_reducer: serde_json::json!({"batteryOptimizer":{"isEnabled":true,"value":50,"batteryChargingOverrideEnabled":false},"powerStatus":"plugged","actionsFromUI":[],"actionsFromLocalStorage":[],"errorActions":[]}),
            paired_info: vec![],
            valid_devices: vec![],
            is_dongle: false,
            master_blocks_pairing: false,
            feature: false,
            edition: 0,
            serial: String::new(),
            mouse_window: None,
            unpair_open: false,
            slider_dragging: false,
            slider: None,
            select: None,
            current_timer_tick: None,
            generation: 0,
            mounted: false,
            pairing: pairing::Pairing::default(),
            connected_device_info: vec![],
            pairing_queries: None,
        }
    }
}
impl State {
    fn mouse(&self) -> Option<&Value> {
        self.paired_info.iter().find(|p| p["category"] == "MOUSE")
    }
    fn mouse_on(&self) -> bool {
        self.mouse().is_some_and(|p| p["status"] == 1)
    }
    fn auto_enabled(&self) -> bool {
        self.auto_reducer["autoPairing"]["isEnabled"] == true
    }
    fn enabled(&self) -> bool {
        truthy(&self.battery_reducer["batteryOptimizer"]["isEnabled"])
    }
    fn charging_override(&self) -> bool {
        truthy(&self.battery_reducer["batteryOptimizer"]["batteryChargingOverrideEnabled"])
    }
    fn limit(&self) -> f32 {
        self.battery_reducer["batteryOptimizer"]["value"]
            .as_f64()
            .unwrap_or(50.) as f32
    }
    fn unmount(&mut self) {
        self.unpair_open = false;
        self.slider_dragging = false;
        self.pairing = pairing::Pairing::default();
        self.pairing_queries = None;
        self.generation = self.generation.wrapping_add(1);
    }
}

#[derive(Clone, Debug)]
pub enum HyperfluxOperation {
    Observe,
    ReadConnectedDevices,
    SourceMessage(Value),
    OpenPairedMouse {
        mw_window_name: String,
    },
    OpenPairing {
        new_utility: bool,
        serial_number: String,
    },
}
#[derive(Clone)]
pub struct HyperfluxRequested {
    owner: EntityId,
    generation: u64,
    operation: HyperfluxOperation,
}
impl HyperfluxRequested {
    pub fn owner(&self) -> EntityId {
        self.owner
    }
    pub fn operation(&self) -> &HyperfluxOperation {
        &self.operation
    }
}
impl EventEmitter<HyperfluxRequested> for SourceControls {}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => v.as_f64() != Some(0.),
        Value::String(v) => !v.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}
fn queue(reducer: &mut Value, tick: u64, kind: &str, mut payload: Value) {
    if let Some(object) = payload.as_object_mut() {
        for field in ["actionsFromUI", "actionsFromLocalStorage", "errorActions"] {
            object.remove(field);
        }
    }
    if let Some(actions) = reducer["actionsFromUI"].as_array_mut() {
        actions.push(serde_json::json!({"timertick":tick,"actionType":kind,"actionPayload":payload,"localStorage":{},"device":{}}));
    }
}

impl SourceControls {
    pub(super) fn hyperflux_unmount(&mut self) {
        if self.spec.product_id == 207 {
            self.hyperflux.unmount();
            self.hyperflux.mounted = false;
        }
    }
    pub(super) fn mount_hyperflux(&mut self, cx: &mut Context<Self>) {
        if self.spec.product_id == 207 && self.page == "TAB_CUSTOMIZE" && !self.hyperflux.mounted {
            self.hyperflux.mounted = true;
            self.hyperflux_request(HyperfluxOperation::Observe, cx);
            self.mount_legacy_pairing_queries(cx);
            if self.hyperflux.mouse().is_some() {
                self.hyperflux_request(HyperfluxOperation::ReadConnectedDevices, cx);
            }
        }
    }
    pub(super) fn restore_hyperflux(
        &mut self,
        value: Option<&Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 207 {
            return;
        }
        self.hyperflux_unmount();
        // Persisted edits are explicitly local UI drafts, not observations.
        if let Some(value) = value {
            if let Some(v) = value
                .pointer("/runtime/autoPairing")
                .filter(|v| v.is_object())
            {
                self.hyperflux.auto_reducer["autoPairing"] = v.clone();
            }
            if let Some(v) = value
                .pointer("/runtime/batteryOptimizer")
                .filter(|v| v.is_object())
            {
                self.hyperflux.battery_reducer["batteryOptimizer"] = v.clone();
            }
        }
        self.sync_hyperflux(window, cx);
    }
    pub(super) fn init_hyperflux(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.spec.product_id != 207 {
            return;
        }
        let slider = cx.new(|_| {
            SliderState::new()
                .min(50.)
                .max(80.)
                .step(5.)
                .default_value(50.)
        });
        self.subscriptions.push(
            cx.subscribe_in(&slider, window, |this, _, event, window, cx| {
                if this.syncing {
                    return;
                }
                let value = match event {
                    SliderEvent::Change(v) | SliderEvent::Release(v) => v.start(),
                };
                if matches!(event, SliderEvent::Release(_)) || !this.hyperflux.slider_dragging {
                    this.hyperflux.slider_dragging = false;
                    this.set_hyperflux_battery(true, value, window, cx);
                } else {
                    cx.notify();
                }
            }),
        );
        let choices = Self::hyperflux_choices();
        let select = cx.new(|cx| SelectState::new(choices, None, window, cx));
        self.subscriptions.push(
            cx.subscribe_in(&select, window, |this, _, event, window, cx| {
                if this.syncing {
                    return;
                }
                if let SelectEvent::Confirm(Some(value)) = event {
                    if let Ok(value) = value.parse::<f32>() {
                        this.set_hyperflux_battery(this.hyperflux.enabled(), value, window, cx);
                    }
                }
            }),
        );
        self.hyperflux.slider = Some(slider);
        self.hyperflux.select = Some(select);
        self.sync_hyperflux(window, cx);
    }
    fn hyperflux_choices() -> Vec<Choice> {
        (50..=80)
            .step_by(5)
            .map(|v| Choice::new(v.to_string(), v.to_string()))
            .collect()
    }
    fn hyperflux_request(&self, operation: HyperfluxOperation, cx: &mut Context<Self>) {
        cx.emit(HyperfluxRequested {
            owner: cx.entity_id(),
            generation: self.hyperflux.generation,
            operation,
        });
    }
    pub fn accepts_hyperflux(&self, request: &HyperfluxRequested, owner: EntityId) -> bool {
        self.spec.product_id == 207
            && request.owner == owner
            && request.generation == self.hyperflux.generation
    }
    pub(super) fn sync_hyperflux(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.spec.product_id != 207 {
            return;
        }
        self.syncing = true;
        let value = self.hyperflux.limit();
        if let Some(slider) = &self.hyperflux.slider {
            slider.update(cx, |s, cx| s.set_value(value, window, cx));
        }
        if let Some(select) = &self.hyperflux.select {
            let value = format!("{value:.0}");
            select.update(cx, |s, cx| s.set_selected_value(&value, window, cx));
        }
        self.syncing = false;
    }
    fn local_hyperflux_draft(&mut self, cx: &mut Context<Self>) {
        self.draft["runtime"]["autoPairing"] = self.hyperflux.auto_reducer["autoPairing"].clone();
        self.draft["runtime"]["batteryOptimizer"] =
            self.hyperflux.battery_reducer["batteryOptimizer"].clone();
        // This event is local profile persistence only, never device completion.
        cx.emit(SourceControlsChanged);
        cx.notify();
    }
    fn source_tick(&self) -> u64 {
        self.hyperflux
            .current_timer_tick
            .filter(|v| *v != 0)
            .unwrap_or_else(|| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64
            })
    }
    fn set_hyperflux_auto(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let tick = self.source_tick();
        let payload = serde_json::json!({"autoPairing":{"isEnabled":enabled}});
        self.hyperflux.auto_reducer["autoPairing"] = payload["autoPairing"].clone();
        self.hyperflux_request(
            HyperfluxOperation::SourceMessage(
                serde_json::json!({"type":"ON_SET_AUTO_PAIRING","payload":payload}),
            ),
            cx,
        );
        // The source's undefined timerTick property is absent in transport JSON.
        queue(
            &mut self.hyperflux.auto_reducer,
            tick,
            "ON_SET_AUTO_PAIRING",
            payload,
        );
        self.local_hyperflux_draft(cx);
    }
    fn set_hyperflux_battery(
        &mut self,
        enabled: bool,
        value: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = (value / 5.).round().mul_add(5., 0.).clamp(50., 80.);
        let tick = self.source_tick();
        let charging_override = self.hyperflux.charging_override();
        self.hyperflux.battery_reducer["batteryOptimizer"] = serde_json::json!({"isEnabled":enabled,"value":value as i32,"batteryChargingOverrideEnabled":charging_override});
        let payload = self.hyperflux.battery_reducer.clone();
        // Current reducer spells this field timertick and sends the full reducer.
        self.hyperflux_request(HyperfluxOperation::SourceMessage(serde_json::json!({"type":"ON_SET_BATTERY_OPTIMIZER","timertick":tick,"payload":payload})),cx);
        queue(
            &mut self.hyperflux.battery_reducer,
            tick,
            "ON_SET_BATTERY_OPTIMIZER",
            payload,
        );
        self.sync_hyperflux(window, cx);
        self.local_hyperflux_draft(cx);
    }
    fn confirm_hyperflux_unpair(&mut self, cx: &mut Context<Self>) {
        if let Some(mouse) = self.hyperflux.mouse() {
            let id = mouse
                .get("dongleId")
                .filter(|v| truthy(v))
                .or_else(|| mouse.get("productId"))
                .cloned();
            if let Some(id) = id {
                let tick = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let payload = serde_json::json!({"productId":id,"category":"MOUSE"});
                self.hyperflux_request(HyperfluxOperation::SourceMessage(serde_json::json!({"type":"DUALLINK_UNBIND_DEVICE","payload":payload,"timerTick":tick})),cx);
                self.hyperflux.auto_reducer["productId"] = payload.clone();
                queue(
                    &mut self.hyperflux.auto_reducer,
                    tick,
                    "DUALLINK_UNBIND_DEVICE",
                    serde_json::json!({"productId":payload}),
                );
            }
        }
        self.hyperflux.unpair_open = false;
        cx.notify();
    }
    /// Exact source reducer observations, including unresolved/failed actions.
    /// Owner validation is done by the outer product workspace before this call.
    pub fn observe_hyperflux(
        &mut self,
        source: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.spec.product_id != 207 || !source.is_object() {
            return false;
        }
        if let Some(reducer) = source.get("autoPairingReducer").filter(|r| r.is_object()) {
            self.hyperflux.auto_reducer = reducer.clone();
        }
        if let Some(reducer) = source.get("systemBatteryReducer").filter(|r| r.is_object()) {
            self.hyperflux.battery_reducer = reducer.clone();
        }
        if let Some(v) = source.get("global.multipairing") {
            let feature = truthy(&v["provisionFlags"]);
            if feature != self.hyperflux.feature {
                self.hyperflux.feature = feature;
                self.hyperflux.pairing = pairing::Pairing::default();
                self.hyperflux.pairing_queries = None;
                if self.hyperflux.mounted && !feature {
                    self.mount_legacy_pairing_queries(cx);
                }
            }
        }
        if let Some(reducer) = source.get("deviceReducer") {
            if let Some(paired) = reducer["pairedInfo"].as_array() {
                self.hyperflux.paired_info = paired.clone();
                if self.hyperflux.mounted && self.hyperflux.mouse().is_some() {
                    self.hyperflux_request(HyperfluxOperation::ReadConnectedDevices, cx);
                }
            }
            if let Some(v) = reducer.get("isDongle") {
                self.hyperflux.is_dongle = truthy(v);
            }
            if let Some(v) = reducer["editionId"].as_u64() {
                self.hyperflux.edition = v as u32;
            }
            if let Some(v) = reducer["serialNumber"].as_str() {
                self.hyperflux.serial = v.into();
            }
        }
        if let Some(v) = source
            .pointer("/commonReducer/validDevices")
            .and_then(Value::as_array)
        {
            self.hyperflux.valid_devices = v.clone();
            if self.hyperflux.mounted && !self.hyperflux.feature {
                self.pairing_refresh_bind_info(cx);
            }
        }
        if let Some(v) = source["currentTimerTick"].as_u64() {
            self.hyperflux.current_timer_tick = Some(v);
        }
        if let Some(devices) = source["connectedDeviceInfo"].as_array() {
            self.hyperflux.connected_device_info = devices.clone();
        }
        self.sync_hyperflux(window, cx);
        cx.notify();
        true
    }
    /// Initial getWindowStorageItem("connectedDevices") matching entry.
    /// Only this path changes MWWindowName; subsequent storage events do not.
    pub fn observe_hyperflux_connected_device(
        &mut self,
        device: &Value,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.spec.product_id != 207 || !self.hyperflux.mounted {
            return false;
        }
        let Some(mouse) = self.hyperflux.mouse() else {
            return false;
        };
        if device["productId"] != mouse["productId"] {
            return false;
        }
        if let Some(name) = device["MWWindowName"].as_str() {
            self.hyperflux.mouse_window = Some(name.into());
        } else {
            self.hyperflux.mouse_window = None;
        }
        self.observe_hyperflux_connected_device_update(device, cx)
    }
    /// Original windowstorageevent newValue is a single parsed device object.
    pub fn observe_hyperflux_connected_device_update(
        &mut self,
        device: &Value,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.spec.product_id != 207 || !self.hyperflux.mounted {
            return false;
        }
        let Some(mouse) = self.hyperflux.mouse() else {
            return false;
        };
        if device["productId"] != mouse["productId"] {
            return false;
        }
        let pid = mouse["productId"].clone();
        let status = if device.pointer("/powerStatus/chargingStatus") == Some(&Value::from("off")) {
            0
        } else {
            1
        };
        for mouse in &mut self.hyperflux.paired_info {
            if mouse["productId"] == pid {
                mouse["status"] = Value::from(status);
            }
        }
        cx.notify();
        true
    }
}
