//! Current 190 CO / gO surface, not the generic product card.
use super::*;
use gpui_kit::base::Button as BaseButton;
use gpui_kit::base::ElementExt as _;
#[path = "mouse_mapping_190.rs"]
pub(super) mod source_mapping;
use razer_widgets::scroll::SourceScrollable as _;
use source_mapping::Next;

impl MouseProductWorkspace {
    pub(super) fn customize_190(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.mapping_190_state;
        let drawer_width = gpui_kit::base::motion::transition(
            ElementId::from(("mouse-190-panel-width", cx.entity_id())),
            if state.panel_open { 230. } else { 0. },
            gpui_kit::base::motion::Transition::new(std::time::Duration::from_millis(200))
                .easing(gpui_kit::base::motion::Easing::Ease),
            window,
            cx,
        );
        let root_bounds = state.surface_bounds.clone();
        let height = (window.viewport_size().height - window.rem_size() * (218. / 16.))
            .max(window.rem_size() * (380. / 16.));
        let content = div()
            .id("mouse-190-config-scroll")
            .relative()
            .h_full()
            .flex_1()
            .min_w_0()
            .on_scroll_wheel(cx.listener(|_, _, _, cx| cx.notify()))
            .scrollable_both()
            .track_scroll(&state.body_scroll)
            .child(
                div()
                    .min_w(surface::css(770.))
                    .w_full()
                    .when(state.panel_open, |body| body.w(surface::css(1000.)))
                    .child(self.customize_190_image(cx)),
            );
        h_flex()
            .id("mouse-190-customize-surface")
            .relative()
            .w_full()
            .h(height)
            .min_w_0()
            .items_stretch()
            .on_prepaint(move |bounds, _, _| root_bounds.set(bounds))
            .child(
                div()
                    .relative()
                    .w(surface::css(drawer_width))
                    .h_full()
                    .flex_shrink_0()
                    .overflow_hidden()
                    .child(self.customize_190_panel(drawer_width, window, cx)),
            )
            .child(content)
            .children(self.mapping_input.as_ref().map(|_| {
                deferred(source_mapping::PopupLayer::new(
                    self.mapping_190_drawer(window, cx),
                    &self.mapping_190_state,
                    self.mapping_input.as_deref().unwrap_or(""),
                ))
                .with_priority(103)
            }))
            .children(state.panel_tooltip.as_ref().map(|text| {
                deferred(source_mapping::BindingTooltip::new(
                    text.clone(),
                    state,
                    window,
                ))
                .with_priority(999)
            }))
            .children(
                self.mapping_190_confirmation(window, cx)
                    .map(|content| deferred(content).with_priority(1100)),
            )
            .into_any_element()
    }

    fn customize_190_panel(
        &self,
        width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = &self.mapping_190_state;
        let mut buttons = self.spec.groups[0]["buttonList"]
            .as_array()
            .expect("190 buttons")
            .iter()
            .filter(|button| {
                button["disabled"] != true
                    && button["isSidePanelList"] != false
                    && (button.get("isKeyToggle").is_none()
                        || button["isKeyToggle"].as_bool() == Some(self.hypershift))
                    && (!self.hypershift
                        || button.get("HID").is_none()
                        || button["disableHypershiftMapping"] != true)
                    && (state.panel_filter == 0
                        || self
                            .mapping_190_current(button["inputID"].as_str().unwrap_or(""))
                            .is_some())
            })
            .collect::<Vec<_>>();
        if buttons
            .iter()
            .all(|button| button["counter"].as_i64().is_some())
        {
            buttons.sort_by_key(|button| button["counter"].as_i64());
        }
        let anchors = state.anchors.clone();
        let mut rows = v_flex()
            .id("mouse-190-panel-list")
            .w_full()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&state.panel_scroll);
        for button in buttons {
            let input = button["inputID"].as_str().unwrap_or("").to_owned();
            let active = self.mapping_input.as_deref() == Some(&input);
            let enabled = self.mapping_190_enabled(button);
            let remapped = self.mapping_190_current(&input).is_some();
            let label = self.mapping_190_label(button);
            let assignment = if remapped {
                self.mapping_190_assignment_label(&input)
            } else {
                t(button["assignment"].as_str().unwrap_or("MOUSE"))
            };
            let color = if !enabled {
                rgb(0x707070)
            } else if remapped {
                rgb(if self.hypershift { 0xfd8611 } else { 0x44d62c })
            } else {
                rgb(0xcccccc)
            };
            let overflow = std::rc::Rc::new(std::cell::Cell::new(false));
            let overflow_measure = overflow.clone();
            let row = std::rc::Rc::new(std::cell::Cell::new(Bounds::default()));
            let row_measure = row.clone();
            let measure_label = label.to_string();
            let hover_label = label.to_string();
            let key = input.clone();
            let row_anchors = anchors.clone();
            rows = rows.child(
                div()
                    .w_full()
                    .on_prepaint(move |bounds, _, _| {
                        row_measure.set(bounds);
                        row_anchors
                            .borrow_mut()
                            .insert(format!("panel:{key}"), bounds);
                    })
                    .child(
                        BaseButton::new(SharedString::from(format!("mouse-190-panel-{input}")))
                            .w_full()
                            .min_h(surface::css(50.))
                            .p(surface::css(9.))
                            .justify_start()
                            .disabled(!enabled)
                            .text_color(color)
                            .bg(if active { rgb(0x111111) } else { rgba(0) })
                            .hover(|style| style.bg(rgb(0x383838)))
                            .child(
                                h_flex()
                                    .items_center()
                                    .w_full()
                                    .child(
                                        div()
                                            .w(surface::css(46.))
                                            .ml(surface::css(-10.))
                                            .text_center()
                                            .text_color(if enabled {
                                                rgb(0xcccccc)
                                            } else {
                                                rgb(0x707070)
                                            })
                                            .text_size(surface::css(14.))
                                            .line_height(surface::css(14.))
                                            .child(
                                                button["counter"].as_i64().unwrap_or(0).to_string(),
                                            ),
                                    )
                                    .child(
                                        v_flex()
                                            .w(surface::css(164.))
                                            .pl(surface::css(5.))
                                            .border_l_1()
                                            .border_color(rgb(0x707070))
                                            .child(
                                                div()
                                                    .text_size(surface::css(10.))
                                                    .text_color(rgb(0x707070))
                                                    .line_height(surface::css(12.))
                                                    .child(assignment.to_uppercase()),
                                            )
                                            .child(
                                                div()
                                                    .id(SharedString::from(format!(
                                                        "mouse-190-binding-{input}"
                                                    )))
                                                    .w_full()
                                                    .text_size(surface::css(14.))
                                                    .line_height(surface::css(16.))
                                                    .text_ellipsis()
                                                    .on_prepaint(move |bounds, window, _| {
                                                        overflow_measure.set(
                                                            remapped
                                                                && surface::label_width(
                                                                    &measure_label,
                                                                    14.,
                                                                    window,
                                                                ) * f32::from(window.rem_size())
                                                                    / 16.
                                                                    > f32::from(bounds.size.width),
                                                        );
                                                    })
                                                    .on_hover(cx.listener(
                                                        move |this, on: &bool, _, cx| {
                                                            this.mapping_190_state.panel_tooltip =
                                                                if *on && overflow.get() {
                                                                    this.mapping_190_state
                                                                        .panel_tooltip_anchor =
                                                                        row.clone();
                                                                    Some(hover_label.clone())
                                                                } else {
                                                                    None
                                                                };
                                                            cx.notify();
                                                        },
                                                    ))
                                                    .child(label),
                                            ),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.mapping_190_request(Next::PanelOpen(input.clone()), window, cx)
                            })),
                    ),
            );
        }
        v_flex()
            .absolute()
            .left(surface::css(width - 230.))
            .top_0()
            .w(surface::css(230.))
            .h_full()
            .bg(rgb(0x2d2d2d))
            .child(
                h_flex()
                    .relative()
                    .h(surface::css(67.))
                    .w_full()
                    .items_center()
                    .justify_center()
                    .border_b_1()
                    .border_color(rgb(0x5d5d5d))
                    .child(
                        div()
                            .relative()
                            .w(surface::css(180.))
                            .mr(surface::css(26.))
                            .child(
                                self.mapping_190_source_dropdown(
                                    "panel",
                                    t(if state.panel_filter == 0 {
                                        "ALL_BUTTONS"
                                    } else {
                                        "CUSTOMIZED"
                                    })
                                    .to_string(),
                                    state.panel_filter as usize,
                                    [(0, "ALL_BUTTONS"), (1, "CUSTOMIZED")]
                                        .into_iter()
                                        .map(|(filter, label)| {
                                            BaseButton::new(SharedString::from(format!(
                                                "mouse-190-panel-filter-{filter}"
                                            )))
                                            .w_full()
                                            .h(surface::css(25.))
                                            .min_h(surface::css(25.))
                                            .flex_shrink_0()
                                            .px(surface::css(4.))
                                            .py_0()
                                            .bg(rgb(0x000000))
                                            .text_color(rgb(if state.panel_filter == filter {
                                                0x44d62c
                                            } else {
                                                0xcccccc
                                            }))
                                            .hover(|style| style.bg(rgba(0xffffff1a)))
                                            .child(
                                                div()
                                                    .w_full()
                                                    .text_ellipsis()
                                                    .text_left()
                                                    .child(t(label)),
                                            )
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.mapping_190_state.panel_filter = filter;
                                                this.mapping_190_state.panel_filter_open = false;
                                                cx.notify();
                                            }))
                                            .into_any_element()
                                        })
                                        .collect(),
                                    window,
                                    cx,
                                ),
                            ),
                    )
                    .child(
                        BaseButton::new("mouse-190-panel-close")
                            .absolute()
                            .right(surface::css(8.))
                            .size(surface::css(26.))
                            .p_0()
                            .border_1()
                            .border_color(rgba(0))
                            .hover(|style| style.border_color(rgb(0x5d5d5d)))
                            .child(img("synapse/drawer-close.svg").size(surface::css(20.)))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.mapping_190_request(Next::Panel, window, cx)
                            })),
                    ),
            )
            .child(rows)
            .into_any_element()
    }

    fn customize_190_image(&self, cx: &Context<Self>) -> AnyElement {
        let buttons = self.spec.groups[0]["buttonList"]
            .as_array()
            .expect("190 source group");
        let hovered = self.customize_hover;
        let selected = buttons
            .iter()
            .position(|button| button["inputID"].as_str() == self.mapping_input.as_deref());
        let hypershift = self.hypershift;
        let enabled: [bool; 8] =
            std::array::from_fn(|index| self.mapping_190_enabled(&buttons[index]));
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
                this.mapping_190_request(Next::Hyper, window, cx);
            }));
        v_flex()
            .relative()
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
                    .child(
                        BaseButton::new("mouse-190-panel-toggle")
                            .w(surface::css(38.))
                            .h(surface::css(27.))
                            .p_0()
                            .border_1()
                            .border_color(if self.mapping_190_state.panel_open {
                                rgb(0x44d62c)
                            } else {
                                rgb(0x5d5d5d)
                            })
                            .rounded(surface::css(14.))
                            .child(
                                img(if self.mapping_190_state.panel_open {
                                    "synapse/drawer-active.svg"
                                } else {
                                    "synapse/drawer.svg"
                                })
                                .size(surface::css(18.)),
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.mapping_190_request(Next::Panel, window, cx)
                            })),
                    )
                    .child(hyper)
                    .child(surface::help_control(
                        "mouse-190-hypershift-help",
                        t("HYPERSHIFT_TOOLTIP"),
                    )),
            )
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
                let enabled = self.mapping_190_enabled(button);
                let anchor_input = input.clone();
                let anchors = self.mapping_190_state.anchors.clone();
                div()
                    .on_prepaint(move |bounds, _, _| {
                        anchors.borrow_mut().insert(anchor_input.clone(), bounds);
                    })
                    .child(
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
                                rgb(0xcccccc)
                            } else {
                                rgba(0xcccccc4d)
                            })
                            .bg(if active { rgb(0x111111) } else { rgba(0) })
                            .disabled(!enabled)
                            .hover(|style| style.bg(rgb(0x383838)))
                            .active(|style| style.bg(rgb(0x111111)))
                            .child(self.mapping_190_label(button))
                            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                this.customize_hover = hovered.then_some(index);
                                cx.notify();
                            }))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.mapping_190_request(Next::Open(input.clone()), window, cx);
                            })),
                    )
            }))
            .into_any_element()
    }
}
