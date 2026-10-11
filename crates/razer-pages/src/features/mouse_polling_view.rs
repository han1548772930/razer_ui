//! Current 226 wi/Xs/zs polling panel. Observations never become profile writes.
use super::*;
use crate::features::mouse_polling::source_spec;
use gpui_kit::{base::Button as BaseButton, prelude::FluentBuilder as _};
use std::collections::BTreeSet;

pub(super) const LOCAL_FIELDS: &str = "_pollingLocalFieldsV1";

#[derive(Default)]
pub(super) struct State {
    runtime: super::super::mouse_polling::RuntimeState,
    local_fields: BTreeSet<String>,
}

impl MouseProductWorkspace {
    pub(super) fn advanced_enabled(&self) -> bool {
        static FLOORS: OnceLock<BTreeMap<String, String>> = OnceLock::new();
        let floors = FLOORS.get_or_init(|| {
            serde_json::from_str(include_str!("mouse_advanced_data.json"))
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
        source_spec(self.spec.product_id)?;
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
        // No Changed event, snapshot write, auto-downgrade or device command.
        cx.notify();
    }

    pub(super) fn restore_polling(&mut self, saved: Option<&Value>) {
        if source_spec(self.spec.product_id).is_none() {
            return;
        }
        self.polling_state.runtime.reset_profile();
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
        let mut panel = surface::panel_with_control(
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
            panel = panel.child(div().mt(surface::css(10.)).opacity(0.7)
                .child(t(if wireless { "POLLING_RATE_WARN" } else { "POLLING_RATE_WARN_NOBATTERY" }))
                .child(h_flex().id("source-polling-learn-more").pl(surface::css(5.)).underline().cursor_pointer()
                    .child(t("LEARN_MORE"))
                    .child(img("synapse/external-link.svg").size(surface::css(16.)).ml(surface::css(5.)))
                    .on_click(|_, _, cx| cx.open_url("https://www.razer.com/technology/razer-hyperpolling#best-practices-tips"))));
        }
        panel
            .when(self.spec.block_widget_columns(), |panel| panel.mb_0())
            .into_any_element()
    }
}
