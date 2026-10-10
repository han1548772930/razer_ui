//! Independently audited 678/679/688 Analog Gamepad Customize and Lighting callers.
//! Product differences remain in statically extracted source metadata.
use super::*;
use gpui_kit::base::Button as BaseButton;

#[derive(Default)]
pub(super) struct State {
    pub(super) effect_menu: bool,
    pub(super) advanced: bool,
    pub(super) gamepad: Option<KeyboardGamepadTesterObservation>,
    pub(super) quick_conflict: bool,
    pub(super) layout: Option<u32>,
    pub(super) key_pressed: bool,
}

/// Publish only host callback values; None means no observed controller data.
#[derive(Clone, Copy)]
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
                                    .child(div().mb(surface::css(10.)).underline().child(t(title)))
                                    .children(keys.map(|key| {
                                        div().mb(surface::css(5.)).child(if key == "Fn" {
                                            SharedString::from("Fn")
                                        } else {
                                            t(key).into()
                                        })
                                    }))
                            }),
                        )
                        .into_any_element()
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
        surface::panel_with_control(
            t("QUICK_REMAPPING"),
            surface::help_control(
                "keyboard-679-quick-remap-help",
                t("QUICK_REMAPPING_TOOLTIPS"),
            ),
            cx,
        )
        .mb_0()
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
                        "synapse/keyboard-679-quick-{name}.svg"
                    )))
                    .size(surface::css(44.)),
                )
                .child(div().flex_1().px(surface::css(10.)).child(t(label)))
                .child(
                    BaseButton::new(SharedString::from(format!("keyboard-679-quick-{name}")))
                        .w(surface::css(136.))
                        .h(surface::css(27.))
                        .rounded(surface::css(3.))
                        .bg(rgb(0x707070))
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
            // 60481.r is analogV1; audited callers are analogV2 and select this curve.
            let mut first = json!({"outputType":"controllerGroup","actuationPoint":points,
                "controllerGroup":{"controllerMode":{"isAnalog":true},"analogSensitivityType":"STANDARD",
                    "analogSensitivityList":{"analogSensitivityAssignment":[{"x":0.1,"y":0},{"x":1.4,"y":86},{"x":2.8,"y":176},{"x":4,"y":255}]},
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
        if self.spec.analog_gamepad_layout() {
            self.analog_gamepad.gamepad = observation;
            cx.notify();
        }
    }

    pub(super) fn huntsman_effects(&self, cx: &Context<Self>) -> AnyElement {
        let advanced = self.analog_gamepad.advanced;
        let selected = self.draft["quickEffects"]["selectedEffectId"]
            .as_u64()
            .unwrap_or(3);
        let effects = self.spec.config["QUICK_EFFECTS"].as_array();
        let selected_name = effects
            .and_then(|effects| effects.iter().find(|effect| effect["id"] == selected))
            .and_then(|effect| effect["name"].as_str())
            .unwrap_or("SPECTRUM_CYCLING");
        let tabs = h_flex()
            .h(surface::css(36.))
            .p(surface::css(5.))
            .border_1()
            .border_color(rgb(0x5d5d5d))
            .rounded(surface::css(18.))
            .bg(rgb(0x111111))
            .children([(false, "QUICK_EFFECTS"), (true, "ADVANCED_EFFECTS")].map(
                |(mode, label)| {
                    BaseButton::new(SharedString::from(format!(
                        "keyboard-679-effects-tab-{mode}"
                    )))
                    .h(surface::css(26.))
                    .px(surface::css(10.))
                    .py(surface::css(5.))
                    .rounded(surface::css(13.))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(16.))
                    .bg(if advanced == mode {
                        rgb(0x44d62c).into()
                    } else {
                        rgba(0x00000000).into()
                    })
                    .text_color(if advanced == mode {
                        rgb(0x212121)
                    } else {
                        rgb(0xcccccc)
                    })
                    .child(t(label))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.analog_gamepad.advanced = mode;
                        this.analog_gamepad.effect_menu = false;
                        cx.notify();
                    }))
                },
            ));
        let mut body = v_flex().mt(surface::css(20.));
        if advanced {
            // Installation/Chroma profiles must come from a real module observation.
            // The installation branch does not start an installer from a local draft.
            body = body.child(div().child(t("ADVANCED_EFFECTS_MSG")));
        } else {
            body = body
                .child(div().mb(surface::css(20.)).child(t("QUICK_EFFECTS_MSG")))
                .child(
                    h_flex()
                        .items_center()
                        .gap(surface::css(20.))
                        .child(
                            v_flex()
                                .w(surface::css(150.))
                                .relative()
                                .child(
                                    BaseButton::new("keyboard-679-effect-dropdown")
                                        .w_full()
                                        .h(surface::css(27.))
                                        .px(surface::css(5.))
                                        .py(surface::css(4.))
                                        .border_1()
                                        .border_color(if self.analog_gamepad.effect_menu {
                                            rgb(0x44d62c)
                                        } else {
                                            rgb(0x515151)
                                        })
                                        .text_size(surface::css(14.))
                                        .line_height(surface::css(17.))
                                        .flex()
                                        .justify_between()
                                        .items_center()
                                        .child(t(selected_name))
                                        .child(
                                            img("synapse/keyboard-679-expand.svg")
                                                .w(surface::css(10.))
                                                .h(surface::css(10.)),
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.analog_gamepad.effect_menu =
                                                !this.analog_gamepad.effect_menu;
                                            cx.notify();
                                        })),
                                )
                                .when(self.analog_gamepad.effect_menu, |dropdown| {
                                    dropdown.child(
                                        div()
                                            .id("keyboard-679-effect-options")
                                            .absolute()
                                            .top(surface::css(27.))
                                            .left_0()
                                            .w(surface::css(150.))
                                            .max_h(surface::css(180.))
                                            .overflow_y_scroll()
                                            .bg(rgb(0x000000))
                                            .border_1()
                                            .border_color(rgb(0x5d5d5d))
                                            .children(effects.into_iter().flatten().filter_map(
                                                |effect| {
                                                    let id = effect["id"].as_u64()?;
                                                    let label = effect["name"].as_str()?;
                                                    Some(
                                                        BaseButton::new(SharedString::from(
                                                            format!(
                                                                "keyboard-679-effect-option-{id}"
                                                            ),
                                                        ))
                                                        .w_full()
                                                        .h(surface::css(25.))
                                                        .px(surface::css(5.))
                                                        .py(surface::css(4.))
                                                        .text_size(surface::css(14.))
                                                        .line_height(surface::css(17.))
                                                        .bg(if id == selected {
                                                            rgba(0xffffff1a)
                                                        } else {
                                                            rgba(0x00000000)
                                                        })
                                                        .hover(|button| button.bg(rgba(0xffffff1a)))
                                                        .child(t(label))
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.analog_gamepad.effect_menu =
                                                                    false;
                                                                this.select_huntsman_effect(id, cx);
                                                            },
                                                        )),
                                                    )
                                                },
                                            )),
                                    )
                                }),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .opacity(0.3)
                                .child(
                                    img("synapse/keyboard-679-chroma-sync.svg")
                                        .size(surface::css(26.))
                                        .mr(surface::css(10.)),
                                )
                                .child(div().underline().child(t("ONLY_ONE_CHROMA_DEVICE"))),
                        ),
                )
                .children(self.huntsman_effect_settings(selected, cx));
        }
        surface::panel_with_control(
            t("EFFECTS"),
            surface::help_control("keyboard-679-effects-help", t("EFFECTS_TOOLTIP")),
            cx,
        )
        .mb_0()
        .child(tabs)
        .child(body)
        .into_any_element()
    }

    fn select_huntsman_effect(&mut self, id: u64, cx: &mut Context<Self>) {
        let previous = self.draft["quickEffects"]["effectSettings"]
            .as_array()
            .and_then(|settings| settings.iter().find(|setting| setting["effectId"] == id))
            .cloned();
        let setting = previous.unwrap_or_else(|| {
            let mut value = match id {
                1 | 6 => json!({"color1":"#00ff00"}),
                2 => json!({"color1":"#00ff00","color2":"no-color","isRandom":false}),
                5 => json!({"color1":"#00ff00","duration":2}),
                7 => json!({"color1":"#00ff00","color2":"no-color","isRandom":false,"duration":2}),
                4 => json!({"direction":2}),
                13 => json!({"direction":1}),
                11 => json!({"screen":"full"}),
                12 => json!({"color1":"#00ff00","colorBoost":1}),
                17 => json!({"type":"static","color":"#00ff00"}),
                19 => json!({"color1":"#00ff00","color2":"#0000FF","isRandom":false,"direction":1}),
                _ => json!({}),
            };
            value["effectId"] = json!(id);
            value
        });
        self.draft["quickEffects"]["selectedEffectId"] = json!(id);
        self.draft["quickEffects"]["selectedEffectSetting"] = setting.clone();
        let effects = self.draft["quickEffects"].as_object_mut().unwrap();
        let settings = effects
            .entry("effectSettings")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .unwrap();
        if !settings.iter().any(|setting| setting["effectId"] == id) {
            settings.push(setting);
        }
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }

    fn huntsman_effect_settings(&self, id: u64, cx: &Context<Self>) -> Option<AnyElement> {
        // Original 679 Wave is Ip: just the two direction icons, no speed control.
        if id != 4 {
            return None;
        }
        let direction = self.draft["quickEffects"]["selectedEffectSetting"]["direction"]
            .as_u64()
            .unwrap_or(2);
        Some(h_flex().mt(surface::css(20.)).gap(surface::css(20.))
            .child(div().child(t("DIRECTION")))
            .child(h_flex().border_1().border_color(rgb(0x5d5d5d)).rounded(surface::css(3.))
                .children([(1,"left"),(2,"right")].map(|(value,name)| {
                    let selected = direction == value;
                    BaseButton::new(SharedString::from(format!("keyboard-679-wave-{name}")))
                        .w(surface::css(51.)).h(surface::css(25.)).flex().items_center().justify_center()
                        .bg(if selected { rgb(0x44d62c) } else { rgb(0x111111) })
                        .child(img(SharedString::from(format!("synapse/direction-{name}{}.svg",if selected {"-active"} else {""}))).size(surface::css(20.)))
                        .on_click(cx.listener(move |this,_,_,cx| {
                            this.draft["quickEffects"]["selectedEffectSetting"]["direction"] = json!(value);
                            if let Some(settings) = this.draft["quickEffects"]["effectSettings"].as_array_mut() {
                                for setting in settings.iter_mut().filter(|setting|setting["effectId"] == 4) { setting["direction"] = json!(value); }
                            }
                            cx.emit(KeyboardProductChanged);cx.notify();
                        }))
                }))).into_any_element())
    }

    pub(super) fn huntsman_gamepad_tester(&self, cx: &Context<Self>) -> AnyElement {
        let observed = self.analog_gamepad.gamepad;
        surface::panel_with_control(
            t("KEYBOARD_ANALOG_OPTIONS"),
            surface::help_control("keyboard-679-gamepad-help", t("GAMEPAD_TESTER_TOOLTIPS")),
            cx,
        )
        .mb_0()
        .child(div().mb(surface::css(6.)).child(t("GAMEPAD_TESTER")))
        .child(
            div()
                .mb(surface::css(16.))
                .text_color(rgb(0x999999))
                .child(t("GAMEPAD_TESTER_DESC")),
        )
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
                "synapse/keyboard-679-trigger-{name}.svg"
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
