//! Mounted Power widgets, traced per current product in `.work/keyboard-power`.
//! These controls edit the retained local draft; device transport is not implied.
use super::*;
use std::time::{Duration, Instant};

/// Source `SET_INDICATOR_LED_STATUS` intent. The UI reducer changes its local
/// selection immediately, while the host/device response is reported through
/// a separate observation path; this event never claims a successful write.
#[derive(Clone)]
pub struct KeyboardIndicatorLedRequested {
    generation: u64,
    status: u8,
}
impl KeyboardIndicatorLedRequested {
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn status(&self) -> u8 {
        self.status
    }
    pub fn payload(&self) -> Value {
        json!({"type":"ON_SET_INDICATOR_LED","payload":{"indicatorLedStatus":self.status}})
    }
}
impl EventEmitter<KeyboardIndicatorLedRequested> for KeyboardProductWorkspace {}

/// Host-side result/observation for the current Indicator LED setting. A
/// missing observation keeps the radio controls disabled rather than inventing
/// a dongle connection; callers may feed this through the workspace adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardIndicatorLedObservation {
    Connected { status: u8 },
    Disconnected,
}

pub(super) struct IndicatorState {
    animation_epoch: Instant,
    generation: u64,
    pending: Option<u64>,
    observed: Option<KeyboardIndicatorLedObservation>,
    error: Option<String>,
}

impl Default for IndicatorState {
    fn default() -> Self {
        Self {
            animation_epoch: Instant::now(),
            generation: 0,
            pending: None,
            observed: None,
            error: None,
        }
    }
}

impl KeyboardProductWorkspace {
    pub fn request_indicator_led(&mut self, status: u8, cx: &mut Context<Self>) {
        if !(1..=3).contains(&status) {
            return;
        }
        let changed = self.draft["indicatorLedStatus"].as_u64() != Some(u64::from(status));
        if self.draft.get("indicatorLedStatus").is_some() {
            self.write("/indicatorLedStatus", json!(status), cx);
        } else {
            self.draft["indicatorLedStatus"] = json!(status);
            cx.emit(KeyboardProductChanged);
            cx.notify();
        }
        self.power_indicator.generation = self.power_indicator.generation.wrapping_add(1);
        self.power_indicator.pending = Some(self.power_indicator.generation);
        self.power_indicator.error = None;
        if changed {
            self.power_indicator.animation_epoch = Instant::now();
        }
        cx.emit(KeyboardIndicatorLedRequested {
            generation: self.power_indicator.generation,
            status,
        });
    }

    pub fn observe_indicator_led(
        &mut self,
        observation: KeyboardIndicatorLedObservation,
        cx: &mut Context<Self>,
    ) {
        self.power_indicator.observed = Some(observation);
        if let KeyboardIndicatorLedObservation::Connected { status } = observation {
            if (1..=3).contains(&status) {
                if self.draft["indicatorLedStatus"].as_u64() != Some(u64::from(status)) {
                    self.power_indicator.animation_epoch = Instant::now();
                }
                self.draft["indicatorLedStatus"] = json!(status);
            }
        }
        cx.notify();
    }
    pub(super) fn restart_indicator_animation(&mut self) {
        self.power_indicator.animation_epoch = Instant::now();
    }

    pub fn finish_indicator_led(
        &mut self,
        generation: u64,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        if self.power_indicator.pending != Some(generation) {
            return;
        }
        self.power_indicator.pending = None;
        self.power_indicator.error = result.err();
        cx.notify();
    }

    pub fn indicator_led_request_current(&self, generation: u64) -> bool {
        self.power_indicator.pending == Some(generation)
    }

    pub fn indicator_led_error(&self) -> Option<&str> {
        self.power_indicator.error.as_deref()
    }

    pub fn cancel_indicator_led_connection(&mut self, cx: &mut Context<Self>) {
        self.power_indicator.generation = self.power_indicator.generation.wrapping_add(1);
        self.power_indicator.pending = None;
        self.power_indicator.observed = None;
        self.power_indicator.error = None;
        cx.notify();
    }

    /// `isDongle` is an observed connection fact. The product's static
    /// `dongleId` is deliberately not used as a substitute for this value.
    fn indicator_led_is_dongle(&self) -> bool {
        matches!(
            self.power_indicator.observed,
            Some(KeyboardIndicatorLedObservation::Connected { .. })
        )
    }

    fn indicator_led_image(&self, status: u8) -> AnyElement {
        let epoch = self.power_indicator.animation_epoch;
        div()
            .relative()
            .w(surface::css(200.))
            .h(surface::css(192.))
            .child(
                img("synapse/keyboard-indicator-connection-body.png")
                    .w(surface::css(243.))
                    .h(surface::css(187.)),
            )
            .with_animation(
                SharedString::from(format!("691-indicator-animation-{status}")),
                Animation::new(Duration::from_secs(1)).repeat(),
                move |element, _| {
                    let elapsed = epoch.elapsed().as_secs_f32();
                    let triangular = |phase: f32| (1. - (phase * 2. - 1.).abs()).clamp(0., 1.);
                    let mut image = element;
                    let led = |color: u32, opacity: f32| {
                        div()
                            .absolute()
                            .left(surface::css(112.))
                            .top(surface::css(167.))
                            .size(surface::css(19.))
                            .opacity(opacity)
                            .child(
                                div()
                                    .absolute()
                                    .size(surface::css(19.))
                                    .rounded_full()
                                    .bg(rgb(color))
                                    .opacity(0.14),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .left(surface::css(3.))
                                    .top(surface::css(3.))
                                    .size(surface::css(13.))
                                    .rounded_full()
                                    .bg(rgb(color))
                                    .opacity(0.14),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .left(surface::css(7.))
                                    .top(surface::css(7.))
                                    .size(surface::css(5.))
                                    .rounded_full()
                                    .bg(rgb(color)),
                            )
                    };
                    match status {
                        2 => {
                            image = image.child(led(0x00ff00, 1.));
                            let red = if elapsed < 2.5 {
                                0.
                            } else {
                                ((elapsed - 2.5).rem_euclid(0.3) / 0.15).min(1.)
                            };
                            let yellow = if elapsed < 1.5 {
                                0.
                            } else {
                                triangular((elapsed - 1.5).rem_euclid(1.3) / 1.3)
                            };
                            image = image.child(led(0xff0000, red)).child(led(0xffff00, yellow));
                        }
                        3 => {
                            let cycle = elapsed.rem_euclid(2.6);
                            let opacity = if cycle < 0.3 {
                                triangular(cycle / 0.3)
                            } else if cycle < 0.6 {
                                triangular((cycle - 0.3) / 0.3)
                            } else {
                                0.
                            };
                            image = image.child(led(0xff0000, opacity));
                            // Original dot_ani3 follows the two 0.3s flashes:
                            // three opaque #111 circles at the same centre,
                            // group opacity 0 -> 1 -> 0 over the next 2 seconds.
                            if cycle >= 0.6 {
                                image = image.child(
                                    div()
                                        .absolute()
                                        .left(surface::css(112.))
                                        .top(surface::css(167.))
                                        .size(surface::css(19.))
                                        .rounded_full()
                                        .bg(rgb(0x111111))
                                        .opacity(triangular((cycle - 0.6) / 2.)),
                                );
                            }
                        }
                        _ => image = image.child(led(0xf8f8ff, 1.)),
                    }
                    image
                },
            )
            .into_any_element()
    }

    fn indicator_led_panel(&self, cx: &Context<Self>) -> AnyElement {
        let status = self.draft["indicatorLedStatus"].as_u64().unwrap_or(1) as u8;
        let is_dongle = self.indicator_led_is_dongle();
        let options = [
            (
                1_u8,
                "CONNECTION_STATUS",
                "POWER_INDICATOR_CONNECTION_STATUS_DESC",
                "keyboard-indicator-connection",
                50.,
            ),
            (
                2_u8,
                "BATTERY_STATUS",
                "BATTERY_STATUS_KEYBOARD_DESC",
                "keyboard-indicator-battery",
                64.,
            ),
            (
                3_u8,
                "BATTERY_WARNING_ONLY",
                "POWER_INDICATOR_BATTERY_WARNING_ONLY_DESC",
                "keyboard-indicator-warning",
                50.,
            ),
        ];
        let radios = v_flex()
            .gap(surface::css(10.))
            .w(surface::css(310.))
            .when(!is_dongle, |column| column.opacity(0.3))
            .children(
                options
                    .into_iter()
                    .map(|(value, label, description, id, height)| {
                        let selected = status == value;
                        v_flex()
                            .w(surface::css(310.))
                            .h(surface::css(height))
                            .child(
                                gpui_kit::base::Button::new(id)
                                    .disabled(!is_dongle)
                                    .accessibility_label(t(label))
                                    .flex()
                                    .items_center()
                                    .gap(surface::css(10.))
                                    .h(surface::css(20.))
                                    .text_size(surface::css(14.))
                                    .line_height(surface::css(20.))
                                    .text_color(rgb(0xcccccc))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .size(surface::css(20.))
                                            .rounded_full()
                                            .border_1()
                                            .border_color(rgb(0x737373))
                                            .child(
                                                div()
                                                    .size(surface::css(10.))
                                                    .rounded_full()
                                                    .bg(rgb(0x44d62c))
                                                    .opacity(if selected { 1. } else { 0. }),
                                            ),
                                    )
                                    .child(t(label).to_uppercase())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.request_indicator_led(value, cx)
                                    })),
                            )
                            .child(
                                div()
                                    .mt(surface::css(2.))
                                    .ml(surface::css(30.))
                                    .w(surface::css(280.))
                                    .text_size(surface::css(12.))
                                    .text_color(rgb(0x999999))
                                    .opacity(0.7)
                                    .child(t(description)),
                            )
                    }),
            );
        IndicatorTooltip {
            disabled: !is_dongle,
            panel: surface::panel(t("INDICATOR_LED_V2"), cx)
                .child(
                    v_flex()
                        .gap(surface::css(20.))
                        .h(surface::css(227.))
                        .w(surface::css(520.))
                        .child(
                            div()
                                .text_size(surface::css(14.))
                                .child(t("INDICATOR_LED_DESC")),
                        )
                        .child(
                            h_flex()
                                .my(surface::css(5.))
                                .child(radios)
                                .child(self.indicator_led_image(status)),
                        ),
                )
                .into_any_element(),
        }
        .into_any_element()
    }

    fn power_numeric_choices(
        &self,
        path: &str,
        values: &[u64],
        enabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        h_flex()
            .relative()
            .flex_wrap()
            .gap(surface::css(10.))
            .children(values.iter().map(|&value| {
                let path = path.to_owned();
                let selected = self.draft.pointer(&path).and_then(Value::as_u64) == Some(value);
                gpui_kit::base::Button::new(SharedString::from(format!(
                    "keyboard-power-{path}-{value}"
                )))
                .accessibility_label(value.to_string())
                .disabled(!enabled)
                .w(surface::css(48.))
                .h(surface::css(27.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(surface::css(3.))
                .bg(rgb(0x222222))
                .text_size(surface::css(14.))
                .text_color(rgb(0xcccccc))
                .border_1()
                .border_color(if selected {
                    rgb(0x44d62c)
                } else {
                    rgb(0x5d5d5d)
                })
                .hover(|button| button.border_color(rgb(0x44d62c)))
                .child(value.to_string())
                .on_click(cx.listener(move |this, _, _, cx| this.write(&path, json!(value), cx)))
            }))
            .when(!enabled, |row| {
                row.child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .w(surface::css(300.))
                        .h(surface::css(30.))
                        .bg(rgb(0x111111))
                        .opacity(0.5)
                        .occlude(),
                )
            })
            .into_any_element()
    }

    fn power_choice_widget(
        &self,
        path: &'static str,
        enabled_key: &'static str,
        title: &str,
        description: &str,
        help: &str,
        values: &str,
        fallback: &[u64],
        gap: f32,
        cx: &Context<Self>,
    ) -> AnyElement {
        let enabled_path = format!("{path}/{enabled_key}");
        let value_path = format!("{path}/value");
        let enabled = self.boolean(&enabled_path);
        let values = self.spec.config[values]
            .as_array()
            .map(|values| values.iter().filter_map(Value::as_u64).collect::<Vec<_>>())
            .unwrap_or_else(|| fallback.to_vec());
        surface::panel_with_title_switch(
            t(title),
            surface::SynapseSwitch::new(SharedString::from(format!(
                "keyboard-power-switch-{path}"
            )))
            .accessibility_label(t(title))
            .checked(enabled)
            .on_change(cx.listener(move |this, enabled: &bool, _, cx| {
                this.write(&enabled_path, json!(*enabled), cx)
            })),
            surface::help_control(
                SharedString::from(format!("keyboard-power-help-{path}")),
                t(help),
            ),
            cx,
        )
        .child(div().child(t(description)))
        .child(
            div()
                .mt(surface::css(gap))
                .child(self.power_numeric_choices(&value_path, &values, enabled, cx)),
        )
        .into_any_element()
    }

    fn power_slider_widget(
        &self,
        path: &'static str,
        title: &str,
        description: &str,
        help: String,
        bounds: (i64, i64),
        has_switch: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let enabled_path = format!("{path}/isEnabled");
        let value_path = if has_switch {
            format!("{path}/value")
        } else {
            path.to_owned()
        };
        let enabled = !has_switch || self.boolean(&enabled_path);
        let switch = has_switch.then(|| {
            surface::SynapseSwitch::new(SharedString::from(format!("keyboard-power-switch-{path}")))
                .accessibility_label(t(title))
                .checked(enabled)
                .on_change(cx.listener(move |this, enabled: &bool, _, cx| {
                    this.write(&enabled_path, json!(*enabled), cx)
                }))
                .into_any_element()
        });
        surface::panel_with_title_switch_opt(
            t(title),
            switch,
            surface::help_control(
                SharedString::from(format!("keyboard-power-help-{path}")),
                help,
            ),
            cx,
        )
        .child(surface::h1_body(t(description), cx))
        .children(self.sliders.get(&value_path).map(|slider| {
            v_flex()
                .child(Slider::new(slider).disabled(!enabled))
                .child(surface::slider_tags(
                    &bounds.0.to_string(),
                    None,
                    &bounds.1.to_string(),
                    None,
                ))
        }))
        .into_any_element()
    }

    fn power_shortcut_mode(&self, cx: &Context<Self>) -> AnyElement {
        // 691 has an OLED by the source's `hasOLED !== false` condition.
        // 717 explicitly supplies hasOLED:false and therefore uses the other copy.
        let oled = self.spec.product_id == 691;
        let shortcut = h_flex()
            .gap(surface::css(5.))
            .items_center()
            .children(["FN", "+", "ESC"].into_iter().map(|key| {
                div()
                    .when(key != "+", |keycap| {
                        keycap
                            .bg(rgb(0x222222))
                            .border_1()
                            .border_color(rgb(0x5d5d5d))
                            .rounded(surface::css(3.))
                            .text_size(surface::css(12.))
                            .px(surface::css(16.))
                            .pt(surface::css(7.))
                            .pb(surface::css(6.))
                    })
                    .child(key)
            }))
            .child(surface::help_control(
                "keyboard-low-power-shortcut",
                t("OLED_LOW_BATTERY_MODE_SHORTCUT_TIPS"),
            ));
        surface::panel_with_control(t("OLED_LOW_BATTERY_MODE_TITLE"), shortcut, cx)
            .relative()
            .child(
                div()
                    .absolute()
                    .top(surface::css(10.))
                    .right(surface::css(10.))
                    .child(surface::help_control(
                        "keyboard-low-power-help",
                        t(if oled {
                            "OLED_LOW_BATTERY_MODE_TIPS"
                        } else {
                            "ACTIVATE_POWER_SAVING_MODE_TIPS"
                        }),
                    )),
            )
            .child(div().child(t(if oled {
                "OLED_LOW_BATTERY_MODE_DESC"
            } else {
                "ACTIVATE_POWER_SAVING_MODE_DESC"
            })))
            .when(oled, |panel| {
                panel.child(
                    div()
                        .mt(surface::css(20.))
                        .child(t("OLED_LOW_BATTERY_MODE_DISABLE_DESC")),
                )
            })
            .into_any_element()
    }

    fn power_button_mode(&self, cx: &Context<Self>) -> AnyElement {
        surface::panel_with_control(
            t("POWER_SAVING_MODE"),
            svg()
                .path("synapse/keyboard-power-saving-button.svg")
                .w(surface::css(32.))
                .h(surface::css(32.)),
            cx,
        )
        .relative()
        .child(
            div()
                .absolute()
                .top(surface::css(10.))
                .right(surface::css(10.))
                .child(surface::help_control(
                    "keyboard-power-mode-help",
                    t("ACTIVATE_POWER_SAVING_MODE_TIPS"),
                )),
        )
        .child(div().child(t("ACTIVATE_POWER_SAVING_MODE_DESC_1")))
        .into_any_element()
    }

    pub(super) fn power(&self, cx: &Context<Self>) -> AnyElement {
        let choices = self.spec.controls["power_kind"] == "choices";
        let mouse = self.spec.controls["power_kind"] == "mouse_slider";
        let mut left = v_flex();
        let mut right = v_flex();
        if choices {
            left = left.child(self.power_choice_widget(
                "/dimKeyboardLighting",
                "isEnabled",
                "DIM_LIGHTING_HEADER",
                "DIM_KEYBOARD_LIGHTING_DESC",
                "DIM_KEYBOARD_LIGHTING_TIPS",
                "DIM_KEYBOARD_LIGHTING_VALUES",
                &[1, 3, 5, 10, 15],
                20.,
                cx,
            ));
            right = right.child(self.power_choice_widget(
                "/powerSaving",
                "isEnabled",
                "KEYBOARD_POWER_SAVING_TITLE",
                "KEYBOARD_POWER_SAVING_DESC",
                "KEYBOARD_POWER_SAVING_TIPS_2",
                "KEYBOARD_WIRELESS_POWER_SAVING_VALUES",
                &[1, 3, 5, 10, 15],
                20.,
                cx,
            ));
            match self.spec.product_id {
                691 => {
                    left = left.child(self.power_choice_widget(
                        "/oledLowBatteryWarningDisplay",
                        "enabled",
                        "OLED_LOW_BATTERY_WARNING_DISPLAY_TITLE",
                        "OLED_LOW_BATTERY_WARNING_DISPLAY_DESC",
                        "OLED_LOW_BATTERY_WARNING_DISPLAY_TIPS",
                        "OLED_LOW_BATTERY_WARNING_DISPLAY_VALUES",
                        &[10, 20, 30, 40, 50],
                        40.,
                        cx,
                    ));
                    right = right.child(self.power_shortcut_mode(cx));
                    left = left.child(self.indicator_led_panel(cx));
                }
                717 => left = left.child(self.power_shortcut_mode(cx)),
                708 | 716 | 724 | 727 => left = left.child(self.power_button_mode(cx)),
                _ => {}
            }
        } else {
            left = left.child(self.power_slider_widget(
                "/dimLighting",
                "DIM_LIGHTING_HEADER",
                "DIM_LIGHTING_DESC",
                t("DIM_LIGHTING_TOOLTIP"),
                (1, 15),
                true,
                cx,
            ));
            right = right.child(if mouse {
                self.power_slider_widget(
                    "/powerSavingValue",
                    "POWER_SAVING_HEADER",
                    "POWER_SAVING_DESC",
                    t("POWER_SAVING_TOOLTIP"),
                    (1, 15),
                    false,
                    cx,
                )
            } else {
                self.power_slider_widget(
                    "/powerSaving",
                    "AUDIO_POWER_SAVING_HEADER",
                    "AUDIO_POWER_SAVING_DESC",
                    format!(
                        "{}\n\n{}",
                        t("AUDIO_POWER_SAVING_TOOLTIP"),
                        t("AUDIO_POWER_SAVING_TOOLTIP_ADDITIONAL")
                    ),
                    (15, 60),
                    true,
                    cx,
                )
            });
        }
        surface::page_columns()
            .child(surface::page_column(left))
            .child(surface::page_column(right))
            .into_any_element()
    }
}

#[derive(IntoElement)]
struct IndicatorTooltip {
    disabled: bool,
    panel: AnyElement,
}
impl RenderOnce for IndicatorTooltip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state("691-indicator-tooltip", cx, |_, _| false);
        let hovered = *state.read(cx);
        div()
            .id("691-indicator-tooltip-row")
            .relative()
            .child(self.panel)
            .on_hover(move |hovered, _, cx| {
                state.update(cx, |value, cx| {
                    *value = *hovered;
                    cx.notify();
                })
            })
            .when(self.disabled && hovered, |wrapper| {
                wrapper.child(
                    div()
                        .absolute()
                        .left(surface::css(100.))
                        .top(surface::css(100.))
                        .w(surface::css(290.))
                        .px(surface::css(10.))
                        .py(surface::css(8.))
                        .bg(rgb(0x000000))
                        .border_1()
                        .border_color(rgb(0x5d5d5d))
                        .text_color(rgb(0xcccccc))
                        .text_size(surface::css(14.))
                        .line_height(surface::css(16.))
                        .child(t("INDICATOR_LED_NOT_WIRELESS_MODE_TIP"))
                        .with_animation(
                            "691-indicator-tooltip-fade",
                            Animation::new(Duration::from_millis(300)),
                            |tip, phase| tip.opacity(phase),
                        ),
                )
            })
    }
}
