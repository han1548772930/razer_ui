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
    scope: MousePollingScope,
    // None is getPowerSavingFromDevice's source fallback, never a device read.
    seconds: Option<u16>,
}

#[derive(Clone)]
pub struct MouseCurrentPowerRequested {
    scope: MousePollingScope,
    sequence: u64,
    settings: Value,
    change: Option<Change>,
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
    initialized_scope: Option<MousePollingScope>,
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
            self.polling_state.invalidate_connection();
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
        if self
            .current_power
            .initialized_scope
            .is_some_and(|old| old.same_device_connection(scope))
        {
            return;
        }
        self.current_power.initialized_scope = Some(scope);
        self.current_power.sequence = self.current_power.sequence.wrapping_add(1);
        let request = MouseCurrentPowerRequested {
            scope,
            sequence: self.current_power.sequence,
            settings: json!({}),
            change: Some(Change::ReadPowerSaving),
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
            .is_some_and(|scope| scope.same_device_connection(read.scope))
        {
            return;
        }
        // READY -> getPowerSavingFromDevice -> WDf is the source's explicit
        // getter-to-UI path. Setter confirmation never reaches this method.
        let minutes = initialization_minutes(read.seconds);
        let path = self.spec.power_path();
        set_pointer(&mut self.draft, path, json!(minutes));
        self.syncing = true;
        if let Some(slider) = self.sliders.get(path) {
            slider.update(cx, |slider, cx| {
                slider.set_value(minutes as f32, window, cx)
            });
        }
        self.syncing = false;
        cx.notify();
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
        self.current_power.sequence = self.current_power.sequence.wrapping_add(1);
        let request = MouseCurrentPowerRequested {
            scope,
            sequence: self.current_power.sequence,
            settings: self.power_settings_snapshot(),
            change: Some(change),
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
                cx.emit(MouseCurrentPowerRead {
                    scope: request.scope,
                    seconds,
                });
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
        self.request_current_power_read(cx);
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
            let selected = self.draft.get("powerSavingValue").and_then(Value::as_u64)
                == Some(u64::from(seconds));
            gpui_kit::base::Button::new(("mouse-power-saving-seconds", u64::from(seconds)))
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
        div()
            .relative()
            .child(
                razer_widgets::source_slider::SourceSlider::new(slider, state.percentage().end)
                    .tip(Some(state.value().start().to_string()))
                    .enabled(enabled),
            )
            .child(
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
        .child(self.power_slider("/lowPowerMode", minimum, maximum, enabled, cx))
        .children((!enabled).then(|| div().mt(surface::css(15.)).child(t("LOW_POWER_MODE_WARN"))))
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

fn initialization_minutes(seconds: Option<u16>) -> f64 {
    // Current helper Z.kp returns timeToSleep / 60 without rounding; its catch
    // returns the default argument 5. The report remains None on that failure.
    seconds
        .map(|seconds| f64::from(seconds) / 60.)
        .unwrap_or(5.)
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
            valid: Arc::new(AtomicBool::new(true)),
        }
    }
    #[::core::prelude::v1::test]
    fn initialization_preserves_fractional_device_minutes_and_source_error_fallback() {
        assert_eq!(initialization_minutes(Some(75)), 1.25);
        assert_eq!(initialization_minutes(None), 5.);
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
