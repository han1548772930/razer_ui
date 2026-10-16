//! 3907's independent `TU/rU -> sU/nU -> um/cm` Armory root.
//! Current-source receipts: armory-remaining-roots-current-evidence.json.
//! No telemetry means `am` settles to displayWarning=true and systemMode=none.
//! This is the source's editable local Smart curve, not measured fan output.
use super::*;
use std::{cell::Cell, collections::BTreeMap, rc::Rc, sync::OnceLock};

#[derive(serde::Deserialize)]
struct AxisText {
    asset: String,
    advance: f32,
}
#[derive(serde::Deserialize)]
struct AxisTexts {
    labels: BTreeMap<String, AxisText>,
    units: BTreeMap<String, AxisText>,
}
fn axis_texts() -> &'static AxisTexts {
    static TEXTS: OnceLock<AxisTexts> = OnceLock::new();
    TEXTS.get_or_init(|| {
        serde_json::from_str(include_str!("cooling_axis_data.json"))
            .expect("validated source axis outlines")
    })
}

#[derive(Clone, Copy)]
struct Node {
    temperature: f32,
    rpm: f32,
}

struct CoolingState {
    enabled: bool,
    celsius: bool,
    percentage: bool,
    sensor: usize,
    mode: usize,
    curves: [[Vec<Node>; 3]; 2],
    selected: Option<usize>,
    hovered: Option<usize>,
    dragging: bool,
    button_hover: Option<usize>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
}

fn preset(sensor: usize, mode: usize) -> Vec<Node> {
    // Rn's CPU curves; An/Sn/ln are precisely ten degrees lower for GPU.
    let points: &[(f32, f32)] = match mode {
        0 => &[
            (40., 500.),
            (50., 500.),
            (85., 1000.),
            (90., 1600.),
            (100., 1600.),
        ],
        2 => &[(40., 1000.), (55., 2000.), (80., 2700.), (100., 2700.)],
        _ => &[(40., 1000.), (90., 2200.), (100., 2200.)],
    };
    points
        .iter()
        .map(|&(temperature, rpm)| Node {
            temperature: temperature - if sensor == 1 { 10. } else { 0. },
            rpm,
        })
        .collect()
}

fn percentage_label(rpm: f32) -> String {
    // Yp keeps its exact 15.625% lower boundary; intermediate values use
    // JavaScript's one-decimal rounding and suppress a trailing .0.
    if rpm == 500. {
        return "15.625%".into();
    }
    let value = (rpm / 32. * 10.).round() / 10.;
    if value.fract() == 0. {
        format!("{value:.0}%")
    } else {
        format!("{value:.1}%")
    }
}

impl CoolingState {
    fn new() -> Self {
        Self {
            enabled: true,
            celsius: true,
            percentage: true,
            sensor: 0,
            mode: 1,
            curves: std::array::from_fn(|sensor| std::array::from_fn(|mode| preset(sensor, mode))),
            selected: None,
            hovered: None,
            dragging: false,
            button_hover: None,
            bounds: Rc::new(Cell::new(Bounds::default())),
        }
    }
    fn nodes(&self) -> &Vec<Node> {
        &self.curves[self.sensor][self.mode]
    }
    fn low_temperature(&self) -> f32 {
        if self.sensor == 0 { 40. } else { 30. }
    }
    fn clear_selection(&mut self) {
        self.selected = None;
        self.hovered = None;
        self.dragging = false;
    }
    fn position(&self, node: Node) -> Point<Pixels> {
        let bounds = self.bounds.get();
        point(
            bounds.left() + bounds.size.width * ((node.temperature - self.low_temperature()) / 60.),
            bounds.bottom() - bounds.size.height * ((node.rpm - 500.) / 2700.),
        )
    }
    fn hit(&self, position: Point<Pixels>) -> Option<usize> {
        let radius = f32::from(self.bounds.get().size.height) / 380. * 7.;
        self.nodes().iter().position(|node| {
            let point = self.position(*node);
            (f32::from(point.x - position.x)).abs() <= radius
                && (f32::from(point.y - position.y)).abs() <= radius
        })
    }
    fn press(&mut self, position: Point<Pixels>) {
        if !self.enabled {
            return;
        }
        self.selected = self.hit(position);
        self.dragging = self.selected.is_some();
        if self.selected.is_some() || self.nodes().len() >= 20 {
            return;
        }
        let bounds = self.bounds.get();
        if !bounds.contains(&position) {
            return;
        }
        let temperature = (self.low_temperature()
            + 60. * f32::from(position.x - bounds.left()) / f32::from(bounds.size.width))
        .round();
        let segment = self.nodes().windows(2).position(|pair| {
            temperature > pair[0].temperature && temperature < pair[1].temperature
        });
        if let Some(index) = segment {
            let first = self.nodes()[index];
            let second = self.nodes()[index + 1];
            let rpm = first.rpm
                + (second.rpm - first.rpm) * (temperature - first.temperature)
                    / (second.temperature - first.temperature);
            if f32::from(self.position(Node { temperature, rpm }).y - position.y).abs()
                <= f32::from(bounds.size.height) / 380. * 7.
            {
                self.curves[self.sensor][self.mode].insert(index + 1, Node { temperature, rpm });
                self.selected = Some(index + 1);
            }
        }
    }
    fn move_pointer(&mut self, position: Point<Pixels>) {
        if !self.enabled {
            return;
        }
        self.hovered = self.hit(position);
        if !self.dragging {
            return;
        }
        let Some(index) = self.selected else {
            return;
        };
        let bounds = self.bounds.get();
        let rpm = 500.
            + (f32::from(bounds.bottom() - position.y) / f32::from(bounds.size.height))
                .clamp(0., 1.)
                * 2700.;
        let low = index.checked_sub(1).map_or(500., |i| self.nodes()[i].rpm);
        let high = self.nodes().get(index + 1).map_or(3200., |node| node.rpm);
        self.curves[self.sensor][self.mode][index].rpm = rpm.round().clamp(low, high);
    }
}

fn tabs(
    id: &'static str,
    options: Vec<(String, usize)>,
    active: usize,
    enabled: bool,
    state: &Entity<CoolingState>,
    window: &mut Window,
    change: fn(&mut CoolingState, usize),
) -> AnyElement {
    h_flex()
        .h(css(36.))
        .p(css(4.))
        .rounded(css(18.))
        .border_1()
        .border_color(rgb(0x5d5d5d))
        .bg(rgb(0x111111))
        .flex_shrink_0()
        .when(!enabled, |view| view.opacity(0.3))
        .children(options.into_iter().map(|(label, value)| {
            BaseButton::new(SharedString::from(format!("{id}-{value}")))
                .h_full()
                .min_w(css(36.))
                .px(css(10.))
                .py(css(5.))
                .rounded(css(13.))
                .text_size(css(14.))
                .selected(active == value)
                .disabled(!enabled)
                .bg(if active == value {
                    rgb(0x44d62c)
                } else {
                    rgb(0x111111)
                })
                .text_color(if active == value {
                    rgb(0x111111)
                } else {
                    rgb(0xcccccc)
                })
                .child(label)
                .on_click(window.listener_for(state, move |state, _, _, cx| {
                    change(state, value);
                    cx.notify();
                }))
        }))
        .into_any_element()
}

fn stats(gpu: bool, viewport: f32) -> AnyElement {
    let compact = viewport <= 1260.;
    let width = if compact {
        175.
    } else if viewport >= 1290. {
        281.
    } else {
        240.
    };
    let mut body = v_flex()
        .w(css(width))
        .p(css(10.))
        .gap(css(5.))
        .rounded(css(8.))
        .bg(rgb(0x111111))
        .text_size(css(12.))
        .line_height(css(14.));
    body = body.child(
        h_flex()
            .h(css(28.))
            .items_start()
            .justify_between()
            .child(
                h_flex()
                    .gap(css(5.))
                    .when(!compact, |view| {
                        view.child(
                            img(if gpu {
                                "synapse/armory-cooling-gpu.svg"
                            } else {
                                "synapse/armory-cooling-cpu.svg"
                            })
                            .h(css(20.)),
                        )
                    })
                    .child(
                        div()
                            .font_family("RazerF5")
                            .text_size(css(16.))
                            .child(if gpu { "GPU" } else { "CPU" }),
                    ),
            )
            .child(div().child("Not detected")),
    );
    body = body.child(
        h_flex()
            .gap(css(5.))
            .child(div().flex_1())
            .child(div().w(css(50.)).text_right().child(i18n::t("CURRENT")))
            .when(!compact, |view| {
                view.child(div().w(css(40.)).text_right().child(i18n::t("MAX")))
            }),
    );
    for label in [
        "TEMPERATURE",
        "UTILIZATION_TABLE_INFO",
        "CLOCK_TABLE_INFO",
        "POWER_TABLE_INFO",
        "FAN_SPEED",
    ] {
        body = body.child(
            h_flex()
                .gap(css(5.))
                .child(div().flex_1().overflow_hidden().child(i18n::t(label)))
                .child(div().w(css(50.)).text_right().child("—"))
                .when(!compact, |view| {
                    view.child(div().w(css(40.)).text_right().child("—"))
                }),
        );
    }
    body.into_any_element()
}

fn configuration(
    device: &Device,
    state: &Entity<CoolingState>,
    viewport: f32,
    window: &mut Window,
    cx: &App,
) -> AnyElement {
    let hovered = state.read(cx).button_hover;
    // The source Armory tree omits #coolingPerformance/.body-widgets, so its
    // descendant flex overrides do not apply. Keep the generic 770x340 block.
    let mut root = div().relative().w(css(770.)).h(css(340.)).mx_auto().child(
        h_flex()
            .gap(css(10.))
            .child(stats(false, viewport))
            .when(viewport > 1000., |view| view.child(stats(true, viewport))),
    );
    let mut configuration = div().relative().w(css(410.)).h_0().child(
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let scale = f32::from(bounds.size.width) / 410.;
                for (index, y) in [155., 170., 190.].into_iter().enumerate() {
                    let color: Hsla = if index == 2 {
                        rgba(0xcccccc1a).into()
                    } else if hovered == Some(index) {
                        rgb(0x44d62c).into()
                    } else {
                        rgb(0x5d5d5d).into()
                    };
                    let pos = |x: f32, y: f32| {
                        point(bounds.left() + px(x * scale), bounds.top() + px(y * scale))
                    };
                    let mut path = PathBuilder::stroke(px(scale));
                    let right_y = 156. + 49. * index as f32;
                    path.move_to(pos(106., right_y));
                    path.line_to(pos(83., right_y));
                    path.line_to(pos(43.5, y + 2.5));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                }
            },
        )
        .absolute()
        .left(css(230.))
        .w(css(410.))
        .h(css(340.)),
    );
    for (index, y) in [155., 170., 190.].into_iter().enumerate() {
        let color = if index == 2 {
            rgba(0xcccccc1a)
        } else if hovered == Some(index) {
            rgba(0x44d62cff)
        } else {
            rgba(0x5d5d5dff)
        };
        for (x, y) in [(106., 156. + 49. * index as f32), (43.5, y + 2.5)] {
            configuration = configuration.child(
                div()
                    .absolute()
                    .left(css(230. + x - 2.5))
                    .top(css(y - 2.5))
                    .size(css(5.))
                    .rounded_full()
                    .bg(color),
            );
        }
        if hovered == Some(index) && index != 2 {
            configuration = configuration.child(
                div()
                    .absolute()
                    .left(css(230. + 43.5 - 5.5))
                    .top(css(y + 2.5 - 5.5))
                    .size(css(11.))
                    .rounded_full()
                    .border_1()
                    .border_color(color),
            );
        }
    }
    if let Some(path) = resources::device_image(
        device.product_id,
        device.edition_id,
        device.layout_id,
        resources::DeviceImage::Product,
    ) {
        configuration = configuration.child(
            img(path)
                .absolute()
                .left(css(20.))
                .h(css(340.))
                .object_fit(ObjectFit::Contain),
        );
    }
    configuration = configuration.child(
        v_flex()
            .absolute()
            .left(css(320.))
            .mt(css(140.))
            .pl(css(15.))
            .children(
                [
                    "CYCLE_CHROMA_LIGHTING",
                    "TOGGLE_FAN_SPEED_MODE",
                    "POWER_ON_OFF",
                ]
                .into_iter()
                .enumerate()
                .map(|(index, label)| {
                    BaseButton::new(SharedString::from(format!("armory-cooling-button-{index}")))
                        .h(css(30.))
                        .mb(css(20.))
                        .ml(css(10.))
                        .px(css(10.))
                        .rounded(css(3.))
                        .whitespace_nowrap()
                        .disabled(index == 2)
                        .when(index == 2, |view| view.opacity(0.3))
                        .when(index != 2, |view| {
                            view.hover(|style| style.bg(rgb(0x808080)))
                        })
                        .child(i18n::t(label))
                        .on_hover(window.listener_for(state, move |state, inside, _, cx| {
                            state.button_hover = if *inside { Some(index) } else { None };
                            cx.notify();
                        }))
                }),
            ),
    );
    root = root.child(configuration);
    root.into_any_element()
}

fn graph(state: &Entity<CoolingState>, window: &mut Window, cx: &App) -> AnyElement {
    let data = state.read(cx);
    let points = data.nodes().clone();
    let low = data.low_temperature();
    let celsius = data.celsius;
    let percentage = data.percentage;
    let active = data.selected.or(data.hovered);
    let selected = data.selected;
    let bounds_cell = data.bounds.clone();
    let points_for_paint = points.clone();
    let drag_pointer = state.downgrade();
    let mut plot = div()
        .id("armory-cooling-curve")
        .relative()
        .w_full()
        .h(css(380.))
        .bg(rgb(0x000000))
        .on_prepaint(move |bounds, _, _| bounds_cell.set(bounds))
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let scale = f32::from(window.rem_size()) / 16.;
                    let pos = |node: Node| {
                        point(
                            bounds.left() + bounds.size.width * ((node.temperature - low) / 60.),
                            bounds.bottom() - bounds.size.height * ((node.rpm - 500.) / 2700.),
                        )
                    };
                    let mut grid = PathBuilder::stroke(px(scale));
                    for index in 0..=12 {
                        let x = bounds.left() + bounds.size.width * (index as f32 / 12.);
                        grid.move_to(point(x, bounds.top()));
                        grid.line_to(point(x, bounds.bottom()));
                    }
                    for index in 0..=4 {
                        let y = bounds.top() + bounds.size.height * (index as f32 / 4.);
                        grid.move_to(point(bounds.left(), y));
                        grid.line_to(point(bounds.right(), y));
                    }
                    if let Ok(path) = grid.build() {
                        window.paint_path(path, rgb(0x5d5d5d));
                    }
                    let mut curve = PathBuilder::stroke(px(2.5 * scale));
                    if let Some(first) = points_for_paint.first() {
                        curve.move_to(pos(Node {
                            temperature: low,
                            rpm: first.rpm,
                        }));
                    }
                    for node in &points_for_paint {
                        curve.line_to(pos(*node));
                    }
                    if let Some(last) = points_for_paint.last() {
                        curve.line_to(pos(Node {
                            temperature: low + 60.,
                            rpm: last.rpm,
                        }));
                    }
                    if let Ok(path) = curve.build() {
                        window.paint_path(path, rgb(0x44d62c));
                    }
                },
            )
            .size_full(),
        )
        .on_mouse_down(
            MouseButton::Left,
            window.listener_for(state, |state, event: &MouseDownEvent, _, cx| {
                state.press(event.position);
                cx.notify();
            }),
        )
        .on_mouse_move(
            window.listener_for(state, |state, event: &MouseMoveEvent, _, cx| {
                state.move_pointer(event.position);
                cx.notify();
            }),
        )
        .on_mouse_up(
            MouseButton::Left,
            window.listener_for(state, |state, _, _, cx| {
                state.dragging = false;
                cx.notify();
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            window.listener_for(state, |state, _, _, cx| {
                state.dragging = false;
                cx.notify();
            }),
        )
        .child(
            canvas(
                |_, _, _| (),
                move |_, _, window, cx| {
                    let Some(entity) = drag_pointer.upgrade() else {
                        return;
                    };
                    if !entity.read(cx).dragging {
                        return;
                    }
                    let motion = entity.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                        if phase.capture() {
                            motion.update(cx, |state, cx| {
                                state.move_pointer(event.position);
                                cx.notify();
                            });
                        }
                    });
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                        if phase.capture() && event.button == MouseButton::Left {
                            entity.update(cx, |state, cx| {
                                state.dragging = false;
                                cx.notify();
                            });
                        }
                    });
                },
            )
            .absolute()
            .size_0(),
        );
    for (index, node) in points.iter().enumerate() {
        plot = plot.child(
            div()
                .absolute()
                .left(relative((node.temperature - low) / 60.))
                .bottom(relative((node.rpm - 500.) / 2700.))
                .ml(css(-7.))
                .mb(css(-7.))
                .size(css(14.))
                .rounded_full()
                .bg(if active == Some(index) {
                    rgb(0x30961f)
                } else {
                    rgb(0x44d62c)
                }),
        );
    }
    if let Some(index) = active {
        let node = points[index];
        let speed = if percentage {
            percentage_label(node.rpm)
        } else {
            format!("{:.0}\nRPM", node.rpm)
        };
        let temperature = if celsius {
            format!("{}°C", node.temperature)
        } else {
            format!("{:.0}°F", node.temperature * 1.8 + 32.)
        };
        let mut tooltip = v_flex()
            .absolute()
            .left(relative((node.temperature - low) / 60.))
            .bottom(relative((node.rpm - 500.) / 2700.))
            .ml(css(26.))
            .mb(css(15.))
            .min_w(css(58.))
            .min_h(css(40.))
            .px(css(17.))
            .py(css(7.))
            .rounded(css(3.))
            .bg(rgb(0x44d62c))
            .text_color(rgb(0x000000))
            .text_size(css(12.))
            // Source tooltips are siblings of the canvas; their buttons never
            // invoke the canvas mousedown handler or clear the selected node.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(speed)
            .child(temperature);
        if selected == Some(index) && points.len() > 2 {
            tooltip = tooltip.child(
                BaseButton::new("armory-cooling-delete-node")
                    .absolute()
                    .right(css(2.))
                    .top(css(2.))
                    .p_0()
                    .child(
                        img("synapse/armory-cooling-delete.svg")
                            .w(css(10.))
                            .h(css(12.)),
                    )
                    .on_click(window.listener_for(state, move |state, _, _, cx| {
                        state.curves[state.sensor][state.mode].remove(index);
                        state.clear_selection();
                        cx.notify();
                    })),
            );
        }
        plot = plot.child(tooltip);
    }
    let units = vertical_speed_units(state, window, cx);
    let sensors = tabs(
        "cooling-sensor",
        vec![("CPU".into(), 0), ("GPU".into(), 1)],
        data.sensor,
        data.enabled,
        state,
        window,
        |s, value| {
            s.sensor = value;
            s.clear_selection();
        },
    );
    let axes = div()
        .relative()
        .w_full()
        .mt(css(30.))
        .mb(css(60.))
        .pl(css(90.))
        // Kp's chart canvas is the widget width minus 200; chart-content is
        // the widget width minus 100. This includes its 90px canvas inset.
        .pr(css(110.))
        .h(css(424.))
        .child(units)
        .child(plot)
        .children((0..=4).map(|index| {
            let rpm = 500. + 675. * index as f32;
            div()
                .absolute()
                .left_0()
                .top(css(380. - 95. * index as f32 - 7.))
                .w(css(75.))
                .text_right()
                .text_size(css(12.))
                .child(if percentage {
                    format!("{:.0}%", rpm / 32.)
                } else {
                    format!("{rpm:.0} RPM")
                })
        }))
        .child(
            h_flex()
                .justify_between()
                .absolute()
                .left(css(90.))
                .right(css(110.))
                .top(css(417.))
                .text_size(css(12.))
                .children((0..=12).map(|index| {
                    let temperature = low + 5. * index as f32;
                    div().child(if celsius {
                        format!("{temperature:.0}°C")
                    } else {
                        format!("{:.0}°F", temperature * 1.8 + 32.)
                    })
                })),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .right(css(100.))
                .top(css(443.))
                .child(
                    h_flex()
                        .justify_center()
                        .gap(css(10.))
                        .text_size(css(12.))
                        .child(i18n::t("TEMPERATURE"))
                        .child(sensors),
                ),
        );
    axes.into_any_element()
}

fn vertical_speed_units(state: &Entity<CoolingState>, window: &mut Window, cx: &App) -> AnyElement {
    let texts = axis_texts();
    let label = texts
        .labels
        .get(&i18n::locale())
        .unwrap_or(&texts.labels["en"]);
    let percent = &texts.units["percent"];
    let rpm = &texts.units["rpm"];
    let selected = state.read(cx).percentage;
    let enabled = state.read(cx).enabled;
    let units_height = (percent.advance + 20.).max(36.) + (rpm.advance + 20.).max(36.) + 10.;
    let rotated_height = label.advance + 5. + units_height;
    let chars = i18n::t("FAN_SPEED").chars().count();
    let source_offset = if chars > 25 {
        50.
    } else if chars > 20 && chars < 25 {
        30.
    } else {
        0.
    };
    // Source rotates .text-x by 270deg about its centre. GPUI rotates only
    // SVG paint, not hitboxes. The prepared outlines are already rotated;
    // these actual vertical buttons occupy the transformed source rectangles.
    v_flex()
        .absolute()
        .w(css(36.))
        .h(css(rotated_height))
        .left(css(-60. - source_offset + rotated_height / 2. - 18.))
        .top(css((424. - 36. - rotated_height) / 2.))
        .gap(css(5.))
        .items_center()
        .child(
            v_flex()
                .w(css(36.))
                .h(css(units_height))
                .p(css(4.))
                .border_1()
                .border_color(rgb(0x5d5d5d))
                .rounded(css(18.))
                .bg(rgb(0x111111))
                .children([(false, rpm), (true, percent)].map(|(percentage, text)| {
                    BaseButton::new(if percentage {
                        "cooling-percent-axis"
                    } else {
                        "cooling-rpm-axis"
                    })
                    .w_full()
                    .h(css((text.advance + 20.).max(36.)))
                    .p_0()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(css(13.))
                    .selected(selected == percentage)
                    .disabled(!enabled)
                    .bg(if selected == percentage {
                        rgb(0x44d62c)
                    } else {
                        rgb(0x111111)
                    })
                    .text_color(if selected == percentage {
                        rgb(0x111111)
                    } else {
                        rgb(0xcccccc)
                    })
                    .accessibility_label(if percentage { "%" } else { "RPM" })
                    .child(
                        svg()
                            .path(text.asset.clone())
                            .w(css(16.))
                            .h(css(text.advance)),
                    )
                    .on_click(window.listener_for(
                        state,
                        move |state, _, _, cx| {
                            state.percentage = percentage;
                            cx.notify();
                        },
                    ))
                })),
        )
        .child(
            svg()
                .path(label.asset.clone())
                .w(css(14.))
                .h(css(label.advance))
                .text_color(rgb(0xcccccc)),
        )
        .into_any_element()
}

pub(super) fn render(device: &Device, window: &mut Window, cx: &mut App) -> AnyElement {
    let state = window.use_keyed_state(
        (
            ElementId::from(SharedString::from(format!(
                "cooling-pad-armory-{}",
                device.device_container_id
            ))),
            "runtime",
        ),
        cx,
        |_, _| CoolingState::new(),
    );
    let data = state.read(cx);
    let (enabled, celsius, mode) = (data.enabled, data.celsius, data.mode);
    let viewport = f32::from(window.viewport_size().width) * 16. / f32::from(window.rem_size());
    let config = configuration(device, &state, viewport, window, cx);
    let metric = tabs(
        "cooling-metric",
        vec![("°C".into(), 0), ("°F".into(), 1)],
        usize::from(!celsius),
        true,
        &state,
        window,
        |s, value| s.celsius = value == 0,
    );
    // `am` sets warning while initStatus is undefined. The original disables
    // only its first Fixed/Smart tabs, leaving secondary curve editors active.
    let kinds = tabs(
        "cooling-kind",
        vec![
            (i18n::t("FIXED_RPM_HEADER"), 0),
            (i18n::t("SMART_FAN_CURVE_HEADER"), 1),
        ],
        1,
        false,
        &state,
        window,
        |_, _| {},
    );
    let modes = tabs(
        "cooling-mode",
        vec![
            (i18n::t("QUIET_FAN_MODE"), 0),
            (i18n::t("PERFORMANCE_BALANCED"), 1),
            (i18n::t("PERFORMANCE_MAXIMUM_POWER"), 2),
        ],
        mode,
        enabled,
        &state,
        window,
        |s, value| {
            s.mode = value;
            s.clear_selection();
        },
    );
    let chart = graph(&state, window, cx);
    let panel = v_flex()
        .relative()
        .w_full()
        .mt(css(10.))
        .max_w(css(1220.))
        .child(
            div()
                .absolute()
                .right(css(10.))
                .top(css(10.))
                .child(surface::help_control(
                    "cooling-fan-help",
                    i18n::t("FAN_CONTROL_TIP"),
                )),
        )
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .child(
                    h_flex()
                        .gap(css(10.))
                        .text_size(css(16.))
                        .font_family("RazerF5")
                        .text_color(rgb(0x44d62c))
                        .child(i18n::t("FAN_CONTROL_TITLE").to_uppercase())
                        .child(
                            surface::SynapseSwitch::new("cooling-enable")
                                .checked(enabled)
                                .on_change(window.listener_for(&state, |state, value, _, cx| {
                                    state.enabled = *value;
                                    state.clear_selection();
                                    cx.notify();
                                })),
                        ),
                )
                .child(metric),
        )
        .child(
            v_flex()
                .gap(css(20.))
                .when(!enabled, |view| view.opacity(0.3))
                .child(
                    h_flex().mt(css(10.)).gap(css(15.)).child(kinds).child(
                        BaseButton::new("cooling-warning")
                            .p_0()
                            .child(img("synapse/armory-cooling-warning.svg").size(css(20.)))
                            .tooltip(|window, cx| {
                                gpui_kit::component::tooltip::Tooltip::new(i18n::t(
                                    "MESSAGE_WARNING_SMART_FAN_DISABLED",
                                ))
                                .build(window, cx)
                            }),
                    ),
                )
                .child(
                    v_flex()
                        .text_size(css(13.))
                        .child(i18n::t("MODE").to_uppercase())
                        .child(
                            h_flex().mt(css(10.)).gap(css(15.)).child(modes).child(
                                BaseButton::new("cooling-reset")
                                    .p_0()
                                    .opacity(0.3)
                                    .hover(|style| style.opacity(1.))
                                    .disabled(!enabled)
                                    .child(img("synapse/armory-cooling-reset.svg").size(css(20.)))
                                    .on_click(window.listener_for(&state, |state, _, _, cx| {
                                        state.curves[state.sensor][state.mode] =
                                            preset(state.sensor, state.mode);
                                        state.clear_selection();
                                        cx.notify();
                                    })),
                            ),
                        ),
                )
                .child(chart),
        );
    div()
        .relative()
        .w_full()
        .min_w(css(770.))
        .max_w(css(1220.))
        .mx_auto()
        .font_family("Roboto")
        .text_size(css(14.))
        .text_color(rgb(0xcccccc))
        .child(surface::dot_background(cx))
        .child(v_flex().relative().child(config).child(panel).pb(css(30.)))
        .into_any_element()
}
