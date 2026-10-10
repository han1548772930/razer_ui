//! Current PID 70 customization surface (`lT`/`rT`, main.8f24b6a1.js).
//! The source mounts an 11-button top-view block, rather than the generic
//! product card. Mapping edits remain local profile drafts.
use super::*;
use gpui_kit::base::Button as BaseButton;

impl MouseProductWorkspace {
    pub(super) fn customize_70(&self, cx: &Context<Self>) -> AnyElement {
        let buttons = self.spec.groups[0]["buttonList"]
            .as_array()
            .expect("70 source group");
        let selected = buttons
            .iter()
            .position(|button| button["inputID"].as_str() == self.mapping_input.as_deref());
        let hovered = self.customize_hover;
        let hypershift = self.hypershift;
        let enabled: Vec<bool> = buttons
            .iter()
            .map(|button| self.customize_70_enabled(button))
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
            .child(self.customize_70_labels(true, [0, 1, 2, 3, 4], buttons, cx))
            .child(self.customize_70_labels(false, [5, 6, 7, 8, 9, 10], buttons, cx));
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
                    .child(hyper)
                    .child(surface::help_control(
                        "mouse-70-hypershift-help",
                        t("HYPERSHIFT_TOOLTIP"),
                    )),
            )
            .children(self.mapping_input.as_ref().map(|_| self.mapping_70(cx)))
            .into_any_element()
    }

    fn customize_70_labels(
        &self,
        left: bool,
        indices: impl IntoIterator<Item = usize>,
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
                let input = button["inputID"].as_str().unwrap_or_default().to_owned();
                let enabled = self.customize_70_enabled(button);
                let active = self.mapping_input.as_deref() == Some(input.as_str());
                BaseButton::new(SharedString::from(format!("mouse-70-label-{index}")))
                    .h(surface::css(30.))
                    .mb(surface::css(20.))
                    .px(surface::css(10.))
                    .py_0()
                    .when(left, |b| b.mr(surface::css(10.)))
                    .when(!left, |b| b.ml(surface::css(10.)))
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
                    .hover(|s| s.bg(rgb(0x383838)))
                    .active(|s| s.bg(rgb(0x111111)))
                    .child(self.customize_70_label(button))
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

    fn mapping_70(&self, cx: &Context<Self>) -> AnyElement {
        let Some(input) = self.mapping_input.as_deref() else {
            return div().into_any_element();
        };
        let input_id = input.to_owned();
        let input_type = self.spec.groups[0]["buttonList"]
            .as_array()
            .and_then(|buttons| buttons.iter().find(|button| button["inputID"] == input))
            .and_then(|button| button["inputType"].as_str())
            .unwrap_or("MouseInput")
            .to_owned();
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
        div().id("mouse-70-mapping-popup").absolute().left(surface::css(50.)).top(surface::css(380.)).w(surface::css(292.)).p(surface::css(20.)).bg(rgb(0x111111)).border_1().border_color(rgb(0x5d5d5d)).rounded(surface::css(5.))
            .child(div().text_center().mb(surface::css(15.)).child(t(input)))
            .child(v_flex().gap(surface::css(5.)).children(choices.into_iter().map(|(id,label)| Button::new(SharedString::from(format!("mouse-70-assignment-{id}"))).label(t(label)).outline().selected(assignment == Some(id)).on_click(cx.listener(move |this,_,_,cx| { this.mapping_assignment = Some(id.to_owned()); cx.notify(); })))))
            .child(h_flex().gap(surface::css(8.)).mt(surface::css(15.)).justify_end()
                .child(Button::new("mouse-70-mapping-cancel").label(t("CANCEL")).outline().on_click(cx.listener(|this,_,_,cx| { this.mapping_input=None; this.mapping_assignment=None; cx.notify(); })))
                .child(Button::new("mouse-70-mapping-save").label(t("SAVE")).primary().disabled(assignment.is_none()).on_click(cx.listener(move |this,_,_,cx| { let Some(assignment)=this.mapping_assignment.take() else{return;}; if let Some(mappings)=this.draft["mappings"].as_array_mut() { mappings.retain(|entry| entry["inputID"] != input_id || entry["isHyperShift"].as_bool().unwrap_or(false) != this.hypershift); mappings.push(json!({"inputID":input_id,"isHyperShift":this.hypershift,"inputType":input_type,"outputType":"mouseGroup","mouseGroup":{"mouseAssignment":assignment}})); cx.emit(MouseProductChanged); } this.mapping_input=None; cx.notify(); })))
            ).into_any_element()
    }

    fn customize_70_enabled(&self, button: &Value) -> bool {
        // Current pT.updateMappingsToButtons enables LeftClick for HyperShift;
        // the standard layer preserves the default primary-click protection.
        if button["buttonKey"] == "LeftClick" && self.hypershift {
            true
        } else {
            button["isEnabled"].as_bool().unwrap_or(true)
        }
    }

    fn customize_70_label(&self, button: &Value) -> String {
        let assignment = self.draft["mappings"]
            .as_array()
            .and_then(|mappings| {
                mappings.iter().rev().find(|entry| {
                    entry["inputID"] == button["inputID"]
                        && entry["isHyperShift"].as_bool().unwrap_or(false) == self.hypershift
                })
            })
            .and_then(|mapping| mapping.pointer("/mouseGroup/mouseAssignment"))
            .and_then(Value::as_str);
        let key = match assignment {
            Some("Click") => "LEFT_CLICK",
            Some("Menu") => "RIGHT_CLICK",
            Some("ScrollButton") => "SCROLL_CLICK",
            Some("Previous") => "MOUSE_BUTTON_4",
            Some("Next") => "MOUSE_BUTTON_5",
            Some("ScrollUp") => "SCROLL_UP",
            Some("ScrollDown") => "SCROLL_DOWN",
            _ => button["assignmentValue"].as_str().unwrap_or("DEFAULT"),
        };
        t(key)
    }
}
