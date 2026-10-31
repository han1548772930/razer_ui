//! Current PID 70 customization surface (`lT`/`rT`, main.8f24b6a1.js).
//! The source mounts an 11-button top-view block, rather than the generic
//! product card. Mapping edits remain local profile drafts.
use super::*;
use razer_widgets::keyboard_navigation::Button as BaseButton;
use gpui_kit::base::{ElementExt as _};
use razer_widgets::theme::MouseCustomizeColors as Colors;

#[derive(Default)]
struct LabelHover {
    button: bool,
    wrapper: bool,
    requires_synapse: bool,
}

fn source_tip() -> Div {
    div()
        .absolute()
        .bg(rgb(0x000000))
        .border_1()
        .border_color(Colors::line())
        .text_color(Colors::text())
        .text_size(surface::css(14.))
        .line_height(surface::css(16.))
        .pt(surface::css(8.))
        .px(surface::css(10.))
        .pb(surface::css(10.))
}

impl MouseProductWorkspace {
    pub(super) fn customize_70(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let original_buttons = self.spec.groups[0]["buttonList"]
            .as_array()
            .expect("70 source group");
        // pT.updateMappingsToButtons feeds the same derived rows to lT/rT and
        // the sidebar. Using the descriptors here hid non-mouse assignments
        // and bypassed the source's sole-primary-click protection.
        let projection = self.derive_current_mapping_buttons(original_buttons, original_buttons, 0);
        let unavailable = projection.is_err();
        *self.button_state_error.borrow_mut() = projection.as_ref().err().cloned();
        let buttons = projection.unwrap_or_else(|_| original_buttons.clone());
        let selected = buttons
            .iter()
            .position(|button| button["inputID"].as_str() == self.mapping_input.as_deref());
        let hovered = self.customize_hover;
        let hypershift = self.hypershift;
        let enabled: Vec<bool> = buttons
            .iter()
            .map(|button| !unavailable && button["isEnabled"] == true)
            .collect();
        let targets = [
            (114., 53.),
            (148., 84.),
            (125., 95.),
            (80., 140.),
            (80., 180.),
            (181., 53.),
            (148., 67.),
            (168., 95.),
            (148., 110.),
            (148., 133.),
            (148., 153.),
        ];
        let lines = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let scale = f32::from(window.rem_size()) / 16.;
                let position = |x: f32, y: f32| bounds.origin + point(px(x * scale), px(y * scale));
                let dot = |x: f32, y: f32, color: Hsla, window: &mut Window| {
                    window.paint_quad(PaintQuad {
                        corner_radii: Corners::all(px(2.5 * scale)),
                        ..fill(
                            Bounds::new(
                                position(x - 2.5, y - 2.5),
                                size(px(5. * scale), px(5. * scale)),
                            ),
                            color,
                        )
                    });
                };
                for index in 0..11 {
                    let left = index < 5;
                    let row = if left { index } else { index - 5 };
                    let (x, y) = if left {
                        (3., 16. + row as f32 * 50.)
                    } else {
                        (307., 16. + row as f32 * 50.)
                    };
                    let (target_x, target_y) = targets[index];
                    let active = hovered == Some(index) || selected == Some(index);
                    let color: Hsla = if !enabled[index] {
                        rgba(0xcccccc1a).into()
                    } else if active {
                        rgb(if hypershift { 0xfd8611 } else { 0x44d62c }).into()
                    } else {
                        rgb(0x5d5d5d).into()
                    };
                    let mut path = PathBuilder::stroke(px(scale));
                    path.move_to(position(x, y));
                    path.line_to(position(if left { x + 53. } else { x - 53. }, y));
                    path.line_to(position(target_x + 7.5, target_y + 2.5));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                    dot(x, y, color, window);
                    dot(target_x + 7.5, target_y + 2.5, color, window);
                    if active {
                        let mut ring = PathBuilder::stroke(px(scale));
                        for step in 0..=32 {
                            let angle = step as f32 / 32. * std::f32::consts::TAU;
                            let point = position(
                                target_x + 7.5 + angle.cos() * 5.5,
                                target_y + 2.5 + angle.sin() * 5.5,
                            );
                            if step == 0 {
                                ring.move_to(point);
                            } else {
                                ring.line_to(point);
                            }
                        }
                        if let Ok(ring) = ring.build() {
                            window.paint_path(ring, color);
                        }
                    }
                }
            },
        )
        .absolute()
        .left(surface::css(230.))
        .w(surface::css(310.))
        .h(surface::css(340.));
        let block = div()
            .relative()
            .w(surface::css(770.))
            .h(surface::css(340.))
            .mx_auto()
            .child(
                img("synapse/mouse-products/70.png")
                    .absolute()
                    .left(surface::css(235.))
                    .top_0()
                    .max_w(surface::css(300.))
                    .max_h(surface::css(340.))
                    .object_fit(ObjectFit::Contain),
            )
            .child(lines)
            .child(self.customize_70_labels(
                true,
                [0, 1, 2, 3, 4],
                &buttons,
                unavailable,
                window,
                cx,
            ))
            .child(self.customize_70_labels(
                false,
                [5, 6, 7, 8, 9, 10],
                &buttons,
                unavailable,
                window,
                cx,
            ));
        let hyper = BaseButton::new("mouse-70-hypershift")
            .role(Role::Switch)
            .selected(hypershift)
            .accessibility_label(t("HYPERSHIFT"))
            .h(surface::css(36.))
            .p(surface::css(5.))
            .border_1()
            .border_color(rgb(0x5d5d5d))
            .rounded(surface::css(18.))
            .bg(rgb(0x111111))
            .hover(|style| style.border_color(rgb(0x44d62c)))
            .active(|style| style.bg(rgb(0x292929)))
            .child(
                h_flex()
                    .child(
                        div()
                            .px(surface::css(10.))
                            .pt(surface::css(6.))
                            .pb(surface::css(5.))
                            .h(surface::css(24.))
                            .mr(surface::css(5.))
                            .rounded(surface::css(12.))
                            .bg(if hypershift { rgba(0) } else { rgb(0x44d62c) })
                            .text_color(rgb(if hypershift { 0xcccccc } else { 0x212121 }))
                            .child(t("STANDARD")),
                    )
                    .child(
                        div()
                            .px(surface::css(10.))
                            .pt(surface::css(6.))
                            .pb(surface::css(5.))
                            .h(surface::css(24.))
                            .rounded(surface::css(12.))
                            .bg(if hypershift { rgb(0xfd8611) } else { rgba(0) })
                            .text_color(rgb(if hypershift { 0x212121 } else { 0xcccccc }))
                            .child(t("HYPERSHIFT")),
                    ),
            )
            .on_click(cx.listener(|this, _, window, cx| {
                this.request_current_hypershift(window, cx);
            }));
        let content = div()
            .w_full()
            .child(
                div()
                    .relative()
                    .w_full()
                    .min_w(surface::css(770.))
                    .max_w(surface::css(1220.))
                    .mx_auto()
                    .h(surface::css(340.))
                    .child(surface::dot_background(cx))
                    .child(block),
            )
            .child(
                h_flex()
                    .items_center()
                    .justify_center()
                    .mt(surface::css(self.customize_footer_margins().0))
                    .mb(surface::css(self.customize_footer_margins().1))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(14.))
                    .children(
                        (self.mouse_customize_observation["customizeReducer"]["displayMode"]
                            != "macro")
                            .then(|| {
                                div()
                                    .mx(surface::css(5.))
                                    .flex_shrink_0()
                                    .child(self.current_sidebar_toggle(cx))
                            }),
                    )
                    .child(div().mx(surface::css(5.)).flex_shrink_0().child(hyper))
                    .child(div().mx(surface::css(5.)).flex_shrink_0().child(
                        surface::hypershift_help_control(
                            "mouse-70-hypershift-help",
                            t("HYPERSHIFT_TOOLTIP"),
                        ),
                    )),
            );
        self.current_sidebar_surface(&buttons, unavailable, content, window, cx)
            .children(
                self.mapping_input
                    .as_ref()
                    .map(|_| self.mapping_70(window, cx)),
            )
            .into_any_element()
    }

    fn customize_70_labels(
        &self,
        left: bool,
        indices: impl IntoIterator<Item = usize>,
        buttons: &[Value],
        unavailable: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut column = v_flex().absolute().top_0().w(surface::css(235.));
        column = if left {
            column
                .right(surface::css(535.))
                .pr(surface::css(15.))
                .items_end()
        } else {
            column
                .left(surface::css(535.))
                .pl(surface::css(15.))
                .items_start()
        };
        column
            .children(indices.into_iter().map(|index| {
                let button = &buttons[index];
                let input = button["inputID"].as_str().unwrap_or_default().to_owned();
                let macro_mode =
                    self.mouse_customize_observation["customizeReducer"]["displayMode"] == "macro";
                let enabled = !unavailable
                    && button["isEnabled"] == true
                    && button["disabled"] != true
                    && (!macro_mode
                        || button["functionList"].as_array().is_some_and(|functions| {
                            functions.iter().any(|function| function == "MACRO")
                        }));
                let active = self.mapping_input.as_deref() == Some(input.as_str());
                let label = t(button["assignmentValue"].as_str().unwrap_or("DEFAULT"));
                let anchor_input = input.clone();
                let anchors = self.mapping_menu_current_state.anchors.clone();
                let inter = button["assignment"] == "INTERDEVICE";
                let target_connected = self.mouse_customize_observation["validDevices"]
                    .as_array()
                    .is_some_and(|rows| {
                        rows.iter()
                            .any(|row| row["productId"] == button["assignmentGroup"]["interPID"])
                    });
                let requires_synapse =
                    button["isMapped"] == true && button["isRequireSynapse"] == true;
                let state = window.use_keyed_state(
                    (
                        ElementId::from(("mouse70-label", cx.entity_id())),
                        input.clone(),
                    ),
                    cx,
                    |_, _| LabelHover::default(),
                );
                let hovered = enabled && state.read(cx).button;
                let require_hovered = enabled && state.read(cx).requires_synapse;
                let source_disabled = button["disabled"] == true || button["isEnabled"] != true;
                let tooltip_hovered = hovered || (source_disabled && state.read(cx).wrapper);
                let top = anchors
                    .borrow()
                    .get(&input)
                    .map(|b| f32::from(b.top()) / (f32::from(window.rem_size()) / 16.))
                    .unwrap_or_default();
                let tooltip = self.customize_70_tip(button, source_disabled, target_connected, top);
                let tooltip_opacity = gpui_kit::base::motion::Presence::new(
                    (
                        ElementId::from(("mouse70-label-tip", cx.entity_id())),
                        input.clone(),
                    ),
                    tooltip_hovered && !require_hovered,
                )
                .transition(
                    gpui_kit::base::motion::Transition::new(std::time::Duration::from_millis(300))
                        .easing(gpui_kit::base::motion::Easing::Linear),
                )
                .sample(window, cx)
                .progress;
                let require_opacity = gpui_kit::base::motion::Presence::new(
                    (
                        ElementId::from(("mouse70-require-tip", cx.entity_id())),
                        input.clone(),
                    ),
                    require_hovered,
                )
                .transition(
                    gpui_kit::base::motion::Transition::new(std::time::Duration::from_millis(300))
                        .easing(gpui_kit::base::motion::Easing::Linear),
                )
                .sample(window, cx)
                .progress;
                let mut label_contents = h_flex()
                    .items_center()
                    .when(inter, |row| row.max_w(surface::css(210.)));
                if inter {
                    label_contents = label_contents.child(
                        img(if !target_connected {
                            "synapse/mouse70-interdevice-disconnected.svg"
                        } else if self.hypershift {
                            "synapse/mouse70-interdevice-hypershift.svg"
                        } else {
                            "synapse/mapping-interdevice-active.svg"
                        })
                        .size(surface::css(20.))
                        .mr(surface::css(4.)),
                    );
                }
                label_contents = label_contents.child(
                    div()
                        .max_w(surface::css(200.))
                        .text_ellipsis()
                        .child(label.clone()),
                );
                let hover_state = state.clone();
                let wrapper_state = state.clone();
                let mut control =
                    BaseButton::new(SharedString::from(format!("mouse-70-label-{input}")))
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
                        .text_color(
                            if button["assignment"] == "DISABLE" || (inter && !target_connected) {
                                Colors::unavailable_mapping()
                            } else if button["isMapped"] == true {
                                if self.hypershift {
                                    Colors::hypershift()
                                } else {
                                    Colors::selected()
                                }
                            } else if !enabled {
                                Colors::disabled_text()
                            } else {
                                Colors::text()
                            },
                        )
                        .bg(if active { rgb(0x111111) } else { rgba(0) })
                        .disabled(!enabled)
                        .when(enabled && !active, |button| {
                            button.hover(|style| style.bg(rgb(0x383838)))
                        })
                        .when(enabled, |button| {
                            button.active(|style| style.bg(rgb(0x111111)))
                        })
                        .child(label_contents)
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            hover_state.update(cx, |state, cx| {
                                state.button = *hovered;
                                cx.notify();
                            });
                            if enabled {
                                this.customize_hover = hovered.then_some(index);
                                cx.notify();
                            }
                        }))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.request_current_canvas_mapping(input.clone(), window, cx);
                        }));
                if requires_synapse {
                    let require_state = state.clone();
                    let mut icon = div()
                        .id(("mouse70-requires", index))
                        .absolute()
                        .top_0()
                        .size(surface::css(32.))
                        .rounded_full()
                        .border_1()
                        .border_color(if hovered || active {
                            if self.hypershift {
                                Colors::hypershift()
                            } else {
                                Colors::selected()
                            }
                        } else {
                            Colors::line()
                        })
                        .bg(if self.hypershift {
                            rgb(0xfd8611)
                        } else {
                            rgb(0x008900)
                        })
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(img("synapse/mouse70-requires-synapse.svg").size(surface::css(20.)))
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            if !enabled {
                                return;
                            }
                            require_state.update(cx, |state, cx| {
                                state.requires_synapse = *hovered;
                                cx.notify();
                            });
                            if *hovered {
                                this.customize_hover = None;
                            }
                            cx.notify();
                        }));
                    icon = if left {
                        icon.right(surface::css(-29.))
                    } else {
                        icon.left(surface::css(-29.))
                    };
                    if require_hovered {
                        icon = icon.child(
                            razer_assets::deferred_resource(
                                source_tip()
                                    .opacity(require_opacity)
                                    .top(relative(1.1))
                                    .left(surface::css(-9.))
                                    .whitespace_nowrap()
                                    .child(t("MAPPING_REQUIRES_RAZER_SYNAPSE")),
                            )
                            .with_priority(100),
                        );
                    }
                    control = control.child(icon);
                }
                div()
                    .id(("mouse70-label-wrapper", index))
                    .relative()
                    .child(control)
                    .children(
                        tooltip
                            .filter(|_| tooltip_hovered && !require_hovered)
                            .map(|tip| {
                                razer_assets::deferred_resource(tip.opacity(tooltip_opacity))
                                    .with_priority(100)
                            }),
                    )
                    .on_hover(cx.listener(move |_, hovered: &bool, _, cx| {
                        wrapper_state.update(cx, |state, cx| {
                            state.wrapper = *hovered;
                            cx.notify();
                        });
                    }))
            }))
            .into_any_element()
    }

    fn customize_70_tip(
        &self,
        button: &Value,
        disabled: bool,
        connected: bool,
        top: f32,
    ) -> Option<Div> {
        self.current_customize_label_tip(button, disabled, connected, top)
    }

    fn mapping_70(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(input) = self.mapping_input.as_deref() else {
            return div().into_any_element();
        };
        let Some(button) = self.current_mapping_source_button(input) else {
            return div().into_any_element();
        };
        let Some(contents) = self.current_mouse_mapping_popup_contents(&button, window, cx) else {
            return div().into_any_element();
        };
        self.current_mouse_mapping_popup_layer(contents.id("mouse-70-mapping-popup"))
    }
}
