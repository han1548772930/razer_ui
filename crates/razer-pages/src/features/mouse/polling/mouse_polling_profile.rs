//! Current 182 OM -> dm/Rm. Runtime observations never modify local profiles.
use super::*;
use crate::features::mouse_polling::{
    MousePollingObservation, MousePollingScope, PollingConnection, PollingField,
};
use crate::features::mouse_polling::{RuntimeState, SourceSpec, source_spec};
use crate::features::mouse_products::{MousePowerSavingRead, PowerSavingValue};
use gpui_kit::base::Button as BaseButton;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct State {
    runtime: RuntimeState,
    source: Option<&'static SourceSpec>,
    profile: String,
    windows_icon: &'static str,
    idle: IdleState,
    polling_pending: Option<(MousePollingScope, PollingField, u32)>,
    polling_valid: Option<Arc<AtomicBool>>,
}
impl State {
    pub(super) fn new(product_id: u32) -> Self {
        Self {
            runtime: RuntimeState::default(),
            source: source_spec(product_id),
            profile: String::new(),
            idle: IdleState::default(),
            polling_pending: None,
            polling_valid: None,
            windows_icon: if razer_platform::system::is_windows_11() {
                "synapse/windows-11.svg"
            } else {
                "synapse/windows.svg"
            },
        }
    }
    pub(super) fn reset_profile(&mut self) {
        if let Some(guard) = &self.polling_valid {
            guard.store(false, Ordering::Release);
        }
        self.runtime.reset_profile();
        // A profile restore invalidates observations, not an in-flight worker.
        // Keep its slot until its real completion; no second write may race it.
    }
    pub(super) fn sync_profile(&mut self, profile: &str) {
        if self.profile != profile {
            self.profile = profile.to_owned();
            self.reset_profile();
        }
    }
    fn spec(&self) -> &SourceSpec {
        self.source.expect("current source polling adapter")
    }
    fn field(&self) -> PollingField {
        self.runtime.field()
    }
    fn rates(&self) -> &[u32] {
        self.runtime.rate_choices(self.spec())
    }
    fn has_hyperpolling_master(&self) -> bool {
        self.runtime.has_hyper_master(self.spec())
    }
}
impl Drop for State {
    fn drop(&mut self) {
        if let Some(guard) = &self.polling_valid {
            guard.store(false, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod profile_polling_observation_tests {
    use super::*;

    fn state() -> State {
        State {
            runtime: RuntimeState::default(),
            source: None,
            profile: String::new(),
            windows_icon: "synapse/windows.svg",
            idle: IdleState::default(),
            polling_pending: None,
            polling_valid: None,
        }
    }

    #[::core::prelude::v1::test]
    fn profile_change_invalidates_selection_without_releasing_the_worker_slot() {
        let mut state = state();
        state.sync_profile("first");
        let scope = MousePollingScope::new(EntityId::from(7), 1, 0);
        state.polling_pending = Some((scope, PollingField::Wired, 2000));
        let valid = Arc::new(AtomicBool::new(true));
        state.polling_valid = Some(valid.clone());
        state.runtime.edit_rate(PollingField::Wired, 2000);
        state.runtime.observe_rate(PollingField::Wired, Some(1000));
        state.sync_profile("second");
        assert_eq!(state.runtime.selected_rate(PollingField::Wired), None);
        assert!(!valid.load(Ordering::Acquire));
        assert!(state.polling_pending == Some((scope, PollingField::Wired, 2000)));
    }

    #[::core::prelude::v1::test]
    fn rereading_same_profile_preserves_an_explicit_edit_and_actual_confirmation() {
        let mut state = state();
        let field = PollingField::Wireless;
        state.sync_profile("current");
        state.runtime.edit_rate(field, 4000);
        state.runtime.observe_rate(field, None);
        state.sync_profile("current");
        assert_eq!(state.runtime.selected_rate(field), Some(4000));
        state.runtime.confirm_rate(field, 4000, 8000);
        assert_eq!(state.runtime.selected_rate(field), Some(8000));
        assert_eq!(state.runtime.edited_rate(field), None);
        state
            .runtime
            .apply(MousePollingObservation::Connection(None));
        assert_eq!(state.runtime.selected_rate(field), None);
    }
}

/// PID182 main installs alwaysReadPowerSavingFromDevice=true. Unlike polling,
/// 30525/pn does not persist this value in a profile or device localStorage.
/// UI Rt owns an optimistic session value; 81988/g retains the running task
/// and only the latest pending NORMAL_SKIPPABLE task of the same name.
struct IdleState {
    value: PowerSavingValue,
    pending: Option<(MousePollingScope, u8)>,
    pending_revision: Option<u64>,
    queued: Option<(MousePollingScope, u8)>,
    queued_revision: Option<u64>,
    valid: Option<Arc<AtomicBool>>,
}

impl Default for IdleState {
    fn default() -> Self {
        Self {
            value: PowerSavingValue::default(),
            pending: None,
            pending_revision: None,
            queued: None,
            queued_revision: None,
            valid: None,
        }
    }
}

impl IdleState {
    fn value(&self) -> Option<f32> {
        self.value.displayed_seconds().map(|seconds| seconds / 60.)
    }

    fn request(&mut self, scope: MousePollingScope, minutes: u8) {
        self.queued_revision = Some(self.value.edit(scope, u16::from(minutes) * 60));
        self.queued = Some((scope, minutes));
    }

    fn next(&mut self, scope: MousePollingScope) -> Option<(MousePollingScope, u8)> {
        if self.pending.is_some() {
            return None;
        }
        let next = self.queued.take()?;
        if !next.0.same_device_connection(scope) {
            self.queued_revision = None;
            return None;
        }
        self.pending = Some(next);
        self.pending_revision = self.queued_revision.take();
        self.valid = Some(Arc::new(AtomicBool::new(true)));
        Some(next)
    }

    fn cancel_connection(&mut self) {
        if let Some(guard) = &self.valid {
            guard.store(false, Ordering::Release);
        }
        self.queued = None;
        self.queued_revision = None;
        self.value.invalidate();
        // Keep the writer slot until its real completion/cleanup.
    }

    fn finish(&mut self, scope: MousePollingScope, minutes: u8) -> bool {
        if self.pending != Some((scope, minutes)) {
            return false;
        }
        self.pending = None;
        self.pending_revision = None;
        if let Some(guard) = self.valid.take() {
            guard.store(false, Ordering::Release);
        }
        // 85191/wt -> HID/$1T records completion/error, never rollback/getter UI.
        true
    }
}

impl Drop for IdleState {
    fn drop(&mut self) {
        self.cancel_connection();
    }
}

#[cfg(test)]
mod idle_tests {
    use super::*;

    fn scope(profile: u64, connection: u64) -> MousePollingScope {
        MousePollingScope::new(EntityId::from(7), profile, connection)
    }

    #[::core::prelude::v1::test]
    fn initial_getter_uses_native_seconds_without_a_failure_default() {
        let mut state = IdleState::default();
        assert_eq!(state.value(), None);
        let read = state.value.begin_requery(scope(0, 0)).unwrap();
        assert!(state.value.finish_read(read, Some(420)));
        assert_eq!(state.value(), Some(7.));
        // A profile change is not a new device initialization.
        assert!(state.value.begin_requery(scope(1, 0)).is_none());
        assert_eq!(state.value(), Some(7.));
        state.cancel_connection();
        let read = state.value.begin_requery(scope(1, 1)).unwrap();
        assert!(state.value.finish_read(read, None));
        assert_eq!(state.value(), None);
        let read = state.value.begin_requery(scope(1, 1)).unwrap();
        assert!(state.value.finish_read(read, Some(75)));
        assert_eq!(state.value(), Some(1.25));
    }

    #[::core::prelude::v1::test]
    fn repeated_input_keeps_running_writer_and_only_latest_pending_value() {
        let mut state = IdleState::default();
        let current = scope(0, 0);
        state.request(current, 2);
        assert!(state.next(current) == Some((current, 2)));
        let guard = state.valid.clone().unwrap();
        state.request(current, 7);
        state.request(current, 11);
        assert!(state.next(current).is_none());
        assert!(guard.load(Ordering::Acquire));
        assert_eq!(state.value(), Some(11.));
        assert!(state.finish(current, 2));
        assert!(!guard.load(Ordering::Acquire));
        assert!(state.next(current) == Some((current, 11)));
        // An older completion cannot release the replacement worker.
        assert!(!state.finish(current, 2));
        assert!(state.pending == Some((current, 11)));
    }

    #[::core::prelude::v1::test]
    fn profile_change_preserves_idle_but_disconnect_cancels_without_early_release() {
        let mut state = IdleState::default();
        state.request(scope(0, 0), 4);
        assert!(state.next(scope(1, 0)).is_some());
        let guard = state.valid.clone().unwrap();
        state.request(scope(1, 0), 8);
        state.cancel_connection();
        assert!(!guard.load(Ordering::Acquire));
        assert!(state.queued.is_none());
        assert!(state.pending.is_some());
        state.request(scope(2, 1), 9);
        assert!(state.next(scope(2, 1)).is_none());
        assert!(state.finish(scope(0, 0), 4));
        assert!(state.next(scope(2, 1)) == Some((scope(2, 1), 9)));
    }

    #[::core::prelude::v1::test]
    fn failure_and_cleanup_do_not_rollback_optimistic_value() {
        let mut state = IdleState::default();
        let current = scope(0, 0);
        state.request(current, 13);
        state.next(current);
        assert!(state.finish(current, 13));
        assert_eq!(state.value(), Some(13.));
        state.request(current, 6);
        state.next(current);
        let guard = state.valid.clone().unwrap();
        drop(state);
        assert!(!guard.load(Ordering::Acquire));
    }
}

impl DeviceWorkspace {
    pub fn begin_mouse_idle_read(
        &mut self,
        scope: MousePollingScope,
        cx: &App,
    ) -> Option<MousePowerSavingRead> {
        if self.device.product_id != 182
            || self.mouse_polling.runtime.connection.is_none()
            || !self
                .mouse_polling_scope(cx)
                .is_some_and(|current| current.same_device_connection(scope))
        {
            return None;
        }
        self.mouse_polling.idle.value.begin_requery(scope)
    }

    pub(super) fn preview_mouse_idle(&mut self, minutes: f32, cx: &App) {
        if self.device.product_id == 182
            && let Some(scope) = self.mouse_polling_scope(cx)
        {
            self.mouse_polling.idle.value.preview(scope, minutes * 60.);
        }
    }

    pub(in crate::features) fn cancel_mouse_idle_preview(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mouse_polling.idle.value.cancel_preview();
        if self.device.product_id == 182
            && let Some(value) = self.mouse_polling.idle.value()
        {
            self.controls.sliders[&Control::Idle]
                .update(cx, |slider, cx| slider.set_value(value, window, cx));
        }
        cx.notify();
    }

    pub(super) fn observe_mouse_polling_values(
        &mut self,
        values: Option<&razer_device::device_reads::DeviceReadValues>,
    ) {
        let field = self.mouse_polling.field();
        self.mouse_polling
            .runtime
            .observe_rate(field, values.and_then(|values| values.polling_hz));
    }

    pub(super) fn invalidate_mouse_polling_connection(&mut self) {
        if let Some(guard) = &self.mouse_polling.polling_valid {
            guard.store(false, Ordering::Release);
        }
        self.mouse_polling
            .runtime
            .apply(MousePollingObservation::Connection(None));
        self.mouse_polling.idle.cancel_connection();
    }

    /// Current UI intents share the original device task category's write slot.
    /// This guards this retained page only; it is not an interprocess HID lock.
    pub(in crate::features) fn mouse_settings_write_pending(&self) -> bool {
        self.mouse_polling.idle.pending.is_some()
            || self.mouse_polling.polling_pending.is_some()
            || self.mouse_dpi_write_pending()
    }

    fn mouse_polling_pending(&self) -> bool {
        self.mouse_polling.polling_pending.is_some()
    }

    fn request_mouse_polling(&mut self, field: PollingField, hz: u32, cx: &mut Context<Self>) {
        // The mounted caller and OBM branch are audited for 182. Other pages
        // retain local drafts until their own submission chains are established.
        if self.device.product_id != 182
            || self.mouse_settings_write_pending()
            || self.mouse_dpi_read_pending()
        {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        if self.mouse_polling.field() != field || !self.mouse_polling.rates().contains(&hz) {
            return;
        }
        self.mouse_polling.polling_pending = Some((scope, field, hz));
        self.mouse_polling.polling_valid = Some(Arc::new(AtomicBool::new(true)));
        cx.emit(WorkspaceEvent::MousePollingRequested { scope, field, hz });
        cx.notify();
    }

    pub fn mouse_polling_request_matches(
        &self,
        scope: MousePollingScope,
        field: PollingField,
        hz: u32,
        cx: &App,
    ) -> bool {
        self.mouse_polling_scope(cx) == Some(scope)
            && self.mouse_polling.field() == field
            && self.mouse_polling_in_flight_matches(scope, field, hz)
            && self
                .mouse_polling
                .polling_valid
                .as_ref()
                .is_some_and(|guard| guard.load(Ordering::Acquire))
    }

    pub fn mouse_polling_write_guard(
        &self,
        scope: MousePollingScope,
        field: PollingField,
        hz: u32,
    ) -> Option<Arc<AtomicBool>> {
        self.mouse_polling_in_flight_matches(scope, field, hz)
            .then(|| self.mouse_polling.polling_valid.clone())
            .flatten()
    }

    pub fn mouse_polling_in_flight_matches(
        &self,
        scope: MousePollingScope,
        field: PollingField,
        hz: u32,
    ) -> bool {
        self.device.product_id == 182
            && self.mouse_polling.polling_pending == Some((scope, field, hz))
    }

    /// Release the retained worker; only its exact current readback is observed.
    pub fn finish_mouse_polling(
        &mut self,
        scope: MousePollingScope,
        field: PollingField,
        hz: u32,
        observed: Option<u32>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.mouse_polling_in_flight_matches(scope, field, hz) {
            return false;
        }
        let scope_current = self.mouse_polling_scope(cx) == Some(scope);
        self.mouse_polling.polling_pending = None;
        if let Some(guard) = self.mouse_polling.polling_valid.take() {
            guard.store(false, Ordering::Release);
        }
        if let Some(observed_hz) = observed.filter(|rate| scope_current && *rate == hz) {
            self.mouse_polling
                .runtime
                .confirm_rate(field, hz, observed_hz);
            if field == PollingField::Wired {
                let mut values = self
                    .device
                    .dashboard
                    .readonly_values
                    .clone()
                    .unwrap_or_default();
                values.polling_hz = Some(observed_hz);
                values.errors.remove("polling");
                self.device.observe_read_values(Some(values.clone()));
                self.saved.observe_read_values(Some(values));
            }
        }
        self.dispatch_queued_mouse_dpi(cx);
        cx.notify();
        true
    }

    pub(super) fn request_mouse_idle(&mut self, minutes: u8, cx: &mut Context<Self>) {
        // Only this adapter's current mounted slider/caller was re-audited.
        // Other product fields cannot inherit the 182 conversion by similarity.
        if self.device.product_id != 182 || !(1..=15).contains(&minutes) {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        self.mouse_polling.idle.request(scope, minutes);
        self.dispatch_queued_mouse_idle(cx);
        cx.notify();
    }

    pub(in crate::features) fn dispatch_queued_mouse_idle(&mut self, cx: &mut Context<Self>) {
        if self.mouse_settings_write_pending() || self.mouse_dpi_read_pending() {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        if let Some((scope, minutes)) = self.mouse_polling.idle.next(scope) {
            cx.emit(WorkspaceEvent::MouseIdleRequested { scope, minutes });
        }
    }

    pub fn observe_mouse_idle_initial(
        &mut self,
        read: MousePowerSavingRead,
        raw: Option<u16>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.device.product_id == 182
            && self
                .mouse_polling_scope(cx)
                .is_some_and(|current| current.same_device_connection(read.scope()))
            && self.mouse_polling.idle.value.finish_read(read, raw)
        {
            if let Some(value) = self.mouse_polling.idle.value() {
                self.controls.sliders[&Control::Idle]
                    .update(cx, |slider, cx| slider.set_value(value, window, cx));
            }
            cx.notify();
        }
    }

    pub(in crate::features) fn mouse_idle_value(&self) -> Option<f32> {
        if self.device.product_id == 182 {
            self.mouse_polling.idle.value()
        } else {
            Some(f32::from(self.settings().idle_minutes))
        }
    }

    pub fn mouse_idle_write_guard(
        &self,
        scope: MousePollingScope,
        minutes: u8,
    ) -> Option<Arc<AtomicBool>> {
        self.mouse_idle_in_flight_matches(scope, minutes)
            .then(|| self.mouse_polling.idle.valid.clone())
            .flatten()
    }

    pub fn mouse_idle_request_matches(
        &self,
        scope: MousePollingScope,
        minutes: u8,
        cx: &App,
    ) -> bool {
        self.device.product_id == 182
            && self
                .mouse_polling_scope(cx)
                .is_some_and(|current| current.same_device_connection(scope))
            && self.mouse_idle_in_flight_matches(scope, minutes)
            && self
                .mouse_polling
                .idle
                .valid
                .as_ref()
                .is_some_and(|guard| guard.load(Ordering::Acquire))
    }

    /// Match the worker token even after a profile/connection invalidates its observations.
    pub fn mouse_idle_in_flight_matches(&self, scope: MousePollingScope, minutes: u8) -> bool {
        self.device.product_id == 182 && self.mouse_polling.idle.pending == Some((scope, minutes))
    }

    /// Complete only the retained request; a failed/old reply never becomes an observation.
    pub fn finish_mouse_idle(
        &mut self,
        scope: MousePollingScope,
        minutes: u8,
        observed: Option<u16>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.mouse_idle_in_flight_matches(scope, minutes) {
            return false;
        }
        let scope_current = self.mouse_idle_request_matches(scope, minutes, cx);
        if let (Some(raw), Some(revision)) = (
            observed.filter(|raw| scope_current && *raw == u16::from(minutes) * 60),
            self.mouse_polling.idle.pending_revision,
        ) {
            self.mouse_polling.idle.value.confirm(scope, revision, raw);
        }
        self.mouse_polling.idle.finish(scope, minutes);
        if let Some(raw_time) =
            observed.filter(|raw| scope_current && *raw == u16::from(minutes) * 60)
        {
            let mut values = self
                .device
                .dashboard
                .readonly_values
                .clone()
                .unwrap_or_default();
            values.idle_raw_time = Some(raw_time);
            values.errors.remove("idle");
            self.device.observe_read_values(Some(values.clone()));
            self.saved.observe_read_values(Some(values));
        }
        self.dispatch_queued_mouse_dpi(cx);
        cx.notify();
        true
    }

    pub fn mouse_polling_scope(&self, cx: &App) -> Option<MousePollingScope> {
        self.mouse_polling.source.map(|_| {
            MousePollingScope::new(
                // The stable retained entity plus two independent generations scope
                // connection and profile reads. Neither is a persisted device ID.
                self.mouse_owner(cx),
                self.mouse_polling.runtime.profile_epoch,
                self.mouse_polling.runtime.connection_epoch,
            )
        })
    }
    fn mouse_owner(&self, cx: &App) -> EntityId {
        // Stored by the retained Controls entity, unique to this workspace.
        let _ = cx;
        self.controls.profile.entity_id()
    }
    pub fn observe_mouse_polling(
        &mut self,
        scope: MousePollingScope,
        observation: MousePollingObservation,
        cx: &mut Context<Self>,
    ) {
        if self.mouse_polling_scope(cx) != Some(scope) {
            return;
        }
        // Connection changes advance the observation epoch. They cannot stop
        // an already dispatched worker or release its write-serialization slot.
        self.mouse_polling.runtime.apply(observation);
        if self.mouse_polling_scope(cx) != Some(scope)
            && let Some(guard) = &self.mouse_polling.polling_valid
        {
            guard.store(false, Ordering::Release);
        }
        if !self
            .mouse_polling_scope(cx)
            .is_some_and(|current| current.same_device_connection(scope))
        {
            self.mouse_polling.idle.cancel_connection();
        }
        self.sync_dpi_scope(cx);
        cx.notify();
    }
    pub(in crate::features) fn mouse_polling_visible(&self) -> bool {
        self.mouse_polling
            .source
            .is_none_or(|source| self.mouse_polling.runtime.visible(source))
    }
    pub(in crate::features) fn mouse_windows_icon(&self) -> &'static str {
        self.mouse_polling.windows_icon
    }
    pub(in crate::features) fn mouse_polling_value(&self, field: PollingField) -> Option<u32> {
        self.mouse_polling.runtime.selected_rate(field)
    }

    pub(in crate::features) fn edit_mouse_polling_value(&mut self, field: PollingField, hz: u32) {
        self.mouse_polling.runtime.edit_rate(field, hz);
    }
    pub(in crate::features) fn mouse_low_power_enabled(&self) -> Option<bool> {
        let field = if matches!(
            self.mouse_polling.runtime.connection,
            Some(PollingConnection::Dongle | PollingConnection::Ble)
        ) {
            PollingField::Wireless
        } else {
            PollingField::Wired
        };
        let threshold = self
            .mouse_polling
            .source
            .map_or(1000, |source| source.high_rate_threshold_hz);
        self.mouse_polling_value(field).map(|hz| hz <= threshold)
    }
    pub(in crate::features) fn mouse_polling_panel(&self, cx: &Context<Self>) -> AnyElement {
        let field = self.mouse_polling.field();
        let wireless = field == PollingField::Wireless;
        let value = self.mouse_polling_value(field);
        let scope = self.mouse_polling_scope(cx).expect("profile polling scope");
        let t = razer_i18n::t;
        let mut panel = surface::panel_with_help(
            t(if wireless {
                "POLLING_RATE_HEADER"
            } else {
                "WIRED_POLLING_RATE_HEADER"
            }),
            surface::help_control(
                "profile-polling-help",
                t(if wireless {
                    "POLLING_RATE_V2_TOOLTIP"
                } else {
                    "WIRED_POLLING_RATE_V2_TOOLTIP"
                }),
            ),
            cx,
        )
        .child(surface::h1_body(t("POLLING_RATE_DESC"), cx))
        .child(h_flex().gap(surface::css(10.)).flex_wrap().children(
            self.mouse_polling.rates().iter().map(|rate| {
                let rate = *rate;
                BaseButton::new(SharedString::from(format!("polling-{rate}")))
                    .accessibility_label(format!("{rate} Hz"))
                    .child(rate.to_string())
                    .selected(value == Some(rate))
                    .disabled(self.mouse_settings_write_pending())
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(surface::css(72.))
                    .h(surface::css(27.))
                    .p_0()
                    .text_size(surface::css(14.))
                    .rounded(surface::css(3.))
                    .bg(rgb(0x222222))
                    .text_color(rgb(0xcccccc))
                    .border_1()
                    .border_color(if value == Some(rate) {
                        rgb(0x44d62c)
                    } else {
                        rgb(0x5d5d5d)
                    })
                    .hover(|s| s.border_color(rgb(0x44d62c)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if this.mouse_polling_scope(cx) != Some(scope)
                            || this.page != Tab::Performance
                            || !this.mouse_polling_visible()
                            || this.mouse_polling.field() != field
                            || !this.mouse_polling.rates().contains(&rate)
                            || this.mouse_settings_write_pending()
                        {
                            return;
                        }
                        this.edit_mouse_polling_value(field, rate);
                        this.edit(window, cx, |settings| match field {
                            PollingField::Wired => {
                                settings.polling_wired = Some(rate);
                                settings.polling = rate;
                            }
                            PollingField::Wireless => settings.polling_wireless = Some(rate),
                        });
                        this.request_mouse_polling(field, rate, cx);
                    }))
            }),
        ))
        .when(
            value.is_some_and(|hz| hz > self.mouse_polling.spec().high_rate_threshold_hz),
            |panel| {
                panel.child(
                    div()
                        .mt(surface::css(10.))
                        .opacity(0.7)
                        .child(t(if wireless {
                            "POLLING_RATE_WARN"
                        } else {
                            "POLLING_RATE_WARN_NOBATTERY"
                        })),
                )
            },
        );
        let edited = self.mouse_polling.runtime.edited_rate(field);
        if self.mouse_polling_pending() {
            panel = panel.child(surface::note("正在提交回报率并等待设备回读确认。", cx));
        } else if self.mouse_settings_write_pending() {
            panel = panel.child(surface::note(
                "当前设备设置正在提交，完成后可修改回报率。",
                cx,
            ));
        } else if self.mouse_polling.runtime.connection.is_none() {
            panel = panel.child(surface::note("连接状态尚未读取，设备回报率尚未确认。", cx));
        } else if edited.is_some() {
            panel = panel.child(surface::note("当前显示本次编辑；设备尚未确认。", cx));
        } else if self.mouse_polling.runtime.rates.contains_key(&field) {
            panel = panel.child(surface::note(
                "当前选择已由设备回读确认；本地草稿单独保存。",
                cx,
            ));
        } else {
            panel = panel.child(surface::note("设备回报率尚未读取。", cx));
        }
        if wireless && self.mouse_polling.runtime.topology.is_none() {
            panel = panel.child(surface::note(
                "HyperPolling 配对能力尚未读取，当前显示基础档位。",
                cx,
            ));
        } else if wireless
            && self.mouse_polling.has_hyperpolling_master()
            && self.mouse_polling.runtime.firmware.is_none()
        {
            panel = panel.child(surface::note(
                "设备固件版本尚未读取，8,000 Hz 支持尚未确认。",
                cx,
            ));
        }
        panel.into_any_element()
    }
}
