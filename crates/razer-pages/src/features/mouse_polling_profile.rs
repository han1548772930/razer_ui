//! Current 182 OM -> dm/Rm. Runtime observations never modify local profiles.
use super::*;
use crate::features::mouse_polling::{
    MousePollingObservation, MousePollingScope, PollingConnection, PollingField,
};
use crate::features::mouse_polling::{RuntimeState, SourceSpec, source_spec};
use gpui_kit::base::Button as BaseButton;

pub(super) struct State {
    runtime: RuntimeState,
    source: Option<&'static SourceSpec>,
    profile: String,
    windows_icon: &'static str,
}
impl State {
    pub(super) fn new(product_id: u32) -> Self {
        Self {
            runtime: RuntimeState::default(),
            source: source_spec(product_id),
            profile: String::new(),
            windows_icon: if razer_platform::system::is_windows_11() {
                "synapse/windows-11.svg"
            } else {
                "synapse/windows.svg"
            },
        }
    }
    pub(super) fn reset_profile(&mut self) {
        self.runtime.reset_profile();
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

impl DeviceWorkspace {
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
        self.mouse_polling.runtime.apply(observation);
        cx.notify();
    }
    pub(in crate::features) fn mouse_polling_visible(&self) -> bool {
        self.mouse_polling
            .runtime
            .visible(self.mouse_polling.spec())
    }
    pub(in crate::features) fn mouse_windows_icon(&self) -> &'static str {
        self.mouse_polling.windows_icon
    }
    fn mouse_polling_value(&self, field: PollingField) -> u32 {
        let settings = self.settings();
        let local = match field {
            PollingField::Wired => settings.polling_wired,
            PollingField::Wireless => settings.polling_wireless,
        };
        local
            .or_else(|| self.mouse_polling.runtime.rates.get(&field).copied())
            .unwrap_or_else(|| {
                match field {
                    PollingField::Wired => settings.polling,
                    // Current yE initial wireless value; never inherit wired edits.
                    PollingField::Wireless => self.mouse_polling.spec().fallback_wireless_hz,
                }
            })
    }
    pub(in crate::features) fn mouse_low_power_enabled(&self) -> bool {
        let field = if matches!(
            self.mouse_polling.runtime.connection,
            Some(PollingConnection::Dongle | PollingConnection::Ble)
        ) {
            PollingField::Wireless
        } else {
            PollingField::Wired
        };
        self.mouse_polling_value(field) <= self.mouse_polling.spec().high_rate_threshold_hz
    }
    pub(in crate::features) fn mouse_polling_panel(&self, cx: &Context<Self>) -> AnyElement {
        let field = self.mouse_polling.field();
        let wireless = field == PollingField::Wireless;
        let value = self.mouse_polling_value(field);
        let scope = self.mouse_polling_scope(cx).expect("profile polling scope");
        let t = razer_i18n::t;
        let mut panel = surface::panel_with_control(
            t(if wireless { "POLLING_RATE_HEADER" } else { "WIRED_POLLING_RATE_HEADER" }),
            surface::help_control("profile-polling-help", t(if wireless { "POLLING_RATE_V2_TOOLTIP" } else { "WIRED_POLLING_RATE_V2_TOOLTIP" })), cx,
        ).child(surface::h1_body(t("POLLING_RATE_DESC"), cx))
        .child(h_flex().gap(surface::css(10.)).flex_wrap().children(self.mouse_polling.rates().iter().map(|rate| {
            let rate = *rate;
            BaseButton::new(SharedString::from(format!("polling-{rate}")))
                .accessibility_label(format!("{rate} Hz")).child(rate.to_string()).selected(value == rate)
                .flex().items_center().justify_center().w(surface::css(72.)).h(surface::css(27.)).p_0()
                .text_size(surface::css(14.)).rounded(surface::css(3.)).bg(rgb(0x222222)).text_color(rgb(0xcccccc))
                .border_1().border_color(if value == rate { rgb(0x44d62c) } else { rgb(0x5d5d5d) })
                .hover(|s| s.border_color(rgb(0x44d62c)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    if this.mouse_polling_scope(cx) != Some(scope) || this.page != Tab::Performance
                        || !this.mouse_polling_visible() || this.mouse_polling.field() != field
                        || !this.mouse_polling.rates().contains(&rate) || this.mouse_polling_value(field) == rate { return; }
                    this.edit(window, cx, |settings| match field {
                        PollingField::Wired => { settings.polling_wired = Some(rate); settings.polling = rate; }
                        PollingField::Wireless => settings.polling_wireless = Some(rate),
                    });
                }))
        })))
        .when(value > self.mouse_polling.spec().high_rate_threshold_hz, |panel| panel.child(div().mt(surface::css(10.)).opacity(0.7)
            .child(t(if wireless { "POLLING_RATE_WARN" } else { "POLLING_RATE_WARN_NOBATTERY" }))
            .child(h_flex().id("polling-learn-more").items_center().pl(surface::css(5.)).underline().cursor_pointer()
                .child(t("LEARN_MORE"))
                .child(img("synapse/external-link.svg").size(surface::css(16.)).ml(surface::css(5.)))
                .on_click(|_, _, cx| cx.open_url("https://www.razer.com/technology/razer-hyperpolling#best-practices-tips")))));
        let local = match field {
            PollingField::Wired => self.settings().polling_wired,
            PollingField::Wireless => self.settings().polling_wireless,
        };
        if self.mouse_polling.runtime.connection.is_none() {
            panel = panel.child(surface::note(
                "连接状态尚未读取；当前编辑本地有线配置。",
                cx,
            ));
        } else if local.is_some() {
            panel = panel.child(surface::note("当前显示本地草稿；尚未发送到设备。", cx));
        } else if !self.mouse_polling.runtime.rates.contains_key(&field) {
            panel = panel.child(surface::note("当前显示本地配置；设备回报率尚未读取。", cx));
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
