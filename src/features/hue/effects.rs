use super::super::lighting_color::LightingColorPicker;
use super::*;
use gpui_kit::base::{NumberInput, StepAction};

impl HueWorkspace {
    pub(super) fn effect_setting(&self) -> Value {
        self.effect_settings
            .get(&self.selected_effect)
            .or_else(|| {
                spec()
                    .lighting
                    .defaults
                    .get(&self.selected_effect.to_string())
            })
            .cloned()
            .unwrap_or_else(|| json!({}))
    }
    pub(super) fn restore_effects(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_effect = self
            .draft
            .pointer("/quickEffects/selectedEffectId")
            .and_then(Value::as_u64)
            .and_then(|id| {
                spec()
                    .lighting
                    .effects
                    .iter()
                    .find(|effect| effect.id as u64 == id)
                    .map(|effect| effect.id)
            })
            .unwrap_or(3);
        self.effect_settings.clear();
        if let Some(value) = self
            .draft
            .pointer("/quickEffects/selectedEffectSetting")
            .filter(|v| v.is_object())
        {
            self.effect_settings
                .insert(self.selected_effect, value.clone());
        }
        self.sync_effects(window, cx);
    }
    fn effects_editable(&self) -> bool {
        self.bridge.controls_enabled() && !self.advanced
    }
    fn edit_effect(&mut self, field: &str, value: Value, cx: &mut Context<Self>) {
        if !self.effects_editable() {
            return;
        }
        let mut setting = self.effect_setting();
        setting[field] = value;
        self.effect_settings.insert(self.selected_effect, setting);
        self.changed(cx);
    }
    pub(super) fn subscribe_effects(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.subscribe_in(
            &self.effect,
            window,
            |this, _, event, window, cx| {
                if !this.effects_editable() {
                    return;
                }
                if let SelectEvent::Confirm(Some(value)) = event {
                    if let Some(id) = value
                        .parse::<u32>()
                        .ok()
                        .filter(|id| spec().lighting.effects.iter().any(|e| e.id == *id))
                    {
                        this.selected_effect = id;
                        this.sync_effects(window, cx);
                        this.changed(cx);
                    }
                }
            },
        ));
        for channel in 0..2 {
            self.subscriptions.push(cx.subscribe_in(
                &self.colors[channel],
                window,
                move |this, _, event, _, cx| {
                    if ![1, 2].contains(&this.selected_effect)
                        || this.effect_setting()["isRandom"].as_bool() == Some(true)
                    {
                        return;
                    }
                    let ColorPickerEvent::Change(color) = event;
                    if this.selected_effect == 1 && color.is_none() {
                        return;
                    }
                    let value = color
                        .map(|c| {
                            let c = Rgba::from(c);
                            format!(
                                "#{:02x}{:02x}{:02x}",
                                (c.r * 255.).round() as u8,
                                (c.g * 255.).round() as u8,
                                (c.b * 255.).round() as u8
                            )
                        })
                        .unwrap_or_else(|| "no-color".into());
                    this.edit_effect(
                        if channel == 0 { "color1" } else { "color2" },
                        json!(value),
                        cx,
                    );
                },
            ));
        }
        self.subscriptions.push(cx.subscribe_in(
            &self.color_boost,
            window,
            |this, input, event, window, cx| {
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    let value = input.read(cx).value().parse::<f32>().ok();
                    if this.selected_effect == 12 {
                        if let Some(value) = value {
                            this.edit_effect(
                                "colorBoost",
                                json!(super::super::settings::normalize_color_boost(value)),
                                cx,
                            );
                        }
                    }
                    this.sync_effects(window, cx);
                }
            },
        ));
    }
    pub(super) fn sync_effects(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.effect.update(cx, |effect, cx| {
            effect.set_selected_value(&self.selected_effect.to_string(), window, cx)
        });
        let setting = self.effect_setting();
        for (channel, picker) in self.colors.iter().enumerate() {
            let color = setting
                .get(if channel == 0 { "color1" } else { "color2" })
                .and_then(Value::as_str)
                .and_then(|color| color.strip_prefix('#'))
                .filter(|color| color.len() == 6)
                .and_then(|color| u32::from_str_radix(color, 16).ok());
            picker.update(cx, |picker, cx| {
                if let Some(color) = color {
                    picker.set_value(rgb(color), window, cx);
                } else {
                    picker.clear_value(window, cx);
                }
            });
        }
        let boost = setting
            .get("colorBoost")
            .and_then(Value::as_f64)
            .unwrap_or(1.)
            .to_string();
        self.color_boost
            .update(cx, |input, cx| input.set_value(boost, window, cx));
    }
    pub(super) fn effects_card(&self, cx: &Context<Self>) -> AnyElement {
        let enabled = self.bridge.controls_enabled();
        let mut card = widget("EFFECTS", Some("EFFECTS_TOOLTIP"), div(), cx)
            .when(!enabled, |v| v.opacity(0.3))
            .child(
                gpui_kit::base::Tabs::new("hue-effects-tabs")
                    .flex()
                    .w_full()
                    .h(surface::css(36.))
                    .p(surface::css(5.))
                    .border_1()
                    .border_color(Colors::ip_border())
                    .rounded(surface::css(18.))
                    .children([(false, "QUICK_EFFECTS"), (true, "ADVANCED_EFFECTS")].map(
                        |(advanced, key)| {
                            gpui_kit::base::Button::new(key)
                                .role(Role::Tab)
                                .selected(self.advanced == advanced)
                                .disabled(!enabled)
                                .h(surface::css(26.))
                                .px(surface::css(10.))
                                .py_0()
                                .rounded(surface::css(13.))
                                .text_size(surface::css(12.))
                                .when(self.advanced == advanced, |button| {
                                    button
                                        .bg(cx.theme().primary)
                                        .text_color(Colors::primary_text())
                                })
                                .child(i18n::t(key))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if this.bridge.controls_enabled() {
                                        this.advanced = advanced;
                                        cx.notify();
                                    }
                                }))
                        },
                    )),
            );
        if self.advanced {
            card = card.child(self.advanced_effects(cx));
        } else {
            card =
                card.child(
                    v_flex()
                        .pt(surface::css(20.))
                        .child(
                            div()
                                .mb(surface::css(20.))
                                .line_height(surface::css(17.))
                                .child(i18n::t("QUICK_EFFECTS_MSG")),
                        )
                        .child(
                            h_flex()
                                .child(div().w(surface::css(150.)).mr(surface::css(20.)).child(
                                    surface::select(&self.effect).disabled(!enabled).w_full(),
                                ))
                                .child(
                                    gpui_kit::base::Button::new("hue-sync-effects")
                                        .disabled(true)
                                        .p_0()
                                        .flex()
                                        .gap(surface::css(10.))
                                        .accessibility_label(i18n::t("ONLY_ONE_CHROMA_DEVICE"))
                                        .child(
                                            img("synapse/chroma-sync.svg")
                                                .size(surface::css(30.))
                                                .opacity(0.3),
                                        )
                                        .child(
                                            div()
                                                .w(surface::css(310.))
                                                .underline()
                                                .child(i18n::t("ONLY_ONE_CHROMA_DEVICE")),
                                        ),
                                ),
                        )
                        .child(self.effect_parameters(enabled, cx)),
                );
        }
        card.into_any_element()
    }
    fn effect_parameters(&self, enabled: bool, cx: &Context<Self>) -> AnyElement {
        let setting = self.effect_setting();
        let step = cx.listener(|this, action: &StepAction, window, cx| {
            if this.effects_editable() && this.selected_effect == 12 {
                let value = this.effect_setting()["colorBoost"].as_f64().unwrap_or(1.) as f32;
                let delta = match action {
                    StepAction::Increment => 0.25,
                    StepAction::Decrement => -0.25,
                };
                this.edit_effect(
                    "colorBoost",
                    json!(super::super::settings::normalize_color_boost(value + delta)),
                    cx,
                );
                this.sync_effects(window, cx);
            }
        });
        match self.selected_effect {
            1 | 2 => {
                let breathing = self.selected_effect == 2;
                let random = setting["isRandom"].as_bool().unwrap_or(false);
                h_flex()
                    .items_start()
                    .flex_wrap()
                    .gap(surface::css(20.))
                    .mt(surface::css(20.))
                    .children((0..if breathing { 2 } else { 1 }).map(|channel| {
                        let label = if breathing {
                            i18n::t("COLOR_DROP_NAME")
                                .replace("{{num}}", &(channel + 1).to_string())
                        } else {
                            i18n::t("COLOR")
                        };
                        LightingColorPicker::new(&self.colors[channel], label)
                            .allow_none(breathing)
                            .disabled(!enabled || random)
                            .into_any_element()
                    }))
                    .when(breathing, |view| {
                        view.child(
                            div().mt(surface::css(25.)).child(
                                checkbox::Checkbox::new("hue-random-color")
                                    .label(i18n::t("RANDOM_COLOR"))
                                    .checked(random)
                                    .disabled(!enabled)
                                    .on_click(cx.listener(|this, value, _, cx| {
                                        this.edit_effect("isRandom", json!(value), cx)
                                    })),
                            ),
                        )
                    })
                    .into_any_element()
            }
            11 => v_flex()
                .relative()
                .mt(surface::css(20.))
                .min_h(surface::css(56.))
                .child(i18n::t("TEXT_SCREEN_REGION"))
                .children(
                    ["full", "left", "top", "right", "bottom"]
                        .into_iter()
                        .enumerate()
                        .map(|(position, region)| {
                            let selected = setting["screen"].as_str() == Some(region);
                            gpui_kit::base::Button::new((
                                ElementId::from("hue-screen-region"),
                                region,
                            ))
                            .absolute()
                            .top(surface::css(40.))
                            .left(surface::css(position as f32 * 30.))
                            .w(surface::css(24.))
                            .h(surface::css(16.))
                            .p_0()
                            .border(surface::css(if selected { 1.5 } else { 1. }))
                            .border_color(if selected {
                                cx.theme().primary
                            } else {
                                Colors::secondary()
                            })
                            .disabled(!enabled)
                            .selected(selected)
                            .accessibility_label(region)
                            .when(region == "full", |b| b.bg(Colors::secondary_text()))
                            .when(region != "full", |b| {
                                b.child(
                                    div()
                                        .absolute()
                                        .bg(Colors::secondary_text())
                                        .when(region == "left", |v| {
                                            v.left_0().top_0().bottom_0().w(surface::css(4.))
                                        })
                                        .when(region == "right", |v| {
                                            v.right_0().top_0().bottom_0().w(surface::css(4.))
                                        })
                                        .when(region == "top", |v| {
                                            v.top_0().left_0().right_0().h(surface::css(4.))
                                        })
                                        .when(region == "bottom", |v| {
                                            v.bottom_0().left_0().right_0().h(surface::css(4.))
                                        }),
                                )
                            })
                            .on_click(cx.listener(
                                move |this, _, _, cx| this.edit_effect("screen", json!(region), cx),
                            ))
                        }),
                )
                .into_any_element(),
            12 => v_flex()
                .mt(surface::css(20.))
                .child(i18n::t("TEXT_COLOR_BOOST"))
                .child(
                    div()
                        .w(surface::css(60.))
                        .h(surface::css(27.))
                        .mt(surface::css(10.))
                        .border_1()
                        .border_color(Colors::ip_border())
                        .child(
                            NumberInput::new(&self.color_boost)
                                .disabled(!enabled)
                                .size_full()
                                .controls_right()
                                .input(
                                    Input::new(&self.color_boost)
                                        .appearance(false)
                                        .bordered(false)
                                        .focus_bordered(false)
                                        .p_0()
                                        .pl(surface::css(5.))
                                        .h(surface::css(25.))
                                        .text_size(surface::css(14.)),
                                )
                                .on_step(move |action, window, cx| step(&action, window, cx)),
                        ),
                )
                .into_any_element(),
            _ => div().into_any_element(),
        }
    }
    fn advanced_effects(&self, cx: &Context<Self>) -> AnyElement {
        match self.chroma_installed {
            None => div().into_any_element(), // Original canShow=false is invisible until the service responds.
            Some(false) => v_flex()
                .pt(surface::css(20.))
                .child(
                    div()
                        .relative()
                        .w(surface::css(520.))
                        .h(surface::css(180.))
                        .child(img("synapse/hue-install_chroma.png").size_full())
                        .child(
                            div()
                                .absolute()
                                .bottom(surface::css(20.))
                                .w_full()
                                .flex()
                                .justify_center()
                                .child(command(
                                    "hue-install-chroma",
                                    "INSTALL_RAZER_CHROMA",
                                    true,
                                    true,
                                    cx,
                                )),
                        ),
                )
                .child(
                    div()
                        .mt(surface::css(20.))
                        .child(i18n::t("ADVANCED_EFFECTS_DETAIL")),
                )
                .into_any_element(),
            Some(true) => v_flex()
                .pt(surface::css(20.))
                .child(i18n::t("ADVANCED_EFFECTS_MSG"))
                .child(
                    div()
                        .mt(surface::css(20.))
                        .child(i18n::t("NO_CHROMA_STUDIO_PROFILE_TEXT")),
                )
                .child(
                    div()
                        .mt(surface::css(5.))
                        .child(surface::select(&self.chroma_profiles).disabled(true)),
                )
                .child(
                    gpui_kit::base::Button::new("hue-launch-chroma")
                        .disabled(true)
                        .w(surface::css(240.))
                        .h(surface::css(50.))
                        .mt(surface::css(20.))
                        .border_1()
                        .rounded(surface::css(3.))
                        .flex()
                        .gap(surface::css(10.))
                        .px(surface::css(16.))
                        .accessibility_label(i18n::t("LAUNCH_CHROMA_STUDIO"))
                        .child(img("synapse/hue-logo_chromastudio.svg").size(surface::css(30.)))
                        .child(i18n::t("LAUNCH_CHROMA_STUDIO").to_uppercase()),
                )
                .into_any_element(),
        }
    }
}
