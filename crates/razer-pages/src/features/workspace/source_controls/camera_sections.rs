//! Current camera wrapper mount, title controls and conditional help. The
//! generated groups hold each source row's children and effective mount props.
use super::*;
use gpui_kit::base::{
    Collapsible,
    motion::{self, Easing, Transition},
};
use razer_widgets::source_tooltip::SourceTooltip;
use std::time::Duration;

#[derive(Deserialize)]
pub(super) struct CameraGroup {
    pub(super) key: String,
    title: String,
    pub(super) controls: Vec<String>,
    collapsible: bool,
    #[serde(default)]
    presentation: Option<String>,
    #[serde(default)]
    divider: Option<String>,
    #[serde(default)]
    header_switch: Option<String>,
    #[serde(default)]
    header_stepper: Option<String>,
    #[serde(default)]
    header_checkbox: Option<String>,
    #[serde(default)]
    header_reset: Option<String>,
    #[serde(default)]
    tooltip: Option<String>,
    #[serde(default)]
    warning: Option<String>,
    #[serde(default)]
    warning_any: Vec<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    description_when_enabled: Option<String>,
    #[serde(default)]
    description_min_height: Option<u64>,
}

pub(super) fn help(id: SharedString, text: String, warning: bool) -> AnyElement {
    let animation_id = id.clone();
    // Current Pa / module 1050 portals `.drop-tips`, measuring `.tip` at
    // 210px (280px in mode-name-s2) and copying its client dimensions.
    SourceTooltip::new(id, text.clone(), if warning { 280. } else { 210. })
        // `.drop-tips.on{z-index:9999}` in all four current camera bundles.
        .hovered_priority(9999)
        .trigger(move |hovered, window, cx| {
            let size = if warning { 20. } else { 14. };
            let background = motion::transition(
                (
                    ElementId::from(animation_id.clone()),
                    "camera-help-background",
                ),
                if hovered {
                    super::camera_theme::help_hover()
                } else {
                    super::camera_theme::help()
                },
                Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
                window,
                cx,
            );
            gpui_kit::base::Button::new((ElementId::from(animation_id.clone()), "icon"))
                .accessibility_label(text.clone())
                .w(surface::css(size))
                .h(surface::css(size))
                .p_0()
                .cursor_pointer()
                .when(!warning, |button| {
                    button.rounded(surface::css(7.)).bg(background)
                })
                .focus_visible(|button| button.border_1().border_color(cx.theme().primary))
                .child(
                    img(if warning {
                        "synapse/camera-section-warning.svg"
                    } else {
                        "synapse/camera-section-help.svg"
                    })
                    .size_full(),
                )
                .into_any_element()
        })
        .into_any_element()
}

impl SourceControls {
    pub(super) fn render_camera_switch(
        &self,
        control: &ControlSpec,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = control.key.clone();
        h_flex()
            .items_center()
            .text_size(surface::css(14.))
            .line_height(surface::css(16.))
            .child(razer_i18n::t(&control.label))
            .child(
                div().ml(surface::css(10.)).child(
                    surface::SynapseSwitch::new(SharedString::from(key.clone()))
                        .accessibility_label(razer_i18n::t(&control.label))
                        .checked(if control.options.len() == 2 {
                            self.value(control) == Some(&control.options[1].value)
                        } else {
                            self.value(control)
                                .and_then(Value::as_bool)
                                .unwrap_or(false)
                        })
                        .disabled(disabled)
                        .on_change(cx.listener(move |this, value, window, cx| {
                            this.edit(&key, Value::Bool(*value), window, cx)
                        })),
                ),
            )
            .when_some(control.tooltip.as_ref(), |row, text| {
                row.child(div().ml(surface::css(10.)).child(help(
                    SharedString::from(format!("{}:help", control.key)),
                    razer_i18n::t(text),
                    false,
                )))
            })
            .into_any_element()
    }

    pub(super) fn render_camera_options(
        &self,
        control: &ControlSpec,
        disabled: bool,
        show_label: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use razer_widgets::theme::CameraProductColors as Colors;
        if control.key.ends_with(":hdr-mode") && control.options.len() == 2 {
            // Current _U toggles on either half, including the selected half.
            let first = self.value(control) == Some(&control.options[0].value);
            let next = control.options[usize::from(first)].value.clone();
            let key = control.key.clone();
            return gpui_kit::base::Button::new(SharedString::from(key.clone()))
                .accessibility_label(razer_i18n::t(&control.label))
                .disabled(disabled)
                .self_start()
                .h(surface::css(36.))
                .p(surface::css(5.))
                .rounded(surface::css(18.))
                .border_1()
                .border_color(Colors::border())
                .bg(Colors::background())
                .when(disabled, |button| button.opacity(0.3))
                .when(!disabled, |button| {
                    button
                        .cursor_pointer()
                        .hover(|button| button.border_color(Colors::focus()))
                })
                .focus_visible(|button| button.border_color(Colors::focus()))
                .child(
                    h_flex()
                        .items_center()
                        .children(control.options.iter().enumerate().map(|(index, option)| {
                            let active = (index == 0) == first;
                            div()
                                .h(surface::css(26.))
                                .px(surface::css(10.))
                                .py(surface::css(5.))
                                .rounded(surface::css(13.))
                                .text_size(surface::css(14.))
                                .line_height(surface::css(17.))
                                .bg(if active {
                                    Colors::focus()
                                } else {
                                    Colors::background()
                                })
                                .text_color(if active {
                                    Colors::background()
                                } else {
                                    Colors::text()
                                })
                                .child(razer_i18n::t(&option.label))
                        })),
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| {
                        this.edit(&key, next.clone(), window, cx)
                    }),
                )
                .into_any_element();
        }
        let title = h_flex()
            .items_center()
            .text_size(surface::css(14.))
            .line_height(surface::css(16.))
            .child(razer_i18n::t(&control.label))
            .when_some(control.tooltip.as_ref(), |title, text| {
                title.child(div().ml(surface::css(10.)).child(help(
                    SharedString::from(format!("{}:help", control.key)),
                    razer_i18n::t(text),
                    false,
                )))
            });
        v_flex()
            .gap(surface::css(10.))
            .when(show_label && !control.hide_label, |column| {
                column.child(title)
            })
            .child(h_flex().gap(surface::css(10.)).flex_wrap().children(
                control.options.iter().map(|option| {
                    let selected = self.value(control) == Some(&option.value);
                    let key = control.key.clone();
                    let value = option.value.clone();
                    gpui_kit::base::Button::new(SharedString::from(format!("{key}:{value}")))
                        .accessibility_label(razer_i18n::t(&option.label))
                        .disabled(disabled)
                        .min_w(surface::css(if key.ends_with(":iso") { 75. } else { 90. }))
                        .px(surface::css(16.))
                        .pt(surface::css(7.))
                        .pb(surface::css(6.))
                        .text_size(surface::css(12.))
                        .line_height(surface::css(14.))
                        .text_color(Colors::text())
                        .rounded(surface::css(3.))
                        .border_1()
                        .border_color(if selected {
                            Colors::focus()
                        } else {
                            Colors::border()
                        })
                        .when(selected, |button| button.bg(Colors::selected()))
                        .when(disabled, |button| button.opacity(0.3))
                        .when(!disabled, |button| {
                            button
                                .cursor_pointer()
                                .hover(|button| {
                                    button.bg(Colors::hover()).border_color(Colors::focus())
                                })
                                .active(|button| button.bg(Colors::pressed()))
                        })
                        .focus_visible(|button| button.border_color(Colors::focus()))
                        .child(razer_i18n::t(&option.label))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.edit(&key, value.clone(), window, cx)
                        }))
                }),
            ))
            .into_any_element()
    }

    /// Current TU/dh/wU mount sliders with noTip, 0/255 tags or translated
    /// COOL/WARM tags and gradient_slider. The retained SliderState continues
    /// to own pointer/keyboard changes and the existing edit subscription.
    pub(super) fn render_camera_slider(
        &self,
        control: &ControlSpec,
        disabled: bool,
        show_label: bool,
    ) -> AnyElement {
        let minimum = self.minimum(control);
        let value = self
            .value(control)
            .and_then(Value::as_f64)
            .unwrap_or(f64::from(minimum)) as f32;
        let progress = (value - minimum) / (control.max - minimum).max(f32::EPSILON);
        let white_balance = control.path == "@image/whiteBalance/value";
        let shutter = control.renderer.as_deref() == Some("camera-shutter");
        let labels = if white_balance {
            Some((razer_i18n::t("COOL"), razer_i18n::t("WARM")))
        } else if shutter {
            Some((razer_i18n::t("AGGRO"), razer_i18n::t("SLOW")))
        } else if control.key.ends_with(":compensation") {
            Some((format!("{minimum:.1}"), "+3.0".into()))
        } else if control.path.starts_with("@image/")
            || control.path == "/camera/autoExposure/exposureValue"
        {
            Some((minimum.to_string(), control.max.to_string()))
        } else {
            None // Current zoom and manual-focus mount noTag:true.
        };
        let slider =
            razer_widgets::source_slider::SourceSlider::new(&self.sliders[&control.key], progress)
                .enabled(!disabled)
                .interaction_enabled(!self.camera_control_blocked(control))
                .proportional_fill()
                .keyboard_steps(if shutter {
                    control
                        .options
                        .iter()
                        .filter_map(|o| o.value.as_f64().map(|v| v as f32))
                        .collect()
                } else {
                    Vec::new()
                })
                .when_some(labels, |slider, (minimum, maximum)| {
                    slider.range_labels(minimum, maximum, super::camera_theme::range_label())
                })
                .when(white_balance, |slider| {
                    slider.track_gradient(
                        super::camera_theme::white_balance_cool(),
                        super::camera_theme::white_balance_warm(),
                    )
                });
        if !show_label {
            return slider.into_any_element();
        }
        let mut title = h_flex()
            .items_center()
            .text_size(surface::css(14.))
            .line_height(surface::css(16.))
            .child(razer_i18n::t(&control.label));
        if let Some(tooltip) = &control.tooltip {
            title = title.child(div().ml(surface::css(10.)).child(help(
                SharedString::from(format!("{}:help", control.key)),
                razer_i18n::t(tooltip),
                false,
            )));
        }
        if let Some(stepper) = self.steppers.get(&control.key) {
            title = title.child(div().ml(surface::css(10.)).child(stepper.clone()));
        }
        if shutter {
            let selected = control
                .options
                .iter()
                .find(|option| self.value(control) == Some(&option.value));
            title = title.child(
                div().ml(surface::css(10.)).child(
                    selected
                        .map(|option| format!("{}s", option.label))
                        .unwrap_or_default(),
                ),
            );
        }
        v_flex()
            // `.shutter-speed-header` has 20px top margin; the mounted group
            // contributes the first 10px between these setting rows.
            .when(shutter || control.key.ends_with(":compensation"), |view| {
                view.mt(surface::css(10.))
            })
            .child(title)
            .child(div().mt(surface::css(10.)).child(slider))
            .into_any_element()
    }

    pub(super) fn render_camera_group(
        &self,
        group: &CameraGroup,
        separator: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use razer_widgets::theme::CameraProductColors as Colors;
        let open = !self.collapsed_camera_groups.contains(&group.key);
        let inherited_disabled = self.camera_group_blocked(group);
        let collapse_key = group.key.clone();
        let title = razer_i18n::t(&group.title).to_uppercase();
        let header_label = div()
            .flex()
            .items_center()
            .when(group.collapsible, |view| {
                view.child(
                    Icon::default()
                        .path("synapse/camera-section-down.svg")
                        .size(surface::css(10.))
                        .mr(surface::css(10.))
                        .text_color(Colors::text())
                        .transform(Transformation::rotate(radians(if open {
                            0.
                        } else {
                            -std::f32::consts::FRAC_PI_2
                        }))),
                )
            })
            .child(title.clone());
        let mut header = h_flex()
            .items_center()
            .text_size(surface::css(14.))
            .line_height(surface::css(16.))
            .child(if group.collapsible {
                gpui_kit::base::Button::new(SharedString::from(format!("{}:toggle", group.key)))
                    .accessibility_label(title)
                    .disabled(inherited_disabled)
                    .p_0()
                    .cursor_pointer()
                    .focus_visible(|button| button.border_1().border_color(cx.theme().primary))
                    .child(header_label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.collapsed_camera_groups.remove(&collapse_key) {
                            this.collapsed_camera_groups.insert(collapse_key.clone());
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            } else {
                header_label.into_any_element()
            });
        if let Some(control) = group
            .header_switch
            .as_deref()
            .and_then(|key| self.control(key))
        {
            let key = control.key.clone();
            header = header.child(
                div().ml(surface::css(10.)).child(
                    surface::SynapseSwitch::new(SharedString::from(format!("{key}:header")))
                        .accessibility_label(razer_i18n::t(&control.label))
                        .checked(
                            self.value(control)
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        )
                        .disabled(self.disabled(control))
                        .on_change(cx.listener(move |this, next, window, cx| {
                            this.edit(&key, Value::Bool(*next), window, cx)
                        })),
                ),
            );
        }
        if let Some(control) = group
            .header_reset
            .as_deref()
            .and_then(|key| self.control(key))
        {
            let key = control.key.clone();
            header = header.child(
                gpui_kit::base::Button::new(SharedString::from(format!("{key}:header")))
                    .accessibility_label(razer_i18n::t(&control.label))
                    .ml(surface::css(10.))
                    .w(surface::css(20.))
                    .h(surface::css(20.))
                    .p_0()
                    .disabled(self.disabled(control))
                    .cursor_pointer()
                    .hover(|button| button.opacity(0.7))
                    .focus_visible(|button| button.border_1().border_color(cx.theme().primary))
                    .child(img("synapse/camera-section-reset.svg").size_full())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit(&key, Value::Null, window, cx)
                    })),
            );
        }
        if let Some(text) = &group.tooltip {
            header = header.child(
                div()
                    .ml(surface::css(10.))
                    .when(
                        group.presentation.as_deref() == Some("preview-source"),
                        |view| view.mr(surface::css(25.)),
                    )
                    .child(help(
                        SharedString::from(format!("{}:help", group.key)),
                        razer_i18n::t(text),
                        false,
                    )),
            );
        }
        if group.presentation.as_deref() == Some("preview-source") {
            header = header.child(self.render_camera_preview_actions(cx));
        }
        if group
            .warning_any
            .iter()
            .any(|path| self.observed_value(path).and_then(Value::as_bool) == Some(true))
        {
            if let Some(text) = &group.warning {
                header = header.child(div().ml_auto().child(help(
                    SharedString::from(format!("{}:warning", group.key)),
                    razer_i18n::t(text),
                    true,
                )));
            }
        }
        if let Some(stepper) = group
            .header_stepper
            .as_deref()
            .and_then(|key| self.steppers.get(key))
        {
            header = header.child(div().ml(surface::css(10.)).child(stepper.clone()));
        }
        if let Some(control) = group
            .header_checkbox
            .as_deref()
            .and_then(|key| self.control(key))
        {
            header = header.child(
                div()
                    .ml(surface::css(10.))
                    .mt(surface::css(3.))
                    .child(self.render_control(control, window, cx)),
            );
        }
        let mut content = v_flex().gap(surface::css(10.));
        let mut has_content = false;
        if group.presentation.as_deref() == Some("preview-source") {
            content = content.child(self.render_camera_source_select());
            if self.camera_third_party_selected() {
                content = content.child(self.render_camera_third_party_notice(false, cx));
            }
            has_content = true;
        }
        for key in &group.controls {
            if [
                group.header_switch.as_ref(),
                group.header_checkbox.as_ref(),
                group.header_reset.as_ref(),
            ]
            .into_iter()
            .flatten()
            .any(|header| header == key)
            {
                continue;
            }
            let Some(control) = self.control(key) else {
                continue;
            };
            if !self.visible(control) {
                continue;
            }
            content = content.child(self.render_control_with_label(
                control,
                control.label != group.title,
                window,
                cx,
            ));
            has_content = true;
        }
        let description_visible =
            group.description_when_enabled.as_ref().is_none_or(|path| {
                self.observed_value(path).and_then(Value::as_bool) == Some(true)
            }) && group.description_min_height.is_none_or(|minimum| {
                self.draft
                    .pointer("/camera/resolution/height")
                    .and_then(Value::as_u64)
                    .is_some_and(|height| height >= minimum)
            });
        if description_visible {
            if let Some(description) = &group.description {
                content = content.child(
                    div()
                        .text_size(surface::css(14.))
                        .line_height(surface::css(17.))
                        .child(razer_i18n::t(description)),
                );
                has_content = true;
            }
        }
        if group.key.ends_with(":CAMERA:PREVIEW_RESOLUTION_2")
            && self.camera_higher_gen_cable_required()
        {
            content = content.child(
                div()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .child(razer_i18n::t("HIGHER_GEN_CABLE_REQUIRED")),
            );
            has_content = true;
        }
        let wrapper = Collapsible::new()
            .open(open)
            .child(header)
            .when(has_content, |wrapper| {
                wrapper.content(content.mt(surface::css(10.)))
            });
        v_flex()
            .id(SharedString::from(format!("{}:group", group.key)))
            .when(
                separator && group.divider.as_deref() != Some("none"),
                |view| {
                    view.child(
                        div()
                            .w_full()
                            .my(surface::css(20.))
                            .border_1()
                            .border_color(if group.divider.as_deref() == Some("transparent") {
                                cx.theme().transparent
                            } else {
                                Colors::divider()
                            })
                            .rounded(surface::css(2.)),
                    )
                },
            )
            .when(inherited_disabled, |group| {
                group
                    .opacity(0.3)
                    .capture_any_mouse_down(|_, _, cx| cx.stop_propagation())
                    .capture_key_down(|event, _, cx| {
                        if event.keystroke.key != "tab" {
                            cx.stop_propagation();
                        }
                    })
            })
            .child(wrapper)
            .into_any_element()
    }
}
