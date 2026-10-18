//! PID190 BA/St and MW25947.Mt/bt. Local configuration is independent of GET.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "u8", into = "u8")]
enum Enabled {
    Off,
    On,
}
impl TryFrom<u8> for Enabled {
    type Error = &'static str;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Off),
            1 => Ok(Self::On),
            _ => Err("In-game polling state must be zero or one"),
        }
    }
}
impl From<Enabled> for u8 {
    fn from(value: Enabled) -> Self {
        match value { Enabled::Off => 0, Enabled::On => 1 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(super) struct InGamePolling {
    state: Enabled,
    wired: u32,
    wireless: u32,
    #[serde(flatten)]
    extensions: BTreeMap<String, Value>,
}
#[derive(Clone, Copy)]
pub struct MouseInGamePollingChanged;
impl EventEmitter<MouseInGamePollingChanged> for MouseProductWorkspace {}

pub fn validate_in_game_polling(value: &Value) -> anyhow::Result<()> {
    InGamePolling::decode(value).map(|_| ())
}
impl Default for InGamePolling {
    fn default() -> Self {
        // Current MW4638.DEFAULTPROFILE overrides the UI reducer's 125Hz.
        Self { state: Enabled::Off, wired: 1000, wireless: 1000, extensions: BTreeMap::new() }
    }
}
impl InGamePolling {
    fn decode(value: &Value) -> anyhow::Result<Self> {
        let setting: Self = serde_json::from_value(value.clone())?;
        anyhow::ensure!([125, 500, 1000, 2000, 4000, 8000].contains(&setting.wired)
            && [125, 500, 1000, 2000, 4000, 8000].contains(&setting.wireless),
            "In-game polling rate is outside the current product choices");
        Ok(setting)
    }
    fn rate(&self, field: PollingField) -> u32 {
        match field { PollingField::Wired => self.wired, PollingField::Wireless => self.wireless }
    }
    pub(super) fn rate_for_current_games(&self, field: PollingField, base: u32, games: Option<usize>) -> u32 {
        if self.state == Enabled::On && games.is_some_and(|count| count > 0) { self.rate(field) } else { base }
    }
    fn rate_for_event(&self, field: PollingField, base: u32, started: bool) -> u32 {
        if started && self.state == Enabled::On { self.rate(field) } else { base }
    }
}

impl MouseProductWorkspace {
    pub(in super::super) fn restore_in_game_polling(&mut self, saved: Option<&Value>) {
        if self.spec.product_id != 190 { return; }
        let decoded = saved.and_then(|value| value.get("inGamePollingRate"))
            .map(InGamePolling::decode).unwrap_or_else(|| Ok(InGamePolling::default()));
        match decoded {
            Ok(setting) => {
                self.draft["inGamePollingRate"] = serde_json::to_value(&setting).expect("typed local configuration");
                self.polling_state.in_game = Some(setting);
                self.polling_state.in_game_error = None;
            }
            Err(error) => {
                self.polling_state.in_game = None;
                self.polling_state.in_game_error = Some(format!("{error:#}"));
            }
        }
        if self.draft.get("pollingRateWireless").is_none() {
            self.draft["pollingRateWireless"] = json!(1000);
        }
    }

    pub fn observe_running_games(&mut self, count: Option<usize>, _cx: &mut Context<Self>) {
        self.polling_state.running_games = count;
    }

    pub fn observe_polling_game_event(&mut self, started: bool, cx: &mut Context<Self>) {
        if self.spec.product_id != 190 || !matches!(self.polling_state.connection(), Some(PollingConnection::Wired | PollingConnection::Dongle)) {
            return;
        }
        let Some(setting) = &self.polling_state.in_game else { return; };
        // The original start caller only dispatches when enabled. Stop always
        // restores the profile rate, including when another game still runs.
        if started && setting.state != Enabled::On { return; }
        let Some(scope) = self.mouse_polling_scope(cx) else { return; };
        let field = self.polling_state.runtime.field();
        let base = self.polling_value(field);
        let hz = setting.rate_for_event(field, base, started);
        if !razer_device::mouse_current_polling::UI_RATES.contains(&hz) { return; }
        self.submit_polling(MousePollingRequested { scope, field, hz, device_hz: hz, initialization: true, synchronize_profile: false }, cx);
    }

    fn change_in_game_polling(&mut self, scope: MousePollingScope, field: PollingField, rate: Option<u32>, cx: &mut Context<Self>) {
        if self.spec.product_id != 190 || self.mouse_polling_scope(cx) != Some(scope)
            || !self.active || self.page != "TAB_PERFORMANCE" || self.polling_state.runtime.field() != field
            || self.polling_state.connection() == Some(PollingConnection::Ble) { return; }
        let Some(setting) = &mut self.polling_state.in_game else { return; };
        if let Some(rate) = rate {
            if setting.state != Enabled::On || !razer_device::mouse_current_polling::UI_RATES.contains(&rate) || setting.rate(field) == rate { return; }
            match field { PollingField::Wired => setting.wired = rate, PollingField::Wireless => setting.wireless = rate }
        } else {
            setting.state = if setting.state == Enabled::On { Enabled::Off } else { Enabled::On };
        }
        self.draft["inGamePollingRate"] = serde_json::to_value(setting).expect("typed local configuration");
        // MW25947.Mt saves only; it does not request a hardware operation.
        cx.emit(MouseInGamePollingChanged);
        cx.notify();
    }

    pub(super) fn in_game_polling_panel(&self, scope: MousePollingScope, field: PollingField, rates: &[u32], cx: &Context<Self>) -> AnyElement {
        let Some(setting) = self.polling_state.in_game.as_ref() else {
            return div().mt(surface::css(20.)).text_color(razer_widgets::theme::ProfilesTransferColors::error())
                .child(self.polling_state.in_game_error.clone().unwrap_or_default()).into_any_element();
        };
        let enabled = setting.state == Enabled::On;
        v_flex().mt(surface::css(20.))
            .child(h_flex().items_center().justify_between().mt(surface::css(20.))
                .font_family("RazerF5").text_size(surface::css(14.))
                .child(t("INGAME_POLLING_RATE_HEADER"))
                .child(surface::SynapseSwitch::new("mouse-in-game-polling-enabled")
                    .accessibility_label(t("INGAME_POLLING_RATE_HEADER")).checked(enabled)
                    .on_change(cx.listener(move |this, _: &bool, _, cx| this.change_in_game_polling(scope, field, None, cx)))))
            .child(div().my(surface::css(12.)).child(t("IN_GAME_POLLING_RATE_DESC")))
            .child(h_flex().flex_wrap().gap(surface::css(10.)).when(!enabled, |row| row.opacity(0.4))
                .children(rates.iter().copied().map(|rate| {
                    polling_button(("in-game-polling", rate as usize).into(), rate, setting.rate(field) == rate, !enabled, true)
                        .on_click(cx.listener(move |this, _, _, cx| this.change_in_game_polling(scope, field, Some(rate), cx)))
                })))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn source_profile_defaults_override_the_unhydrated_reducer() {
        assert_eq!(serde_json::to_value(InGamePolling::default()).unwrap(), json!({"state":0,"wired":1000,"wireless":1000}));
    }
    #[::core::prelude::v1::test]
    fn game_writes_keep_the_base_profile_rate_and_treat_stop_as_restore() {
        let setting = InGamePolling::decode(&json!({"state":1,"wired":8000,"wireless":4000})).unwrap();
        assert_eq!(setting.rate_for_current_games(PollingField::Wired, 500, Some(2)), 8000);
        assert_eq!(setting.rate_for_current_games(PollingField::Wireless, 1000, None), 1000);
        assert_eq!(setting.rate_for_event(PollingField::Wireless, 500, true), 4000);
        assert_eq!(setting.rate_for_event(PollingField::Wireless, 500, false), 500);
    }
    #[::core::prelude::v1::test]
    fn invalid_settings_do_not_silently_become_source_defaults() {
        for value in [json!({"state":2,"wired":1000,"wireless":1000}), json!({"state":1,"wireless":1000}), json!({"state":1,"wired":250,"wireless":1000})] {
            assert!(InGamePolling::decode(&value).is_err());
        }
        let setting = InGamePolling::decode(&json!({"state":0,"wired":1000,"wireless":1000,"extension":{"x":3}})).unwrap();
        assert_eq!(serde_json::to_value(setting).unwrap()["extension"], json!({"x":3}));
    }
}
