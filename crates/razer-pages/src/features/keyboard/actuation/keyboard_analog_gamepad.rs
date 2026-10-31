//! Independently audited 678/679/688 Analog Gamepad Customize and Lighting callers.
//! Product differences remain in statically extracted source metadata.
use super::*;
use razer_widgets::keyboard_navigation::Button as BaseButton;
use razer_widgets::theme::KeyboardAnalogControlColors as Colors;

#[path = "../mapping/keyboard_mapping_drawer.rs"]
mod mapping_drawer;
pub use mapping_drawer::KeyboardMappingAiObservation;
pub use mapping_drawer::KeyboardMappingInterDeviceConfigureRequested;
pub use mapping_drawer::KeyboardMappingInterDeviceObservation;
pub use mapping_drawer::KeyboardMappingMacroObservation;
pub use mapping_drawer::KeyboardMappingProfilesObservation;
pub(super) use mapping_drawer::KeypadSurface;
pub(super) use mapping_drawer::Next as MappingNext;
pub use mapping_drawer::source_keypad_product_image;
pub use mapping_drawer::{KeyboardMappingDialObservation, KeyboardMappingDialReadRequested};
pub use mapping_drawer::{
    KeyboardMappingSensitivityWarningHidden, KeyboardMappingSensitivityWarningReadRequested,
};

#[derive(Default)]
pub(super) struct State {
    pub(super) floating: floating_controller::State,
    mapping: mapping_drawer::State,
    pub(super) effect_menu: bool,
    pub(super) gamepad: Option<KeyboardGamepadTesterObservation>,
    pub(super) quick_conflict: bool,
    pub(super) layout: Option<u32>,
    pub(super) key_pressed: bool,
}

/// Mapping output callback values, not physical controller readback.
/// None means the current owner has not published an output sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyboardGamepadTesterObservation {
    pub left_thumb_x: i16,
    pub left_thumb_y: i16,
    pub right_thumb_x: i16,
    pub right_thumb_y: i16,
    pub left_trigger: u8,
    pub right_trigger: u8,
}

impl KeyboardProductWorkspace {
    pub(super) fn huntsman_mod_tap(&self, cx: &Context<Self>) -> AnyElement {
        let enabled = self.boolean("/modTap");
        let japanese = self.analog_gamepad.layout == Some(12);
        let help = BaseButton::new("keyboard-mod-tap-help")
            .size(surface::css(14.))
            .p_0()
            .rounded_full()
            .bg(rgb(0x4a4a4a))
            .accessibility_label(t("MOD_TAP"))
            .child(img("synapse/automation-tooltip_questionmark.svg").size_full())
            .tooltip(move |window, cx| {
                gpui_kit::component::tooltip::Tooltip::element(move |_, _| {
                    razer_assets::resource_scope(
                        h_flex()
                            .p(surface::css(10.))
                            .border_1()
                            .border_color(rgb(0x5d5d5d))
                            .rounded(surface::css(3.))
                            .bg(rgb(0x000000))
                            .text_color(rgb(0xffffff))
                            .gap(surface::css(20.))
                            .children(
                                [
                                    (
                                        "PRIMARY",
                                        [
                                            "RIGHT_SHIFT",
                                            "RIGHT_ALT",
                                            if japanese { "Fn" } else { "MENU" },
                                            "RIGHT_CTRL",
                                        ],
                                    ),
                                    ("SECONDARY", ["UP", "LEFT", "DOWN", "RIGHT"]),
                                ]
                                .map(|(title, keys)| {
                                    v_flex()
                                        .child(
                                            div().mb(surface::css(10.)).underline().child(t(title)),
                                        )
                                        .children(keys.map(|key| {
                                            div().mb(surface::css(5.)).child(if key == "Fn" {
                                                SharedString::from("Fn")
                                            } else {
                                                t(key).into()
                                            })
                                        }))
                                }),
                            )
                            .into_any_element(),
                    )
                })
                .build(window, cx)
            });
        surface::panel_with_title_switch(
            t("MOD_TAP"),
            surface::SynapseSwitch::new("keyboard-mod-tap-enabled")
                .checked(enabled)
                .disabled(self.analog_gamepad.key_pressed)
                .on_change(cx.listener(move |this, _, _, cx| {
                    if this.analog_gamepad.key_pressed {
                        return;
                    }
                    let Some(mapping_list) = this.draft.pointer(&this.mapping_path()).cloned() else {
                        return;
                    };
                    // Hl changes the UI reducer immediately; this is a local draft,
                    // while the separately emitted middleware request can fail.
                    this.write("/modTap", json!(!enabled), cx);
                    if let Some(state) = &mut this.snap_tap {
                        state.set_mod_tap(!enabled);
                    }
                    this.request_actuation_message(json!({"type":"ON_SET_MOD_TAP", "payload":{"mappingList":mapping_list,"isEnabled":!enabled}}), cx);
                })),
            help,
            cx,
        )
        .mb_0()
        .child(h_flex()
            .gap(surface::css(20.))
            .child(img("synapse/keyboard-688-mod-tap.svg").w(surface::css(141.)).h(surface::css(74.)).flex_shrink_0())
            .child(div().flex_1().child(t("MOD_TAP_DESC"))))
        .into_any_element()
    }

    pub(super) fn huntsman_quick_remapping(&self, cx: &Context<Self>) -> AnyElement {
        surface::panel_with_help(
            t("QUICK_REMAPPING"),
            surface::help_control(
                "keyboard-analog-quick-remap-help",
                t("QUICK_REMAPPING_TOOLTIPS"),
            ),
            cx,
        )
        .mb_0()
        .child(self.huntsman_quick_mapping_body(cx))
        .into_any_element()
    }

    pub(super) fn huntsman_quick_gamepad_settings(&self, cx: &Context<Self>) -> AnyElement {
        // Current742 Xt/Jt and756 ha own one widget; the tester has no
        // independent title bar, padding, border or bottom margin.
        surface::panel_with_help(
            t("GAMEPAD_SETTINGS"),
            surface::help_control(
                "keyboard-analog-gamepad-settings-help",
                t("GAMEPAD_SETTINGS_TOOLTIPS"),
            ),
            cx,
        )
        .mb_0()
        .child(self.huntsman_quick_mapping_body(cx))
        .child(
            h_flex()
                .items_start()
                .gap(surface::css(5.))
                .pt(surface::css(20.))
                .text_color(Colors::muted())
                .child(
                    img("synapse/keyboard-gamepad-settings-info.svg")
                        .size(surface::css(20.))
                        .m(surface::css(2.))
                        .flex_shrink_0(),
                )
                .child(t("GAMEPAD_SETTINGS_WARN")),
        )
        .child(
            div()
                .w(surface::css(508.))
                .border_1()
                .border_color(Colors::border())
                .mx_auto()
                .mt(surface::css(14.))
                .mb(surface::css(24.)),
        )
        .child(self.huntsman_gamepad_tester_body(true))
        .into_any_element()
    }

    fn huntsman_quick_mapping_body(&self, cx: &Context<Self>) -> AnyElement {
        let mappings = self
            .draft
            .pointer(&self.mapping_path())
            .and_then(Value::as_array);
        let groups: [(&str, &str, &[(&str, &str)]); 2] = [
            (
                "wasd",
                "BIND_WASD",
                &[
                    ("KEY_W", "LEFT_JS_UP"),
                    ("KEY_A", "LEFT_JS_LEFT"),
                    ("KEY_S", "LEFT_JS_DOWN"),
                    ("KEY_D", "LEFT_JS_RIGHT"),
                ],
            ),
            (
                "qe",
                "BIND_QE",
                &[("KEY_Q", "LEFTTRIGGER"), ("KEY_E", "RIGHTTRIGGER")],
            ),
        ];
        div()
            .child(div().child(t("QUICK_MAPPING_TITLE")))
            .children(groups.map(|(name, label, keys)| {
                let active = mappings.is_some_and(|mappings| {
                    mappings.iter().any(|mapping| {
                        mapping["isHyperShift"].as_bool().unwrap_or(false) == self.hypershift
                        && keys.iter().any(|(id, assignment)| {
                            mapping["inputID"] == *id
                                && mapping["mapping"][0]["controllerGroup"]["controllerAssignment"]
                                    == *assignment
                        })
                    })
                });
                h_flex()
                    .items_center()
                    .py(surface::css(10.))
                    .child(
                        img(SharedString::from(format!(
                            "synapse/controller_icon_{name}.svg"
                        )))
                        .size(surface::css(44.)),
                    )
                    .child(div().flex_1().px(surface::css(10.)).child(t(label)))
                    .child(
                        BaseButton::new(SharedString::from(format!(
                            "keyboard-analog-quick-{name}"
                        )))
                        .w(surface::css(136.))
                        .h(surface::css(27.))
                        .rounded(surface::css(3.))
                        .bg(Colors::button())
                        .flex()
                        .justify_center()
                        .items_center()
                        .child(t(if active { "RESET_BTN" } else { "BIND_BTN" }))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.huntsman_apply_quick_mapping(keys, active, cx)
                        })),
                    )
            }))
            .when(self.analog_gamepad.quick_conflict, |panel| {
                panel.child(
                    div()
                        .mt(surface::css(10.))
                        .child(t("CONFLICT_DETECTED"))
                        .child(t("CONFLICT_DETECTED_DESC")),
                )
            })
            .into_any_element()
    }

    fn huntsman_apply_quick_mapping(
        &mut self,
        keys: &'static [(&'static str, &'static str)],
        reset: bool,
        cx: &mut Context<Self>,
    ) {
        let path = self.mapping_path();
        let Some(mut mappings) = self.draft.pointer(&path).and_then(Value::as_array).cloned()
        else {
            return;
        };
        let shift = self.hypershift;
        if !reset
            && mappings.iter().any(|mapping| {
                if mapping["isHyperShift"].as_bool().unwrap_or(false) != shift {
                    return false;
                }
                let Some((_, assignment)) = keys.iter().find(|(id, _)| mapping["inputID"] == *id)
                else {
                    return false;
                };
                let first = &mapping["mapping"][0];
                !(first["keyboardGroup"]
                    .as_object()
                    .is_some_and(|group| group.is_empty())
                    || first["controllerGroup"]["controllerAssignment"] == *assignment)
            })
        {
            // WM refuses to overwrite existing assignments; Ch displays the conflict.
            self.analog_gamepad.quick_conflict = true;
            cx.notify();
            return;
        }
        self.analog_gamepad.quick_conflict = false;
        let (make, release) = self.default_actuation();
        for (id, assignment) in keys {
            let index = mappings.iter().position(|mapping| {
                mapping["inputID"] == *id
                    && mapping["isHyperShift"].as_bool().unwrap_or(false) == shift
            });
            if reset {
                if let Some(index) = index {
                    let first = &mut mappings[index]["mapping"][0];
                    if first["outputType"] == "controllerGroup" {
                        first.as_object_mut().unwrap().remove("controllerGroup");
                        first["outputType"] = json!("keyboardGroup");
                        first["keyboardGroup"] = json!({});
                    }
                }
                continue;
            }
            let existing = index.map(|index| mappings[index].clone());
            let points = existing
                .as_ref()
                .and_then(|mapping| mapping.pointer("/mapping/0/actuationPoint"))
                .cloned()
                .unwrap_or_else(|| json!({"0":make as u32,"1":release as u32}));
            let curve = quick_mapping_curve(&self.spec.config["DeviceInfo"]);
            let mut first = json!({"outputType":"controllerGroup","actuationPoint":points,
                "controllerGroup":{"controllerMode":{"isAnalog":true},"analogSensitivityType":"STANDARD",
                    "analogSensitivityList":{"analogSensitivityAssignment":curve},
                    "controllerAssignment":assignment}});
            if let Some(rapid) = existing
                .as_ref()
                .and_then(|mapping| mapping.pointer("/mapping/0/rapidTrigger"))
            {
                first["rapidTrigger"] = rapid.clone();
            }
            let secondary = existing.as_ref().and_then(|mapping|mapping.pointer("/mapping/1")).cloned().unwrap_or_else(||json!({"outputType":"defaultGroup","isDefault":false,"isHyperShift":false,"actuationPoint":{"0":make as u32,"1":release as u32}}));
            let mapping = json!({"inputID":id,"inputType":"AnalogInput","isHyperShift":shift,"mapping":[first,secondary]});
            if let Some(index) = index {
                mappings[index] = mapping;
            } else {
                mappings.push(mapping);
            }
        }
        if let Some(target) = self.draft.pointer_mut(&path) {
            *target = json!(mappings);
        }
        cx.emit(KeyboardProductChanged);
        self.request_actuation_mapping(cx);
        cx.notify();
    }
    pub fn observe_gamepad_tester(
        &mut self,
        observation: Option<KeyboardGamepadTesterObservation>,
        cx: &mut Context<Self>,
    ) {
        if self.spec.analog_customize_layout() {
            self.analog_gamepad.gamepad = observation;
            cx.notify();
        }
    }

    pub(super) fn huntsman_gamepad_tester(&self, cx: &Context<Self>) -> AnyElement {
        surface::panel_with_help(
            t("KEYBOARD_ANALOG_OPTIONS"),
            surface::help_control("keyboard-analog-gamepad-help", t("GAMEPAD_TESTER_TOOLTIPS")),
            cx,
        )
        .mb_0()
        .child(self.huntsman_gamepad_tester_body(false))
        .into_any_element()
    }

    fn huntsman_gamepad_tester_body(&self, modern: bool) -> AnyElement {
        let observed = self.analog_gamepad.gamepad;
        v_flex()
            .child(div().mb(surface::css(6.)).child(t("GAMEPAD_TESTER")))
            .child(
                div()
                    .mb(surface::css(16.))
                    .text_color(rgb(0x999999))
                    .child(t(if modern {
                        "GAMEPAD_TESTER_DESC_TITLE"
                    } else {
                        "GAMEPAD_TESTER_DESC"
                    })),
            )
            .when(modern, |tester| {
                tester.child(
                    div()
                        .mt(surface::css(10.))
                        .text_color(Colors::muted())
                        .child(t("GAMEPAD_TESTER_DESC")),
                )
            })
            .child(
                h_flex()
                    .items_center()
                    .ml(surface::css(-10.))
                    .mt(surface::css(-8.))
                    .child(joystick(
                        "L",
                        observed.map(|value| (value.left_thumb_x, value.left_thumb_y)),
                    ))
                    .child(joystick(
                        "R",
                        observed.map(|value| (value.right_thumb_x, value.right_thumb_y)),
                    ))
                    .child(
                        v_flex()
                            .ml(surface::css(53.))
                            .child(trigger("left", observed.map(|value| value.left_trigger)))
                            .child(trigger("right", observed.map(|value| value.right_trigger))),
                    ),
            )
            .into_any_element()
    }
}

fn quick_mapping_curve(info: &Value) -> Value {
    // Current7428995 analogSensitivityAssignment uses this exact low-profile
    // default list, not the six-point controllerGraph STANDARD editor curve.
    if info["isLowProfile"] == true && info["defaultActuation"].is_array() {
        return info["defaultActuation"].clone();
    }
    if info["AnalogGenVersion"] == "analogV1" {
        json!([{"x":1.5,"y":0},{"x":2.2,"y":85},{"x":2.9,"y":170},{"x":3.6,"y":255}])
    } else {
        json!([{"x":0.1,"y":0},{"x":1.4,"y":86},{"x":2.8,"y":176},{"x":4,"y":255}])
    }
}

#[cfg(test)]
mod tests {
    use super::quick_mapping_curve;
    use serde_json::json;

    #[::core::prelude::v1::test]
    fn quick_bind_uses_low_profile_defaults_instead_of_controller_graph() {
        let source = super::source_product(742).unwrap();
        let curve = quick_mapping_curve(&source.config["DeviceInfo"]);
        assert_eq!(
            curve,
            json!([{"x":0.1,"y":0},{"x":1,"y":86},{"x":1.9,"y":176},{"x":2.8,"y":255}])
        );
        assert_ne!(
            curve[1]["x"],
            source.config["DeviceInfo"]["analogSpecs"]["controllerGraph"]["STANDARD"]["p2"]["x"]
        );
        assert_eq!(
            quick_mapping_curve(&super::source_product(756).unwrap().config["DeviceInfo"])[3]["x"],
            4
        );
    }
}

fn joystick(label: &'static str, observed: Option<(i16, i16)>) -> AnyElement {
    let position = observed.map(|(x, y)| {
        let mut x = (x as f32 + 32768.) / 65535. * 106. + 11.5;
        let mut y = (-y as f32 + 32768.) / 65535. * 106. + 11.5;
        let length = ((x - 65.).powi(2) + (y - 65.).powi(2)).sqrt();
        if length > 54. {
            x = 65. + (x - 65.) / length * 54.;
            y = 65. + (y - 65.) / length * 54.;
        }
        (x, y)
    });
    div()
        .relative()
        .w(surface::css(130.))
        .h(surface::css(130.))
        .flex_shrink_0()
        .child(
            div()
                .absolute()
                .left(surface::css(11.))
                .top(surface::css(11.))
                .size(surface::css(108.))
                .rounded_full()
                .border_2()
                .border_color(rgb(0x707070)),
        )
        .child(
            div()
                .absolute()
                .left(surface::css(64.))
                .top(surface::css(11.))
                .w(surface::css(2.))
                .h(surface::css(108.))
                .bg(rgb(0x707070)),
        )
        .child(
            div()
                .absolute()
                .left(surface::css(11.))
                .top(surface::css(64.))
                .w(surface::css(108.))
                .h(surface::css(2.))
                .bg(rgb(0x707070)),
        )
        .children(position.map(|(x, y)| {
            div()
                .absolute()
                .left(surface::css(x - 10.))
                .top(surface::css(y - 10.))
                .size(surface::css(20.))
                .rounded_full()
                .border_1()
                .border_color(rgb(0x44d62c))
                .bg(rgb(0x000000))
                .text_size(surface::css(8.))
                .flex()
                .justify_center()
                .items_center()
                .child(label)
        }))
        .into_any_element()
}
fn trigger(name: &'static str, value: Option<u8>) -> AnyElement {
    h_flex()
        .items_center()
        .child(
            img(SharedString::from(format!(
                "synapse/xbox-trigger-{name}.svg"
            )))
            .w(surface::css(34.))
            .mr(surface::css(10.)),
        )
        .child(
            div()
                .w(surface::css(160.))
                .h(surface::css(12.))
                .my(surface::css(20.))
                .rounded(surface::css(15.))
                .bg(rgba(0x44d62c40))
                .children(value.map(|value| {
                    div()
                        .w(surface::css(value as f32 / 255. * 160.))
                        .h_full()
                        .bg(rgb(0x44d62c))
                })),
        )
        .into_any_element()
}

pub use mapping_drawer::{
    KeyboardMappingBrightnessObservation, KeyboardMappingBrightnessReadRequested,
};
