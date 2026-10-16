//! PID 70's current calibration reducer and requests. Requests are not acknowledgements.
use super::*;

// Current PID 70 main.8f24b6a1.js calibration reducer, case dpX.
// iR.goBack keeps the backdrop open; iR.slideDown closes it before changing
// the panel. The ERROR branch deliberately precedes the aborted branch.
fn calibration_panel_command_70(
    previous: &str,
    next: &str,
    ready: &str,
    backdrop_open: bool,
) -> Option<&'static str> {
    if (next == "manualCalibrationMain"
        && !matches!(
            previous,
            "error" | "manualCalibrationReady" | "manualCalibrationCalibrating"
        ))
        || (ready == "ERROR" && backdrop_open)
    {
        Some("ON_CALIBRATION_START")
    } else if matches!(
        previous,
        "manualCalibrationMain" | "error" | "manualCalibrationReady"
    ) && next == "mats"
        && !backdrop_open
    {
        Some("ON_CALIBRATION_ABORTED")
    } else {
        None
    }
}

#[derive(Clone)]
pub struct MouseCalibrationRequested {
    owner: EntityId,
    generation: u64,
    request_id: u64,
    product_id: u32,
    command: &'static str,
    payload: Value,
}
impl MouseCalibrationRequested {
    pub fn request_id(&self) -> u64 {
        self.request_id
    }
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn command(&self) -> &str {
        self.command
    }
    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

#[derive(Clone)]
pub struct MouseCalibrationWelcomeChanged(bool);
impl MouseCalibrationWelcomeChanged {
    pub fn show(&self) -> bool {
        self.0
    }
}
impl EventEmitter<MouseCalibrationRequested> for MouseProductWorkspace {}
impl EventEmitter<MouseCalibrationWelcomeChanged> for MouseProductWorkspace {}

pub(crate) struct State {
    pub(in crate::features::mouse_products) welcome: bool,
    pub(in crate::features::mouse_products) open: bool,
    pub(in crate::features::mouse_products) panel: String,
    pub(in crate::features::mouse_products) mode: &'static str,
    pub(in crate::features::mouse_products) can_go_back: bool,
    pub(in crate::features::mouse_products) feature_access: String,
    pub(in crate::features::mouse_products) ready: String,
    pub(in crate::features::mouse_products) sensor_type: String,
    pub(in crate::features::mouse_products) mini_calibrated: bool,
    pub(in crate::features::mouse_products) show_add: bool,
    pub(in crate::features::mouse_products) too_fast: bool,
    pub(in crate::features::mouse_products) left_hand: bool,
    pub(in crate::features::mouse_products) recalibrated_mat: String,
    pub(in crate::features::mouse_products) selected_mat: Option<String>,
    pub(in crate::features::mouse_products) calibration_type: &'static str,
    pub(in crate::features::mouse_products) focus: FocusHandle,
    previous_focus: Option<FocusHandle>,
    generation: u64,
    next_request: u64,
    pending: BTreeMap<u64, &'static str>,
    last_error: Option<String>,
    pub(in crate::features::mouse_products) calibration_smart: Value,
    pub(in crate::features::mouse_products) preset_tracking_ui: Option<Value>,
    pub(in crate::features::mouse_products) babel_lift_using_default: bool,
    pub(in crate::features::mouse_products) babel_surface_hover: Option<usize>,
    too_fast_observed: bool,
    babel_lift_props: Option<(Value, Value)>,
    too_fast_task: Option<Task<()>>,
}
impl State {
    pub(crate) fn new(cx: &mut Context<MouseProductWorkspace>) -> Self {
        Self {
            welcome: true,
            open: false,
            panel: "mats".into(),
            mode: "add",
            can_go_back: false,
            feature_access: "ALL".into(),
            ready: "NOT_READY".into(),
            sensor_type: String::new(),
            mini_calibrated: false,
            show_add: true,
            too_fast: false,
            left_hand: false,
            recalibrated_mat: String::new(),
            selected_mat: None,
            calibration_type: "custom",
            focus: cx.focus_handle(),
            previous_focus: None,
            generation: 0,
            next_request: 0,
            pending: BTreeMap::new(),
            last_error: None,
            calibration_smart: json!({"calibrationTrackingMode":false,
                "smartTracking":{"isAsymmetric":false,"trackingDistance":2,"liftOffDistance":3,"landingDistance":1},
                "presetTracking":{"isAsymmetric":false,"surfaceDistance":5,"liftOffDistance":5,"landingDistance":3}}),
            preset_tracking_ui: None,
            babel_lift_using_default: false,
            babel_surface_hover: None,
            too_fast_observed: false,
            babel_lift_props: None,
            too_fast_task: None,
        }
    }
}

impl MouseProductWorkspace {
    pub fn restore_calibration_welcome(&mut self, show: bool, cx: &mut Context<Self>) {
        if self.spec.product_id == 185 {
            return;
        }
        if self.calibration_state.welcome != show {
            self.calibration_state.welcome = show;
            cx.notify();
        }
    }
    pub(in crate::features::mouse_products) fn close_calibration_welcome(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.calibration_state.welcome = false;
        // 185's welcome is component state only; all seven other preset roots
        // retain the original showCalibrationMessage storage key.
        if self.spec.product_id != 185 {
            cx.emit(MouseCalibrationWelcomeChanged(false));
        }
        cx.notify();
    }
    pub fn calibration_request_error(&self) -> Option<&str> {
        self.calibration_state.last_error.as_deref()
    }
    pub(crate) fn calibration_lift_enabled(&self) -> bool {
        if self.calibration_state.feature_access == "NONE" {
            return false;
        }
        let profiles = self.calibration_profiles();
        if matches!(self.spec.product_id, 70 | 162) {
            return profiles.len() > 1 && !self.calibration_state.babel_lift_using_default;
        }
        let selected = self.draft.pointer("/calibration/selectedProfile/guid");
        profiles.len() > 1
            && profiles
                .iter()
                .find(|profile| profile.get("guid") == selected)
                .is_some_and(|profile| profile["type"] != "default")
    }
    pub(crate) fn reset_calibration_session(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_smart_calibration_drag(window, cx);
        self.dismiss_calibration(window, cx);
        self.calibration_state.generation = self.calibration_state.generation.wrapping_add(1);
        self.calibration_state.pending.clear();
        self.calibration_state.last_error = None;
        self.calibration_state.ready = "NOT_READY".into();
        self.calibration_state.too_fast_task = None;
        self.calibration_state.too_fast = false;
        self.calibration_state.preset_tracking_ui = None;
        self.calibration_state.babel_lift_props = None;
        self.calibration_state.babel_lift_using_default = false;
        self.calibration_state.babel_surface_hover = None;
        self.calibration_state.too_fast_observed = false;
    }
    pub(in crate::features::mouse_products) fn sync_babel_calibration_lift(&mut self) {
        self.sync_babel_calibration_lift_inner(false);
    }
    fn sync_babel_calibration_lift_inner(&mut self, props_changed: bool) {
        if !matches!(self.spec.product_id, 70 | 162) {
            return;
        }
        let default_field = if self.spec.product_id == 70 {
            "type"
        } else {
            "surfaceId"
        };
        let profiles = self.draft["calibration"]["profiles"].clone();
        let selected = self.draft["calibration"]["selectedProfile"].clone();
        if self.page != "TAB_CALIBRATION" || profiles.as_array().is_none_or(|rows| rows.len() <= 1)
        {
            self.calibration_state.babel_lift_props = None;
            self.calibration_state.babel_lift_using_default = false;
            return;
        }
        let state = &mut self.calibration_state;
        if let Some((previous_profiles, previous_selected)) = &state.babel_lift_props {
            if props_changed || previous_profiles != &profiles || previous_selected != &selected {
                state.babel_lift_using_default = selected.is_null()
                    || profiles.as_array().is_some_and(|rows| {
                        rows.iter().any(|profile| {
                            profile[default_field] == "default"
                                && profile.get("guid") == selected.get("guid")
                        })
                    });
            }
        } else {
            // Current 70 LS / 162 rs constructors start false; didMount only changes range to 3.
            state.babel_lift_using_default = false;
        }
        state.babel_lift_props = Some((profiles, selected));
    }
    fn emit_calibration(&mut self, command: &'static str, payload: Value, cx: &mut Context<Self>) {
        let state = &mut self.calibration_state;
        state.next_request = state.next_request.wrapping_add(1);
        state.pending.insert(state.next_request, command);
        state.last_error = None;
        cx.emit(MouseCalibrationRequested {
            owner: cx.entity_id(),
            generation: state.generation,
            request_id: state.next_request,
            product_id: self.spec.product_id,
            command,
            payload,
        });
    }
    fn accept_calibration_response(
        &mut self,
        request: &MouseCalibrationRequested,
        cx: &Context<Self>,
    ) -> bool {
        request.owner == cx.entity_id()
            && request.generation == self.calibration_state.generation
            && self
                .calibration_state
                .pending
                .remove(&request.request_id)
                .is_some()
    }
    pub fn reject_calibration_request(
        &mut self,
        request: &MouseCalibrationRequested,
        error: String,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accept_calibration_response(request, cx) {
            return false;
        }
        // Config errors belong to the source errorActions/adapter path. They do
        // not authorize a fabricated calibration error screen or successful save.
        self.calibration_state.last_error = Some(error);
        cx.notify();
        true
    }
    pub fn finish_calibration_request(
        &mut self,
        request: &MouseCalibrationRequested,
        update: Option<&Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accept_calibration_response(request, cx) {
            return false;
        }
        if let Some(update) = update {
            self.observe_calibration(update, window, cx);
        }
        cx.notify();
        true
    }
    /// Apply only a real reducer snapshot received by the current device owner.
    pub fn observe_calibration(
        &mut self,
        value: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous_selected = self.draft["calibration"]["selectedProfile"].clone();
        if calibration_190::supports_smart_only_calibration(self.spec.product_id)
            || calibration_preset::supports(self.spec.product_id)
        {
            let smart = value.get("smartTracking").or_else(|| {
                value
                    .get("calibrationSmart")
                    .and_then(|state| state.get("smartTracking"))
            });
            if let Some(smart) = smart.filter(|smart| smart.is_object()) {
                let previous = self.draft["smartTracking"].clone();
                if previous["isAsymmetric"] != smart["isAsymmetric"] {
                    self.cancel_smart_calibration_drag(window, cx);
                }
                set_pointer(&mut self.draft, "/smartTracking", smart.clone());
                let was_syncing = self.syncing;
                self.syncing = true;
                for field in ["trackingDistance", "liftOffDistance", "landingDistance"] {
                    let path = format!("/smartTracking/{field}");
                    if self.smart_calibration_drag.as_deref() == Some(path.as_str())
                        && previous[field] != smart[field]
                    {
                        if calibration_190::smart_calibration_cancels_on_value_change(
                            self.spec.product_id,
                        ) {
                            self.smart_calibration_drag_cancelled = true;
                            self.smart_calibration_preview.remove(&path);
                        } else if let Some(value) = smart[field].as_f64() {
                            self.smart_calibration_preview
                                .insert(path.clone(), value as f32);
                        }
                    }
                    if let (Some(value), Some(slider)) =
                        (smart[field].as_f64(), self.sliders.get(&path))
                    {
                        slider.update(cx, |slider, cx| slider.set_value(value as f32, window, cx));
                    }
                    if let (Some(value), Some(input)) =
                        (smart[field].as_i64(), self.inputs.get(&path))
                    {
                        input.update(cx, |input, cx| {
                            input.set_value(value.to_string(), window, cx)
                        });
                    }
                }
                if let Some(slider) = self.sliders.get("smart-preset") {
                    let value = self.smart_preset() as f32;
                    if self.smart_calibration_drag.as_deref() == Some("smart-preset") {
                        self.smart_calibration_preview
                            .insert("smart-preset".into(), value);
                    }
                    slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
                }
                self.syncing = was_syncing;
            }
        }
        for key in [
            "profiles",
            "selectedProfile",
            "liftOffRangeValue",
            "presetSensorData",
            "tempManualCalibratedRawData",
        ] {
            if let Some(value) = value.get(key) {
                set_pointer(
                    &mut self.draft,
                    &format!("/calibration/{key}"),
                    value.clone(),
                );
            }
        }
        let state = &mut self.calibration_state;
        for (key, target) in [
            ("panelContent", &mut state.panel),
            ("featureAccess", &mut state.feature_access),
            ("calibrationReady", &mut state.ready),
            ("sensorType", &mut state.sensor_type),
            ("reCalibratedMat", &mut state.recalibrated_mat),
        ] {
            if let Some(value) = value[key].as_str() {
                *target = value.into();
            }
        }
        for (key, target) in [
            ("isMiniCalibrated", &mut state.mini_calibrated),
            ("isShowAddSurface", &mut state.show_add),
            ("leftHandMode", &mut state.left_hand),
            ("canGoBack", &mut state.can_go_back),
        ] {
            if let Some(value) = value[key].as_bool() {
                *target = value;
            }
        }
        if matches!(self.spec.product_id, 70 | 162) {
            if let Some(observed) = value["isTooFast"].as_bool() {
                state.too_fast_observed = observed;
            }
            // PID 70 iR.componentDidUpdate and /162 Gs only add .show while
            // the retained prop is true. A false update leaves the visible
            // warning until the previously scheduled two-second timer expires.
            if state.too_fast_observed {
                state.too_fast = true;
            }
        } else if let Some(observed) = value["isTooFast"].as_bool() {
            state.too_fast = observed;
        }
        if let Some(mode) = value["mode"]
            .as_str()
            .filter(|mode| matches!(*mode, "add" | "recalibrate"))
        {
            state.mode = if mode == "add" { "add" } else { "recalibrate" };
        }
        if let Some(kind) = value["calibrationType"]
            .as_str()
            .filter(|kind| matches!(*kind, "custom" | "mini"))
        {
            state.calibration_type = if kind == "mini" { "mini" } else { "custom" };
        }
        if let Some(open) = value["isBackdropOpen"].as_bool() {
            if open && !state.open {
                state.previous_focus = window.focused(cx);
                state.focus.focus(window, cx);
            } else if !open && state.open {
                if let Some(focus) = state.previous_focus.take() {
                    focus.focus(window, cx);
                }
                state.too_fast_task = None;
                state.too_fast = false;
            }
            state.open = open;
        }
        // PID 70's ERROR action sets both fields; a later reducer snapshot can
        // retain ERROR while explicitly returning to mats via iR.goBack.
        let error_panel = if self.spec.product_id == 70 {
            value["calibrationReady"] == "ERROR" && !value["panelContent"].is_string()
        } else {
            state.ready == "ERROR" && self.spec.product_id != 162
        };
        if error_panel {
            state.panel = "error".into();
        }
        if let Some(smart) = value.get("calibrationSmart") {
            state.calibration_smart = smart.clone();
        }
        if value["isTooFast"] == true
            || (matches!(self.spec.product_id, 70 | 162) && state.too_fast_observed)
        {
            let generation = state.generation;
            state.too_fast_task = Some(cx.spawn_in(window, async move |view, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(2))
                    .await;
                let _ = view.update(cx, |this, cx| {
                    if this.calibration_state.generation == generation {
                        this.calibration_state.too_fast = false;
                        cx.notify();
                    }
                });
            }));
        }
        if let Some(value) = self
            .draft
            .pointer("/calibration/liftOffRangeValue")
            .and_then(Value::as_f64)
        {
            if self.spec.product_id == 162
                && self.smart_calibration_drag.as_deref() == Some("/calibration/liftOffRangeValue")
            {
                self.smart_calibration_preview
                    .insert("/calibration/liftOffRangeValue".into(), value as f32);
            }
            if let Some(slider) = self.sliders.get("/calibration/liftOffRangeValue") {
                self.syncing = true;
                slider.update(cx, |slider, cx| slider.set_value(value as f32, window, cx));
                self.syncing = false;
            }
        }
        self.sync_preset_calibration_mount(window, cx);
        self.sync_babel_calibration_lift();
        if previous_selected != self.draft["calibration"]["selectedProfile"]
            && self.calibration_state.preset_tracking_ui.is_some()
        {
            self.submit_preset_tracking(cx);
        }
        cx.notify();
    }
    /// Current de.kHF lock observer payload, separate from calibration snapshots.
    pub fn observe_calibration_locks(&mut self, value: &Value, cx: &mut Context<Self>) {
        self.calibration_state.feature_access = if value["macroRecorderExclusiveHeld"] == true {
            "NONE"
        } else if value["deviceLockHeld"] == true {
            "PRESET_ONLY"
        } else {
            "ALL"
        }
        .into();
        cx.notify();
    }
    pub(in crate::features::mouse_products) fn calibration_profiles(&self) -> Vec<Value> {
        self.draft
            .pointer("/calibration/profiles")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_else(|| vec![json!({"surfaceId":"default","type":"default"})])
    }
    pub(crate) fn emit_calibration_config(&mut self, cx: &mut Context<Self>) {
        self.sync_babel_calibration_lift();
        let mut payload = self.calibration_context();
        payload["profiles"] = json!(self.calibration_profiles());
        if let Some(guid) = self.draft.pointer("/calibration/selectedProfile/guid") {
            payload["selectedProfileGuid"] = guid.clone();
        }
        self.emit_calibration("ON_CALIBRATION_CONFIG_CHANGE", payload, cx);
    }
    pub(crate) fn submit_mouse_smart_calibration_field(
        &mut self,
        path: &str,
        cx: &mut Context<Self>,
    ) {
        if calibration_preset::supports(self.spec.product_id)
            && (path == "/smartTracking" || path.starts_with("/smartTracking/"))
        {
            self.submit_preset_smart_tracking(self.draft["smartTracking"].clone(), cx);
            return;
        }
        if !calibration_190::supports_smart_only_calibration(self.spec.product_id)
            || !path.starts_with("/smartTracking/")
        {
            return;
        }
        // Current pS merges the actual smart-only state with the edited fields,
        // then posts these two distinct middleware messages in this order.
        // Neither the UI draft nor posting a request acknowledges either write.
        let payload = json!({"smartTracking":self.draft["smartTracking"]});
        self.emit_calibration("SET_SMART_TRACKING_VALUE", payload.clone(), cx);
        self.emit_calibration(
            "SET_SMART_CALIBRATION_DATA_TO_LOCAL_STORAGE_FROM_UI",
            payload,
            cx,
        );
    }

    pub(in crate::features::mouse_products) fn set_preset_smart_tracking(
        &mut self,
        value: Value,
        cx: &mut Context<Self>,
    ) {
        self.submit_preset_smart_tracking(value, cx);
        cx.emit(MouseProductChanged);
        cx.notify();
    }

    fn submit_preset_smart_tracking(&mut self, value: Value, cx: &mut Context<Self>) {
        self.draft["smartTracking"] = value.clone();
        self.calibration_state.calibration_smart["smartTracking"] = value.clone();
        self.emit_calibration(
            "SET_SMART_TRACKING_VALUE",
            json!({"smartTracking":value}),
            cx,
        );
        self.emit_calibration(
            "SET_SMART_CALIBRATION_DATA_TO_LOCAL_STORAGE_FROM_UI",
            self.calibration_state.calibration_smart.clone(),
            cx,
        );
    }

    pub(in crate::features::mouse_products) fn toggle_preset_calibration_mode(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_smart_calibration_drag(window, cx);
        let mode = self.calibration_state.calibration_smart["calibrationTrackingMode"] != true;
        self.calibration_state.calibration_smart["calibrationTrackingMode"] = json!(mode);
        if mode && self.preset_selected_profile()["surfaceId"] == "default" {
            self.emit_calibration("DISABLE_CALIBRATION", json!({}), cx);
        }
        self.emit_calibration(
            "SET_SMART_CALIBRATION_DATA_TO_LOCAL_STORAGE_FROM_UI",
            self.calibration_state.calibration_smart.clone(),
            cx,
        );
        self.sync_preset_calibration_mount(window, cx);
        cx.notify();
    }

    fn preset_selected_profile(&self) -> Value {
        let profiles = self.calibration_profiles();
        let selected = self.draft.pointer("/calibration/selectedProfile/guid");
        profiles
            .iter()
            .find(|profile| profile.get("guid") == selected)
            .cloned()
            .unwrap_or_else(|| self.draft["calibration"]["selectedProfile"].clone())
    }

    pub(in crate::features::mouse_products) fn sync_preset_calibration_mount(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !calibration_preset::supports(self.spec.product_id) || self.spec.product_id == 185 {
            return;
        }
        let mounted = self.page == "TAB_CALIBRATION"
            && self.calibration_state.calibration_smart["calibrationTrackingMode"] == true
            && self.calibration_profiles().len() > 1;
        if !mounted {
            if self.calibration_state.preset_tracking_ui.take().is_some() {
                self.cancel_smart_calibration_drag(window, cx);
            }
            return;
        }
        if self.calibration_state.preset_tracking_ui.is_none() {
            let mut tracking = self.calibration_state.calibration_smart["presetTracking"].clone();
            tracking["maxLandingDistance"] = json!(
                tracking["liftOffDistance"]
                    .as_u64()
                    .unwrap_or(5)
                    .saturating_sub(1)
            );
            self.calibration_state.preset_tracking_ui = Some(tracking.clone());
            self.draft["presetTracking"] = tracking;
            self.sync_preset_tracking_controls(window, cx);
            self.submit_preset_tracking(cx);
        }
    }

    fn sync_preset_tracking_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tracking = self.draft["presetTracking"].clone();
        let max = tracking["liftOffDistance"]
            .as_u64()
            .unwrap_or(5)
            .saturating_sub(1) as f32;
        let was_syncing = self.syncing;
        self.syncing = true;
        for field in ["surfaceDistance", "liftOffDistance", "landingDistance"] {
            let path = format!("/presetTracking/{field}");
            if let Some(slider) = self.sliders.get(&path) {
                let value = tracking[field].as_f64().unwrap_or(0.) as f32;
                slider.update(cx, |slider, cx| {
                    if field == "landingDistance" {
                        let retained = std::mem::replace(slider, SliderState::new());
                        *slider = retained.min(if max > 1. { 1. } else { 0. }).max(max);
                    }
                    slider.set_value(value, window, cx);
                });
            }
        }
        self.syncing = was_syncing;
    }

    pub(in crate::features::mouse_products) fn preset_tracking_enabled(&self) -> bool {
        let selected = &self.draft["calibration"]["selectedProfile"];
        selected.is_object()
            && !self.calibration_profiles().iter().any(|profile| {
                profile["type"] == "default" && profile.get("guid") == selected.get("guid")
            })
    }

    pub(in crate::features::mouse_products) fn set_preset_tracking_field(
        &mut self,
        field: &str,
        value: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.preset_tracking_enabled() {
            return;
        }
        if field == "isAsymmetric" {
            self.cancel_smart_calibration_drag(window, cx);
        }
        let Some(tracking) = self.calibration_state.preset_tracking_ui.as_mut() else {
            return;
        };
        tracking[field] = value;
        if field == "liftOffDistance" {
            let lift = tracking[field].as_u64().unwrap_or(5);
            if tracking["landingDistance"].as_u64().unwrap_or(0) >= lift {
                tracking["landingDistance"] = json!(lift.saturating_sub(1));
            }
            tracking["maxLandingDistance"] = json!(lift.saturating_sub(1));
        }
        self.draft["presetTracking"] = tracking.clone();
        self.sync_preset_tracking_controls(window, cx);
        self.submit_preset_tracking(cx);
        cx.emit(MouseProductChanged);
        cx.notify();
    }

    fn submit_preset_tracking(&mut self, cx: &mut Context<Self>) {
        let selected = self.preset_selected_profile();
        let Some(tracking) = self.calibration_state.preset_tracking_ui.clone() else {
            return;
        };
        if selected["surfaceId"].as_str().is_none() {
            self.calibration_state.last_error =
                Some("Preset calibration selected surface is absent".into());
            return;
        }
        if selected["surfaceId"] == "default" {
            self.emit_calibration("DISABLE_CALIBRATION", json!({}), cx);
            self.emit_calibration(
                "SET_SMART_CALIBRATION_DATA_TO_LOCAL_STORAGE_FROM_UI",
                self.calibration_state.calibration_smart.clone(),
                cx,
            );
            return;
        }
        if selected["surfaceId"] == "Custom" {
            let mut payload = json!({"presetTracking":tracking});
            if let Some(data) = selected.get("data") {
                payload["manualData"] = data.clone();
            }
            self.emit_calibration("SET_MANUAL_TRACKING_VALUES", payload, cx);
        } else {
            self.emit_calibration(
                "SET_PRESET_TRACKING_VALUE",
                json!({"presetTracking":tracking}),
                cx,
            );
        }
        self.calibration_state.calibration_smart["presetTracking"] = tracking;
        self.emit_calibration(
            "SET_SMART_CALIBRATION_DATA_TO_LOCAL_STORAGE_FROM_UI",
            self.calibration_state.calibration_smart.clone(),
            cx,
        );
    }

    pub(in crate::features::mouse_products) fn select_preset_calibration(
        &mut self,
        guid: Option<Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.calibration_state.feature_access == "NONE" {
            return;
        }
        if self.spec.product_id == 185 {
            if let Some(profile) = self
                .calibration_profiles()
                .into_iter()
                .find(|profile| profile.get("guid") == guid.as_ref())
            {
                if let Some(calibration) = self.draft["calibration"].as_object_mut() {
                    calibration.remove("tempManualCalibratedRawData");
                }
                self.write("/calibration/selectedProfile", profile, cx);
                self.emit_calibration_config(cx);
                self.calibration_state.calibration_smart["presetTracking"] =
                    json!({"surfaceDistance":5});
                self.emit_calibration(
                    "SET_PRESET_TRACKING_VALUE",
                    json!({"presetTracking":{"surfaceDistance":5}}),
                    cx,
                );
                self.emit_calibration(
                    "SET_SMART_CALIBRATION_DATA_TO_LOCAL_STORAGE_FROM_UI",
                    self.calibration_state.calibration_smart.clone(),
                    cx,
                );
            }
        } else {
            self.select_calibration(guid.clone(), cx);
            self.sync_preset_calibration_mount(window, cx);
            self.submit_preset_tracking(cx);
        }
    }
    fn calibration_context(&self) -> Value {
        json!({"mode":self.calibration_state.mode, "panelContent":self.calibration_state.panel,
            "reCalibratedMat":self.calibration_state.recalibrated_mat,
            "liftOffRangeValue":self.number("/calibration/liftOffRangeValue") as i64})
    }
    pub(in crate::features::mouse_products) fn select_calibration(
        &mut self,
        guid: Option<Value>,
        cx: &mut Context<Self>,
    ) {
        if self.calibration_state.feature_access == "NONE" {
            return;
        }
        let selected = guid
            .map(|guid| json!({"guid":guid}))
            .unwrap_or_else(|| json!({}));
        if self.spec.product_id != 70
            && let Some(calibration) = self
                .draft
                .get_mut("calibration")
                .and_then(Value::as_object_mut)
        {
            calibration.remove("tempManualCalibratedRawData");
        }
        self.write("/calibration/selectedProfile", selected, cx);
        if matches!(self.spec.product_id, 70 | 162) {
            // 70 aD1 / 162 qi dispatch a new selected object even for an equal GUID.
            self.sync_babel_calibration_lift_inner(true);
            // PID 70 calibration case aD1 retains the selected object but
            // submits the first profile when its GUID cannot be resolved.
            let profiles = self.calibration_profiles();
            let selected = self.draft.pointer("/calibration/selectedProfile/guid");
            let resolved = profiles
                .iter()
                .find(|profile| profile.get("guid") == selected)
                .or_else(|| profiles.first());
            let mut payload = self.calibration_context();
            payload["profiles"] = json!(profiles);
            if let Some(guid) = resolved.and_then(|profile| profile.get("guid")) {
                payload["selectedProfileGuid"] = guid.clone();
            }
            self.emit_calibration("ON_CALIBRATION_CONFIG_CHANGE", payload, cx);
        } else {
            self.emit_calibration_config(cx);
        }
    }
    pub(in crate::features::mouse_products) fn delete_calibration(
        &mut self,
        guid: &str,
        cx: &mut Context<Self>,
    ) {
        if self.calibration_state.feature_access == "NONE" {
            return;
        }
        let mut profiles = self.calibration_profiles();
        let Some(index) = profiles.iter().position(|profile| {
            profile["guid"].as_str() == Some(guid) && profile["type"] != "default"
        }) else {
            return;
        };
        profiles.remove(index);
        if self
            .draft
            .pointer("/calibration/selectedProfile/guid")
            .and_then(Value::as_str)
            == Some(guid)
        {
            let selected = profiles
                .first()
                .and_then(|profile| profile.get("guid"))
                .map(|guid| json!({"guid":guid}))
                .unwrap_or_else(|| json!({}));
            set_pointer(&mut self.draft, "/calibration/selectedProfile", selected);
        }
        self.write("/calibration/profiles", json!(profiles), cx);
        self.emit_calibration_config(cx);
    }
    pub(in crate::features::mouse_products) fn open_calibration(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.calibration_state.open {
            self.calibration_state.previous_focus = window.focused(cx);
        }
        self.calibration_state.panel = "mats".into();
        self.calibration_state.open = true;
        self.calibration_state.focus.focus(window, cx);
        cx.notify();
    }
    pub(in crate::features::mouse_products) fn begin_calibration(
        &mut self,
        name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.calibration_state.feature_access == "NONE" {
            return;
        }
        let previous = self.calibration_state.panel.clone();
        self.open_calibration(window, cx);
        self.calibration_state.mode = if name.is_some() { "recalibrate" } else { "add" };
        self.calibration_state.can_go_back = name.is_none();
        self.calibration_state.recalibrated_mat = name.unwrap_or_default();
        self.calibration_state.panel = "manualCalibrationMain".into();
        if !matches!(
            previous.as_str(),
            "error" | "manualCalibrationReady" | "manualCalibrationCalibrating"
        ) || self.calibration_state.ready == "ERROR"
        {
            let mut payload = self.calibration_context();
            payload["profiles"] = json!(self.calibration_profiles());
            payload["selectedProfile"] = self
                .draft
                .pointer("/calibration/selectedProfile")
                .cloned()
                .unwrap_or_else(|| json!({}));
            self.emit_calibration("ON_CALIBRATION_START", payload, cx);
        }
        cx.notify();
    }
    pub(crate) fn dismiss_calibration(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.calibration_state.open {
            return;
        }
        self.calibration_state.open = false;
        let abort = matches!(
            self.calibration_state.panel.as_str(),
            "manualCalibrationMain" | "error" | "manualCalibrationReady"
        );
        self.calibration_state.panel = "mats".into();
        self.calibration_state.mode = "add";
        self.calibration_state.can_go_back = false;
        self.calibration_state.too_fast_task = None;
        self.calibration_state.too_fast = false;
        if abort {
            self.emit_calibration("ON_CALIBRATION_ABORTED", json!({}), cx);
        }
        if let Some(focus) = self.calibration_state.previous_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }
    pub(in crate::features::mouse_products) fn back_calibration(&mut self, cx: &mut Context<Self>) {
        if self.spec.product_id == 70 {
            let command = calibration_panel_command_70(
                &self.calibration_state.panel,
                "mats",
                &self.calibration_state.ready,
                self.calibration_state.open,
            );
            self.calibration_state.panel = "mats".into();
            self.calibration_state.can_go_back = false;
            if let Some(command) = command {
                let payload = if command == "ON_CALIBRATION_START" {
                    let mut payload = self.calibration_context();
                    payload["profiles"] = json!(self.calibration_profiles());
                    payload["selectedProfile"] =
                        self.draft["calibration"]["selectedProfile"].clone();
                    payload
                } else {
                    json!({})
                };
                self.emit_calibration(command, payload, cx);
            }
            cx.notify();
            return;
        }
        let abort = matches!(
            self.calibration_state.panel.as_str(),
            "manualCalibrationMain" | "error" | "manualCalibrationReady"
        );
        self.calibration_state.panel = "mats".into();
        self.calibration_state.can_go_back = false;
        if abort {
            self.emit_calibration("ON_CALIBRATION_ABORTED", json!({}), cx);
        }
        cx.notify();
    }
    pub(in crate::features::mouse_products) fn choose_calibration_mat(
        &mut self,
        surface_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.calibration_state.feature_access == "NONE"
            || (surface_id == "Custom" && self.calibration_state.feature_access != "ALL")
        {
            return;
        }
        if !self.spec.mats.iter().any(|mat| mat["id"] == surface_id) {
            return;
        }
        self.calibration_state.selected_mat = Some(surface_id.into());
        if surface_id == "Custom" {
            self.begin_calibration(None, window, cx);
        } else if self.calibration_state.sensor_type == "philips"
            && !self.calibration_state.mini_calibrated
        {
            self.begin_calibration(None, window, cx);
            self.start_calibration(true, cx);
        } else {
            let mut profiles = self.calibration_profiles();
            if profiles
                .iter()
                .any(|profile| profile["surfaceId"] == surface_id)
            {
                return;
            }
            let guid = uuid::Uuid::new_v4().to_string();
            let mat = artwork(self.spec.product_id, surface_id);
            profiles.push(
                json!({"name":mat.name,"guid":guid,"surfaceId":surface_id,"type":"preset",
                "calibrationDate":calibration_date(self.spec.product_id)}),
            );
            self.dismiss_calibration(window, cx);
            set_pointer(
                &mut self.draft,
                "/calibration/selectedProfile",
                json!({"guid":guid}),
            );
            self.write("/calibration/profiles", json!(profiles), cx);
            self.emit_calibration_config(cx);
        }
    }
    pub(in crate::features::mouse_products) fn start_calibration(
        &mut self,
        mini: bool,
        cx: &mut Context<Self>,
    ) {
        if !mini && self.calibration_state.ready != "READY" {
            return;
        }
        if self.spec.product_id == 162 {
            // Gs invokes es(), whose only dispatch is CALIBRATION_RESET.
            // That reducer resets Je and retains just profiles/supported mats;
            // it does not post START_MANUAL_CALIBRATION to the host.
            let selected = self
                .calibration_profiles()
                .into_iter()
                .find(|profile| profile["surfaceId"] == "default")
                .and_then(|profile| profile.get("guid").cloned())
                .map(|guid| json!({"guid":guid}))
                .unwrap_or_else(|| json!({}));
            set_pointer(&mut self.draft, "/calibration/selectedProfile", selected);
            set_pointer(&mut self.draft, "/calibration/liftOffRangeValue", json!(3));
            if let Some(state) = self.draft["calibration"].as_object_mut() {
                state.remove("presetSensorData");
                state.remove("tempManualCalibratedRawData");
            }
            let state = &mut self.calibration_state;
            state.open = false;
            state.panel = "mats".into();
            state.mode = "add";
            state.can_go_back = false;
            state.recalibrated_mat.clear();
            state.ready = "NOT_READY".into();
            state.sensor_type.clear();
            state.show_add = true;
            state.feature_access = "ALL".into();
            state.too_fast = false;
            state.mini_calibrated = false;
            state.too_fast_observed = false;
            state.too_fast_task = None;
            self.sync_babel_calibration_lift();
            cx.emit(MouseProductChanged);
            cx.notify();
            return;
        }
        self.calibration_state.calibration_type = if mini { "mini" } else { "custom" };
        self.calibration_state.panel = "manualCalibrationReady".into();
        let mut payload = json!({"calibrationType":self.calibration_state.calibration_type,
            "calibrationSmart":self.calibration_state.calibration_smart});
        if mini {
            if let Some(surface_id) = &self.calibration_state.selected_mat {
                let mat = artwork(self.spec.product_id, surface_id);
                payload["mat"] = json!({"surfaceId":surface_id,"name":mat.name,"type":"preset","edition":mat.edition,"img":mat.source_image,"url":mat.url});
            }
        }
        self.emit_calibration("START_MANUAL_CALIBRATION", payload, cx);
        cx.notify();
    }
}

#[cfg(test)]
mod panel_transition_tests {
    use super::calibration_panel_command_70;

    #[::core::prelude::v1::test]
    fn returning_to_catalog_keeps_the_open_calibration_session() {
        for panel in ["manualCalibrationMain", "manualCalibrationReady", "error"] {
            assert_eq!(
                calibration_panel_command_70(panel, "mats", "READY", true),
                None
            );
            assert_eq!(
                calibration_panel_command_70(panel, "mats", "READY", false),
                Some("ON_CALIBRATION_ABORTED"),
            );
        }
    }

    #[::core::prelude::v1::test]
    fn errored_open_session_preserves_source_restart_before_abort_precedence() {
        assert_eq!(
            calibration_panel_command_70("error", "mats", "ERROR", true),
            Some("ON_CALIBRATION_START"),
        );
        assert_eq!(
            calibration_panel_command_70("error", "mats", "ERROR", false),
            Some("ON_CALIBRATION_ABORTED"),
        );
        assert_eq!(
            calibration_panel_command_70("manualCalibrationCalibrating", "mats", "READY", false),
            None,
        );
    }
}
