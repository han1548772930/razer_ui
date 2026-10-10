//! 167/231 current RS/uS haptic graph and custom popup, statically re-audited.
use super::*;
use gpui_kit::base::{Button as BaseButton, Dialog};
use gpui_kit::prelude::FluentBuilder as _;
use razer_widgets::source_slider::SourceSlider;
use razer_widgets::theme::{DrawerColors, ScrollWheelColors as Colors};
use std::{cell::Cell, rc::Rc};

const ROOT: &str = "/scrollWheelStages/scrollWheelStages/5";
const KEYS: [&str; 5] = ["pLeft", "pMidLeft", "pMid", "pMidRight", "pRight"];
type Points = [[f32; 2]; 5];

fn presets(pid: u32, stage: &str) -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    &DATA.get_or_init(|| {
        serde_json::from_str(include_str!("mouse_haptic_data.json"))
            .expect("statically parsed current 167/231 haptic settings")
    })[pid.to_string()][stage]
}

impl MouseProductWorkspace {
    fn curve_points(&self, direction: &str) -> Points {
        std::array::from_fn(|ix| {
            let p = &self
                .draft
                .pointer(&format!("{ROOT}/{direction}/coordinates"))
                .unwrap_or(&Value::Null)[KEYS[ix]];
            [
                p["x"].as_f64().unwrap_or(0.) as f32 + 7.,
                p["y"].as_f64().unwrap_or(0.) as f32 + 7.,
            ]
        })
    }
    fn store_curve(&mut self, direction: &str, points: Points) {
        for (ix, key) in KEYS.iter().enumerate() {
            let path = format!("{ROOT}/{direction}/coordinates/{key}");
            let mut p = self
                .draft
                .pointer(&path)
                .cloned()
                .unwrap_or_else(|| json!({}));
            p["id"] = json!(key);
            p["x"] = json!(points[ix][0] - 7.);
            p["y"] = json!(points[ix][1] - 7.);
            set_pointer(&mut self.draft, &path, p);
        }
    }
    fn mirror_scroll_curve(&mut self) {
        // RS: add the 7px canvas inset, reverse the five points, abs(534-x),
        // then remove the inset on Save. Flags follow the mirrored source point.
        let up = self
            .draft
            .pointer(&format!("{ROOT}/upDirection/coordinates"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        let mut down = json!({});
        for (ix, key) in KEYS.iter().enumerate() {
            let mut p = up[KEYS[4 - ix]].clone();
            p["id"] = json!(key);
            p["x"] = json!((527. - p["x"].as_f64().unwrap_or(0.)).abs() - 7.);
            down[*key] = p;
        }
        set_pointer(
            &mut self.draft,
            &format!("{ROOT}/downDirection/coordinates"),
            down,
        );
    }
    fn begin_scroll_custom(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.scroll_custom_saved = self.draft.pointer(ROOT).cloned();
        self.scroll_custom_direction = "upDirection";
        self.scroll_curve_drag = None;
        self.scroll_custom_dirty = false;
        if self.boolean(&format!("{ROOT}/isLinkedCurves")) {
            self.mirror_scroll_curve();
        }
        self.sync_scroll_custom(window, cx);
        self.scroll_popup_focus.focus(window, cx);
        cx.notify();
    }
    fn move_scroll_curve(
        &mut self,
        position: Point<Pixels>,
        area: Bounds<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ix) = self.scroll_curve_drag else {
            return;
        };
        if self.scroll_custom_saved.is_none() {
            return;
        }
        let unit = f32::from(window.rem_size()) / 16.;
        let x = f32::from(position.x - area.origin.x) / unit;
        let y = f32::from(position.y - area.origin.y) / unit;
        let direction = self.scroll_custom_direction;
        let mut p = self.curve_points(direction);
        // Exact RS limits are asymmetric and include its 7px inset arithmetic.
        if ix == 1 {
            p[ix][0] = if x <= 11. {
                7.
            } else if x > 527. {
                521.8
            } else {
                x - 7.
            };
            if p[1][0] >= p[2][0] {
                p[2][0] = p[1][0];
            }
            if p[1][0] >= p[3][0] {
                p[3][0] = p[1][0];
            }
        } else if ix == 2 || ix == 3 {
            p[ix][0] = if x <= 16.2 {
                12.2
            } else if x >= 527. {
                521.8
            } else {
                x - 7.
            };
            if ix == 3 {
                if p[3][0] <= p[2][0] {
                    p[2][0] = p[3][0];
                }
                if p[3][0] <= p[1][0] {
                    p[1][0] = p[3][0];
                }
            } else {
                if p[2][0] <= p[1][0] {
                    p[1][0] = p[2][0];
                }
                if p[2][0] >= p[3][0] || x > 527. {
                    p[3][0] = p[2][0];
                }
            }
        }
        p[ix][1] = match ix {
            0 | 4 => {
                if y <= 27.5 {
                    23.5
                } else if y > 344. {
                    337.
                } else {
                    y - 7.
                }
            }
            1 | 3 => {
                if y <= 14.3 {
                    10.3
                } else if y > 330.8 {
                    323.8
                } else {
                    y - 7.
                }
            }
            _ => {
                if y <= 11. {
                    7.
                } else if y > 327.5 {
                    320.5
                } else {
                    y - 7.
                }
            }
        };
        match ix {
            0 => {
                if p[0][1] <= p[1][1] {
                    p[1][1] = p[0][1];
                }
                if p[0][1] <= p[2][1] {
                    p[2][1] = p[0][1];
                }
            }
            1 => {
                if p[1][1] <= p[2][1] {
                    p[2][1] = p[1][1];
                }
                if p[1][1] >= p[0][1] {
                    p[0][1] = p[1][1];
                }
            }
            2 => {
                for j in [0, 1, 3, 4] {
                    if p[2][1] >= p[j][1] {
                        p[j][1] = p[2][1];
                    }
                }
            }
            3 => {
                if p[3][1] <= p[2][1] {
                    p[2][1] = p[3][1];
                }
                if p[3][1] >= p[4][1] {
                    p[4][1] = p[3][1];
                }
            }
            _ => {
                if p[4][1] <= p[2][1] {
                    p[2][1] = p[4][1];
                }
                if p[4][1] <= p[3][1] {
                    p[3][1] = p[4][1];
                }
            }
        }
        self.store_curve(direction, p);
        self.scroll_custom_dirty = true;
        cx.notify();
    }
    fn release_scroll_curve(&mut self, cx: &mut Context<Self>) {
        if self.scroll_curve_drag.take().is_some() {
            if self.boolean(&format!("{ROOT}/isLinkedCurves")) {
                self.mirror_scroll_curve();
            }
            cx.notify();
        }
    }
    fn haptic_graph(
        &self,
        stage: &str,
        editable: bool,
        area: Rc<Cell<Bounds<Pixels>>>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let custom = stage == "SW_CUSTOM";
        let linked = self.boolean(&format!("{ROOT}/isLinkedCurves"));
        let direction = if editable {
            self.scroll_custom_direction
        } else {
            "upDirection"
        };
        let points = self.curve_points(direction);
        let preset = presets(self.spec.product_id, stage)["coordinates"].clone();
        let color = if editable && !linked && direction == "downDirection" {
            DrawerColors::new().hypershift()
        } else {
            Colors::accent()
        };
        let painted_area = area.clone();
        let pressed_area = area.clone();
        div()
            .id("mouse-haptic-curve")
            .relative()
            .w(surface::css(534.))
            .h(surface::css(344.))
            .when(editable, |v| {
                v.cursor_pointer().on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                        let unit = f32::from(window.rem_size()) / 16.;
                        let origin = pressed_area.get().origin;
                        let x = f32::from(e.position.x - origin.x) / unit;
                        let y = f32::from(e.position.y - origin.y) / unit;
                        this.scroll_curve_drag = this
                            .curve_points(this.scroll_custom_direction)
                            .iter()
                            .position(|p| (x - p[0]).abs() < 7. && (y - p[1]).abs() < 7.);
                        cx.notify();
                    }),
                )
            })
            .child(
                canvas(
                    move |bounds, _, _| {
                        painted_area.set(bounds);
                    },
                    move |bounds, _, window, _| {
                        let unit = f32::from(window.rem_size()) / 16.;
                        let at =
                            |p: [f32; 2]| bounds.origin + point(px(p[0] * unit), px(p[1] * unit));
                        window.paint_quad(fill(
                            Bounds::new(at([7., 7.]), size(px(520. * unit), px(330. * unit))),
                            Colors::tooltip(),
                        ));
                        let mut grid = PathBuilder::stroke(px(unit));
                        for y in [7., 89.5, 172., 254.5, 337.] {
                            grid.move_to(at([7., y]));
                            grid.line_to(at([527., y]));
                        }
                        for x in [7., 137., 267., 397., 527.] {
                            grid.move_to(at([x, 7.]));
                            grid.line_to(at([x, 337.]));
                        }
                        if let Ok(path) = grid.build() {
                            window.paint_path(path, Colors::border());
                        }
                        let mut curve = PathBuilder::stroke(px(2. * unit));
                        if custom {
                            curve.move_to(at(points[0]));
                            curve.curve_to(at(points[2]), at(points[1]));
                            curve.move_to(at(points[2]));
                            curve.curve_to(at(points[4]), at(points[3]));
                        } else if let Some(points) = preset.as_array() {
                            let point = |p: &Value| {
                                at([
                                    p["x"].as_f64().unwrap_or(0.) as f32,
                                    p["y"].as_f64().unwrap_or(0.) as f32,
                                ])
                            };
                            for segment in points.windows(3).step_by(2) {
                                curve.move_to(point(&segment[0]));
                                curve.curve_to(point(&segment[2]), point(&segment[1]));
                            }
                        }
                        if let Ok(path) = curve.build() {
                            window.paint_path(path, color);
                        }
                        if editable {
                            for (ix, p) in points.iter().enumerate() {
                                let box_ = Bounds::new(
                                    at([p[0] - 7., p[1] - 7.]),
                                    size(px(14. * unit), px(14. * unit)),
                                );
                                window.paint_quad(if ix == 1 || ix == 3 {
                                    outline(box_, color, BorderStyle::Solid)
                                        .border_widths(px(2. * unit))
                                        .corner_radii(px(7. * unit))
                                } else {
                                    fill(box_, color).corner_radii(px(7. * unit))
                                });
                            }
                        }
                    },
                )
                .size_full(),
            )
            .into_any_element()
    }
    pub(super) fn haptic_panel(&self, stage: &str, cx: &Context<Self>) -> AnyElement {
        let settings = if stage == "SW_CUSTOM" {
            self.draft
                .pointer(&format!("{ROOT}/upDirection"))
                .unwrap_or(&Value::Null)
        } else {
            presets(self.spec.product_id, stage)
        };
        let area = Rc::new(Cell::new(Bounds::default()));
        surface::panel_with_control(
            t("ACTIVE_SCROLL_WHEEL_HAPTICS"),
            surface::help_control(
                "mouse-haptic-help",
                t("ACTIVE_SCROLL_WHEEL_HAPTICS_TOOLTIP"),
            ),
            cx,
        )
        .child(
            div()
                .my(surface::css(10.))
                .text_center()
                .text_size(surface::css(16.))
                .child(t(stage)),
        )
        .child(
            div()
                .relative()
                .w(surface::css(520.))
                .ml(surface::css(-7.))
                .child(self.haptic_graph(stage, false, area, cx))
                .when(stage == "SW_CUSTOM", |view| {
                    view.child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Button::new("mouse-scroll-customize")
                                    .label(t("CUSTOMIZE"))
                                    .primary()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.begin_scroll_custom(window, cx)
                                    })),
                            ),
                    )
                }),
        )
        .child(
            div()
                .text_center()
                .text_size(surface::css(12.))
                .child(t("DEGREE_OF_ROTATION")),
        )
        .child(
            h_flex().justify_between().mt(surface::css(20.)).children(
                [
                    ("scrollTension", "SCROLL_TENSION"),
                    ("scrollSteps", "SCROLL_STEPS"),
                ]
                .map(|(field, key)| {
                    v_flex()
                        .w(surface::css(255.))
                        .py(surface::css(20.))
                        .rounded(surface::css(4.))
                        .bg(cx.theme().secondary)
                        .text_center()
                        .child(
                            div()
                                .text_size(surface::css(28.))
                                .mb(surface::css(10.))
                                .child(settings[field].as_i64().unwrap_or(0).to_string()),
                        )
                        .child(t(key))
                }),
            ),
        )
        .into_any_element()
    }
    pub(super) fn scroll_custom_popup(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let linked = self.boolean(&format!("{ROOT}/isLinkedCurves"));
        let direction = self.scroll_custom_direction;
        let area = Rc::new(Cell::new(Bounds::default()));
        let moving_area = area.clone();
        let tabs = h_flex()
            .items_center()
            .mb(surface::css(-7.))
            .child(h_flex().mr(surface::css(10.)).children(if linked {
                vec![
                    div()
                        .px(surface::css(16.))
                        .py(surface::css(8.))
                        .border_1()
                        .border_b_0()
                        .border_color(Colors::border())
                        .bg(Colors::tooltip())
                        .text_color(Colors::accent())
                        .child(t("SCROLL_UP_AND_DOWN"))
                        .into_any_element(),
                ]
            } else {
                [
                    ("upDirection", "SCROLL_UP"),
                    ("downDirection", "SCROLL_DOWN"),
                ]
                .into_iter()
                .map(|(value, key)| {
                    BaseButton::new(SharedString::from(format!("mouse-haptic-tab-{value}")))
                        .px(surface::css(16.))
                        .py(surface::css(8.))
                        .border_1()
                        .border_b_0()
                        .border_color(Colors::border())
                        .when(value == direction, |b| {
                            b.bg(Colors::tooltip()).text_color(Colors::accent())
                        })
                        .child(t(key))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.scroll_custom_direction = value;
                            this.scroll_curve_drag = None;
                            cx.notify();
                        }))
                        .into_any_element()
                })
                .collect()
            }))
            .child(
                BaseButton::new("mouse-haptic-link")
                    .child(
                        img(if linked {
                            "synapse/mouse-scroll-link.svg"
                        } else {
                            "synapse/mouse-scroll-unlink.svg"
                        })
                        .size(surface::css(20.)),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        let linked = !this.boolean(&format!("{ROOT}/isLinkedCurves"));
                        set_pointer(
                            &mut this.draft,
                            &format!("{ROOT}/isLinkedCurves"),
                            json!(linked),
                        );
                        if linked {
                            this.scroll_custom_direction = "upDirection";
                            this.mirror_scroll_curve();
                            // uS deliberately copies the saved up values when linking.
                            if let Some(saved) = &this.scroll_custom_saved {
                                for field in ["scrollTension", "scrollSteps"] {
                                    let value = saved["upDirection"][field].clone();
                                    set_pointer(
                                        &mut this.draft,
                                        &format!("{ROOT}/downDirection/{field}"),
                                        value,
                                    );
                                }
                            }
                        }
                        this.scroll_curve_drag = None;
                        this.sync_scroll_custom(window, cx);
                        cx.notify();
                    })),
            )
            .child(div().ml(surface::css(10.)).child(surface::help_control(
                "mouse-haptic-link-help",
                t("SCROLLING_CUSTOM_TAB_TOOLTIP"),
            )));
        let chart = v_flex()
            .w(surface::css(520.))
            .flex_shrink_0()
            .child(tabs)
            .child(div().ml(surface::css(-7.)).child(self.haptic_graph(
                "SW_CUSTOM",
                true,
                area,
                cx,
            )))
            .child(
                div()
                    .text_center()
                    .text_size(surface::css(12.))
                    .child(t("DEGREE_OF_ROTATION")),
            );
        let controls = v_flex()
            .w(surface::css(230.))
            .ml(surface::css(20.))
            .mt(surface::css(34.))
            .justify_between()
            .child(
                v_flex().children(
                    [
                        ("scrollTension", "SCROLL_TENSION", "0", "100"),
                        ("scrollSteps", "SCROLL_STEPS", "8", "96"),
                    ]
                    .map(|(field, key, min, max)| {
                        let path = format!("{ROOT}/{direction}/{field}");
                        let state = &self.sliders[&path];
                        v_flex()
                            .mb(surface::css(20.))
                            .child(
                                h_flex()
                                    .items_center()
                                    .mb(surface::css(5.))
                                    .child(t(key))
                                    .child(div().ml(surface::css(10.)).child(
                                        surface::help_control(
                                            SharedString::from(format!(
                                                "mouse-haptic-{field}-help"
                                            )),
                                            t(&format!("{key}_TOOLTIP")),
                                        ),
                                    )),
                            )
                            .child(
                                Input::new(&self.inputs[&path])
                                    .w_full()
                                    .mb(surface::css(15.)),
                            )
                            .child(
                                div()
                                    .relative()
                                    .h(surface::css(36.))
                                    .child(
                                        SourceSlider::new(
                                            state,
                                            (self.number(&path) - state.read(cx).min_value())
                                                / (state.read(cx).max_value()
                                                    - state.read(cx).min_value()),
                                        )
                                        .enabled(true),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .bottom_0()
                                            .w_full()
                                            .child(surface::slider_tags(min, None, max, None)),
                                    ),
                            )
                    }),
                ),
            )
            .child(
                h_flex()
                    .justify_end()
                    .gap(surface::css(10.))
                    .mb(surface::css(20.))
                    .child(
                        Button::new("mouse-scroll-cancel")
                            .label(t("CANCEL"))
                            .outline()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.cancel_scroll_custom(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("mouse-scroll-save")
                            .label(t("SAVE"))
                            .primary()
                            .disabled(!self.scroll_custom_dirty)
                            .on_click(cx.listener(|this, _, _, cx| {
                                if !this.scroll_custom_dirty {
                                    return;
                                }
                                this.release_scroll_curve(cx);
                                this.scroll_custom_saved = this.draft.pointer(ROOT).cloned();
                                this.scroll_custom_dirty = false;
                                cx.emit(MouseProductChanged);
                                cx.notify();
                            })),
                    ),
            );
        let unit = window.rem_size() / 16.;
        let viewport = window.viewport_size();
        let width = unit * 850.;
        let top = unit * 105.;
        let panel = v_flex()
            .absolute()
            .occlude()
            .left((viewport.width - width) / 2.)
            .top(top)
            .w(width)
            .h((viewport.height - top).max(px(0.)))
            .rounded_t(surface::css(5.))
            .bg(cx.theme().popover)
            .text_color(Colors::text())
            .child(
                div()
                    .relative()
                    .h(surface::css(36.))
                    .flex_shrink_0()
                    .text_center()
                    .border_b_1()
                    .border_color(Colors::border())
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .py(surface::css(9.))
                    .child(t("CUSTOMIZE_SCROLL_WHEEL_HAPTICS"))
                    .child(
                        BaseButton::new("mouse-haptic-close")
                            .absolute()
                            .right_0()
                            .top_0()
                            .size(surface::css(36.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(img("synapse/mouse-scroll-close.svg").size(surface::css(20.)))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.cancel_scroll_custom(window, cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                v_flex()
                    .id("mouse-haptic-popup-content")
                    .px(surface::css(30.))
                    .py(surface::css(20.))
                    .overflow_y_scroll()
                    .child(
                        div()
                            .text_center()
                            .mb(surface::css(30.))
                            .child(t("CUSTOMIZE_HAPTICS_DESC")),
                    )
                    .child(h_flex().items_start().child(chart).child(controls)),
            );
        Dialog::new(cx)
            .focus_handle(self.scroll_popup_focus.clone())
            .close_on_backdrop_press(false)
            .on_ok(|_, _, _| false)
            .on_close(cx.listener(|this, _, window, cx| {
                this.cancel_scroll_custom(window, cx);
                cx.notify();
            }))
            .backdrop(
                div()
                    .absolute()
                    .inset_0()
                    .bg(razer_widgets::theme::StreamMixerColors::backdrop())
                    .child(
                        img("synapse/mouse-scroll-glow.svg")
                            .absolute()
                            .bottom_0()
                            .w_full(),
                    ),
            )
            .popup(
                div()
                    .absolute()
                    .inset_0()
                    .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, window, cx| {
                        this.move_scroll_curve(e.position, moving_area.get(), window, cx)
                    }))
                    .capture_any_mouse_up(
                        cx.listener(|this, _, _, cx| this.release_scroll_curve(cx)),
                    )
                    .child(panel),
            )
            .into_any_element()
    }
}
