//! Current mounted polling controls; shared-but-unused components are excluded.
use super::*;

#[derive(Deserialize)]
pub(super) struct Mount {
    pid: u32,
    pub(super) left: bool,
    hide_ble: bool,
    keyboard_category: bool,
}
pub(super) fn mount(pid: u32) -> Option<&'static Mount> {
    static MOUNTS: OnceLock<Vec<Mount>> = OnceLock::new();
    MOUNTS
        .get_or_init(|| {
            serde_json::from_str(include_str!("keyboard_polling_data.json"))
                .expect("validated mounted polling callers")
        })
        .iter()
        .find(|m| m.pid == pid)
}

/// Real source reducer observations, separate from the local profile draft.
/// Callers must supply observed mode/capability, never infer it from product PID.
#[derive(Clone, Copy, Default)]
pub struct KeyboardPollingConnection {
    pub is_dongle: bool,
    pub is_ble: bool,
    pub max_rate_hz: Option<u64>,
    pub dual_link_limit: bool,
}

impl KeyboardProductWorkspace {
    pub fn observe_polling_connection(
        &mut self,
        value: KeyboardPollingConnection,
        cx: &mut Context<Self>,
    ) {
        self.polling_connection = Some(value);
        cx.notify();
    }

    pub(super) fn polling_on_left(&self) -> bool {
        mount(self.spec.product_id).is_some_and(|m| m.left)
    }

    pub(super) fn polling_panel(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let mount = mount(self.spec.product_id)?;
        // An unobserved development draft starts with the source wired view.
        // This does not represent a successful device query.
        let connection = self.polling_connection.unwrap_or_default();
        if mount.hide_ble && connection.is_ble {
            return None;
        }
        let modern_ble = matches!(self.spec.product_id, 740 | 742 | 746)
            && self.spec.config["DeviceInfo"]["supportBluetoothPollingRate"].as_bool()
                != Some(false);
        let wireless = connection.is_dongle || (modern_ble && connection.is_ble);
        let ble = modern_ble && connection.is_ble;
        let path = if ble {
            "/pollingRateBle"
        } else if wireless {
            "/pollingRateWireless"
        } else {
            "/pollingRate"
        };
        let values_key = if ble {
            "POLLING_RATE_BLUETOOTH"
        } else if connection.is_dongle || connection.is_ble {
            "POLLING_RATE_WIRELESS"
        } else {
            "POLLING_RATE"
        };
        let values = self.spec.config[values_key]
            .as_array()
            .or_else(|| self.spec.config["POLLING_RATE"].as_array())?;
        let selected = self.draft.pointer(path).and_then(Value::as_u64);
        let wireless_title = connection.is_dongle || connection.is_ble;
        let mut panel = surface::panel_with_control(
            t(if wireless_title {
                "POLLING_RATE_HEADER"
            } else {
                "WIRED_POLLING_RATE_HEADER"
            }),
            surface::help_control(
                "keyboard-polling-help",
                t(if wireless_title {
                    "POLLING_RATE_V2_TOOLTIP"
                } else {
                    "WIRED_POLLING_RATE_V2_TOOLTIP"
                }),
            ),
            cx,
        )
        .child(surface::h1_body(t("POLLING_RATE_DESC"), cx))
        .child(
            h_flex()
                .flex_wrap()
                .gap(surface::css(10.))
                .children(values.iter().filter_map(|entry| {
                    let rate = entry["content"].as_str()?.parse::<u64>().ok()?;
                    let active = selected == Some(rate) || values.len() == 1;
                    let disabled = connection.max_rate_hz.is_some_and(|max| rate > max);
                    Some(
                        gpui_kit::base::Button::new(SharedString::from(format!(
                            "keyboard-polling-{rate}"
                        )))
                        .accessibility_label(rate.to_string())
                        .disabled(disabled)
                        .w(surface::css(if mount.keyboard_category {
                            65.
                        } else {
                            72.
                        }))
                        .h(surface::css(27.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(surface::css(3.))
                        .border_1()
                        .border_color(if active { rgb(0x44d62c) } else { rgb(0x5d5d5d) })
                        .bg(rgb(0x222222))
                        .text_color(rgb(0xcccccc))
                        .text_size(surface::css(14.))
                        .hover(|button| button.border_color(rgb(0x44d62c)))
                        .child(rate.to_string())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !active && !disabled {
                                this.write(path, json!(rate), cx);
                            }
                        })),
                    )
                })),
        );
        if ble {
            panel = panel.child(
                div()
                    .mt(surface::css(15.))
                    .text_color(rgb(0x999999))
                    .child(t("POLLING_RATE_BLUETOOTH_INFO")),
            );
        }
        if connection.max_rate_hz.is_some() {
            panel = panel.child(
                div()
                    .mt(surface::css(15.))
                    .text_color(rgb(0x999999))
                    .child(t(if connection.dual_link_limit {
                        "POLLING_RATE_DUAL_LINK_LIMITED"
                    } else {
                        "POLLING_RATE_MULTI_DEVICE_DOCK_LIMITED_KEYBOARD"
                    })),
            );
        } else if selected.is_some_and(|rate| rate > 1000) {
            panel = panel.child(div().mt(surface::css(10.)).opacity(0.7)
                .child(t(if wireless_title {"POLLING_RATE_WARN"} else {"POLLING_RATE_WARN_NOBATTERY"}))
                .child(gpui_kit::base::Button::new("keyboard-polling-learn-more").accessibility_label(t("LEARN_MORE"))
                    .flex().items_center().text_color(rgb(0xcccccc)).child(t("LEARN_MORE"))
                    .child(svg().path("synapse/external-link.svg").w(surface::css(16.)).h(surface::css(16.)).ml(surface::css(2.)))
                    .on_click(|_,_,cx|cx.open_url("https://www.razer.com/technology/razer-hyperpolling#best-practices-tips"))));
        }
        Some(panel.into_any_element())
    }
}
