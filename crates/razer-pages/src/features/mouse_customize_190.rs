//! Current 190 CO / gO surface, not the generic product card.
use super::*;
use gpui_kit::base::Button as BaseButton;

impl MouseProductWorkspace {
    pub(super) fn customize_190(&self, cx: &Context<Self>) -> AnyElement {
        let buttons = self.spec.groups[0]["buttonList"]
            .as_array()
            .expect("190 source group");
        let hovered = self.customize_hover;
        let selected = buttons
            .iter()
            .position(|button| button["inputID"].as_str() == self.mapping_input.as_deref());
        let hypershift = self.hypershift;
        let enabled: [bool; 8] = std::array::from_fn(|index| buttons[index]["isEnabled"] == true);
        let lines = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let scale = f32::from(window.rem_size()) / 16.;
                let position = |x, y| bounds.origin + point(px(x * scale), px(y * scale));
                let circle = |x, y, radius, color: Hsla, window: &mut Window| {
                    window.paint_quad(PaintQuad {
                        corner_radii: Corners::all(px(radius * scale)),
                        ..fill(
                            Bounds::new(
                                position(x - radius, y - radius),
                                size(px(radius * 2. * scale), px(radius * 2. * scale)),
                            ),
                            color,
                        )
                    });
                };
                let cycle_shown = hovered == Some(7) || selected == Some(7);
                let targets = [
                    (114., 55.),
                    (181., 55.),
                    (147., 88.),
                    (148., 70.),
                    (148., 105.),
                    (88., 135.),
                    (88., 175.),
                    if cycle_shown {
                        (186., 173.)
                    } else {
                        (214., 169.)
                    },
                ];
                for (left, indices) in [(true, [0, 2, 5, 6]), (false, [1, 3, 4, 7])] {
                    for (row, index) in indices.into_iter().enumerate() {
                        let x = if left { 3. } else { 307. };
                        let y = 16. + row as f32 * 50.;
                        let (target_x, target_y) = targets[index];
                        let target_x = target_x + 7.5;
                        let target_y = target_y + 2.5;
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
                        path.line_to(position(x + if left { 53. } else { -53. }, y));
                        path.line_to(position(target_x, target_y));
                        if let Ok(path) = path.build() {
                            window.paint_path(path, color);
                        }
                        circle(x, y, 2.5, color, window);
                        // Source passes a non-number radius for the hidden cycle
                        // endpoint; preserve that omission rather than invent a dot.
                        if index != 7 || cycle_shown {
                            circle(target_x, target_y, 2.5, color, window);
                        }
                        if active {
                            let mut ring = PathBuilder::stroke(px(scale));
                            for step in 0..=32 {
                                let angle = step as f32 / 32. * std::f32::consts::TAU;
                                let point = position(
                                    target_x + angle.cos() * 5.5,
                                    target_y + angle.sin() * 5.5,
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
                img("synapse/mouse-products/190.svg")
                    .absolute()
                    // Source `.config-img.svg-image`: left `50%-87px`,
                    // top 26px, natural 176x296 SVG dimensions.
                    .left(surface::css(298.))
                    .top(surface::css(26.))
                    .h(surface::css(296.))
                    .w(surface::css(176.)),
            )
            .child(lines)
            .child(self.customize_190_labels(true, [0, 2, 5, 6], buttons, cx))
            .child(self.customize_190_labels(false, [1, 3, 4, 7], buttons, cx));
        let hyper = BaseButton::new("mouse-190-hypershift")
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
                            .bg(if hypershift {
                                rgba(0).into()
                            } else {
                                rgb(0x44d62c).into()
                            })
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
                            .bg(if hypershift {
                                rgb(0xfd8611).into()
                            } else {
                                rgba(0).into()
                            })
                            .text_color(rgb(if hypershift { 0x212121 } else { 0xcccccc }))
                            .child(t("HYPERSHIFT")),
                    ),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.hypershift = !this.hypershift;
                this.mapping_input = None;
                this.mapping_assignment = None;
                this.customize_hover = None;
                cx.notify();
            }));
        v_flex()
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
                    .my(surface::css(20.))
                    .gap(surface::css(10.))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(14.))
                    .child(hyper)
                    .child(surface::help_control(
                        "mouse-190-hypershift-help",
                        t("HYPERSHIFT_TOOLTIP"),
                    )),
            )
            .children(self.mapping_input.as_ref().map(|_| self.mapping_190(cx)))
            .into_any_element()
    }

    fn customize_190_labels(
        &self,
        left: bool,
        indices: [usize; 4],
        buttons: &[Value],
        cx: &Context<Self>,
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
                let input = button["inputID"].as_str().expect("190 input").to_owned();
                let active = self.mapping_input.as_deref() == Some(&input);
                let enabled = button["isEnabled"] == true;
                BaseButton::new(SharedString::from(format!("mouse-190-label-{index}")))
                    .h(surface::css(30.))
                    .mb(surface::css(20.))
                    .px(surface::css(10.))
                    .py_0()
                    .when(left, |button| button.mr(surface::css(10.)))
                    .when(!left, |button| button.ml(surface::css(10.)))
                    .rounded(surface::css(3.))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .text_color(if enabled {
                        rgb(0xcccccc).into()
                    } else {
                        rgba(0xcccccc4d).into()
                    })
                    .bg(if active {
                        rgb(0x111111).into()
                    } else {
                        rgba(0).into()
                    })
                    .disabled(!enabled)
                    .hover(|style| style.bg(rgb(0x383838)))
                    .active(|style| style.bg(rgb(0x111111)))
                    .child(t(button["assignmentValue"].as_str().unwrap_or("DEFAULT")))
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        this.customize_hover = hovered.then_some(index);
                        cx.notify();
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.mapping_input = Some(input.clone());
                        this.mapping_assignment = None;
                        cx.notify();
                    }))
            }))
            .into_any_element()
    }

    fn mapping_190(&self, cx: &Context<Self>) -> AnyElement {
        let Some(input) = self.mapping_input.as_deref() else {
            return div().into_any_element();
        };
        let assignment = self.mapping_assignment.as_deref();
        let choices = [
            ("Click", "LEFT_CLICK"),
            ("Menu", "RIGHT_CLICK"),
            ("ScrollButton", "SCROLL_CLICK"),
            ("Previous", "MOUSE_BUTTON_4"),
            ("Next", "MOUSE_BUTTON_5"),
            ("ScrollUp", "SCROLL_UP"),
            ("ScrollDown", "SCROLL_DOWN"),
        ];
        let input_id = input.to_owned();
        div()
            .id("mouse-190-mapping-popup")
            .absolute()
            .left(surface::css(50.))
            .top(surface::css(380.))
            .w(surface::css(292.))
            .min_h(surface::css(220.))
            .p(surface::css(20.))
            .bg(rgb(0x111111))
            .border_1()
            .border_color(rgb(0x5d5d5d))
            .rounded(surface::css(5.))
            .child(div().text_center().mb(surface::css(15.)).child(t(input)))
            .child(
                v_flex()
                    .gap(surface::css(5.))
                    .children(choices.into_iter().map(|(id, label)| {
                        Button::new(SharedString::from(format!("mouse-190-assignment-{id}")))
                            .label(t(label))
                            .outline()
                            .selected(assignment == Some(id))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.mapping_assignment = Some(id.to_owned());
                                cx.notify();
                            }))
                    })),
            )
            .child(
                h_flex()
                    .gap(surface::css(8.))
                    .mt(surface::css(15.))
                    .justify_end()
                    .child(
                        Button::new("mouse-190-mapping-cancel")
                            .label(t("CANCEL"))
                            .outline()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mapping_input = None;
                                this.mapping_assignment = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("mouse-190-mapping-save")
                            .label(t("SAVE"))
                            .primary()
                            .disabled(assignment.is_none())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let Some(assignment) = this.mapping_assignment.take() else {
                                    return;
                                };
                                let hypershift = this.hypershift;
                                let mappings = this.draft["mappings"].as_array_mut();
                                let Some(mappings) = mappings else {
                                    return;
                                };
                                mappings.retain(|entry| {
                                    entry["inputID"] != input_id
                                        || entry["isHyperShift"] != hypershift
                                });
                                mappings.push(json!({
                                    "inputID": input_id,
                                    "isHyperShift": hypershift,
                                    "inputType": "MouseInput",
                                    "outputType": "mouseGroup",
                                    "mouseGroup": {"mouseAssignment": assignment}
                                }));
                                cx.emit(MouseProductChanged);
                                this.mapping_input = None;
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }
}
