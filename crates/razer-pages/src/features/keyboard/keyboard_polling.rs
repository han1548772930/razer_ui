//! Current mounted polling controls; shared-but-unused components are excluded.
use super::*;
use razer_widgets::theme::KeyboardPollingColors as Colors;

#[cfg(test)]
#[path = "keyboard_polling_tests.rs"]
mod tests;

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
        .get_or_init(|| read_keyboard_polling_catalog())
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

/// The older mounted component selects its BLE button list independently of
/// its setter (which still uses the wired field). Current 728/739/740/742/746 instead
/// have three explicit modes; disabling BLE polling routes BLE to wireless.
struct Selection {
    path: &'static str,
    values_key: &'static str,
    bluetooth_info: bool,
    disabled_in_ble: bool,
    max_rate_hz: Option<u64>,
}

fn selection(pid: u32, config: &Value, connection: KeyboardPollingConnection) -> Selection {
    let modern = matches!(pid, 728 | 739 | 740 | 742 | 746 | 756);
    let disabled_in_ble = pid == 756
        && connection.is_ble
        && config["DeviceInfo"]["supportBluetoothPollingRate"].as_bool() == Some(false);
    let bluetooth_info = modern
        && connection.is_ble
        && config["DeviceInfo"]["supportBluetoothPollingRate"].as_bool() != Some(false);
    let wireless = connection.is_dongle || (modern && connection.is_ble);
    Selection {
        path: if bluetooth_info {
            "/pollingRateBle"
        } else if wireless {
            "/pollingRateWireless"
        } else {
            "/pollingRate"
        },
        values_key: if disabled_in_ble {
            "POLLING_RATE"
        } else if bluetooth_info || (!modern && !connection.is_dongle && connection.is_ble) {
            "POLLING_RATE_BLUETOOTH"
        } else if wireless {
            "POLLING_RATE_WIRELESS"
        } else {
            "POLLING_RATE"
        },
        bluetooth_info,
        disabled_in_ble,
        // The old mounted component has no limit state or disabled prop.
        max_rate_hz: modern.then_some(connection.max_rate_hz).flatten(),
    }
}

impl Selection {
    fn values<'a>(&self, config: &'a Value) -> Option<&'a Vec<Value>> {
        config[self.values_key]
            .as_array()
            .or_else(|| config["POLLING_RATE"].as_array())
    }
}

fn info_row(icon: &'static str, label: String) -> AnyElement {
    h_flex()
        .items_start()
        .mt(surface::css(15.))
        .text_color(Colors::muted())
        .text_size(surface::css(14.))
        .line_height(surface::css(17.))
        .child(
            img(icon)
                .size(surface::css(20.))
                .flex_shrink_0()
                .mr(surface::css(5.)),
        )
        .child(div().flex_1().child(label))
        .into_any_element()
}

impl KeyboardProductWorkspace {
    /// Observed reducer state only; an unobserved draft has no connection mode.
    pub fn observed_bluetooth(&self) -> Option<bool> {
        self.polling_connection
            .map(|value| value.is_ble)
            .or_else(|| {
                (self.spec.product_id == 756)
                    .then_some(self.calibration_v2.is_ble)
                    .flatten()
            })
    }

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
        let selection = selection(self.spec.product_id, &self.spec.config, connection);
        let path = selection.path;
        let values = selection.values(&self.spec.config)?;
        let selected = self.draft.pointer(path).and_then(Value::as_u64);
        let wireless_title = connection.is_dongle || connection.is_ble;
        let mut panel = surface::panel_with_help(
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
        .when(self.spec.modern_analog_controls(), |panel| panel.mb_0())
        .child(surface::h1_body(t("POLLING_RATE_DESC"), cx))
        .child(
            h_flex()
                .flex_wrap()
                .gap(surface::css(10.))
                .children(values.iter().filter_map(|entry| {
                    let rate = entry["content"].as_str()?.parse::<u64>().ok()?;
                    let active = selected == Some(rate) || values.len() == 1;
                    let disabled = self.customize_controls_disabled()
                        || selection.disabled_in_ble
                        || selection.max_rate_hz.is_some_and(|max| rate > max);
                    Some(
                        razer_widgets::keyboard_navigation::Button::new(SharedString::from(
                            format!("keyboard-polling-{rate}"),
                        ))
                        .accessibility_label(rate.to_string())
                        .disabled(disabled)
                        .when(disabled, |button| button.opacity(0.4))
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
                        .border_color(if active { Colors::accent() } else { Colors::border() })
                        .bg(Colors::background())
                        .text_color(Colors::text())
                        .text_size(surface::css(14.))
                        .when(!disabled, |button| {
                            button.hover(|button| button.border_color(Colors::accent()))
                        })
                        .child(rate.to_string())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !active && !disabled {
                                this.write(path, json!(rate), cx);
                            }
                        })),
                    )
                })),
        );
        if selection.bluetooth_info || selection.disabled_in_ble {
            panel = panel.child(info_row(
                "synapse/keyboard-polling-bluetooth-info.svg",
                t(if selection.disabled_in_ble {
                    "POLLING_RATE_BLE_CONFIG_DISABLED_INFO"
                } else {
                    "POLLING_RATE_BLUETOOTH_INFO"
                }),
            ));
        }
        if selection.max_rate_hz.is_some() {
            panel = panel.child(info_row(
                "synapse/keyboard-polling-limit-info.svg",
                t(if connection.dual_link_limit {
                    "POLLING_RATE_DUAL_LINK_LIMITED"
                } else {
                    "POLLING_RATE_MULTI_DEVICE_DOCK_LIMITED_KEYBOARD"
                }),
            ));
        } else if selected.is_some_and(|rate| rate > 1000) {
            panel = panel.child(div().mt(surface::css(10.)).opacity(0.7).child(t(
                if wireless_title {
                    "POLLING_RATE_WARN"
                } else {
                    "POLLING_RATE_WARN_NOBATTERY"
                },
            )));
        }
        Some(panel.into_any_element())
    }
}

// Generated catalog reader: razer-pages/keyboard_polling_data.
fn read_keyboard_polling_catalog() -> Vec<Mount> {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<Mount>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Mount> {
        let value = match r.tag()? {
            0 => Mount {
                pid: r.child(decode_2)?,
                left: r.child(decode_3)?,
                hide_ble: r.child(decode_3)?,
                keyboard_category: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_POLLING_DATA,
        0,
        "b12e802ac3021ccec8a0338e632868dcbe325a5d1a8871d36fe8ef6d78505f9e",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod keyboard_polling_data_catalog_tests {
    #[test]
    fn packaged_catalog_keyboard_polling_data_decodes() {
        let _ = super::read_keyboard_polling_catalog();
    }
}
// End generated catalog reader: razer-pages/keyboard_polling_data.
