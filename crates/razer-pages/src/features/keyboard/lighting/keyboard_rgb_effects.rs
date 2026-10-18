//! Current keyboard RGB Effects widgets; profile changes remain local drafts.
use super::*;
use gpui_kit::base::Button as BaseButton;
use razer_widgets::theme::KeyboardRgbEffectColors as Colors;

impl KeyboardProductWorkspace {
    fn keyboard_advanced_effects(&self, cx: &Context<Self>) -> AnyElement {
        let Some(observation) = &self.lighting_observation else {
            return div().into_any_element();
        };
        let Some(fully_installed) = observation.check_is_fully_installed else {
            return div().into_any_element();
        };
        let disabled = self.rgb_disabled();
        let Some(module_installed) = observation.chroma_installed_module else {
            return div().into_any_element();
        };
        if fully_installed && module_installed {
            if observation.is_chroma_enabled.is_none() {
                return div().into_any_element();
            }
            return v_flex()
                .line_height(surface::css(18.))
                .child(t("ADVANCED_EFFECTS_MSG"))
                .child(div().mt(surface::css(20.)).child(t(
                    if observation.chroma_profiles.is_empty() {
                        "NO_CHROMA_STUDIO_PROFILE_TEXT"
                    } else {
                        "CHROMA_STUDIO"
                    },
                )))
                .children(self.rgb_controls.as_ref().map(|state| {
                    div().mt(surface::css(5.)).w(surface::css(200.)).child(
                        surface::select(&state.chroma_profile)
                            .items(self.chroma_profile_choices())
                            .disabled(disabled)
                            .accessibility_label(t("CHROMA_STUDIO"))
                            .w_full(),
                    )
                }))
                .child(
                    BaseButton::new("keyboard-launch-chroma-studio")
                        .disabled(disabled)
                        .mt(surface::css(20.))
                        .w(surface::css(240.))
                        .h(surface::css(50.))
                        .border_1()
                        .border_color(Colors::text())
                        .rounded(surface::css(3.))
                        .bg(Colors::background())
                        .hover(|button| button.bg(Colors::black()))
                        .active(|button| button.bg(Colors::background()))
                        .flex()
                        .items_center()
                        .px(surface::css(16.))
                        .gap(surface::css(10.))
                        .child(
                            img("synapse/keyboard-logo-chromastudio.svg")
                                .size(surface::css(30.))
                                .flex_shrink_0(),
                        )
                        .child(t("LAUNCH_CHROMA_STUDIO").to_uppercase())
                        .on_click(cx.listener(|this, _, _, cx| {
                            if !this.rgb_disabled() {
                                this.request_lighting_command(
                                    KeyboardLightingCommand::LaunchStudio,
                                    cx,
                                );
                            }
                        })),
                )
                .into_any_element();
        }
        let busy = observation.installer_busy;
        v_flex()
            .child(
                div()
                    .relative()
                    .w(surface::css(520.))
                    .h(surface::css(180.))
                    .child(img("synapse/install-chroma.png").size_full())
                    .child(
                        h_flex()
                            .absolute()
                            .left_0()
                            .bottom(surface::css(20.))
                            .w_full()
                            .justify_center()
                            .child(
                                BaseButton::new("keyboard-install-chroma")
                                    .disabled(busy || disabled)
                                    .flex()
                                    .items_center()
                                    .px(surface::css(15.))
                                    .py(surface::css(4.))
                                    .border_1()
                                    .border_color(Colors::accent())
                                    .rounded(surface::css(2.))
                                    .bg(Colors::accent())
                                    .text_color(Colors::background())
                                    .text_size(surface::css(12.))
                                    .when(!busy && !disabled, |button| {
                                        button.hover(|button| button.bg(Colors::install_hover()))
                                    })
                                    .active(|button| button.opacity(0.8))
                                    .when(busy, |button| {
                                        button.child(
                                            div().mr(surface::css(5.)).child(ChromaInstallSpinner),
                                        )
                                    })
                                    .child(t("INSTALL_RAZER_CHROMA"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if this.rgb_disabled() {
                                            return;
                                        }
                                        let Some(observation) = &mut this.lighting_observation
                                        else {
                                            return;
                                        };
                                        if observation.installer_busy {
                                            return;
                                        }
                                        observation.installer_busy = true;
                                        this.request_lighting_command(
                                            KeyboardLightingCommand::InstallChroma,
                                            cx,
                                        );
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .mt(surface::css(20.))
                    .line_height(surface::css(18.))
                    .child(t("ADVANCED_EFFECTS_DETAIL")),
            )
            .into_any_element()
    }

    pub(super) fn huntsman_effects(&self, cx: &Context<Self>) -> AnyElement {
        let hardware = self.rgb_hardware_effect();
        let mode = if hardware { Some(false) } else { self.chroma_mode() };
        let advanced = mode == Some(true);
        let selected = self.rgb_effect_id();
        let disabled = self.rgb_disabled();
        let can_sync = self.rgb_can_sync();
        let tabs = h_flex()
            .opacity(if self.rgb_visualizer_disabled() { 0.3 } else { 1. })
            .h(surface::css(36.))
            .p(surface::css(5.))
            .border_1()
            .border_color(Colors::border())
            .rounded(surface::css(18.))
            .bg(Colors::background())
            .children([(false, "QUICK_EFFECTS"), (true, "ADVANCED_EFFECTS")].map(
                |(mode, label)| {
                    BaseButton::new(SharedString::from(format!(
                        "keyboard-analog-effects-tab-{mode}"
                    )))
                    .disabled(disabled)
                    .h(surface::css(26.))
                    .px(surface::css(10.))
                    .py(surface::css(5.))
                    .rounded(surface::css(13.))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(16.))
                    .bg(if self.chroma_mode() == Some(mode) {
                        Colors::accent()
                    } else {
                        Colors::transparent()
                    })
                    .text_color(if self.chroma_mode() == Some(mode) {
                        Colors::selected_text()
                    } else {
                        Colors::text()
                    })
                    .child(t(label))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.rgb_disabled() {
                            return;
                        }
                        this.dismiss_rgb_controls(cx);
                        this.lighting_runtime.advanced_draft = Some(mode);
                        this.request_lighting_message(
                            "TOGGLE_CHROMA",
                            json!({"isChromaEnabled":mode}),
                            cx,
                        );
                        this.refresh_lighting(cx);
                        this.analog_gamepad.effect_menu = false;
                        cx.notify();
                    }))
                },
            ));
        let mut body = v_flex().mt(surface::css(20.));
        if advanced {
            body = body.child(self.keyboard_advanced_effects(cx));
        } else {
            body = body
                .child(div().mb(surface::css(20.)).child(t(if hardware {
                    if self.spec.config["DeviceInfo"]["dongleId"]
                        .as_u64()
                        .is_some_and(|id| id != 0)
                    {
                        "BLE_QUICK_EFFECTS_MSG"
                    } else {
                        "BLE_NO_24GHZ_QUICK_EFFECTS_MSG"
                    }
                } else {
                    "QUICK_EFFECTS_MSG"
                })))
                .child(
                    h_flex()
                        .items_center()
                        .gap(surface::css(20.))
                        .children(self.rgb_controls.as_ref().map(|state| {
                            div().w(surface::css(150.)).flex_shrink_0().child(
                                surface::select(&state.effect)
                                    .items(self.rgb_choices())
                                    .disabled(self.rgb_quick_disabled())
                                    .accessibility_label(t("QUICK_EFFECTS"))
                                    .w_full(),
                            )
                        }))
                        .when(!hardware, |row| {
                            row.child(
                                BaseButton::new("keyboard-chroma-sync")
                                    .disabled(!can_sync || self.rgb_quick_disabled())
                                    .p_0()
                                    .flex()
                                    .items_center()
                                    .opacity(if can_sync { 1. } else { 0.3 })
                                    .child(self.rgb_sync_icon())
                                    .hover(|button| button.text_color(Colors::accent()))
                                    .active(|button| {
                                        button.text_color(Colors::accent()).opacity(0.7)
                                    })
                                    .child(div().underline().child(t(if can_sync {
                                        "APPLY_TO_ALL_CHROMA_DEVICES"
                                    } else if selected == 17 {
                                        "NOT_APPLICABLE_CHROMA_ENABLED_DEVICES"
                                    } else {
                                        "ONLY_ONE_CHROMA_DEVICE"
                                    })))
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.sync_rgb_effect(cx)),
                                    ),
                            )
                        }),
                )
                .child(self.rgb_parameters(cx));
        }
        surface::panel_with_help(
            t("EFFECTS"),
            surface::help_control(
                "keyboard-effects-help",
                t(if hardware {
                    "EFFECTS_BLE_TOOLTIP"
                } else {
                    "EFFECTS_TOOLTIP"
                }),
            ),
            cx,
        )
        .mb_0()
        .opacity(if self.rgb_nanoleaf_disabled() { 0.3 } else { 1. })
        .when(!hardware, |panel| panel.child(tabs))
        .child(body.opacity(
            (if !hardware && self.rgb_visualizer_disabled() {
                0.3
            } else {
                1.
            }) * (if !advanced
                && (self.rgb_visualizer_disabled() || self.rgb_power_saving_disabled())
            {
                0.3
            } else {
                1.
            }),
        ))
        .children(self.chroma_status(cx))
        .into_any_element()
    }

    pub(super) fn select_huntsman_effect(
        &mut self,
        id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.rgb_quick_disabled() {
            return;
        }
        let previous_id = self.rgb_effect_id();
        let previous = self.rgb_setting();
        self.store_rgb_setting(previous_id, previous);
        let setting = self.draft["quickEffects"]["effectSettings"]
            .as_array()
            .and_then(|settings| settings.iter().find(|setting| setting["effectId"] == id))
            .cloned()
            .unwrap_or_else(|| self.default_rgb_setting(id));
        self.store_rgb_setting(id, setting);
        self.request_rgb_effect(cx);
        self.sync_rgb_controls(window, cx);
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
}

#[derive(IntoElement)]
struct ChromaInstallSpinner;
impl RenderOnce for ChromaInstallSpinner {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        razer_assets::resource_svg()
            .path("synapse/keyboard-chroma-install-spinner.svg")
            .size(surface::css(16.))
            .text_color(Colors::background())
            .with_animation(
                "keyboard-chroma-install-spin",
                Animation::new(std::time::Duration::from_secs(1))
                    .repeat()
                    .with_easing(linear),
                |icon, phase| icon.with_transformation(Transformation::rotate(percentage(phase))),
            )
    }
}
