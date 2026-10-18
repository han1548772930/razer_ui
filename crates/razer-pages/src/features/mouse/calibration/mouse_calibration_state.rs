//! PID 70's current calibration reducer and requests. Requests are not acknowledgements.
use super::*;
use razer_device::mouse_philips::{self, measurement};
use razer_device::mouse_tracking::SmartTracking;
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

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

fn measurement_update(
    kind: measurement::Kind,
    event: measurement::CalibrationEvent,
    measured: Option<&mouse_philips::RawCalibration>,
) -> Value {
    use measurement::CalibrationEvent::*;
    match event {
        Start => json!({"panelContent":"manualCalibrationCalibrating","canGoBack":false}),
        End => match measured {
            Some(raw) => {
                json!({"panelContent":"calibrationComplete","canGoBack":false,"tempManualCalibratedRawData":raw})
            }
            None => json!({"panelContent":"error"}),
        },
        Halt => {
            json!({"panelContent":if kind == measurement::Kind::Mini { "mats" } else { "error" },"calibrationReady":"ERROR"})
        }
        Cancelled => json!({"panelContent":"manualCalibrationMain","canGoBack":false}),
        NormalSpeed => json!({"isTooFast":false}),
        TooFast => json!({"isTooFast":true}),
        Reset => json!({}),
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
    scope: Option<MousePollingScope>,
    valid: Arc<AtomicBool>,
    tracking: Option<SmartTracking>,
    philips: Option<razer_device::mouse_philips::ConfigChange>,
    measurement: Option<razer_device::mouse_philips::measurement::Kind>,
    measurement_context: Option<MeasurementContext>,
    measurement_save: Option<razer_device::mouse_philips::MeasurementSave>,
}
#[derive(Clone)]
struct MeasurementContext {
    destination: razer_device::mouse_philips::MeasurementDestination,
    mode: razer_device::mouse_philips::MeasurementMode,
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
    pub fn tracking(&self) -> Option<SmartTracking> {
        self.tracking
    }
    pub fn philips(&self) -> Option<&razer_device::mouse_philips::ConfigChange> {
        self.philips.as_ref()
    }
    pub fn measurement(&self) -> Option<razer_device::mouse_philips::measurement::Kind> {
        self.measurement
    }
    pub fn measurement_save(&self) -> Option<&razer_device::mouse_philips::MeasurementSave> {
        self.measurement_save.as_ref()
    }
    pub fn is_valid(&self) -> bool {
        self.valid.load(Ordering::Acquire)
    }
    pub fn guard(&self) -> Arc<AtomicBool> {
        self.valid.clone()
    }
    pub fn cancel(&self) {
        self.valid.store(false, Ordering::Release);
    }
    fn same_request(&self, other: &Self) -> bool {
        self.owner == other.owner
            && self.generation == other.generation
            && self.request_id == other.request_id
            && self.product_id == other.product_id
            && self.command == other.command
            && self.scope == other.scope
    }
    fn has_philips_local_save(&self) -> bool {
        self.product_id == mouse_philips::PRODUCT_ID
            && (self.philips.is_some() || self.measurement_save.is_some())
    }
}

struct LocalSave {
    request: MouseCalibrationRequested,
    submitted: bool,
    preserved_for_exit: bool,
}
struct Requests {
    owner: EntityId,
    generation: u64,
    next_request: u64,
    pending: BTreeMap<u64, MouseCalibrationRequested>,
    device_active: Option<MouseCalibrationRequested>,
    device_queued: VecDeque<MouseCalibrationRequested>,
    local_saves: BTreeMap<u64, LocalSave>,
    exiting: bool,
}
impl Requests {
    fn new(owner: EntityId) -> Self {
        Self {
            owner,
            generation: 0,
            next_request: 0,
            pending: BTreeMap::new(),
            device_active: None,
            device_queued: VecDeque::new(),
            local_saves: BTreeMap::new(),
            exiting: false,
        }
    }
    fn matches(&self, request: &MouseCalibrationRequested) -> bool {
        request.is_valid()
            && request.owner == self.owner
            && request.generation == self.generation
            && self
                .pending
                .get(&request.request_id)
                .is_some_and(|pending| pending.same_request(request))
    }
    fn enqueue(&mut self, request: MouseCalibrationRequested, device: bool) -> bool {
        if self.exiting {
            request.cancel();
            return false;
        }
        if request.has_philips_local_save() {
            self.local_saves.insert(
                request.request_id,
                LocalSave {
                    request: request.clone(),
                    submitted: false,
                    preserved_for_exit: false,
                },
            );
        }
        self.pending.insert(request.request_id, request.clone());
        if device {
            if self.device_active.is_some() {
                self.device_queued.push_back(request);
                return false;
            }
            self.device_active = Some(request);
        }
        true
    }
    fn local_save_identity_matches(&self, request: &MouseCalibrationRequested) -> bool {
        request.owner == self.owner
            && self
                .local_saves
                .get(&request.request_id)
                .is_some_and(|save| {
                    save.request.same_request(request)
                        && (save.preserved_for_exit || self.matches(request))
                })
    }
    fn begin_local_save(&mut self, request: &MouseCalibrationRequested) -> bool {
        if !self.local_save_identity_matches(request) {
            return false;
        }
        let save = self
            .local_saves
            .get_mut(&request.request_id)
            .expect("matched local save exists");
        if save.submitted {
            return false;
        }
        save.submitted = true;
        true
    }
    fn local_save_matches(&self, request: &MouseCalibrationRequested) -> bool {
        self.local_save_identity_matches(request) && self.local_saves[&request.request_id].submitted
    }
    fn finish_local_save(&mut self, request: &MouseCalibrationRequested) {
        if self
            .local_saves
            .get(&request.request_id)
            .is_some_and(|save| save.request.same_request(request))
        {
            self.local_saves.remove(&request.request_id);
        }
    }
    fn prepare_exit(
        &mut self,
        product_id: u32,
        scope: Option<MousePollingScope>,
    ) -> Vec<MouseCalibrationRequested> {
        self.exiting = true;
        let accepted = std::mem::take(&mut self.local_saves);
        let mut unsubmitted = Vec::new();
        for (id, mut save) in accepted {
            if save.request.product_id == product_id
                && save.request.scope == scope
                && (save.preserved_for_exit || self.matches(&save.request))
            {
                // Source storage completes independently of the following
                // skippable hardware task. Keep its accepted identity after
                // canceling device work, including later editor dismissal.
                save.preserved_for_exit = true;
                if !save.submitted {
                    unsubmitted.push(save.request.clone());
                }
                self.local_saves.insert(id, save);
            }
        }
        self.cancel();
        unsubmitted
    }
    fn cancel_exit(&mut self) {
        self.exiting = false;
    }
    fn complete(&mut self, request: &MouseCalibrationRequested) -> bool {
        let current = self.matches(request);
        if self
            .pending
            .get(&request.request_id)
            .is_some_and(|pending| pending.same_request(request))
        {
            self.pending.remove(&request.request_id);
        }
        request.cancel();
        if self
            .device_active
            .as_ref()
            .is_some_and(|active| active.same_request(request))
        {
            self.device_active = None;
        }
        current
    }
    fn next_device(&mut self) -> Option<MouseCalibrationRequested> {
        if self.device_active.is_some() {
            return None;
        }
        while let Some(next) = self.device_queued.pop_front() {
            if self.matches(&next) {
                self.device_active = Some(next.clone());
                return Some(next);
            }
            next.cancel();
        }
        None
    }
    fn cancel(&mut self) {
        for request in self.pending.values() {
            request.cancel();
        }
        if let Some(active) = &self.device_active {
            active.cancel();
        }
        for request in self.device_queued.drain(..) {
            request.cancel();
        }
        self.pending.clear();
        self.local_saves.retain(|_, save| save.preserved_for_exit);
        self.generation = self.generation.wrapping_add(1);
    }
}
impl Drop for Requests {
    fn drop(&mut self) {
        self.cancel();
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
    recalibrated_profile: Option<mouse_philips::ProfileId>,
    pub(in crate::features::mouse_products) selected_mat: Option<String>,
    pub(in crate::features::mouse_products) calibration_type: &'static str,
    pub(in crate::features::mouse_products) focus: FocusHandle,
    previous_focus: Option<FocusHandle>,
    requests: Requests,
    last_error: Option<String>,
    tracking_report: Option<razer_device::mouse_tracking::Progress>,
    philips_report: Option<razer_device::mouse_philips::Progress>,
    measurement_report: Option<razer_device::mouse_philips::measurement::Progress>,
    measurement_notifications: usize,
    pub(in crate::features::mouse_products) calibration_smart: Value,
    pub(in crate::features::mouse_products) preset_tracking_ui: Option<Value>,
    pub(in crate::features::mouse_products) babel_lift_using_default: bool,
    pub(in crate::features::mouse_products) babel_surface_hover: Option<usize>,
    too_fast_observed: bool,
    babel_lift_props: Option<(Value, Value)>,
    too_fast_task: Option<Task<()>>,
}
impl State {
    pub(crate) fn new(product_id: u32, cx: &mut Context<MouseProductWorkspace>) -> Self {
        Self {
            welcome: true,
            open: false,
            panel: "mats".into(),
            mode: "add",
            can_go_back: false,
            feature_access: "ALL".into(),
            ready: "NOT_READY".into(),
            // Current PID70 feature injection selects the Philips engine; its
            // MW 6120 initial projection emits sensorType="philips".
            sensor_type: if product_id == 70 {
                "philips".into()
            } else {
                String::new()
            },
            mini_calibrated: false,
            show_add: true,
            too_fast: false,
            left_hand: false,
            recalibrated_mat: String::new(),
            recalibrated_profile: None,
            selected_mat: None,
            calibration_type: "custom",
            focus: cx.focus_handle(),
            previous_focus: None,
            requests: Requests::new(cx.entity_id()),
            last_error: None,
            tracking_report: None,
            philips_report: None,
            measurement_report: None,
            measurement_notifications: 0,
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
    fn cancel_requests(&mut self) {
        self.requests.cancel();
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
    pub fn record_tracking_report(
        &mut self,
        request: &MouseCalibrationRequested,
        report: razer_device::mouse_tracking::Progress,
        cx: &mut Context<Self>,
    ) {
        if self.calibration_request_matches(request, cx) {
            self.calibration_state.tracking_report = Some(report);
        }
    }
    pub fn record_philips_report(
        &mut self,
        request: &MouseCalibrationRequested,
        report: razer_device::mouse_philips::Progress,
        cx: &mut Context<Self>,
    ) {
        if self.calibration_request_matches(request, cx) {
            self.calibration_state.philips_report = Some(report);
        }
    }
    fn measurement_context(&self, kind: measurement::Kind) -> anyhow::Result<MeasurementContext> {
        let destination = match kind {
            measurement::Kind::Manual => mouse_philips::MeasurementDestination::Custom,
            measurement::Kind::Mini => {
                let id = self
                    .calibration_state
                    .selected_mat
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Mini calibration surface is absent"))?;
                let surface = serde_json::from_value(json!(id))?;
                mouse_philips::MeasurementDestination::Preset {
                    surface,
                    name: artwork(self.spec.product_id, id).name.to_string(),
                }
            }
        };
        let mode = if self.calibration_state.mode == "recalibrate" {
            let profile = self
                .calibration_state
                .recalibrated_profile
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Recalibration target has no retained GUID"))?;
            let guid: String = profile.clone().into();
            anyhow::ensure!(
                self.calibration_profiles()
                    .iter()
                    .any(|row| row["guid"].as_str() == Some(guid.as_str())),
                "Recalibration target no longer exists"
            );
            mouse_philips::MeasurementMode::Recalibrate { profile }
        } else {
            mouse_philips::MeasurementMode::Add
        };
        Ok(MeasurementContext { destination, mode })
    }
    pub fn complete_philips_context(
        &mut self,
        request: &MouseCalibrationRequested,
        cx: &mut Context<Self>,
    ) {
        if self.calibration_request_matches(request, cx) {
            match request.command {
                "ON_CALIBRATION_START" => self.calibration_state.ready = "READY".into(),
                "ON_CALIBRATION_ABORTED" => self.calibration_state.ready = "NOT_READY".into(),
                _ => return,
            }
        }
        self.complete_calibration_operation(request, None, cx);
    }
    pub fn record_philips_measurement(
        &mut self,
        request: &MouseCalibrationRequested,
        progress: measurement::Progress,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.calibration_request_matches(request, cx) {
            return;
        }
        if self
            .calibration_state
            .measurement_report
            .as_ref()
            .is_none_or(|previous| previous.operation_id != progress.operation_id)
        {
            self.calibration_state.measurement_notifications = 0;
        }
        let from = self.calibration_state.measurement_notifications;
        for notification in progress.notifications.iter().skip(from) {
            self.observe_calibration(
                &measurement_update(
                    progress.kind,
                    notification.event,
                    progress.measured.as_ref(),
                ),
                window,
                cx,
            );
            if notification.event == measurement::CalibrationEvent::Reset {
                set_pointer(&mut self.draft, "/calibration/selectedProfile", json!({}));
                self.emit_calibration_config(cx);
            }
        }
        self.calibration_state.measurement_notifications = progress.notifications.len();
        let diagnostics: Vec<_> = progress
            .initialization_error
            .iter()
            .chain(progress.errors.iter())
            .cloned()
            .collect();
        if !diagnostics.is_empty() {
            self.calibration_state.last_error = Some(diagnostics.join("; "));
        }
        self.calibration_state.measurement_report = Some(progress);
        cx.notify();
    }
    pub fn finish_philips_measurement(
        &mut self,
        request: &MouseCalibrationRequested,
        progress: Option<measurement::Progress>,
        error: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.calibration_request_matches(request, cx) {
            if let Some(progress) = progress {
                self.record_philips_measurement(request, progress.clone(), window, cx);
                if progress.terminal
                    && progress.cleanup_confirmed
                    && !progress.canceled
                    && error.is_none()
                {
                    if let (Some(raw), Some(context)) =
                        (progress.measured, request.measurement_context.clone())
                    {
                        let guid =
                            mouse_philips::ProfileId::try_from(uuid::Uuid::new_v4().to_string())
                                .expect("UUID is a valid calibration identity");
                        let date = chrono::Local::now().format("%-d %b %Y").to_string();
                        match mouse_philips::MeasurementSave::new(
                            context.destination,
                            context.mode,
                            guid,
                            date,
                            raw,
                        ) {
                            Ok(save) => {
                                // Queue behind the measurement owner before releasing
                                // it, so all source device tasks keep their order.
                                self.emit_calibration_inner(
                                    "ON_CALIBRATION_END",
                                    json!({}),
                                    Some(save),
                                    cx,
                                );
                            }
                            Err(error) => {
                                self.calibration_state.last_error = Some(format!("{error:#}"))
                            }
                        }
                    }
                }
            }
        }
        self.complete_calibration_operation(request, error, cx);
    }
    pub fn publish_philips_measurement_save(
        &mut self,
        request: &MouseCalibrationRequested,
        projection: Value,
        cx: &mut Context<Self>,
    ) {
        if !self.calibration_request_matches(request, cx) {
            return;
        }
        set_pointer(&mut self.draft, "/calibration", projection);
        if request
            .measurement_save
            .as_ref()
            .is_some_and(|save| save.kind() == measurement::Kind::Mini)
        {
            self.calibration_state.mini_calibrated = true;
        }
        self.calibration_state.ready = "READY".into();
        self.sync_babel_calibration_lift();
        cx.notify();
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
        self.calibration_state.cancel_requests();
        self.calibration_state.last_error = None;
        self.calibration_state.tracking_report = None;
        self.calibration_state.philips_report = None;
        self.calibration_state.measurement_report = None;
        self.calibration_state.measurement_notifications = 0;
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
        self.emit_calibration_inner(command, payload, None, cx);
    }
    fn emit_calibration_inner(
        &mut self,
        command: &'static str,
        payload: Value,
        measurement_save: Option<razer_device::mouse_philips::MeasurementSave>,
        cx: &mut Context<Self>,
    ) {
        if self.calibration_state.requests.exiting {
            return;
        }
        let scope = self.mouse_polling_scope(cx);
        let tracking = if razer_device::mouse_tracking::capability(self.spec.product_id).is_some()
            && matches!(
                command,
                "SET_SMART_TRACKING_VALUE" | "SET_SMART_CALIBRATION_DATA_TO_LOCAL_STORAGE_FROM_UI"
            ) {
            match payload
                .get("smartTracking")
                .cloned()
                .ok_or_else(|| "Smart tracking payload is absent".to_owned())
                .and_then(|value| serde_json::from_value(value).map_err(|error| error.to_string()))
            {
                Ok(tracking) => Some(tracking),
                Err(error) => {
                    self.calibration_state.last_error = Some(error);
                    cx.notify();
                    return;
                }
            }
        } else {
            None
        };
        let philips = if self.spec.product_id == razer_device::mouse_philips::PRODUCT_ID
            && command == "ON_CALIBRATION_CONFIG_CHANGE"
        {
            match razer_device::mouse_philips::ConfigChange::parse(payload.clone()) {
                Ok(configuration) => Some(configuration),
                Err(error) => {
                    self.calibration_state.last_error = Some(format!("{error:#}"));
                    cx.notify();
                    return;
                }
            }
        } else {
            None
        };
        let measurement = (self.spec.product_id == 70 && command == "START_MANUAL_CALIBRATION")
            .then_some(if payload["calibrationType"] == "mini" {
                razer_device::mouse_philips::measurement::Kind::Mini
            } else {
                razer_device::mouse_philips::measurement::Kind::Manual
            });
        let measurement_context = if let Some(kind) = measurement {
            match self.measurement_context(kind) {
                Ok(context) => Some(context),
                Err(error) => {
                    self.calibration_state.last_error = Some(format!("{error:#}"));
                    cx.notify();
                    return;
                }
            }
        } else {
            None
        };
        let device_operation = (tracking.is_some() && command == "SET_SMART_TRACKING_VALUE")
            || philips.is_some()
            || measurement.is_some()
            || measurement_save.is_some();
        let state = &mut self.calibration_state;
        state.requests.next_request = state.requests.next_request.wrapping_add(1);
        state.last_error = None;
        let request = MouseCalibrationRequested {
            owner: cx.entity_id(),
            generation: state.requests.generation,
            request_id: state.requests.next_request,
            product_id: self.spec.product_id,
            command,
            payload,
            scope,
            valid: Arc::new(AtomicBool::new(true)),
            tracking,
            philips,
            measurement,
            measurement_context,
            measurement_save,
        };
        if state.requests.enqueue(request.clone(), device_operation) {
            cx.emit(request);
        }
    }
    pub fn calibration_request_matches(
        &self,
        request: &MouseCalibrationRequested,
        cx: &App,
    ) -> bool {
        self.calibration_state.requests.matches(request)
            && request.product_id == self.spec.product_id
            && ((request.tracking.is_none()
                && request.philips.is_none()
                && request.measurement.is_none()
                && request.measurement_save.is_none())
                || self.mouse_polling_scope(cx) == request.scope)
    }
    pub fn cancel_calibration_requests(&mut self) {
        self.calibration_state.cancel_requests();
    }
    pub fn prepare_calibration_exit(&mut self, cx: &App) -> Vec<MouseCalibrationRequested> {
        let scope = self.mouse_polling_scope(cx);
        self.calibration_state
            .requests
            .prepare_exit(self.spec.product_id, scope)
    }
    pub fn cancel_calibration_exit(&mut self) {
        self.calibration_state.requests.cancel_exit();
    }
    pub fn begin_philips_local_save(
        &mut self,
        request: &MouseCalibrationRequested,
        cx: &App,
    ) -> bool {
        request.product_id == self.spec.product_id
            && request.scope == self.mouse_polling_scope(cx)
            && self.calibration_state.requests.begin_local_save(request)
    }
    pub fn philips_local_save_matches(
        &self,
        request: &MouseCalibrationRequested,
        cx: &App,
    ) -> bool {
        request.product_id == self.spec.product_id
            && request.scope == self.mouse_polling_scope(cx)
            && self.calibration_state.requests.local_save_matches(request)
    }
    pub fn finish_philips_local_save(&mut self, request: &MouseCalibrationRequested) {
        self.calibration_state.requests.finish_local_save(request);
    }
    pub fn complete_calibration_operation(
        &mut self,
        request: &MouseCalibrationRequested,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let current = self.calibration_request_matches(request, cx);
        self.calibration_state.requests.complete(request);
        if current {
            if let Some(error) = error {
                self.calibration_state.last_error = Some(error);
            }
        }
        while let Some(next) = self.calibration_state.requests.next_device() {
            if self.calibration_request_matches(&next, cx) {
                cx.emit(next);
                break;
            }
            self.calibration_state.requests.complete(&next);
        }
        cx.notify();
    }
    fn accept_calibration_response(
        &mut self,
        request: &MouseCalibrationRequested,
        cx: &Context<Self>,
    ) -> bool {
        let current =
            request.owner == cx.entity_id() && self.calibration_request_matches(request, cx);
        if current {
            self.calibration_state.requests.complete(request);
        }
        current
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
            let generation = state.requests.generation;
            state.too_fast_task = Some(cx.spawn_in(window, async move |view, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(2))
                    .await;
                let _ = view.update(cx, |this, cx| {
                    if this.calibration_state.requests.generation == generation {
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
        profile: Option<(String, String)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.calibration_state.feature_access == "NONE" {
            return;
        }
        let previous = self.calibration_state.panel.clone();
        self.open_calibration(window, cx);
        let profile = match profile
            .map(|(guid, name)| mouse_philips::ProfileId::try_from(guid).map(|guid| (guid, name)))
            .transpose()
        {
            Ok(profile) => profile,
            Err(error) => {
                self.calibration_state.last_error = Some(format!("{error:#}"));
                cx.notify();
                return;
            }
        };
        self.calibration_state.mode = if profile.is_some() { "recalibrate" } else { "add" };
        self.calibration_state.can_go_back = profile.is_none();
        self.calibration_state.recalibrated_mat = profile
            .as_ref()
            .map(|(_, name)| name.clone())
            .unwrap_or_default();
        self.calibration_state.recalibrated_profile = profile.map(|(guid, _)| guid);
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
        if self
            .calibration_state
            .requests
            .device_active
            .as_ref()
            .is_some_and(|request| request.measurement.is_some())
        {
            self.calibration_state.cancel_requests();
        }
        self.calibration_state.open = false;
        let abort = matches!(
            self.calibration_state.panel.as_str(),
            "manualCalibrationMain" | "error" | "manualCalibrationReady"
        );
        self.calibration_state.panel = "mats".into();
        self.calibration_state.mode = "add";
        self.calibration_state.recalibrated_profile = None;
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
            state.recalibrated_profile = None;
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
    use super::{calibration_panel_command_70, measurement, measurement_update};

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

    #[::core::prelude::v1::test]
    fn philips_source_notifications_keep_manual_mini_error_and_cancel_semantics() {
        use measurement::{CalibrationEvent as Event, Kind};
        assert_eq!(
            measurement_update(Kind::Manual, Event::Halt, None),
            serde_json::json!({"panelContent":"error","calibrationReady":"ERROR"})
        );
        assert_eq!(
            measurement_update(Kind::Mini, Event::Halt, None),
            serde_json::json!({"panelContent":"mats","calibrationReady":"ERROR"})
        );
        let canceled = measurement_update(Kind::Manual, Event::Cancelled, None);
        assert_eq!(
            canceled,
            serde_json::json!({"panelContent":"manualCalibrationMain","canGoBack":false})
        );
        assert!(canceled.get("calibrationReady").is_none());
        assert_eq!(
            measurement_update(Kind::Manual, Event::End, None),
            serde_json::json!({"panelContent":"error"})
        );
        assert_eq!(
            measurement_update(Kind::Manual, Event::Reset, None),
            serde_json::json!({})
        );
        assert_eq!(
            measurement_update(Kind::Manual, Event::NormalSpeed, None),
            serde_json::json!({"isTooFast":false})
        );
    }
}

#[cfg(test)]
mod request_lifecycle_tests {
    use super::*;
    fn request(
        owner: EntityId,
        generation: u64,
        id: u64,
        device: bool,
    ) -> MouseCalibrationRequested {
        MouseCalibrationRequested {
            owner,
            generation,
            request_id: id,
            product_id: 170,
            command: if device {
                "SET_SMART_TRACKING_VALUE"
            } else {
                "SET_SMART_CALIBRATION_DATA_TO_LOCAL_STORAGE_FROM_UI"
            },
            payload: json!({"smartTracking":{"isAsymmetric":false,"trackingDistance":2,"liftOffDistance":13,"landingDistance":12}}),
            scope: Some(MousePollingScope::new(owner, generation, 1)),
            valid: Arc::new(AtomicBool::new(true)),
            tracking: None,
            philips: None,
            measurement: None,
            measurement_context: None,
            measurement_save: None,
        }
    }
    fn philips_request(
        owner: EntityId,
        generation: u64,
        id: u64,
        lift: u8,
    ) -> MouseCalibrationRequested {
        let mut request = request(owner, generation, id, true);
        request.product_id = mouse_philips::PRODUCT_ID;
        request.command = "ON_CALIBRATION_CONFIG_CHANGE";
        request.scope = Some(MousePollingScope::new(owner, 0, 1));
        request.payload =
            json!({"profiles":[{"type":"default","surfaceId":"default"}],"liftOffRangeValue":lift});
        request.philips =
            Some(mouse_philips::ConfigChange::parse(request.payload.clone()).unwrap());
        request
    }
    fn measurement_request(owner: EntityId, id: u64) -> MouseCalibrationRequested {
        let mut request = philips_request(owner, 0, id, 3);
        request.command = "START_MANUAL_CALIBRATION";
        request.payload = json!({"calibrationType":"manual"});
        request.philips = None;
        request.measurement = Some(measurement::Kind::Manual);
        request.measurement_context = Some(MeasurementContext {
            destination: mouse_philips::MeasurementDestination::Custom,
            mode: mouse_philips::MeasurementMode::Add,
        });
        request
    }
    fn measurement_save_request(owner: EntityId, id: u64) -> MouseCalibrationRequested {
        let mut request = philips_request(owner, 0, id, 3);
        request.command = "ON_CALIBRATION_END";
        request.philips = None;
        request.measurement_save = Some(
            mouse_philips::MeasurementSave::new(
                mouse_philips::MeasurementDestination::Custom,
                mouse_philips::MeasurementMode::Add,
                "measured-guid".to_owned().try_into().unwrap(),
                "18 Oct 2026".into(),
                serde_json::from_value(json!({"xOffset":1,"xSwing":2,"xTIAGain":3,
                    "xReferenceHeightA":4,"xReferenceHeightB":5,"temperatureVarA":6,
                    "temperatureVarB":7,"yOffset":8,"ySwing":9,"yTIAGain":10,
                    "yReferenceHeightA":11,"yReferenceHeightB":12,
                    "mean":13,"standardDeviation":14,"VPower":15}))
                .unwrap(),
            )
            .unwrap(),
        );
        request
    }
    #[::core::prelude::v1::test]
    fn exit_persists_queued_edits_while_canceling_an_active_hardware_task() {
        let owner = EntityId::from(1);
        let mut requests = Requests::new(owner);
        let active = philips_request(owner, 0, 1, 3);
        let queued = philips_request(owner, 0, 2, 5);
        assert!(requests.enqueue(active.clone(), true));
        assert!(requests.begin_local_save(&active));
        requests.finish_local_save(&active);
        assert!(requests.matches(&active));
        assert!(!requests.enqueue(queued.clone(), true));

        let drain = requests.prepare_exit(mouse_philips::PRODUCT_ID, active.scope);
        assert_eq!(drain.len(), 1);
        assert!(drain[0].same_request(&queued));
        assert!(!active.is_valid() && !queued.is_valid());
        assert!(!requests.matches(&queued));
        assert!(requests.begin_local_save(&queued));
        assert!(requests.local_save_matches(&queued));
        requests.cancel();
        assert!(requests.local_save_matches(&queued));
        requests.finish_local_save(&queued);
        assert!(!requests.local_save_matches(&queued));
        assert!(!requests.begin_local_save(&queued));
        assert!(requests.next_device().is_none());
    }
    #[::core::prelude::v1::test]
    fn exit_keeps_the_submitted_writer_and_never_submits_it_twice() {
        let owner = EntityId::from(1);
        let mut requests = Requests::new(owner);
        let submitted = philips_request(owner, 0, 1, 3);
        let queued = philips_request(owner, 0, 2, 6);
        assert!(requests.enqueue(submitted.clone(), true));
        assert!(requests.begin_local_save(&submitted));
        assert!(!requests.enqueue(queued.clone(), true));
        let drain = requests.prepare_exit(mouse_philips::PRODUCT_ID, submitted.scope);
        assert_eq!(drain.len(), 1);
        assert!(drain[0].same_request(&queued));
        assert!(requests.local_save_matches(&submitted));
        assert!(!requests.begin_local_save(&submitted));

        let mut root = None;
        submitted
            .philips()
            .unwrap()
            .merge_local_settings(&mut root)
            .unwrap();
        requests.finish_local_save(&submitted);
        assert!(requests.begin_local_save(&queued));
        queued
            .philips()
            .unwrap()
            .merge_local_settings(&mut root)
            .unwrap();
        requests.finish_local_save(&queued);
        assert!(!requests.begin_local_save(&submitted));
        assert!(!requests.begin_local_save(&queued));
        let saved = root.unwrap();
        assert_eq!(saved["version"], 2);
        assert_eq!(saved["calibration"]["liftOffRangeValue"], 6);
    }
    #[::core::prelude::v1::test]
    fn exit_keeps_accepted_measurement_saves_in_the_configuration_fifo() {
        let owner = EntityId::from(1);
        let mut requests = Requests::new(owner);
        let measuring = measurement_request(owner, 1);
        let measured = measurement_save_request(owner, 2);
        let configured = philips_request(owner, 0, 3, 7);
        assert!(requests.enqueue(measuring.clone(), true));
        assert!(!requests.enqueue(measured.clone(), true));
        assert!(!requests.enqueue(configured.clone(), true));
        let drain = requests.prepare_exit(mouse_philips::PRODUCT_ID, measured.scope);
        assert_eq!(
            drain.iter().map(|r| r.request_id()).collect::<Vec<_>>(),
            [2, 3]
        );
        assert!(!requests.begin_local_save(&measuring));
        assert!(requests.begin_local_save(&drain[0]));
        assert!(drain[0].measurement_save().is_some());
        requests.finish_local_save(&drain[0]);
        assert!(requests.begin_local_save(&drain[1]));
        assert!(drain[1].philips().is_some());
        requests.finish_local_save(&drain[1]);
        assert!(
            requests
                .prepare_exit(mouse_philips::PRODUCT_ID, measured.scope)
                .is_empty()
        );
    }
    #[::core::prelude::v1::test]
    fn unfinished_measurement_does_not_become_a_local_save_on_exit() {
        let owner = EntityId::from(1);
        let mut requests = Requests::new(owner);
        let measuring = measurement_request(owner, 1);
        assert!(requests.enqueue(measuring.clone(), true));
        assert!(
            requests
                .prepare_exit(mouse_philips::PRODUCT_ID, measuring.scope)
                .is_empty()
        );
        assert!(!requests.begin_local_save(&measuring));
        assert!(!measuring.is_valid());
    }
    #[::core::prelude::v1::test]
    fn exit_local_identity_rejects_foreign_owners_generations_and_connections() {
        let owner = EntityId::from(1);
        let mut requests = Requests::new(owner);
        let current = philips_request(owner, 0, 1, 3);
        assert!(requests.enqueue(current.clone(), true));
        let drain = requests.prepare_exit(mouse_philips::PRODUCT_ID, current.scope);
        assert_eq!(drain.len(), 1);
        for mut foreign in [
            philips_request(EntityId::from(2), 0, 1, 3),
            philips_request(owner, 1, 1, 3),
            current.clone(),
        ] {
            if foreign.same_request(&current) {
                foreign.scope = Some(MousePollingScope::new(owner, 0, 2));
            }
            assert!(!requests.begin_local_save(&foreign));
            requests.finish_local_save(&foreign);
        }
        assert!(requests.begin_local_save(&current));
        assert!(requests.local_save_matches(&current));

        let mut disconnected = Requests::new(owner);
        let old_connection = philips_request(owner, 0, 2, 4);
        assert!(disconnected.enqueue(old_connection.clone(), true));
        assert!(
            disconnected
                .prepare_exit(
                    mouse_philips::PRODUCT_ID,
                    Some(MousePollingScope::new(owner, 0, 2))
                )
                .is_empty()
        );
        assert!(!disconnected.begin_local_save(&old_connection));
    }
    #[::core::prelude::v1::test]
    fn failed_local_write_can_finish_without_confirming_hardware_or_blocking_recovery() {
        let owner = EntityId::from(1);
        let mut requests = Requests::new(owner);
        let failed = philips_request(owner, 0, 1, 3);
        let remaining = philips_request(owner, 0, 2, 4);
        assert!(requests.enqueue(failed.clone(), true));
        assert!(requests.begin_local_save(&failed));
        assert!(!requests.enqueue(remaining.clone(), true));
        let drain = requests.prepare_exit(mouse_philips::PRODUCT_ID, failed.scope);
        assert_eq!(drain.len(), 1);
        requests.finish_local_save(&failed);
        assert!(!requests.complete(&failed));
        requests.cancel_exit();
        assert!(requests.begin_local_save(&remaining));
        assert!(!requests.matches(&remaining));
        let recovered = philips_request(owner, requests.generation, 3, 5);
        assert!(requests.enqueue(recovered.clone(), true));
        assert!(requests.matches(&recovered));
        assert!(requests.begin_local_save(&recovered));
        assert!(!requests.begin_local_save(&failed));
    }
    #[::core::prelude::v1::test]
    fn local_storage_events_remain_independent_of_serialized_device_tasks() {
        let owner = EntityId::from(1);
        let mut requests = Requests::new(owner);
        let first = request(owner, 0, 1, true);
        let second = request(owner, 0, 3, true);
        assert!(requests.enqueue(first.clone(), true));
        assert!(requests.enqueue(request(owner, 0, 2, false), false));
        assert!(!requests.enqueue(second.clone(), true));
        assert!(requests.enqueue(request(owner, 0, 4, false), false));
        assert!(requests.next_device().is_none());
        assert!(requests.complete(&first));
        assert!(requests.next_device().unwrap().same_request(&second));
    }
    #[::core::prelude::v1::test]
    fn canceled_generation_waits_for_actual_cleanup_before_starting_its_successor() {
        let owner = EntityId::from(1);
        let mut requests = Requests::new(owner);
        let old = request(owner, 0, 1, true);
        let canceled_queue = request(owner, 0, 2, true);
        assert!(requests.enqueue(old.clone(), true));
        assert!(!requests.enqueue(canceled_queue.clone(), true));
        requests.cancel();
        assert!(!old.is_valid() && !canceled_queue.is_valid());
        let successor = request(owner, 1, 3, true);
        assert!(!requests.enqueue(successor.clone(), true));
        assert!(requests.next_device().is_none());
        assert!(!requests.complete(&old));
        assert!(requests.next_device().unwrap().same_request(&successor));
        assert!(!requests.complete(&old));
        assert!(requests.matches(&successor));
        assert!(
            requests
                .device_active
                .as_ref()
                .unwrap()
                .same_request(&successor)
        );
        drop(requests);
        assert!(!successor.is_valid());
    }
    #[::core::prelude::v1::test]
    fn equal_local_counters_from_a_different_owner_cannot_remove_a_request() {
        let owner = EntityId::from(1);
        let mut requests = Requests::new(owner);
        let current = request(owner, 0, 1, true);
        let foreign = request(EntityId::from(2), 0, 1, true);
        assert!(requests.enqueue(current.clone(), true));
        assert!(!requests.complete(&foreign));
        assert!(requests.matches(&current));
        assert!(
            requests
                .device_active
                .as_ref()
                .unwrap()
                .same_request(&current)
        );
    }

    #[::core::prelude::v1::test]
    fn completed_canceled_request_releases_its_exact_pending_record() {
        let owner = EntityId::from(1);
        let mut requests = Requests::new(owner);
        let canceled = request(owner, 0, 1, true);
        assert!(requests.enqueue(canceled.clone(), true));
        canceled.cancel();
        assert!(!requests.complete(&canceled));
        assert!(requests.pending.is_empty());
        assert!(requests.device_active.is_none());
    }
}
