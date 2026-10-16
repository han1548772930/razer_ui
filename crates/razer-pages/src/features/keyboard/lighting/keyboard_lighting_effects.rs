//! The mounted monochrome Effects widget (eight current product callers).
//! This edits the local profile; no successful hardware write is implied.
use super::*;
use gpui_kit::component::radio::Radio;
use razer_widgets::theme::KeyboardEffectColors as Colors;

const PRESETS: [&str; 4] = [
    "#00ff00",
    "#0000ff",
    "#00ffff",
    "linear-gradient(90deg,#00ff00,#0000ff,#00ffff)",
];

pub(super) fn radio_effects(pid: u32) -> bool {
    matches!(pid, 565 | 567 | 585 | 591 | 592 | 593 | 631 | 654)
}

fn default_setting(static_effect: bool) -> Value {
    if static_effect {
        json!({"color1":"#00ff00"})
    } else {
        json!({"color1":"#00ff00", "color2":"no-color", "isRandom":false})
    }
}

/// Mirrors the mounted `change(name, activeColor, inactiveColor)` callback.
/// The gradient is the source's Extended_Breathing_Effect (194), never Static.
fn selected_settings(
    static_effect: bool,
    colors: Option<(String, String)>,
) -> (Value, Option<Value>) {
    let mut selected_id = if static_effect { 1 } else { 2 };
    let mut inactive_id = if static_effect { 2 } else { 1 };
    let mut selected = default_setting(static_effect);
    let mut inactive = default_setting(!static_effect);
    let inactive = colors.map(|(active_color, inactive_color)| {
        if active_color == PRESETS[3] {
            selected_id = 194;
        }
        if inactive_color == PRESETS[3] {
            inactive_id = 194;
        }
        selected["color1"] = json!(active_color);
        inactive["color1"] = json!(inactive_color);
        json!({"inActiveEffectId":inactive_id, "inActiveEffectSetting":inactive})
    });
    (
        json!({"selectedEffectId":selected_id, "selectedEffectSetting":selected}),
        inactive,
    )
}

impl KeyboardProductWorkspace {
    fn change_radio_effect(
        &mut self,
        static_effect: bool,
        color: Option<&'static str>,
        cx: &mut Context<Self>,
    ) {
        if self.lighting_controls_disabled() {
            return;
        }
        let active_color = self.draft["quickEffects"]["selectedEffectSetting"]["color1"].as_str();
        let inactive_color =
            self.draft["inActiveQuickEffect"]["inActiveEffectSetting"]["color1"].as_str();
        // Radio check swaps active/inactive colors. Clicking an active preset
        // changes only its color and retains the inactive effect's color.
        let colors = if self.spec.product_id == 654 {
            match color {
                Some(color) => inactive_color.map(|inactive| (color.into(), inactive.into())),
                None => inactive_color
                    .zip(active_color)
                    .map(|(a, b)| (a.into(), b.into())),
            }
        } else {
            None
        };
        let (selected, inactive) = selected_settings(static_effect, colors);
        let mut request = json!({"selectedEffectId":selected["selectedEffectId"],
            "selectedEffectSetting":selected["selectedEffectSetting"], "regionId":0});
        if let Some(inactive) = &inactive {
            request["inActiveEffectId"] = inactive["inActiveEffectId"].clone();
            request["inActiveEffectSetting"] = inactive["inActiveEffectSetting"].clone();
        }
        if let Some(profile) = self.draft.as_object_mut() {
            let target = profile.entry("quickEffects").or_insert_with(|| json!({}));
            if let Some(target) = target.as_object_mut() {
                target.extend(selected.as_object().expect("effect object").clone());
            }
            if let Some(inactive) = inactive {
                profile.insert("inActiveQuickEffect".into(), inactive);
            }
            self.request_lighting_message("ON_SET_EFFECTS", json!({"effectSetting":request}), cx);
            cx.emit(KeyboardProductChanged);
            cx.notify();
        }
    }

    pub(super) fn radio_effects_panel(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let static_effect = self.draft["quickEffects"]["selectedEffectId"] == 1;
        let custom = self.spec.product_id == 654;
        let mut body = v_flex()
            .h(surface::css(if custom { 90. } else { 50. }))
            .justify_between();
        for (is_static, label, id) in [
            (false, "BREATHING", "CheckBreathing"),
            (true, "STATIC", "CheckStatic"),
        ] {
            let active = static_effect == is_static;
            body = body.child(
                Radio::new(id)
                    .label(t(label))
                    .checked(active)
                    .disabled(self.lighting_controls_disabled())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if (this.draft["quickEffects"]["selectedEffectId"] == 1) != is_static {
                            this.change_radio_effect(is_static, None, cx);
                        }
                    })),
            );
            if custom {
                body = body.child(self.radio_effect_presets(is_static, active, window, cx));
            }
        }
        surface::panel_with_help(
            t("EFFECTS"),
            surface::help_control("keyboard-radio-effects-help", t("EFFECTS_TOOLTIP_RADIO")),
            cx,
        )
        .child(body)
        .into_any_element()
    }

    fn radio_effect_presets(
        &self,
        static_effect: bool,
        active: bool,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let unit = window.rem_size() / 16.;
        let selected = if active {
            self.draft["quickEffects"]["selectedEffectSetting"]["color1"].as_str()
        } else {
            self.draft["inActiveQuickEffect"]["inActiveEffectSetting"]["color1"].as_str()
        };
        h_flex()
            .w(surface::css(if static_effect { 82. } else { 112. }))
            .mx(surface::css(54.))
            .my(surface::css(6.))
            .justify_between()
            .children(
                PRESETS
                    .iter()
                    .take(if static_effect { 3 } else { 4 })
                    .enumerate()
                    .map(|(index, &color)| {
                        let is_selected = selected == Some(color);
                        let colors = [Colors::green(), Colors::blue(), Colors::cyan()];
                        div()
                            .id(SharedString::from(format!(
                                "keyboard-effect-preset-{static_effect}-{index}"
                            )))
                            .relative()
                            .left(surface::css(-3.))
                            .top(surface::css(-1.))
                            .size(surface::css(20.))
                            .flex_shrink_0()
                            .rounded(surface::css(3.))
                            .border_1()
                            .border_color(if is_selected {
                                Colors::black()
                            } else {
                                Colors::black().opacity(0.3)
                            })
                            .opacity(if active { 1. } else { 0.3 })
                            .when(active, |preset| preset.cursor_pointer())
                            .when(active, |preset| {
                                preset
                                    .hover(move |preset| {
                                        preset.border_color(Colors::black()).shadow(vec![
                                            BoxShadow {
                                                color: Colors::white(),
                                                offset: point(px(0.), px(0.)),
                                                blur_radius: px(0.),
                                                spread_radius: unit * 2.,
                                                inset: false,
                                            },
                                        ])
                                    })
                                    .active(|preset| preset.opacity(0.7))
                            })
                            .when(index < 3, |preset| preset.bg(colors[index.min(2)]))
                            .when(index == 3, |preset| {
                                preset.child(
                                    h_flex()
                                        .size_full()
                                        .rounded(surface::css(2.))
                                        .overflow_hidden()
                                        .children((0..2).map(|part| {
                                            div().flex_1().h_full().bg(linear_gradient(
                                                90.,
                                                linear_color_stop(colors[part], 0.),
                                                linear_color_stop(colors[part + 1], 1.),
                                            ))
                                        })),
                                )
                            })
                            .when(is_selected, |preset| {
                                preset
                                    .shadow(vec![BoxShadow {
                                        color: Colors::white(),
                                        offset: point(px(0.), px(0.)),
                                        blur_radius: px(0.),
                                        spread_radius: unit * 2.,
                                        inset: false,
                                    }])
                                    .child(
                                        div()
                                            .absolute()
                                            .left(surface::css(4.))
                                            .top(surface::css(4.))
                                            .size(surface::css(10.))
                                            .border_2()
                                            .border_color(Colors::white())
                                            .rounded(surface::css(5.))
                                            .shadow(
                                                [false, true]
                                                    .map(|inset| BoxShadow {
                                                        color: Colors::black().opacity(179. / 255.),
                                                        offset: point(px(0.), px(0.)),
                                                        blur_radius: unit * 2.,
                                                        spread_radius: unit,
                                                        inset,
                                                    })
                                                    .to_vec(),
                                            ),
                                    )
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if (this.draft["quickEffects"]["selectedEffectId"] == 1)
                                    == static_effect
                                {
                                    this.change_radio_effect(static_effect, Some(color), cx);
                                }
                            }))
                    }),
            )
            .into_any_element()
    }
}
