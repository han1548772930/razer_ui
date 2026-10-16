//! Current 226 wi/Xs/zs polling panel. Observations never become profile writes.
use super::*;
use crate::features::mouse_polling::source_spec;
use gpui_kit::base::Button as BaseButton;
use std::collections::BTreeSet;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) const LOCAL_FIELDS: &str = "_pollingLocalFieldsV1";

#[derive(Default)]
pub(super) struct State {
    runtime: super::super::mouse_polling::RuntimeState,
    local_fields: BTreeSet<String>,
    pending: Option<(MousePollingScope, PollingField, u32)>,
    pending_valid: Option<Arc<AtomicBool>>,
    queued: Option<(MousePollingScope, PollingField, u32, bool)>,
    initialization_key: Option<String>,
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
}
impl MousePollingRequested {
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

impl MouseProductWorkspace {
    /// Current90012.Mn applies the stored wired rate. Its wireless branch
    /// reloads the actual rate and breaks before ON_POLLING_RATE.
    pub(super) fn initialize_mouse182_polling(
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
        if self.spec.product_id != 182
            || self.polling_state.connection() != Some(PollingConnection::Wired)
        {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        let hz = self.number(self.spec.polling_path()) as u32;
        if ![125, 500, 1000].contains(&hz) {
            return;
        }
        self.polling_state.initialization_key = Some(key);
        if self.polling_state.pending.is_some() {
            self.polling_state.queued = Some((scope, PollingField::Wired, hz, true));
        } else {
            self.polling_state.pending = Some((scope, PollingField::Wired, hz));
            self.polling_state.pending_valid = Some(Arc::new(AtomicBool::new(true)));
            cx.emit(MousePollingRequested {
                scope,
                field: PollingField::Wired,
                hz,
                initialization: true,
            });
        }
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
        let value = self.number(self.spec.polling_path()) as u32;
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
                        .selected(value == hz)
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
                        .border_color(rgb(if value == hz { 0x44d62c } else { 0x5d5d5d }))
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
        if self.polling_state.pending.is_some() {
            self.polling_state.queued = Some((scope, PollingField::Wired, hz, false));
        } else {
            self.polling_state.pending = Some((scope, PollingField::Wired, hz));
            self.polling_state.pending_valid = Some(Arc::new(AtomicBool::new(true)));
            cx.emit(MousePollingRequested {
                scope,
                field: PollingField::Wired,
                hz,
                initialization: false,
            });
        }
    }
    /// PID70 UI main.8f24b6a1:133060 preserves its optimistic value; MW
    /// 6120.7fff31f083f844383e26:92960 persists it before the skippable task.
    pub(super) fn choose_legacy_polling(&mut self, hz: u32, cx: &mut Context<Self>) {
        if self.spec.product_id != 70
            || !self.active
            || self.page != "TAB_PERFORMANCE"
            || ![125, 500, 1000].contains(&hz)
            || self.number(self.spec.polling_path()) as u32 == hz
        {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        self.write(self.spec.polling_path(), json!(hz), cx);
        if self.polling_state.pending.is_some() {
            self.polling_state.queued = Some((scope, PollingField::Wired, hz, false));
        } else {
            self.polling_state.pending = Some((scope, PollingField::Wired, hz));
            self.polling_state.pending_valid = Some(Arc::new(AtomicBool::new(true)));
            cx.emit(MousePollingRequested {
                scope,
                field: PollingField::Wired,
                hz,
                initialization: false,
            });
        }
    }

    pub fn mouse_polling_in_flight_matches(
        &self,
        scope: MousePollingScope,
        field: PollingField,
        hz: u32,
    ) -> bool {
        matches!(self.spec.product_id, 70 | 182)
            && self.polling_state.pending == Some((scope, field, hz))
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
        self.polling_state.pending = None;
        if let Some(guard) = self.polling_state.pending_valid.take() {
            guard.store(false, Ordering::Release);
        }
        if self.mouse_polling_scope(cx) == Some(scope) && observed == Some(hz) {
            // Device diagnostics remain separate from the optimistic profile.
            self.polling_state.runtime.rates.insert(field, hz);
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
        if let Some((next_scope, next_field, next_hz, initialization)) =
            self.polling_state.queued.take()
        {
            if self.mouse_polling_scope(cx) == Some(next_scope) {
                self.polling_state.pending = Some((next_scope, next_field, next_hz));
                self.polling_state.pending_valid = Some(Arc::new(AtomicBool::new(true)));
                cx.emit(MousePollingRequested {
                    scope: next_scope,
                    field: next_field,
                    hz: next_hz,
                    initialization,
                });
            }
        }
        cx.notify();
        true
    }

    pub(super) fn advanced_enabled(&self) -> bool {
        static FLOORS: OnceLock<BTreeMap<String, String>> = OnceLock::new();
        let floors = FLOORS.get_or_init(|| {
            serde_json::from_str(include_str!("../mouse_advanced_data.json"))
                .expect("validated current advanced firmware floors")
        });
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
            self.legacy_mapping.stop();
        }
        self.sync_receiver_indicator(cx);
        self.request_current_power_read(cx);
        // No Changed event, snapshot write, auto-downgrade or device command.
        cx.notify();
    }

    pub(super) fn restore_polling(&mut self, saved: Option<&Value>) {
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

    fn polling_value(&self, field: PollingField) -> u32 {
        if !self.polling_state.local_fields.contains(field.key()) {
            if let Some(value) = self.polling_state.runtime.rates.get(&field) {
                return *value;
            }
        }
        self.draft[field.key()].as_u64().unwrap_or(0) as u32
    }

    /// Low-power gating uses the displayed connection's rate, including local
    /// edits. A missing wireless value falls back to wired as the source does.
    pub(super) fn effective_power_polling_rate(&self) -> u32 {
        let field = self.polling_state.runtime.field();
        if field == PollingField::Wireless
            && !self.polling_state.local_fields.contains(field.key())
            && !self.polling_state.runtime.rates.contains_key(&field)
            && self.draft.get(field.key()).is_none()
        {
            return self.polling_value(PollingField::Wired);
        }
        self.polling_value(field)
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
            || self.polling_value(field) == rate
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
        self.polling_state.local_fields.insert(field.key().into());
        self.draft[LOCAL_FIELDS] = json!(self.polling_state.local_fields);
        // Even if the old default draft equals rate, replacing a different
        // displayed observation is an explicit local edit and must persist.
        if self.spec.product_id == 182 {
            if self.polling_state.pending.is_some() {
                self.polling_state.queued = Some((scope, field, rate, false));
            } else {
                self.polling_state.pending = Some((scope, field, rate));
                self.polling_state.pending_valid = Some(Arc::new(AtomicBool::new(true)));
                cx.emit(MousePollingRequested {
                    initialization: false,
                    scope,
                    field,
                    hz: rate,
                });
            }
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
                    BaseButton::new(SharedString::from(format!("source-polling-{rate}")))
                        .accessibility_label(format!("{rate} Hz"))
                        .selected(value == rate)
                        .disabled(disabled)
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
                        .border_color(if value == rate {
                            rgb(0x44d62c)
                        } else {
                            rgb(0x5d5d5d)
                        })
                        .when(disabled, |button| button.opacity(0.4))
                        .hover(|style| style.border_color(rgb(0x44d62c)))
                        .child(rate.to_string())
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
            && value
                > source_spec(self.spec.product_id)
                    .expect("current polling source")
                    .high_rate_threshold_hz
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
        panel
            .when(self.spec.block_widget_columns(), |panel| panel.mb_0())
            .into_any_element()
    }
}
