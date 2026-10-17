//! Mounted mouse label `rederTip` / `renderRequireSynapeTip` and source CSS.
//! PID229 main.551458e6.js oT / main.377583f3.css `.config-btns-wrapper`.
use super::*;
use razer_widgets::theme::TooltipColors;
use gpui_kit::base::motion::{transition,Transition};

#[derive(Clone, Copy, PartialEq, Eq)]
enum HoverPart {
    Button,
    DisabledWrapper,
    RequiresSynapse,
}

pub(in super::super) struct LabelHover {
    input: String,
    part: HoverPart,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LabelTip {
    RemappingBlocked,
    PrimaryClickBlocked,
    ScrollMode,
    InterDevice,
    Text,
}

pub(super) fn label_tip(button: &Value, source_disabled: bool) -> Option<LabelTip> {
    if source_disabled && button["isRemappedDisabled"] == true {
        Some(LabelTip::RemappingBlocked)
    } else if source_disabled && button["assignmentValue"] == "LEFT_CLICK" {
        Some(LabelTip::PrimaryClickBlocked)
    } else if button["assignmentValue"] == "SCROLL_MODE_SWITCH" {
        Some(LabelTip::ScrollMode)
    } else if button["assignment"] == "INTERDEVICE" {
        Some(LabelTip::InterDevice)
    } else if button["isTooltipVisible"] == true && button["toolTip"].is_string() {
        Some(LabelTip::Text)
    } else {
        None
    }
}

fn tip_surface() -> Div {
    div()
        .absolute()
        .bg(TooltipColors::background())
        .border_1()
        .border_color(Colors::line())
        .text_color(Colors::text())
        .font_family("Roboto")
        .text_size(surface::css(14.))
        .line_height(surface::css(16.))
        .pt(surface::css(8.))
        .px(surface::css(10.))
        .pb(surface::css(10.))
}

/// Default CSS ease between the source's 0/47/50/97/100 percent keyframes.
pub(super) fn scroll_mode_phase(phase: f32) -> f32 {
    let phase = phase.rem_euclid(1.);
    if phase < 0.47 {
        0.
    } else if phase < 0.5 {
        Easing::Ease.sample((phase - 0.47) / 0.03)
    } else if phase < 0.97 {
        1.
    } else {
        1. - Easing::Ease.sample((phase - 0.97) / 0.03)
    }
}

impl MouseProductWorkspace {
    fn set_customize_label_hover(&mut self, input: &str, part: HoverPart, hovered: bool) {
        if hovered {
            self.customize_label_hover = Some(LabelHover {
                input: input.to_owned(),
                part,
            });
        } else if self
            .customize_label_hover
            .as_ref()
            .is_some_and(|hover| hover.input == input && hover.part == part)
        {
            self.customize_label_hover = None;
        }
    }

    pub(super) fn current_customize_label(
        &self,
        button: &Value,
        index: usize,
        left: bool,
        margin_top: f32,
        unavailable: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let input = button["inputID"].as_str().unwrap_or_default().to_owned();
        let label = t(button["assignmentValue"].as_str().unwrap_or("DEFAULT"));
        let source_disabled = button["disabled"] == true || button["isEnabled"] != true;
        let macro_mode =
            self.mouse_customize_observation["customizeReducer"]["displayMode"] == "macro";
        let enabled = !unavailable
            && !source_disabled
            && (!macro_mode
                || button["functionList"]
                    .as_array()
                    .is_some_and(|functions| functions.iter().any(|name| name == "MACRO")));
        let active = self.mapping_input.as_deref() == Some(input.as_str());
        let part = self
            .customize_label_hover
            .as_ref()
            .filter(|hover| hover.input == input)
            .map(|hover| hover.part);
        let hovered = enabled && part == Some(HoverPart::Button);
        let requires_hovered = enabled && part == Some(HoverPart::RequiresSynapse);
        let tip_hovered = hovered || source_disabled && part == Some(HoverPart::DisabledWrapper);
        let inter = button["assignment"] == "INTERDEVICE";
        let connected = self.mouse_customize_observation["validDevices"]
            .as_array()
            .is_some_and(|devices| {
                devices
                    .iter()
                    .any(|device| device["productId"] == button["assignmentGroup"]["interPID"])
            });
        let requires = button["isMapped"] == true && button["isRequireSynapse"] == true;
        let accent = if self.hypershift {
            Colors::hypershift()
        } else {
            Colors::selected()
        };
        let color = if button["assignment"] == "DISABLE" || inter && !connected {
            Colors::unavailable_mapping()
        } else if button["isMapped"] == true {
            accent
        } else if source_disabled || !enabled {
            Colors::disabled_text()
        } else {
            Colors::text()
        };
        let color = surface::fade_color(
            (
                ElementId::from(("mouse-source-label-color", cx.entity_id())),
                input.clone(),
            ),
            color,
            200,
            window,
            cx,
        );
        let background = surface::fade_color(
            (
                ElementId::from(("mouse-source-label-background", cx.entity_id())),
                input.clone(),
            ),
            if active {
                Colors::selected_surface()
            } else if hovered {
                Colors::hover()
            } else {
                Colors::transparent()
            },
            200,
            window,
            cx,
        );
        let tip_opacity = transition(
            (
                ElementId::from(("mouse-source-label-tip-opacity", cx.entity_id())),
                input.clone(),
            ),
            if tip_hovered && !requires_hovered { 1. } else { 0. },
            Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
            window,
            cx,
        );
        let requires_opacity = transition(
            (
                ElementId::from(("mouse-source-label-requires-opacity", cx.entity_id())),
                input.clone(),
            ),
            if requires_hovered { 1. } else { 0. },
            Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
            window,
            cx,
        );
        let anchor_input = input.clone();
        let anchors = self.mapping_menu_current_state.anchors.clone();
        let top = anchors
            .borrow()
            .get(&input)
            .map(|bounds| f32::from(bounds.top()) * 16. / f32::from(window.rem_size()))
            .unwrap_or_default();
        let mut contents = h_flex()
            .items_center()
            .when(inter, |row| row.max_w(surface::css(210.)));
        if inter {
            contents = contents.child(
                img(if !connected {
                    "synapse/mouse-customize/interdevice-disconnected.svg"
                } else if self.hypershift {
                    "synapse/mouse-customize/interdevice-hypershift.svg"
                } else {
                    "synapse/mouse-customize/interdevice-normal.svg"
                })
                .size(surface::css(20.))
                .mr(surface::css(4.)),
            );
        }
        let mut description = v_flex().max_w(surface::css(200.)).overflow_hidden();
        if let Some(text) = button["buttonDescription"]
            .as_str()
            .filter(|text| !text.is_empty())
        {
            description = description.py(surface::css(2.)).child(
                div()
                    .text_size(surface::css(10.))
                    .line_height(surface::css(12.))
                    .text_color(accent)
                    .child(t(text).to_uppercase()),
            );
        }
        contents = contents.child(description.child(div().text_ellipsis().child(label.clone())));
        let hovered_input = input.clone();
        let mut control = BaseButton::new(SharedString::from(format!(
            "mouse-{}-input-{input}",
            self.spec.product_id
        )))
        .relative()
        .accessibility_label(label)
        .on_prepaint(move |bounds, _, _| {
            anchors.borrow_mut().insert(anchor_input.clone(), bounds);
        })
        .h(surface::css(30.))
        .mb(surface::css(20.))
        .when(left, |button| button.mr(surface::css(10.)))
        .when(!left, |button| button.ml(surface::css(10.)))
        .px(surface::css(10.))
        .py_0()
        .rounded(surface::css(3.))
        .text_size(surface::css(14.))
        .line_height(surface::css(17.))
        .text_color(color)
        .bg(background)
        .disabled(!enabled)
        .when(enabled, |button| {
            button.active(|style| style.bg(Colors::selected_surface()))
        })
        .child(contents)
        .on_hover(cx.listener(move |this, on: &bool, _, cx| {
            if !enabled {
                return;
            }
            this.set_customize_label_hover(&hovered_input, HoverPart::Button, *on);
            if *on || this.customize_hover == Some(index) {
                this.hover_current_customize(index, *on);
            }
            cx.notify();
        }))
        .on_click(cx.listener({
            let input = input.clone();
            move |this, _, window, cx| {
                if this.mapping_input.as_deref() != Some(input.as_str()) {
                    this.open_current_mouse_mapping(input.clone(), window, cx);
                }
            }
        }));
        if requires {
            let requires_input = input.clone();
            control = control.child(
                div()
                    .id(SharedString::from(format!(
                        "mouse-source-label-requires-{input}"
                    )))
                    .test_support()
                    .absolute()
                    .top_0()
                    .when(left, |icon| icon.right(surface::css(-29.)))
                    .when(!left, |icon| icon.left(surface::css(-29.)))
                    .size(surface::css(32.))
                    .rounded_full()
                    .border_1()
                    .border_color(if hovered || active { accent } else { Colors::line() })
                    .bg(if self.hypershift {
                        Colors::hypershift()
                    } else {
                        Colors::requires_synapse()
                    })
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        img("synapse/mouse-customize/requires-synapse.svg").size(surface::css(20.)),
                    )
                    .children(requires_hovered.then(|| {
                        razer_assets::deferred_resource(
                            tip_surface()
                                .id("mouse-source-label-requires-tip")
                                .test_support()
                                .opacity(requires_opacity)
                                .top(relative(1.1))
                                .left(surface::css(-9.))
                                .whitespace_nowrap()
                                .child(t("MAPPING_REQUIRES_RAZER_SYNAPSE")),
                        )
                        .with_priority(100)
                    }))
                    .on_hover(cx.listener(move |this, on: &bool, _, cx| {
                        if !enabled {
                            return;
                        }
                        this.set_customize_label_hover(
                            &requires_input,
                            HoverPart::RequiresSynapse,
                            *on,
                        );
                        if *on {
                            this.hover_current_customize(index, false);
                        }
                        cx.notify();
                    })),
            );
        }
        div()
            .id(SharedString::from(format!(
                "mouse-source-label-wrapper-{input}"
            )))
            .test_support()
            .relative()
            .mt(surface::css(margin_top))
            .child(control)
            .children(
                (tip_hovered && !requires_hovered)
                    .then(|| {
                        self.current_customize_label_tip(button, source_disabled, connected, top)
                            .map(|tip| {
                                razer_assets::deferred_resource(
                                    tip.id("mouse-source-label-tip")
                                        .test_support()
                                        .opacity(tip_opacity),
                                )
                                .with_priority(100)
                            })
                    })
                    .flatten(),
            )
            .on_hover(cx.listener(move |this, on: &bool, _, cx| {
                if source_disabled && !unavailable {
                    this.set_customize_label_hover(&input, HoverPart::DisabledWrapper, *on);
                }
                if !on
                    && this
                        .customize_label_hover
                        .as_ref()
                        .is_some_and(|hover| hover.input == input)
                {
                    this.customize_label_hover = None;
                }
                cx.notify();
            }))
            .into_any_element()
    }

    pub(in super::super) fn current_customize_label_tip(
        &self,
        button: &Value,
        source_disabled: bool,
        connected: bool,
        top: f32,
    ) -> Option<Div> {
        match label_tip(button, source_disabled)? {
            LabelTip::RemappingBlocked | LabelTip::PrimaryClickBlocked => {
                let remapping_blocked = button["isRemappedDisabled"] == true;
                Some(
                    tip_surface()
                        .w(surface::css(if remapping_blocked { 280. } else { 315. }))
                        .top(relative(if remapping_blocked { 0.6 } else { 0.9 }))
                        .left(surface::css(if remapping_blocked { -70. } else { -113. }))
                        .p_0()
                        .text_center()
                        .child(
                            div()
                                .mt(surface::css(5.))
                                .text_size(surface::css(12.))
                                .child(t("MOUSE")),
                        )
                        .child(
                            div()
                                .mt(surface::css(5.))
                                .text_size(surface::css(18.))
                                .mb(surface::css(11.))
                                .child(t(if remapping_blocked {
                                    button["defaultValue"].as_str().unwrap_or_default()
                                } else {
                                    "LEFT_CLICK"
                                })),
                        )
                        .child(
                            div()
                                .mt(surface::css(5.))
                                .bg(Colors::remapping_blocked_message())
                                .text_size(surface::css(12.))
                                .py(surface::css(3.))
                                .px(surface::css(20.))
                                .child(t(if remapping_blocked {
                                    "BUTTON_CANNOT_BE_REMAPPED"
                                } else {
                                    "LEFT_CLICK_DISABLE_MESSAGE"
                                })),
                        ),
                )
            }
            LabelTip::ScrollMode => Some(scroll_mode_tip(source_disabled)),
            LabelTip::InterDevice => {
                let accent = if !connected {
                    Colors::unavailable_mapping()
                } else if self.hypershift {
                    Colors::hypershift()
                } else {
                    Colors::selected()
                };
                let group = &button["assignmentGroup"];
                let function = match group["interDeviceType"].as_str() {
                    Some("profileNavigationGroup") => "SWITCH_PROFILE",
                    Some("sensitivityGroup") => "SENSITIVITY",
                    _ => "",
                };
                let name = t(button["assignmentValue"].as_str().unwrap_or_default());
                let name = if group["interDeviceType"] == "profileNavigationGroup" {
                    name.to_string()
                } else {
                    sidebar_current::capitalize(&name)
                };
                let requires = mapping_target_info(self.spec.product_id)
                    .is_some_and(|info| info["isOBMDevice"] == true);
                let mut tip =
                    tip_surface()
                        .top(relative(if top <= 225. { 0.65 } else { -2.56 }))
                        .left(surface::css(-45.))
                        .border_color(accent)
                        .whitespace_nowrap()
                        .when(!connected || requires, |tip| tip.p_0())
                        .child(
                            v_flex()
                                .pt(surface::css(3.))
                                .pb(surface::css(6.))
                                .text_center()
                                .text_size(surface::css(12.))
                                .child(
                                    div()
                                        .mt(surface::css(5.))
                                        .text_color(Colors::tooltip_secondary_text())
                                        .text_size(surface::css(14.))
                                        .child(
                                            t(button["defaultValue"].as_str().unwrap_or_default())
                                                .to_uppercase(),
                                        ),
                                )
                                .child(div().mt(surface::css(5.)).text_color(Colors::tooltip_secondary_text()).child(
                                    group["name"].as_str().unwrap_or_default().to_uppercase(),
                                ))
                                .children(button.get("assignmentGroup").map(|_| {
                                    v_flex()
                                        .child(
                                            div()
                                                .mt(surface::css(5.))
                                                .child(t(function).to_uppercase()),
                                        )
                                        .child(
                                            div()
                                                .mt(surface::css(5.))
                                                .text_size(surface::css(18.))
                                                .child(name),
                                        )
                                })),
                        );
                if !connected || requires {
                    tip = tip.child(
                        div()
                            .mt(surface::css(5.))
                            .bg(accent)
                            .text_color(Colors::tooltip_banner_text())
                            .text_size(surface::css(12.))
                            .py(surface::css(3.))
                            .px(surface::css(26.))
                            .text_center()
                            .child(t(if connected {
                                "MAPPING_REQUIRES_RAZER_SYNAPSE"
                            } else {
                                "DEVICE_NOT_CONNECTED"
                            })),
                    );
                }
                Some(tip)
            }
            LabelTip::Text => Some(
                tip_surface()
                    .top(relative(1.))
                    .w(surface::css(300.))
                    .whitespace_normal()
                    .child(t(button["toolTip"].as_str()?)),
            ),
        }
    }
}

fn scroll_mode_tip(animate: bool) -> Div {
    tip_surface()
        .top(relative(-2.))
        .left(relative(-0.3))
        .w(surface::css(305.))
        .flex()
        .items_center()
        .text_size(surface::css(12.))
        .child(
            div()
                .relative()
                .w(surface::css(22.))
                .h(surface::css(56.))
                .flex_shrink_0()
                .child(img("synapse/mouse-customize/scrollmode-switch-bg.svg").size_full())
                .child(
                    div()
                        .absolute()
                        .left(surface::css(4.))
                        .top(surface::css(3.))
                        .w(surface::css(14.))
                        .h(surface::css(43.))
                        .child(img("synapse/mouse-customize/scrollmode-switch-top.svg").size_full())
                        .map(|image| {
                            if animate {
                                image
                                    .with_animation(
                                        "mouse-source-scroll-mode-switch",
                                        Animation::new(Duration::from_millis(6400)).repeat(),
                                        |image, phase| {
                                            image.top(surface::css(
                                                3. + scroll_mode_phase(phase) * 7.,
                                            ))
                                        },
                                    )
                                    .into_any_element()
                            } else {
                                image.into_any_element()
                            }
                        }),
                ),
        )
        .child(
            v_flex()
                .ml(surface::css(10.))
                .flex_1()
                .child(
                    div()
                        .mb(surface::css(10.))
                        .child(t("SCROLL_MODE_SWITCH_DESCRIPTION")),
                )
                .child(div().child(t("FREE_SPIN_SCROLLING_MODE")).map(|text| {
                    if animate {
                        text.with_animation(
                            "mouse-source-scroll-mode-forward",
                            Animation::new(Duration::from_millis(6400)).repeat(),
                            |text, phase| text.opacity(1. - scroll_mode_phase(phase) * 0.7),
                        )
                        .into_any_element()
                    } else {
                        text.into_any_element()
                    }
                }))
                .child(
                    div()
                        .opacity(0.3)
                        .child(t("TACTILE_CYCLING_MODE"))
                        .map(|text| {
                            if animate {
                                text.with_animation(
                                    "mouse-source-scroll-mode-backward",
                                    Animation::new(Duration::from_millis(6400)).repeat(),
                                    |text, phase| {
                                        text.opacity(0.3 + scroll_mode_phase(phase) * 0.7)
                                    },
                                )
                                .into_any_element()
                            } else {
                                text.into_any_element()
                            }
                        }),
                ),
        )
}
