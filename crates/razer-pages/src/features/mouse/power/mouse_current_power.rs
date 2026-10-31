//! Mounted current mouse power widgets. The page/card and reducer bodies are
//! retained by the mouse-page-layout and mouse-power queries in mouse-main-audit.
use super::super::mouse_polling::MousePollingScope;
use super::*;
use razer_device::mouse_current_power::{Change, Report};
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone)]
pub struct MouseCurrentPowerRead {
    request: MousePowerSavingRead,
}

/// Identity of an initial GET, independent of profile selection and edits.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MousePowerSavingRead {
    scope: MousePollingScope,
    sequence: u64,
    edit_revision: u64,
}
impl MousePowerSavingRead {
    pub fn scope(self) -> MousePollingScope {
        self.scope
    }
}

#[derive(Clone, Copy)]
struct PowerSavingEdit {
    seconds: f32,
    revision: u64,
}

/// Device seconds, active drag and committed optimistic edit have one owner.
/// The source reducer's default remains in the draft, outside this observation.
#[derive(Default)]
pub(in crate::features) struct PowerSavingValue {
    scope: Option<MousePollingScope>,
    observed_seconds: Option<u16>,
    preview: Option<PowerSavingEdit>,
    edited: Option<PowerSavingEdit>,
    edit_revision: u64,
    read_sequence: u64,
    read_pending: Option<MousePowerSavingRead>,
}
impl PowerSavingValue {
    fn retain_connection(&mut self, scope: MousePollingScope) {
        if self
            .scope
            .is_some_and(|old| !old.same_device_connection(scope))
        {
            self.invalidate();
        }
        self.scope = Some(scope);
    }

    pub(in crate::features) fn invalidate(&mut self) {
        self.scope = None;
        self.observed_seconds = None;
        self.preview = None;
        self.edited = None;
        self.read_pending = None;
        self.edit_revision = self.edit_revision.wrapping_add(1);
    }

    pub(in crate::features) fn begin_read(
        &mut self,
        scope: MousePollingScope,
    ) -> Option<MousePowerSavingRead> {
        self.retain_connection(scope);
        if self.observed_seconds.is_some() || self.read_pending.is_some() {
            return None;
        }
        self.read_sequence = self.read_sequence.wrapping_add(1);
        let request = MousePowerSavingRead {
            scope,
            sequence: self.read_sequence,
            edit_revision: self.edit_revision,
        };
        self.read_pending = Some(request);
        Some(request)
    }

    pub(in crate::features) fn begin_requery(
        &mut self,
        scope: MousePollingScope,
    ) -> Option<MousePowerSavingRead> {
        self.read_pending = None;
        self.begin_read(scope)
    }

    pub(in crate::features) fn finish_read(
        &mut self,
        request: MousePowerSavingRead,
        seconds: Option<u16>,
    ) -> bool {
        if self.read_pending != Some(request)
            || !self
                .scope
                .is_some_and(|scope| scope.same_device_connection(request.scope))
        {
            return false;
        }
        self.read_pending = None;
        if request.edit_revision == self.edit_revision {
            self.observed_seconds = seconds;
        }
        // A GET begun before an edit cannot replace that edit's confirmation.
        // Failure never installs the source helper's default argument as data.
        true
    }

    pub(in crate::features) fn preview(&mut self, scope: MousePollingScope, seconds: f32) {
        self.retain_connection(scope);
        self.edit_revision = self.edit_revision.wrapping_add(1);
        self.preview = Some(PowerSavingEdit {
            seconds,
            revision: self.edit_revision,
        });
    }

    pub(in crate::features) fn edit(&mut self, scope: MousePollingScope, seconds: u16) -> u64 {
        self.retain_connection(scope);
        self.edit_revision = self.edit_revision.wrapping_add(1);
        self.preview = None;
        self.edited = Some(PowerSavingEdit {
            seconds: f32::from(seconds),
            revision: self.edit_revision,
        });
        self.edit_revision
    }

    pub(in crate::features) fn confirm(
        &mut self,
        scope: MousePollingScope,
        revision: u64,
        seconds: u16,
    ) {
        if !self
            .scope
            .is_some_and(|current| current.same_device_connection(scope))
        {
            return;
        }
        self.observed_seconds = Some(seconds);
        if self.edited.is_some_and(|edit| edit.revision == revision) {
            self.edited = None;
        }
    }

    pub(in crate::features) fn cancel_preview(&mut self) {
        if self.preview.take().is_some() {
            self.edit_revision = self.edit_revision.wrapping_add(1);
        }
    }

    pub(in crate::features) fn displayed_seconds(&self) -> Option<f32> {
        self.preview
            .or(self.edited)
            .map(|edit| edit.seconds)
            .or_else(|| self.observed_seconds.map(f32::from))
    }
}

pub(in crate::features) fn unobserved_power_range(state: &Entity<SliderState>) -> Stateful<Div> {
    div()
        .id(("power-saving-unobserved", state.entity_id()))
        .aria_label("Device power saving has not been read")
        .relative()
        .w_full()
        .h(surface::css(64.))
        .child(
            div()
                .absolute()
                .bottom(surface::css(25.))
                .w_full()
                .h(surface::css(6.))
                .rounded(surface::css(3.))
                .bg(razer_widgets::theme::SliderColors::track()),
        )
}

#[derive(Clone)]
pub struct MouseCurrentPowerRequested {
    scope: MousePollingScope,
    sequence: u64,
    settings: Value,
    change: Option<Change>,
    read: Option<MousePowerSavingRead>,
    edit_revision: Option<u64>,
    valid: Arc<AtomicBool>,
}
impl MouseCurrentPowerRequested {
    pub fn scope(&self) -> MousePollingScope {
        self.scope
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn settings(&self) -> &Value {
        &self.settings
    }
    pub fn change(&self) -> Option<&Change> {
        self.change.as_ref()
    }
    pub fn is_read(&self) -> bool {
        self.change == Some(Change::ReadPowerSaving)
    }
    pub fn requires_local_persistence(&self, product_id: u32) -> bool {
        match self.change.as_ref() {
            Some(Change::ReadPowerSaving) => false,
            Some(Change::PowerSaving(_)) => !always_read_power_saving(product_id),
            _ => true,
        }
    }
    pub fn is_valid(&self) -> bool {
        self.valid.load(Ordering::Acquire)
    }
    pub fn guard(&self) -> Arc<AtomicBool> {
        self.valid.clone()
    }
    fn cancel(&self) {
        self.valid.store(false, Ordering::Release);
    }
}

#[derive(Default)]
pub(super) struct State {
    sequence: u64,
    pending: Option<MouseCurrentPowerRequested>,
    queued: VecDeque<MouseCurrentPowerRequested>,
    // Source task results are diagnostics, never optimistic reducer values.
    observed: Option<Report>,
    restored: bool,
    saving: PowerSavingValue,
}
impl State {
    fn enqueue(
        &mut self,
        request: MouseCurrentPowerRequested,
    ) -> Option<MouseCurrentPowerRequested> {
        if self.pending.is_none() {
            self.pending = Some(request.clone());
            return Some(request);
        }
        // Keep distinct fields in source task order; a dragged field keeps
        // only its latest requested value while another field is in flight.
        if let Some(ix) = self.queued.iter().position(|old| {
            old.change.as_ref().map(Change::key) == request.change.as_ref().map(Change::key)
        }) {
            if let Some(old) = self.queued.remove(ix) {
                old.cancel();
            }
            self.queued.insert(ix, request);
        } else {
            self.queued.push_back(request);
        }
        None
    }
    pub(super) fn cancel(&mut self) {
        if let Some(pending) = &self.pending {
            pending.cancel();
        }
        for queued in self.queued.drain(..) {
            queued.cancel();
        }
        self.observed = None;
        self.saving.invalidate();
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}
impl EventEmitter<MouseCurrentPowerRequested> for MouseProductWorkspace {}
impl EventEmitter<MouseCurrentPowerRead> for MouseProductWorkspace {}

/// Current basic settings belong to the device, independently of profiles.
pub fn device_fields(product_id: u32) -> &'static [&'static str] {
    if source_product(product_id).is_some_and(|spec| {
        spec.power_slider
            || spec.performance_power
            || spec.low_power_slider
            || spec.low_battery_slider
    }) {
        &[
            "powerSavingValue",
            "PowerSavingValue",
            "lowPowerMode",
            "lowBatteryEffects",
            "dimLighting",
            "batteryType",
        ]
    } else {
        &[]
    }
}

impl MouseProductWorkspace {
    pub fn invalidate_current_power_connection(&mut self) {
        if razer_device::mouse_current_power::capability(self.spec.product_id).is_some() {
            self.current_power.cancel();
        }
    }
    pub(super) fn snapshot_power(&self, snapshot: &mut Value) {
        filter_power_persistence(self.spec.product_id, snapshot);
    }
    pub(super) fn restore_power_session(&mut self, previous: &Value) {
        restore_power_settings(
            self.spec.product_id,
            self.current_power.restored,
            previous,
            &mut self.draft,
        );
        self.current_power.restored = true;
    }
    pub(super) fn request_current_power_read(&mut self, cx: &mut Context<Self>) {
        if !self.active
            || self.current_power.pending.is_some()
            || !self.polling_state.has_connection()
            || !razer_device::mouse_current_power::capability(self.spec.product_id)
                .is_some_and(|cap| cap.always_read_power_saving())
        {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        let Some(read) = self.current_power.saving.begin_read(scope) else {
            return;
        };
        self.current_power.sequence = self.current_power.sequence.wrapping_add(1);
        let request = MouseCurrentPowerRequested {
            scope,
            sequence: self.current_power.sequence,
            settings: json!({}),
            change: Some(Change::ReadPowerSaving),
            read: Some(read),
            edit_revision: None,
            valid: Arc::new(AtomicBool::new(true)),
        };
        if let Some(request) = self.current_power.enqueue(request) {
            cx.emit(request);
        }
    }
    pub fn observe_current_power_read(
        &mut self,
        read: &MouseCurrentPowerRead,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self
            .mouse_polling_scope(cx)
            .is_some_and(|scope| scope.same_device_connection(read.request.scope))
        {
            return;
        }
        // READY -> getPowerSavingFromDevice -> WDf is the source's explicit
        // getter-to-UI path. Setter confirmation never reaches this method.
        self.sync_current_power_controls(window, cx);
        cx.notify();
    }

    fn power_saving_value(&self) -> Option<f32> {
        if always_read_power_saving(self.spec.product_id) {
            self.current_power
                .saving
                .displayed_seconds()
                .map(|seconds| {
                    if self.spec.performance_power {
                        seconds
                    } else {
                        seconds / 60.
                    }
                })
        } else {
            self.draft
                .pointer(self.spec.power_path())
                .and_then(Value::as_f64)
                .map(|value| value as f32)
        }
    }

    pub(super) fn sync_current_power_controls(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(value) = self.power_saving_value() else {
            return;
        };
        self.syncing = true;
        if let Some(slider) = self.sliders.get(self.spec.power_path()) {
            slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
        }
        self.syncing = false;
    }

    pub(super) fn preview_current_power(&mut self, path: &str, value: f32, cx: &App) {
        if path == self.spec.power_path()
            && always_read_power_saving(self.spec.product_id)
            && let Some(scope) = self.mouse_polling_scope(cx)
        {
            self.current_power.saving.preview(
                scope,
                if self.spec.performance_power {
                    value
                } else {
                    value * 60.
                },
            );
        }
    }

    pub(super) fn cancel_current_power_preview(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.current_power.saving.cancel_preview();
        self.sync_current_power_controls(window, cx);
    }

    pub(super) fn has_observed_power_saving_control(&self, path: &str) -> bool {
        path == self.spec.power_path() && always_read_power_saving(self.spec.product_id)
    }

    pub(super) fn power_saving_submission_needed(&self, path: &str, value: &Value) -> bool {
        self.has_observed_power_saving_control(path)
            && self.power_saving_value().map(f64::from) != value.as_f64()
    }
    fn power_settings_snapshot(&self) -> Value {
        let mut source = self.draft.clone();
        self.snapshot_power(&mut source);
        let mut settings = serde_json::Map::new();
        for &key in device_fields(self.spec.product_id) {
            if let Some(value) = source.get(key) {
                settings.insert(key.into(), value.clone());
            }
        }
        Value::Object(settings)
    }

    pub(super) fn submit_current_power_field(&mut self, path: &str, cx: &mut Context<Self>) {
        let change = match path {
            "/powerSavingValue" | "/PowerSavingValue" => self
                .draft
                .pointer(path)
                .and_then(Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .map(Change::PowerSaving),
            "/lowPowerMode" => self
                .draft
                .pointer(path)
                .and_then(Value::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .map(Change::LowPower),
            "/lowBatteryEffects" => self
                .draft
                .pointer(path)
                .and_then(Value::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .map(Change::LowBattery),
            "/batteryType" => self
                .draft
                .pointer(path)
                .and_then(Value::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .map(Change::BatteryType),
            "/dimLighting/value" | "/dimLighting/isEnabled" => self
                .draft
                .pointer("/dimLighting/value")
                .and_then(Value::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .map(|minutes| Change::DimLighting {
                    enabled: self.boolean("/dimLighting/isEnabled"),
                    minutes,
                }),
            _ => None,
        };
        let Some(change) = change.filter(|change| {
            razer_device::mouse_current_power::validate_change(self.spec.product_id, change).is_ok()
        }) else {
            return;
        };
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        let edit_revision = if let Change::PowerSaving(value) = change {
            let seconds = if self.spec.performance_power {
                value
            } else {
                value * 60
            };
            Some(self.current_power.saving.edit(scope, seconds))
        } else {
            None
        };
        self.current_power.sequence = self.current_power.sequence.wrapping_add(1);
        let request = MouseCurrentPowerRequested {
            scope,
            sequence: self.current_power.sequence,
            settings: self.power_settings_snapshot(),
            change: Some(change),
            read: None,
            edit_revision,
            valid: Arc::new(AtomicBool::new(true)),
        };
        if let Some(request) = self.current_power.enqueue(request) {
            cx.emit(request);
        }
    }

    pub fn current_power_request_matches(
        &self,
        request: &MouseCurrentPowerRequested,
        cx: &App,
    ) -> bool {
        request.is_valid()
            && self
                .mouse_polling_scope(cx)
                .is_some_and(|scope| scope.same_device_connection(request.scope))
            && self.current_power.pending.as_ref().is_some_and(|pending| {
                pending.scope == request.scope && pending.sequence == request.sequence
            })
    }
    pub fn finish_current_power(
        &mut self,
        request: &MouseCurrentPowerRequested,
        observed: Option<Report>,
        cx: &mut Context<Self>,
    ) {
        if !self.current_power.pending.as_ref().is_some_and(|pending| {
            pending.scope == request.scope && pending.sequence == request.sequence
        }) {
            return;
        }
        if self.current_power_request_matches(request, cx) {
            if request.is_read() {
                let seconds = observed.as_ref().and_then(|report| report.time_to_sleep);
                if let Some(read) = request.read
                    && self.current_power.saving.finish_read(read, seconds)
                {
                    // The observation belongs to this entity, even if its
                    // window has closed. The retained subscriber only syncs UI.
                    cx.emit(MouseCurrentPowerRead { request: read });
                }
            } else if let (Some(revision), Some(seconds)) = (
                request.edit_revision,
                observed.as_ref().and_then(|report| report.time_to_sleep),
            ) {
                self.current_power
                    .saving
                    .confirm(request.scope, revision, seconds);
            }
            self.current_power.observed = observed;
        }
        if let Some(pending) = self.current_power.pending.take() {
            pending.cancel();
        }
        while let Some(next) = self.current_power.queued.pop_front() {
            if next.is_valid()
                && self
                    .mouse_polling_scope(cx)
                    .is_some_and(|scope| scope.same_device_connection(next.scope))
            {
                self.current_power.pending = Some(next.clone());
                cx.emit(next);
                break;
            }
            next.cancel();
        }
        // A connection replacement can leave its initialization behind
        // the cancelled worker. Recheck only after that serialization slot ends.
        if !self
            .mouse_polling_scope(cx)
            .is_some_and(|scope| scope.same_device_connection(request.scope))
        {
            self.request_current_power_read(cx);
        }
        cx.notify();
    }
    /// Drain already requested basic settings and cancel device work on exit.
    pub fn prepare_current_power_exit(&mut self, cx: &App) -> Option<MouseCurrentPowerRequested> {
        let latest = self
            .current_power
            .queued
            .iter()
            .rev()
            .chain(self.current_power.pending.iter())
            .find(|request| {
                request.is_valid() && request.requires_local_persistence(self.spec.product_id)
            })
            .cloned();
        self.current_power.cancel();
        self.current_power.pending = None;
        let mut latest = latest?;
        if !self
            .mouse_polling_scope(cx)
            .is_some_and(|scope| scope.same_device_connection(latest.scope))
        {
            return None;
        }
        self.current_power.sequence = self.current_power.sequence.wrapping_add(1);
        latest.sequence = self.current_power.sequence;
        latest.settings = self.power_settings_snapshot();
        latest.change = None;
        latest.valid = Arc::new(AtomicBool::new(true));
        self.current_power.pending = Some(latest.clone());
        Some(latest)
    }

    pub(super) fn seed_power_card_defaults(&mut self) {
        if self.spec.power_slider || self.spec.performance_power {
            // All 42 mounted mousePowerSavingReducer initial states use 5.
            // Profile templates can instead contain 7/15 or omit this field;
            // those are not the independent basic-device reducer default.
            self.draft["powerSavingValue"] = json!(5);
        }
        if self.spec.low_power_slider {
            // Current independent lowPowerModeReducer initial state is 30;
            // restored device basic settings are overlaid afterwards.
            self.draft["lowPowerMode"] = json!(30);
        }
        if self.low_battery_range().is_some() {
            // Current lowBatteryEffectReducer initial state lE is
            // {blinkLedPercentage:5}; restored device settings overlay it.
            self.draft["lowBatteryEffects"] = json!(5);
        }
        if matches!(self.spec.product_id, 117 | 2594) && self.draft.get("dimLighting").is_none() {
            // dimLightingReducer's JE: a UI initial value, never a device read.
            self.draft["dimLighting"] = json!({"isEnabled": false, "value": 2});
        }
        if matches!(self.spec.product_id, 184 | 224) && self.draft.get("batteryType").is_none() {
            self.draft["batteryType"] = json!(0);
        }
    }

    pub(super) fn power_saving_card(&self, cx: &Context<Self>) -> Div {
        let panel = surface::panel_with_help(
            t("POWER_SAVING_HEADER"),
            surface::help_control("mouse-power-saving-help", t("POWER_SAVING_TOOLTIP")),
            cx,
        );
        if !self.spec.performance_power {
            // Current PowerSaving's CD/801 branch: .h1-body then a 64px
            // .slider-container with the ordinary numeric bubble, min/max
            // tags and a 6px rail. The retained state owns the drag preview.
            return panel
                .child(surface::h1_body(t("POWER_SAVING_DESC"), cx))
                .child(self.power_slider(self.spec.power_path(), "1", "15", true, cx));
        }
        use razer_widgets::theme::MousePowerColors as Colors;
        let choices = [15u16, 30, 60, 120, 300, 600].into_iter().map(|seconds| {
            let label = if seconds >= 60 {
                razer_i18n::t_value("MIN", i64::from(seconds / 60))
            } else {
                razer_i18n::t_value("SEC", i64::from(seconds))
            };
            let selected = self.power_saving_value() == Some(f32::from(seconds));
            razer_widgets::keyboard_navigation::Button::new(("mouse-power-saving-seconds", u64::from(seconds)))
                .accessibility_label(label.clone())
                .selected(selected)
                .flex_shrink_0()
                .min_w(surface::css(78.))
                .px(surface::css(16.))
                .py(surface::css(7.))
                .rounded(surface::css(3.))
                .border_1()
                .border_color(if selected {
                    Colors::accent()
                } else {
                    Colors::border()
                })
                .bg(if selected {
                    Colors::selected_surface()
                } else {
                    Colors::surface()
                })
                .text_color(Colors::text())
                .text_size(surface::css(12.))
                .line_height(surface::css(14.))
                .hover(|style| style.border_color(Colors::accent()))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.write("/powerSavingValue", json!(seconds), cx)
                }))
        });
        panel
            .child(surface::h1_body(t("POWER_SAVING_DESC_V4"), cx))
            .child(
                h_flex()
                    .mt(surface::css(20.))
                    .gap(surface::css(10.))
                    .children(choices),
            )
    }

    fn power_slider(
        &self,
        path: &str,
        min_tag: &str,
        max_tag: &str,
        enabled: bool,
        cx: &Context<Self>,
    ) -> Div {
        let slider = &self.sliders[path];
        let state = slider.read(cx);
        let range = if self.has_observed_power_saving_control(path)
            && self
                .power_saving_value()
                .is_none_or(|value| state.value().start() != value)
        {
            unobserved_power_range(slider).into_any_element()
        } else {
            razer_widgets::source_slider::SourceSlider::new(slider, state.percentage().end)
                .tip(Some(state.value().start().to_string()))
                .enabled(enabled)
                .into_any_element()
        };
        div().relative().child(range).child(
            h_flex()
                .absolute()
                .bottom(surface::css(-2.))
                .w_full()
                .justify_between()
                .text_size(surface::css(14.))
                .when(!enabled, |labels| labels.opacity(0.3))
                .child(min_tag.to_owned())
                .child(max_tag.to_owned()),
        )
    }

    pub(super) fn low_power_card(&self, cx: &Context<Self>) -> Div {
        let enabled = self.low_power_enabled();
        // Four current mounted cards omit percent signs from their endpoints.
        let (minimum, maximum) = if matches!(self.spec.product_id, 226 | 229 | 231 | 45066) {
            ("5", "100")
        } else {
            ("5%", "100%")
        };
        surface::panel_with_help(
            t("LOW_POWER_MODE_HEADER"),
            surface::help_control("mouse-low-power-help", t("LOW_POWER_MODE_TOOLTIP")),
            cx,
        )
        .child(surface::h1_body(t("LOW_POWER_MODE_DESC"), cx))
        .child(self.power_slider("/lowPowerMode", minimum, maximum, enabled == Some(true), cx))
        .children(
            (enabled == Some(false))
                .then(|| div().mt(surface::css(15.)).child(t("LOW_POWER_MODE_WARN"))),
        )
    }

    /// Mounted LowBatteryEffects slider bounds; the device capability owns
    /// the DeviceInfo minLBE/maxLBE/StepLBE values used for validation.
    pub(super) fn low_battery_range(
        &self,
    ) -> Option<razer_device::mouse_current_power::LowBatteryRange> {
        self.spec
            .low_battery_slider
            .then(|| razer_device::mouse_current_power::capability(self.spec.product_id))
            .flatten()
            .and_then(|cap| cap.low_battery())
    }

    /// Current sA: fs{title G7P, tips kG1, hasSwitch:false}, .h1-body kM8 and
    /// the shared slider with numeric min/max tags (no percent sign).
    pub(super) fn low_battery_card(&self, cx: &Context<Self>) -> Option<Div> {
        let range = self.low_battery_range()?;
        Some(
            surface::panel_with_help(
                t("LOW_BATTERY_EFFECTS_HEADER"),
                surface::help_control("mouse-low-battery-help", t("LOW_BATTERY_EFFECTS_TOOLTIP")),
                cx,
            )
            .child(surface::h1_body(t("LOW_BATTERY_EFFECTS_DESC"), cx))
            .child(self.power_slider(
                "/lowBatteryEffects",
                &range.min.to_string(),
                &range.max.to_string(),
                true,
                cx,
            )),
        )
    }

    pub(super) fn power_dim_lighting(&self, cx: &Context<Self>) -> Div {
        let enabled = self.boolean("/dimLighting/isEnabled");
        surface::panel_with_title_switch(
            t("DIM_LIGHTING_HEADER"),
            surface::SynapseSwitch::new("mouse-power-dim-enabled")
                .accessibility_label(t("DIM_LIGHTING_HEADER"))
                .checked(enabled)
                .on_change(cx.listener(|this, next: &bool, _, cx| {
                    this.write("/dimLighting/isEnabled", json!(*next), cx);
                })),
            surface::help_control("mouse-power-dim-help", t("DIM_LIGHTING_TOOLTIP")),
            cx,
        )
        .when(self.spec.block_widget_columns(), |panel| panel.mb_0())
        .child(surface::h1_body(t("DIM_LIGHTING_DESC"), cx))
        .child(self.power_slider("/dimLighting/value", "1", "15", enabled, cx))
    }

    pub(super) fn power_battery_type(&self, cx: &Context<Self>) -> Div {
        let selected = self.number("/batteryType") as u32;
        // Current HNU preserves Alkaline=0, Lithium=2, Rechargeable_NiMh=1.
        let choices = [
            ("alkaline", 0, "BATTERY_TYPE_AKALINE"),
            ("lithium", 2, "BATTERY_TYPE_LITHIUM"),
            ("rechargeable-NiMh", 1, "BATTERY_TYPE_RECHARGEABLE_NIMH"),
        ]
        .map(|(id, value, label)| {
            div().pt(surface::css(17.)).child(
                gpui_kit::component::radio::Radio::new(id)
                    .label(t(label))
                    .checked(selected == value)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.write("/batteryType", json!(value), cx);
                    })),
            )
        });
        surface::panel_with_help(
            t("BATTERY_SELECTION"),
            surface::help_control("mouse-battery-type-help", t("BATTERY_SELECTION_TOOLTIP")),
            cx,
        )
        .when(self.spec.block_widget_columns(), |panel| panel.mb_0())
        .child(surface::h1_body(t("BATTERY_SELECTION_DESC"), cx))
        .child(v_flex().ml(surface::css(-20.)).children(choices))
    }
}

fn always_read_power_saving(product_id: u32) -> bool {
    razer_device::mouse_current_power::capability(product_id)
        .is_some_and(|cap| cap.always_read_power_saving())
}

/// The current pn action only calls Pf when !alwaysReadPowerSavingFromDevice.
/// Its taskMaker/taskRunner caches are host memory, never profile/disk drafts.
pub fn filter_power_persistence(product_id: u32, snapshot: &mut Value) {
    if let Some(snapshot) = snapshot.as_object_mut() {
        snapshot.remove("_mousePowerDraft");
        if always_read_power_saving(product_id) {
            snapshot.remove("powerSavingValue");
            snapshot.remove("PowerSavingValue");
        }
    }
}

fn restore_power_settings(product_id: u32, restored: bool, previous: &Value, draft: &mut Value) {
    if let Some(draft) = draft.as_object_mut() {
        draft.remove("_mousePowerDraft");
        if restored {
            // Device basic reducers survive profile selection, including an
            // optimistic value awaiting the serialized device task.
            for &key in device_fields(product_id) {
                if let Some(value) = previous.get(key) {
                    draft.insert(key.into(), value.clone());
                }
            }
        } else if always_read_power_saving(product_id) {
            draft.remove("PowerSavingValue");
            draft.insert("powerSavingValue".into(), json!(5));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(sequence: u64, change: Change) -> MouseCurrentPowerRequested {
        MouseCurrentPowerRequested {
            scope: MousePollingScope::new(EntityId::from(1), 2, 3),
            sequence,
            settings: json!({"powerSavingValue":7,"lowPowerMode":30}),
            change: Some(change),
            read: None,
            edit_revision: None,
            valid: Arc::new(AtomicBool::new(true)),
        }
    }
    #[::core::prelude::v1::test]
    fn initial_read_uses_actual_seconds_and_failure_remains_retryable() {
        let scope = MousePollingScope::new(EntityId::from(1), 2, 3);
        let mut value = PowerSavingValue::default();
        assert_eq!(value.displayed_seconds(), None);
        let first = value.begin_read(scope).unwrap();
        assert!(value.finish_read(first, None));
        assert_eq!(value.displayed_seconds(), None);
        let retry = value.begin_read(scope).unwrap();
        assert!(retry != first);
        assert!(!value.finish_read(first, Some(300)));
        assert!(value.finish_read(retry, Some(75)));
        assert_eq!(value.displayed_seconds(), Some(75.));
        assert!(value.begin_read(scope).is_none());
    }

    #[::core::prelude::v1::test]
    fn get_begun_before_drag_cannot_replace_preview_or_its_confirmed_write() {
        let scope = MousePollingScope::new(EntityId::from(1), 2, 3);
        let mut value = PowerSavingValue::default();
        let read = value.begin_read(scope).unwrap();
        value.preview(scope, 420.);
        assert_eq!(value.displayed_seconds(), Some(420.));
        let edit = value.edit(scope, 540);
        value.confirm(scope, edit, 540);
        assert!(value.finish_read(read, Some(75)));
        assert_eq!(value.displayed_seconds(), Some(540.));
    }

    #[::core::prelude::v1::test]
    fn get_failure_does_not_erase_newer_drag_or_unconfirmed_edit() {
        let scope = MousePollingScope::new(EntityId::from(1), 2, 3);
        let mut value = PowerSavingValue::default();
        let read = value.begin_read(scope).unwrap();
        value.preview(scope, 480.);
        assert!(value.finish_read(read, None));
        assert_eq!(value.displayed_seconds(), Some(480.));
        value.edit(scope, 660);
        let retry = value.begin_read(scope).unwrap();
        assert!(value.finish_read(retry, Some(75)));
        assert_eq!(value.displayed_seconds(), Some(660.));
    }

    #[::core::prelude::v1::test]
    fn same_connection_profile_switch_retains_hardware_but_reconnect_rejects_old_get() {
        let owner = EntityId::from(1);
        let mut value = PowerSavingValue::default();
        let read = value
            .begin_read(MousePollingScope::new(owner, 2, 3))
            .unwrap();
        value.retain_connection(MousePollingScope::new(owner, 4, 3));
        assert!(value.finish_read(read, Some(420)));
        assert_eq!(value.displayed_seconds(), Some(420.));
        value.invalidate();
        assert_eq!(value.displayed_seconds(), None);
        let reconnected = value
            .begin_read(MousePollingScope::new(owner, 4, 5))
            .unwrap();
        assert!(!value.finish_read(read, Some(300)));
        assert!(value.finish_read(reconnected, Some(75)));
        assert_eq!(value.displayed_seconds(), Some(75.));
    }

    #[::core::prelude::v1::test]
    fn replaced_read_identity_cannot_complete_its_successor() {
        let scope = MousePollingScope::new(EntityId::from(1), 2, 3);
        let mut value = PowerSavingValue::default();
        let old = value.begin_read(scope).unwrap();
        let new = value.begin_requery(scope).unwrap();
        assert!(!value.finish_read(old, Some(300)));
        assert_eq!(value.displayed_seconds(), None);
        assert!(value.finish_read(new, Some(75)));
        assert_eq!(value.displayed_seconds(), Some(75.));
    }

    #[::core::prelude::v1::test]
    fn older_write_confirmation_preserves_newer_edit_and_preview_cancellation() {
        let scope = MousePollingScope::new(EntityId::from(1), 2, 3);
        let mut value = PowerSavingValue::default();
        let first = value.edit(scope, 420);
        let second = value.edit(scope, 540);
        value.confirm(scope, first, 420);
        assert_eq!(value.displayed_seconds(), Some(540.));
        value.preview(scope, 660.);
        value.cancel_preview();
        assert_eq!(value.displayed_seconds(), Some(540.));
        value.confirm(scope, second, 540);
        assert_eq!(value.displayed_seconds(), Some(540.));
    }
    #[::core::prelude::v1::test]
    fn device_observation_and_optimistic_power_edits_never_enter_always_read_disk_storage() {
        let mut snapshot = json!({"powerSavingValue":1.25,"lowPowerMode":30,
            "_mousePowerDraft":{"powerSavingValue":7}});
        filter_power_persistence(117, &mut snapshot);
        assert_eq!(snapshot, json!({"lowPowerMode":30}));
        let mut edited = json!({"powerSavingValue":9,"lowPowerMode":30,
            "_mousePowerDraft":{"powerSavingValue":7}});
        filter_power_persistence(117, &mut edited);
        assert_eq!(edited, json!({"lowPowerMode":30}));
    }
    #[::core::prelude::v1::test]
    fn only_source_local_storage_branches_require_disk_before_submission() {
        for product_id in [117, 190, 204, 235] {
            assert!(!request(1, Change::ReadPowerSaving).requires_local_persistence(product_id));
            assert!(!request(1, Change::PowerSaving(9)).requires_local_persistence(product_id));
            assert!(request(1, Change::LowPower(30)).requires_local_persistence(product_id));
        }
        for product_id in [199, 208, 214, 229] {
            assert!(request(1, Change::PowerSaving(7)).requires_local_persistence(product_id));
            let mut snapshot =
                json!({"powerSavingValue":7, "_mousePowerDraft":{"powerSavingValue":9}});
            filter_power_persistence(product_id, &mut snapshot);
            assert_eq!(snapshot, json!({"powerSavingValue":7}));
        }
    }
    #[::core::prelude::v1::test]
    fn restore_ignores_stale_disk_power_but_preserves_device_reducers_across_profiles() {
        let previous = json!({"powerSavingValue":11, "lowPowerMode":30});
        let mut draft = json!({"powerSavingValue":8, "lowPowerMode":25, "dpi":1600,
            "_mousePowerDraft":{"powerSavingValue":9}});
        restore_power_settings(117, false, &previous, &mut draft);
        assert_eq!(
            draft,
            json!({"powerSavingValue":5, "lowPowerMode":25, "dpi":1600})
        );
        restore_power_settings(117, true, &previous, &mut draft);
        assert_eq!(
            draft,
            json!({"powerSavingValue":11, "lowPowerMode":30, "dpi":1600})
        );
    }
    #[::core::prelude::v1::test]
    fn device_power_initialization_survives_profile_selection_but_not_connection_replacement() {
        let owner = EntityId::from(1);
        let initialized = MousePollingScope::new(owner, 2, 3);
        assert!(initialized.same_device_connection(MousePollingScope::new(owner, 4, 3)));
        assert!(!initialized.same_device_connection(MousePollingScope::new(owner, 4, 5)));
        assert!(!initialized.same_device_connection(MousePollingScope::new(
            EntityId::from(2),
            2,
            3
        )));
    }
    #[::core::prelude::v1::test]
    fn dragged_field_is_coalesced_without_losing_a_distinct_queued_field() {
        let mut state = State::default();
        let pending = request(1, Change::BatteryType(0));
        assert!(state.enqueue(pending.clone()).is_some());
        let superseded = request(2, Change::PowerSaving(5));
        assert!(state.enqueue(superseded.clone()).is_none());
        assert!(state.enqueue(request(3, Change::LowPower(30))).is_none());
        assert!(state.enqueue(request(4, Change::PowerSaving(7))).is_none());
        assert!(!superseded.is_valid());
        assert!(pending.is_valid());
        assert_eq!(state.queued.len(), 2);
        assert_eq!(state.queued[0].change(), Some(&Change::PowerSaving(7)));
        assert_eq!(state.queued[1].change(), Some(&Change::LowPower(30)));
    }
    #[::core::prelude::v1::test]
    fn cancellation_invalidates_every_guard_and_retains_the_inflight_serialization_slot() {
        let mut state = State::default();
        let pending = request(1, Change::PowerSaving(5));
        let queued = request(2, Change::LowPower(30));
        state.enqueue(pending.clone());
        state.enqueue(queued.clone());
        state.cancel();
        assert!(!pending.is_valid());
        assert!(!queued.is_valid());
        assert!(state.queued.is_empty());
        assert!(state.pending.is_some());
        assert!(state.enqueue(request(3, Change::BatteryType(2))).is_none());
    }
    #[::core::prelude::v1::test]
    fn dropping_the_owner_cancels_service_guards() {
        let pending = request(1, Change::PowerSaving(5));
        let guard = pending.guard();
        let mut state = State::default();
        state.enqueue(pending);
        drop(state);
        assert!(!guard.load(Ordering::Acquire));
    }
}
