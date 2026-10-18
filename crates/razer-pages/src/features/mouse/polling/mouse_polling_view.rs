//! Current 226 wi/Xs/zs polling panel. Observations never become profile writes.
use super::*;
use crate::features::mouse_polling::source_spec;
use gpui_kit::base::Button as BaseButton;
use std::collections::BTreeSet;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[path = "mouse_polling_ingame.rs"]
mod ingame;
use ingame::InGamePolling;
pub use ingame::{MouseInGamePollingChanged, validate_in_game_polling};

pub(super) const LOCAL_FIELDS: &str = "_pollingLocalFieldsV1";

#[derive(Default)]
pub(super) struct State {
    runtime: super::super::mouse_polling::RuntimeState,
    local_fields: BTreeSet<String>,
    pending: Option<MousePollingRequested>,
    pending_valid: Option<Arc<AtomicBool>>,
    queued: Option<MousePollingRequested>,
    initialization_key: Option<String>,
    in_game: Option<InGamePolling>,
    in_game_error: Option<String>,
    running_games: Option<usize>,
}

impl State {
    fn cancel_pending(&mut self) {
        if let Some(guard) = &self.pending_valid {
            guard.store(false, Ordering::Release);
        }
        self.queued = None;
    }
    pub(super) fn invalidate_connection(&mut self) {
        self.cancel_pending();
        self.runtime
            .apply(MousePollingObservation::Connection(None));
    }

    pub(super) fn has_connection(&self) -> bool {
        self.runtime.connection.is_some()
    }

    pub(super) fn connection(&self) -> Option<PollingConnection> {
        self.runtime.connection
    }

    pub(super) fn field(&self) -> PollingField {
        self.runtime.field()
    }

    pub(super) fn edit_rate(&mut self, field: PollingField, hz: u32) {
        self.runtime.edit_rate(field, hz);
    }

    fn publish_rate(&mut self, field: PollingField, requested_hz: u32, observed_hz: u32) {
        self.runtime.confirm_rate(field, requested_hz, observed_hz);
    }

    fn selected_rate(&self, field: PollingField) -> Option<u32> {
        self.runtime.selected_rate(field)
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel_pending();
    }
}

#[derive(Clone, Copy)]
pub struct MousePollingRequested {
    scope: MousePollingScope,
    field: PollingField,
    hz: u32,
    initialization: bool,
    device_hz: u32,
    synchronize_profile: bool,
}
impl MousePollingRequested {
    fn matches(self, scope: MousePollingScope, field: PollingField, hz: u32) -> bool {
        self.scope == scope && self.field == field && self.hz == hz
    }
    pub fn is_initialization(self) -> bool {
        self.initialization
    }
    pub fn field(self) -> PollingField {
        self.field
    }
    pub fn scope(self) -> MousePollingScope {
        self.scope
    }
    pub fn hz(self) -> u32 {
        self.hz
    }
}
impl EventEmitter<MousePollingRequested> for MouseProductWorkspace {}

#[cfg(test)]
mod observation_tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn actual_get_replaces_the_profile_selection_without_rewriting_the_profile() {
        let mut state = State::default();
        let field = PollingField::Wired;
        let profile = Some(1000);
        state.local_fields.insert(field.key().into());
        assert_eq!(state.selected_rate(field), None);
        state.publish_rate(field, 1000, 2000);
        assert_eq!(state.selected_rate(field), Some(2000));
        assert_eq!(profile, Some(1000));
        assert!(state.local_fields.contains(field.key()));
    }

    #[::core::prelude::v1::test]
    fn old_get_does_not_overwrite_a_later_optimistic_edit() {
        let mut state = State::default();
        let field = PollingField::Wireless;
        state.runtime.edit_rate(field, 4000);
        state.publish_rate(field, 1000, 1000);
        assert_eq!(state.selected_rate(field), Some(4000));
        state.publish_rate(field, 4000, 4000);
        assert_eq!(state.runtime.edited_rate(field), None);
        assert_eq!(state.selected_rate(field), Some(4000));
        state.invalidate_connection();
        assert_eq!(state.selected_rate(field), None);
    }

    #[::core::prelude::v1::test]
    fn saved_local_fields_never_supply_an_unobserved_rate_after_reconnection() {
        let mut state = State::default();
        let field = PollingField::Wireless;
        state.local_fields.insert(field.key().into());
        state
            .runtime
            .apply(MousePollingObservation::Connection(Some(
                PollingConnection::Dongle,
            )));
        assert_eq!(state.selected_rate(field), None);
        state.publish_rate(field, 1000, 2000);
        assert_eq!(state.selected_rate(field), Some(2000));
        state.invalidate_connection();
        assert_eq!(state.selected_rate(field), None);
        assert!(state.local_fields.contains(field.key()));
        state
            .runtime
            .apply(MousePollingObservation::Connection(Some(
                PollingConnection::Dongle,
            )));
        assert_eq!(state.selected_rate(field), None);
        state.publish_rate(field, 1000, 8000);
        assert_eq!(state.selected_rate(field), Some(8000));
    }

    #[::core::prelude::v1::test]
    fn cancelling_pending_io_preserves_an_explicit_edit_until_connection_changes() {
        let mut state = State::default();
        let field = PollingField::Wired;
        state.local_fields.insert(field.key().into());
        state.runtime.edit_rate(field, 500);
        state.cancel_pending();
        assert_eq!(state.selected_rate(field), Some(500));
        assert!(!state.runtime.rates.contains_key(&field));
        state.invalidate_connection();
        assert_eq!(state.selected_rate(field), None);
        assert!(state.local_fields.contains(field.key()));
    }
}

fn polling_button(
    id: ElementId,
    rate: u32,
    selected: bool,
    disabled: bool,
    colored: bool,
) -> BaseButton {
    use razer_widgets::theme::MousePollingColors as Colors;
    BaseButton::new(id)
        .accessibility_label(format!("{rate} Hz"))
        .selected(selected)
        .disabled(disabled)
        .flex()
        .items_center()
        .justify_center()
        .w(surface::css(72.))
        .h(surface::css(27.))
        .p_0()
        .rounded(surface::css(3.))
        .text_size(surface::css(14.))
        .bg(Colors::surface())
        .text_color(Colors::text())
        .border_1()
        .border_color(if selected {
            Colors::selected()
        } else {
            Colors::border()
        })
        .hover(|style| style.border_color(Colors::selected()))
        .when(colored, |button| {
            button.when_some(
                super::receiver_indicator::polling_color(rate),
                |button, color| {
                    button.child(
                        div()
                            .size(surface::css(6.))
                            .mr(surface::css(4.))
                            .rounded_full()
                            .bg(rgb(color)),
                    )
                },
            )
        })
        .child(rate.to_string())
}

impl MouseProductWorkspace {
    pub fn invalidate_mouse_polling_connection(&mut self) {
        self.polling_state.invalidate_connection();
    }

    pub fn observe_mouse_polling_values(
        &mut self,
        values: Option<&razer_device::device_reads::DeviceReadValues>,
        cx: &mut Context<Self>,
    ) {
        let field = self.polling_state.runtime.field();
        self.polling_state
            .runtime
            .observe_rate(field, values.and_then(|values| values.polling_hz));
        cx.notify();
    }

    fn submit_polling(&mut self, request: MousePollingRequested, cx: &mut Context<Self>) {
        if self.polling_state.pending.is_some() {
            self.polling_state.queued = Some(request);
        } else {
            self.polling_state.pending = Some(request);
            self.polling_state.pending_valid = Some(Arc::new(AtomicBool::new(true)));
            cx.emit(request);
        }
    }
    /// Current182 90012.Mn and190 31468.Mn dispatch profile polling after the
    /// serial/profile initialization chain has selected the actual profile.
    /// A failed getter does not create a profile or trigger initialization.
    pub fn initialize_current_mouse_polling(
        &mut self,
        key: Option<&str>,
        guid: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(key) = key else {
            self.polling_state.initialization_key = None;
            return;
        };
        let key = format!("{key}:{guid}");
        if guid.is_empty() || self.polling_state.initialization_key.as_ref() == Some(&key) {
            return;
        }
        let connection = self.polling_state.connection();
        if !matches!(self.spec.product_id, 182 | 190)
            || !(connection == Some(PollingConnection::Wired)
                || self.spec.product_id == 190 && connection == Some(PollingConnection::Dongle))
        {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        let field = self.polling_state.runtime.field();
        let Some(hz) = self.profile_polling_value(field) else {
            return;
        };
        if !source_spec(self.spec.product_id)
            .is_some_and(|spec| self.polling_state.runtime.rate_choices(spec).contains(&hz))
        {
            return;
        }
        self.polling_state.initialization_key = Some(key);
        let device_hz = self.polling_state.in_game.as_ref().map_or(hz, |setting| {
            setting.rate_for_current_games(field, hz, self.polling_state.running_games)
        });
        self.submit_polling(
            MousePollingRequested {
                scope,
                field,
                hz,
                device_hz,
                initialization: true,
                synchronize_profile: self.spec.product_id == 190,
            },
            cx,
        );
    }
    pub(super) fn invalidate_legacy_connection(&mut self) {
        if self.spec.product_id == 70 {
            self.polling_state.cancel_pending();
            self.legacy_dpi.cancel();
            self.legacy_mapping.stop();
            self.dpi_editing_observed = None;
            self.polling_state
                .runtime
                .apply(MousePollingObservation::Connection(None));
            self.polling_state.queued = None;
        }
    }
    /// Current 70 yI/LI and main.dd229426.css polling widget. PID70 is wired;
    /// its mounted branch has no high-rate warning or in-game settings.
    pub(super) fn legacy_polling_panel(&self, cx: &Context<Self>) -> AnyElement {
        let value = self.polling_value(PollingField::Wired);
        surface::panel_with_help(
            t("WIRED_POLLING_RATE_HEADER"),
            surface::help_control("legacy-polling-help", t("WIRED_POLLING_RATE_V2_TOOLTIP")),
            cx,
        )
        .mb_0()
        .child(
            div()
                .mb(surface::css(10.))
                .text_color(rgb(0xcccccc))
                .child(t("POLLING_RATE_DESC")),
        )
        .child(
            h_flex()
                .flex_wrap()
                .gap(surface::css(10.))
                .children([125, 500, 1000].map(|hz| {
                    BaseButton::new(("legacy-polling", hz as usize))
                        .accessibility_label(format!("{hz} Hz"))
                        .selected(value == Some(hz))
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(surface::css(72.))
                        .h(surface::css(27.))
                        .p_0()
                        .rounded(surface::css(3.))
                        .text_size(surface::css(14.))
                        .bg(rgb(0x222222))
                        .text_color(rgb(0xcccccc))
                        .border_1()
                        .border_color(rgb(if value == Some(hz) { 0x44d62c } else { 0x5d5d5d }))
                        .hover(|style| style.border_color(rgb(0x44d62c)))
                        .child(hz.to_string())
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.choose_legacy_polling(hz, cx)),
                        )
                })),
        )
        .into_any_element()
    }
    /// Current kn profile reload dispatches ON_STAGES_INIT and ON_POLLING_RATE
    /// irrespective of the mounted tab. It does not consume device getters as
    /// profile values. The selected profile was persisted before this callback.
    pub fn apply_legacy_performance_profile(
        &mut self,
        scope: MousePollingScope,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 70 || self.mouse_polling_scope(cx) != Some(scope) {
            return;
        }
        self.submit_legacy_dpi_profile(true, cx);
        let hz = self.number(self.spec.polling_path()) as u32;
        if ![125, 500, 1000].contains(&hz) {
            return;
        }
        self.submit_polling(
            MousePollingRequested {
                scope,
                field: PollingField::Wired,
                hz,
                device_hz: hz,
                initialization: false,
                synchronize_profile: false,
            },
            cx,
        );
    }
    /// PID70 UI main.8f24b6a1:133060 preserves its optimistic value; MW
    /// 6120.7fff31f083f844383e26:92960 persists it before the skippable task.
    pub(super) fn choose_legacy_polling(&mut self, hz: u32, cx: &mut Context<Self>) {
        if self.spec.product_id != 70
            || !self.active
            || self.page != "TAB_PERFORMANCE"
            || ![125, 500, 1000].contains(&hz)
            || self.polling_value(PollingField::Wired) == Some(hz)
        {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        self.write(self.spec.polling_path(), json!(hz), cx);
        self.polling_state
            .runtime
            .edit_rate(PollingField::Wired, hz);
        self.submit_polling(
            MousePollingRequested {
                scope,
                field: PollingField::Wired,
                hz,
                device_hz: hz,
                initialization: false,
                synchronize_profile: false,
            },
            cx,
        );
    }

    pub fn mouse_polling_in_flight_matches(
        &self,
        scope: MousePollingScope,
        field: PollingField,
        hz: u32,
    ) -> bool {
        matches!(self.spec.product_id, 70 | 182 | 190)
            && self
                .polling_state
                .pending
                .is_some_and(|request| request.matches(scope, field, hz))
    }

    pub fn mouse_polling_device_hz(
        &self,
        scope: MousePollingScope,
        field: PollingField,
        hz: u32,
    ) -> Option<u32> {
        self.polling_state
            .pending
            .filter(|request| request.matches(scope, field, hz))
            .map(|request| request.device_hz)
    }
    pub fn mouse_polling_synchronizes_profile(
        &self,
        scope: MousePollingScope,
        field: PollingField,
        hz: u32,
    ) -> bool {
        self.polling_state
            .pending
            .is_some_and(|request| request.matches(scope, field, hz) && request.synchronize_profile)
    }

    pub fn mouse_polling_write_guard(
        &self,
        scope: MousePollingScope,
        field: PollingField,
        hz: u32,
    ) -> Option<Arc<AtomicBool>> {
        self.mouse_polling_in_flight_matches(scope, field, hz)
            .then(|| self.polling_state.pending_valid.clone())
            .flatten()
    }

    pub fn mouse_polling_request_matches(
        &self,
        scope: MousePollingScope,
        field: PollingField,
        hz: u32,
        cx: &App,
    ) -> bool {
        self.mouse_polling_scope(cx) == Some(scope)
            && self.mouse_polling_in_flight_matches(scope, field, hz)
            && self
                .polling_state
                .pending_valid
                .as_ref()
                .is_some_and(|guard| guard.load(Ordering::Acquire))
    }

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
        let pending = self
            .polling_state
            .pending
            .take()
            .expect("matched pending request");
        if let Some(guard) = self.polling_state.pending_valid.take() {
            guard.store(false, Ordering::Release);
        }
        if self.mouse_polling_scope(cx) == Some(scope) && observed == Some(pending.device_hz) {
            // The independent getter supplies the displayed device value.
            // An earlier completion must not replace a later queued edit.
            self.polling_state
                .publish_rate(field, pending.hz, pending.device_hz);
        }
        //85191.ot publishes Q.RA (wireless rate) when an ordinary master caps
        // a previous high-rate profile. This changes source UI state only.
        if self.spec.product_id == 182
            && hz > 1000
            && observed == Some(1000)
            && self.mouse_polling_scope(cx) == Some(scope)
        {
            self.polling_state
                .runtime
                .rates
                .insert(PollingField::Wireless, 1000);
            self.polling_state
                .local_fields
                .remove(PollingField::Wireless.key());
        }
        if let Some(next) = self.polling_state.queued.take() {
            if self.mouse_polling_scope(cx) == Some(next.scope) {
                self.submit_polling(next, cx);
            }
        }
        cx.notify();
        true
    }

    pub(super) fn advanced_enabled(&self) -> bool {
        static FLOORS: OnceLock<BTreeMap<String, String>> = OnceLock::new();
        let floors = FLOORS.get_or_init(|| read_mouse_advanced_catalog());
        floors
            .get(&self.spec.product_id.to_string())
            .is_none_or(|required| {
                super::super::mouse_polling::source_advanced_supported(
                    self.dynamic_state.firmware.as_deref().or(self
                        .polling_state
                        .runtime
                        .firmware
                        .as_deref()),
                    required,
                )
            })
    }
    pub(super) fn source_polling_visible(&self) -> bool {
        source_spec(self.spec.product_id)
            .is_some_and(|spec| self.polling_state.runtime.visible(spec))
    }
    pub fn mouse_polling_scope(&self, _cx: &App) -> Option<MousePollingScope> {
        let owner = self.polling_owner?;
        if self.spec.product_id != 70
            && razer_device::mouse_current_dpi::capability(self.spec.product_id).is_none()
            && razer_device::mouse_current_power::capability(self.spec.product_id).is_none()
        {
            source_spec(self.spec.product_id)?;
        }
        Some(MousePollingScope::new(
            owner,
            self.polling_state.runtime.profile_epoch,
            self.polling_state.runtime.connection_epoch,
        ))
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
        self.polling_state.runtime.apply(observation);
        if self.mouse_polling_scope(cx) != Some(scope) {
            self.polling_state.cancel_pending();
            self.legacy_dpi.cancel();
            self.current_dpi.cancel();
            self.current_power.cancel();
            self.scroll_task.cancel();
            self.cancel_calibration_requests();
            self.legacy_mapping.stop();
        }
        self.sync_receiver_indicator(cx);
        self.request_current_power_read(cx);
        // No Changed event, snapshot write, auto-downgrade or device command.
        cx.notify();
    }

    pub(super) fn restore_polling(&mut self, saved: Option<&Value>) {
        self.restore_in_game_polling(saved);
        let dpi_supported = self.spec.product_id == 70
            || razer_device::mouse_current_dpi::capability(self.spec.product_id).is_some();
        let polling_supported = source_spec(self.spec.product_id).is_some();
        let power_supported =
            razer_device::mouse_current_power::capability(self.spec.product_id).is_some();
        if !dpi_supported && !polling_supported && !power_supported {
            return;
        }
        if dpi_supported {
            self.polling_state.cancel_pending();
            self.legacy_dpi.cancel();
            self.current_dpi.cancel();
            self.polling_state.queued = None;
            // Retain the old worker until completion; no concurrent write.
        }
        self.polling_state.runtime.reset_profile();
        self.scroll_task.cancel();
        if !polling_supported {
            return;
        }
        self.polling_state.local_fields.clear();
        for field in [PollingField::Wired, PollingField::Wireless] {
            let Some(saved) = saved.filter(|saved| saved.get(field.key()).is_some()) else {
                continue;
            };
            let local = match saved.get(LOCAL_FIELDS) {
                Some(Value::Array(fields)) => fields.iter().any(|key| key == field.key()),
                None => true, // Legacy saved profile values remain local drafts.
                _ => false,
            };
            if local {
                self.polling_state.local_fields.insert(field.key().into());
            }
        }
        self.draft[LOCAL_FIELDS] = json!(self.polling_state.local_fields);
    }

    fn profile_polling_value(&self, field: PollingField) -> Option<u32> {
        let value = if self.spec.product_id == 70 {
            self.draft.pointer(self.spec.polling_path())
        } else {
            self.draft.get(field.key())
        }?;
        u32::try_from(value.as_u64()?).ok()
    }

    pub(super) fn polling_value(&self, field: PollingField) -> Option<u32> {
        self.polling_state.selected_rate(field)
    }

    /// Low-power gating uses this connection's observation or explicit edit.
    pub(super) fn effective_power_polling_rate(&self) -> Option<u32> {
        self.polling_value(self.polling_state.runtime.field())
    }

    fn choose_polling(
        &mut self,
        scope: MousePollingScope,
        field: PollingField,
        rate: u32,
        cx: &mut Context<Self>,
    ) {
        let spec = source_spec(self.spec.product_id).expect("current polling source");
        if self.mouse_polling_scope(cx) != Some(scope)
            || !self.active
            || self.page != "TAB_PERFORMANCE"
            || self.polling_state.runtime.connection == Some(PollingConnection::Ble)
            || self.polling_state.runtime.field() != field
            || self.polling_value(field) == Some(rate)
            || self
                .polling_state
                .runtime
                .limit(spec)
                .is_some_and(|limit| rate > limit.hz)
            || !self
                .polling_state
                .runtime
                .rate_choices(spec)
                .contains(&rate)
        {
            return;
        }
        self.draft[field.key()] = json!(rate);
        self.polling_state.runtime.edit_rate(field, rate);
        self.polling_state.local_fields.insert(field.key().into());
        self.draft[LOCAL_FIELDS] = json!(self.polling_state.local_fields);
        // Even if the old default draft equals rate, replacing a different
        // displayed observation is an explicit local edit and must persist.
        if matches!(self.spec.product_id, 182 | 190) {
            let device_hz = self.polling_state.in_game.as_ref().map_or(rate, |setting| {
                setting.rate_for_current_games(field, rate, self.polling_state.running_games)
            });
            self.submit_polling(
                MousePollingRequested {
                    scope,
                    field,
                    hz: rate,
                    device_hz,
                    initialization: false,
                    synchronize_profile: self.spec.product_id == 190,
                },
                cx,
            );
        }
        cx.emit(MouseProductChanged);
        cx.notify();
    }

    pub(super) fn source_polling_panel(&self, cx: &Context<Self>) -> AnyElement {
        if !self.source_polling_visible() {
            return div().into_any_element();
        }
        let field = self.polling_state.runtime.field();
        let wireless = field == PollingField::Wireless;
        let value = self.polling_value(field);
        let limit = self
            .polling_state
            .runtime
            .limit(source_spec(self.spec.product_id).expect("current polling source"));
        let scope = self.mouse_polling_scope(cx).expect("source polling scope");
        let rates = self
            .polling_state
            .runtime
            .rate_choices(source_spec(self.spec.product_id).expect("current polling source"));
        let mut panel = surface::panel_with_help(
            t(if wireless {
                "POLLING_RATE_HEADER"
            } else {
                "WIRED_POLLING_RATE_HEADER"
            }),
            surface::help_control(
                "source-polling-help",
                t(if wireless {
                    "POLLING_RATE_V2_TOOLTIP"
                } else {
                    "WIRED_POLLING_RATE_V2_TOOLTIP"
                }),
            ),
            cx,
        )
        .child(
            div()
                .mb(surface::css(10.))
                .text_color(rgb(0xcccccc))
                .child(t("POLLING_RATE_DESC")),
        )
        .child(
            h_flex()
                .flex_wrap()
                .gap(surface::css(10.))
                .children(rates.iter().map(|rate| {
                    let rate = *rate;
                    let disabled = limit.is_some_and(|limit| rate > limit.hz);
                    polling_button(
                        SharedString::from(format!("source-polling-{rate}")).into(),
                        rate,
                        value == Some(rate),
                        disabled,
                        self.spec.product_id == 190,
                    )
                    .when(disabled, |button| button.opacity(0.4))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.choose_polling(scope, field, rate, cx)
                    }))
                })),
        );
        if let Some(limit) = limit {
            panel = panel.child(
                h_flex()
                    .items_start()
                    .mt(surface::css(15.))
                    .text_color(rgb(0x999999))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .child(
                        img("synapse/polling-info.svg")
                            .size(surface::css(20.))
                            .mr(surface::css(5.))
                            .flex_shrink_0(),
                    )
                    .child(t(limit.label())),
            );
        }
        if limit.is_none()
            && value.is_some_and(|value| {
                value
                    > source_spec(self.spec.product_id)
                        .expect("current polling source")
                        .high_rate_threshold_hz
            })
        {
            panel = panel.child(
                div()
                    .mt(surface::css(10.))
                    .opacity(0.7)
                    .child(t(if wireless {
                        "POLLING_RATE_WARN"
                    } else {
                        "POLLING_RATE_WARN_NOBATTERY"
                    })),
            );
        }
        if self.spec.product_id == 190 {
            panel = panel.child(self.in_game_polling_panel(scope, field, rates, cx));
        }
        panel
            .when(self.spec.block_widget_columns(), |panel| panel.mb_0())
            .into_any_element()
    }
}

// Generated catalog reader: razer-pages/mouse_advanced_data.
fn read_mouse_advanced_catalog() -> BTreeMap<String, String> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_1, decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_MOUSE_ADVANCED_DATA,
        0,
        "d4561f32549c1fefa5bc9910110ca7991902290ab115878516ab9f247482a8a5",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod mouse_advanced_data_catalog_tests {
    #[test]
    fn packaged_catalog_mouse_advanced_data_decodes() {
        let _ = super::read_mouse_advanced_catalog();
    }
}
// End generated catalog reader: razer-pages/mouse_advanced_data.
