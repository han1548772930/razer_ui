//! Parameter controls mounted by the current keyboard QuickEffects renderer.
use super::*;
use crate::features::controls::Choice;
use crate::features::lighting_color::LightingColorPicker;
use gpui_kit::component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_kit::component::select::{SelectEvent, SelectState};
use razer_widgets::keyboard_navigation::Button as BaseButton;
use razer_widgets::stepper::{Stepper, StepperEvent};
use razer_widgets::theme::{KeyboardEffectColors, KeyboardRgbEffectColors as Colors};

pub(super) struct State {
    pub(super) effect: Entity<SelectState<Vec<Choice>>>,
    pub(super) chroma_profile: Entity<SelectState<Vec<Choice>>>,
    colors: [Entity<ColorPickerState>; 2],
    duration: Entity<SliderState>,
    boost: Entity<Stepper>,
}

pub(super) fn default_setting(id: u64) -> Value {
    match id {
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
    }
}

impl KeyboardProductWorkspace {
    pub(super) fn init_rgb_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if lighting_effects::radio_effects(self.spec.product_id) {
            return;
        }
        let effect = cx.new(|cx| SelectState::new(self.rgb_choices(), None, window, cx));
        let chroma_profile = cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx));
        let colors = std::array::from_fn(|_| cx.new(|cx| ColorPickerState::new(window, cx)));
        let duration = cx.new(|_| {
            SliderState::new()
                .min(1.)
                .max(3.)
                .step(1.)
                .default_value(2.)
        });
        let boost = cx.new(|cx| {
            Stepper::new(
                "keyboard-effect-boost",
                1.,
                (0.25, 4., 0.25),
                true,
                true,
                Some(4),
                window,
                cx,
            )
            .in_modes_area()
            .reveal_spinners_on_hover()
        });
        for (channel, color) in colors.iter().enumerate() {
            self.subscriptions.push(cx.subscribe_in(
                color,
                window,
                move |this, _, event, window, cx| {
                    let id = this.rgb_effect_id();
                    let setting = this.rgb_setting();
                    if this.syncing
                        || this.rgb_quick_disabled()
                        || !matches!(id, 1 | 2 | 5 | 6 | 7 | 17 | 19)
                        || (id != 17 && setting["isRandom"] == true)
                        || (id == 17 && setting["type"] == "gradient pattern")
                    {
                        return;
                    }
                    let ColorPickerEvent::Change(color) = event;
                    if color.is_none() && matches!(id, 1 | 5 | 6 | 17) {
                        return;
                    }
                    let color = color
                        .map(|color| {
                            let c = Rgba::from(color);
                            format!(
                                "#{:02x}{:02x}{:02x}",
                                (c.r * 255.).round() as u8,
                                (c.g * 255.).round() as u8,
                                (c.b * 255.).round() as u8
                            )
                        })
                        .unwrap_or_else(|| "no-color".into());
                    let key = if id == 17 {
                        "color"
                    } else if channel == 0 {
                        "color1"
                    } else {
                        "color2"
                    };
                    this.change_rgb_setting(key, json!(color), window, cx);
                },
            ));
        }
        self.subscriptions.push(cx.subscribe_in(
            &duration,
            window,
            |this, _, event, window, cx| {
                if !this.syncing
                    && !this.rgb_quick_disabled()
                    && matches!(this.rgb_effect_id(), 5 | 7)
                {
                    if let SliderEvent::Release(value) = event {
                        this.change_rgb_setting(
                            "duration",
                            json!(value.start().round().clamp(1., 3.) as u64),
                            window,
                            cx,
                        );
                    }
                }
            },
        ));
        self.subscriptions.push(cx.subscribe_in(
            &boost,
            window,
            |this, _, event: &StepperEvent, window, cx| {
                if !this.syncing
                    && !this.rgb_quick_disabled()
                    && this.rgb_effect_id() == 12
                    && event.value.is_finite()
                {
                    this.change_rgb_setting(
                        "colorBoost",
                        json!((event.value.clamp(0.25, 4.) * 4.).ceil() / 4.),
                        window,
                        cx,
                    );
                }
            },
        ));
        self.subscriptions.push(
            cx.subscribe_in(&effect, window, |this, _, event, window, cx| {
                if this.syncing || this.rgb_quick_disabled() {
                    return;
                }
                if let SelectEvent::Confirm(Some(value)) = event {
                    if let Ok(id) = value.parse::<u64>() {
                        if this
                            .rgb_effect_list()
                            .is_some_and(|effects| effects.iter().any(|effect| effect["id"] == id))
                        {
                            this.select_huntsman_effect(id, window, cx);
                        }
                    }
                }
            }),
        );
        self.subscriptions.push(cx.subscribe_in(
            &chroma_profile,
            window,
            |this, _, event, _, cx| {
                if this.syncing || this.rgb_disabled() {
                    return;
                }
                if let SelectEvent::Confirm(Some(id)) = event {
                    let valid = this.lighting_observation.as_ref().is_some_and(|observed| {
                        observed
                            .chroma_profiles
                            .iter()
                            .any(|profile| profile["id"] == *id)
                    });
                    if valid {
                        this.lighting_runtime.chroma_profile_draft = Some(id.clone());
                        this.request_lighting_message(
                            "ON_SET_CHROMA_PROFILE",
                            json!({"selectedChromaProfileGuid":id}),
                            cx,
                        );
                        cx.notify();
                    }
                }
            },
        ));
        self.rgb_controls = Some(State {
            effect,
            chroma_profile,
            colors,
            duration,
            boost,
        });
        self.sync_rgb_controls(window, cx);
    }

    pub(super) fn rgb_effect_id(&self) -> u64 {
        self.draft["quickEffects"]["selectedEffectId"]
            .as_u64()
            .unwrap_or(3)
    }

    pub(super) fn rgb_hardware_effect(&self) -> bool {
        self.observed_bluetooth() == Some(true)
            && self.spec.config["DeviceInfo"]["ble"]["useHardwareEffect"] == true
    }

    pub(super) fn rgb_effect_list(&self) -> Option<&Vec<Value>> {
        self.spec.config[if self.rgb_hardware_effect() {
            "HARDWARE_QUICK_EFFECTS"
        } else {
            "QUICK_EFFECTS"
        }]
        .as_array()
    }

    pub(super) fn rgb_choices(&self) -> Vec<Choice> {
        self.rgb_effect_list()
            .into_iter()
            .flatten()
            .filter_map(|effect| {
                Some(Choice::localized(
                    effect["id"].as_u64()?.to_string(),
                    effect["name"].as_str()?,
                ))
            })
            .collect()
    }

    pub(super) fn default_rgb_setting(&self, id: u64) -> Value {
        // The mounted Tartarus caller explicitly passes this override.
        self.spec.config["DEFAULT_CUSTOM_EFFECT_SETTINGS"]
            .get(id.to_string())
            .filter(|value| value.is_object())
            .cloned()
            .unwrap_or_else(|| default_setting(id))
    }

    pub(super) fn rgb_setting(&self) -> Value {
        let id = self.rgb_effect_id();
        self.draft["quickEffects"]["effectSettings"]
            .as_array()
            .and_then(|settings| settings.iter().find(|setting| setting["effectId"] == id))
            .or_else(|| self.draft["quickEffects"].get("selectedEffectSetting"))
            .cloned()
            .unwrap_or_else(|| self.default_rgb_setting(id))
    }

    pub(super) fn sync_rgb_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &self.rgb_controls else {
            return;
        };
        let setting = self.rgb_setting();
        let id = self.rgb_effect_id();
        let choices = self.rgb_choices();
        let chroma_choices = self.chroma_profile_choices();
        let was_syncing = self.syncing;
        self.syncing = true;
        state.effect.update(cx, |effect, cx| {
            effect.set_items(choices, window, cx);
            effect.set_selected_value(&id.to_string(), window, cx)
        });
        state.chroma_profile.update(cx, |profile, cx| {
            profile.set_items(chroma_choices, window, cx);
            if let Some(id) = self.selected_chroma_profile() {
                profile.set_selected_value(id, window, cx);
            } else {
                profile.set_selected_index(None, window, cx);
            }
        });
        for (channel, color) in state.colors.iter().enumerate() {
            let key = if id == 17 {
                "color"
            } else if channel == 0 {
                "color1"
            } else {
                "color2"
            };
            let value = setting[key]
                .as_str()
                .and_then(|s| s.strip_prefix('#'))
                .filter(|s| s.len() == 6)
                .and_then(|s| u32::from_str_radix(s, 16).ok());
            color.update(cx, |color, cx| {
                if let Some(value) = value {
                    color.set_value(rgb(value), window, cx);
                } else {
                    color.clear_value(window, cx);
                }
            });
        }
        state.duration.update(cx, |slider, cx| {
            slider.set_value(
                setting["duration"].as_f64().unwrap_or(2.) as f32,
                window,
                cx,
            )
        });
        state.boost.update(cx, |boost, cx| {
            boost.sync_value(
                setting["colorBoost"].as_f64().unwrap_or(1.),
                0.25,
                id != 12 || self.rgb_quick_disabled(),
                window,
                cx,
            )
        });
        self.syncing = was_syncing;
    }

    pub(super) fn dismiss_rgb_controls(&self, cx: &mut Context<Self>) {
        if let Some(state) = &self.rgb_controls {
            for color in &state.colors {
                color.update(cx, |color, cx| color.set_open(false, cx));
            }
        }
    }

    fn change_rgb_setting(
        &mut self,
        key: &str,
        value: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.rgb_quick_disabled() {
            return;
        }
        let id = self.rgb_effect_id();
        // Static and the desktop Audio Meter component replace their setting;
        // the other mounted children spread the old setting before the patch.
        let mut setting = if id == 1 || (id == 12 && key == "colorBoost") {
            json!({})
        } else {
            self.rgb_setting()
        };
        setting[key] = value;
        self.store_rgb_setting(id, setting);
        self.request_rgb_effect(cx);
        self.sync_rgb_controls(window, cx);
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }

    pub(super) fn store_rgb_setting(&mut self, id: u64, mut setting: Value) {
        if let Some(object) = setting.as_object_mut() {
            object.remove("effectId");
        }
        self.draft["quickEffects"]["selectedEffectId"] = json!(id);
        self.draft["quickEffects"]["selectedEffectSetting"] = setting.clone();
        let Some(effects) = self.draft["quickEffects"].as_object_mut() else {
            return;
        };
        let settings = effects.entry("effectSettings").or_insert_with(|| json!([]));
        let Some(settings) = settings.as_array_mut() else {
            return;
        };
        settings.retain(|setting| setting["effectId"] != id);
        setting["effectId"] = json!(id);
        settings.push(setting);
    }

    pub(super) fn rgb_parameters(&self, cx: &Context<Self>) -> AnyElement {
        let Some(state) = &self.rgb_controls else {
            return div().into_any_element();
        };
        let id = self.rgb_effect_id();
        let setting = self.rgb_setting();
        match id {
            1 | 2 | 5 | 6 | 7 | 17 | 19 => {
                let multiple = matches!(id, 2 | 7 | 19);
                let random = setting["isRandom"] == true;
                let analog_v2 = self.spec.config["DeviceInfo"]["AnalogGenVersion"] == "analogV2";
                let gradient = setting["type"] == "gradient pattern";
                let mut controls = h_flex().items_start().flex_wrap();
                for channel in 0..if multiple { 2 } else { 1 } {
                    let owner = self.lighting_owner();
                    let label = if multiple {
                        t("COLOR_DROP_NAME").replace("{{num}}", &(channel + 1).to_string())
                    } else {
                        t("COLOR")
                    };
                    controls = controls.child(
                        v_flex()
                            .mt(surface::css(20.))
                            .mr(surface::css(20.))
                            .child(label.clone())
                            .child(
                                div().mt(surface::css(5.)).child(
                                    LightingColorPicker::new(&state.colors[channel], label)
                                        .allow_none(multiple)
                                        .on_custom_color_edit(cx.listener(
                                            move |this,
                                                  edit: &razer_model::preferences::CustomColorEdit,
                                                  _,
                                                  cx| {
                                                this.commit_rgb_custom_colors(&owner, *edit, cx);
                                            },
                                        ))
                                        .disabled(
                                            self.rgb_quick_disabled()
                                                || if id == 17 { gradient } else { random },
                                        ),
                                ),
                            ),
                    );
                }
                if multiple || id == 17 {
                    controls = controls.child(
                        div().mt(surface::css(45.)).child(
                            Checkbox::new("keyboard-effect-random")
                                .disabled(self.rgb_quick_disabled())
                                .label(t(if id == 17 && analog_v2 {
                                    "GRADIENT_PATTERN"
                                } else {
                                    "RANDOM_COLOR"
                                }))
                                .checked(if id == 17 && analog_v2 { gradient } else { random })
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    if id == 17 && analog_v2 {
                                        this.change_rgb_setting(
                                            "type",
                                            json!(if gradient {
                                                "static"
                                            } else {
                                                "gradient pattern"
                                            }),
                                            window,
                                            cx,
                                        );
                                    } else {
                                        this.change_rgb_setting(
                                            "isRandom",
                                            json!(!random),
                                            window,
                                            cx,
                                        );
                                    }
                                })),
                        ),
                    );
                }
                if matches!(id, 5 | 7) {
                    controls = controls.child(self.rgb_duration());
                }
                v_flex()
                    .child(controls)
                    .when(id == 19, |view| view.child(self.rgb_direction(19, cx)))
                    .into_any_element()
            }
            4 | 13 => self.rgb_direction(id, cx),
            11 => self.rgb_screen(cx),
            12 => v_flex()
                .mt(surface::css(20.))
                .child(t("TEXT_COLOR_BOOST"))
                .child(div().mt(surface::css(10.)).child(state.boost.clone()))
                .into_any_element(),
            // Spectrum and Fire render no parameter controls in these callers.
            _ => div().into_any_element(),
        }
    }

    fn rgb_duration(&self) -> AnyElement {
        let state = self.rgb_controls.as_ref().expect("RGB controls");
        v_flex()
            .mt(surface::css(20.))
            .ml(surface::css(10.))
            .when(self.rgb_effect_id() == 7, |control| {
                control.w(surface::css(230.))
            })
            .when(self.rgb_effect_id() == 5, |control| control.flex_1())
            .child(t("TEXT_DURATION"))
            .child(Slider::new(&state.duration).disabled(self.rgb_quick_disabled()))
            .child(surface::slider_tags(
                &t("TEXT_SLIDER_SHORT"),
                Some(&t("TEXT_SLIDER_MEDIUM")),
                &t("TEXT_SLIDER_LONG"),
                None,
            ))
            .into_any_element()
    }

    fn rgb_direction(&self, id: u64, cx: &Context<Self>) -> AnyElement {
        let setting = self.rgb_setting();
        let direction = setting["direction"].as_u64();
        let choices = match id {
            13 => [(1, "cw"), (2, "ccw")],
            19 => [(1, "out"), (0, "in")],
            _ => [(1, "left"), (2, "right")],
        };
        v_flex()
            .mt(surface::css(20.))
            .child(t("TEXT_DIRECTION"))
            .child(
                h_flex()
                    .mt(surface::css(5.))
                    .border_1()
                    .border_color(Colors::border())
                    .rounded(surface::css(3.))
                    .children(choices.map(|(value, name)| {
                        let selected = direction == Some(value);
                        BaseButton::new(SharedString::from(format!(
                            "keyboard-effect-direction-{name}"
                        )))
                        .disabled(self.rgb_quick_disabled())
                        .w(surface::css(51.))
                        .h(surface::css(25.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(if selected {
                            Colors::accent()
                        } else {
                            Colors::background()
                        })
                        .child(
                            img(SharedString::from(format!(
                                "synapse/direction-{name}{}.svg",
                                if selected { "-active" } else { "" }
                            )))
                            .size(surface::css(20.)),
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.change_rgb_setting("direction", json!(value), window, cx)
                            },
                        ))
                    })),
            )
            .into_any_element()
    }

    fn rgb_screen(&self, cx: &Context<Self>) -> AnyElement {
        let setting = self.rgb_setting();
        v_flex()
            .relative()
            .mt(surface::css(20.))
            .h(surface::css(60.))
            .child(t("TEXT_SCREEN_REGION"))
            .children(
                ["full", "left", "top", "right", "bottom"]
                    .into_iter()
                    .enumerate()
                    .map(|(index, region)| {
                        let selected = setting["screen"] == region;
                        BaseButton::new(SharedString::from(format!(
                            "keyboard-effect-screen-{region}"
                        )))
                        .disabled(self.rgb_quick_disabled())
                        .absolute()
                        .top(surface::css(40.))
                        .left(surface::css(index as f32 * 30.))
                        .w(surface::css(24.))
                        .h(surface::css(16.))
                        .p_0()
                        .border(surface::css(if selected { 1.5 } else { 1. }))
                        .border_color(if selected {
                            Colors::accent()
                        } else {
                            Colors::screen_border()
                        })
                        .when(region == "full", |screen| {
                            screen.bg(KeyboardEffectColors::white())
                        })
                        .when(region != "full", |screen| {
                            screen.child(
                                div()
                                    .absolute()
                                    .bg(KeyboardEffectColors::white())
                                    .when(matches!(region, "left" | "right"), |stripe| {
                                        stripe.w(surface::css(4.)).h_full()
                                    })
                                    .when(matches!(region, "top" | "bottom"), |stripe| {
                                        stripe.h(surface::css(4.)).w_full()
                                    })
                                    .when(region == "right", |stripe| stripe.right_0())
                                    .when(region == "bottom", |stripe| stripe.bottom_0())
                                    .when(matches!(region, "top" | "left"), |stripe| {
                                        stripe.top_0().left_0()
                                    }),
                            )
                        })
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.change_rgb_setting("screen", json!(region), window, cx)
                            },
                        ))
                    }),
            )
            .into_any_element()
    }
}
