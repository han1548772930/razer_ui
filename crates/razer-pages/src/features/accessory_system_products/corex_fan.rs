//! Core X V2's current fanCurveReducer and ExpandingLineChart contracts.
use super::*;
use crate::features::Choice;
use gpui_kit::component::{
    radio::Radio,
    select::{SelectEvent, SelectState},
};
use std::{cell::Cell, rc::Rc};

pub(super) struct GraphInteraction {
    bounds: Rc<Cell<Bounds<Pixels>>>,
    selected: Option<usize>,
    pub(super) dragging: bool,
    focus: FocusHandle,
    selects: BTreeMap<&'static str, Entity<SelectState<Vec<Choice>>>>,
}
impl GraphInteraction {
    pub(super) fn new(cx: &mut App) -> Self {
        Self {
            bounds: Default::default(),
            selected: None,
            dragging: false,
            focus: cx.focus_handle().tab_stop(true),
            selects: BTreeMap::new(),
        }
    }
    pub(super) fn clear_selection(&mut self) {
        self.selected = None;
        self.dragging = false;
    }
}

fn source() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../corex_fan_data.json"))
            .expect("validated Core X fan source")
    })
}
fn label(key: &str) -> String {
    let locale = razer_i18n::locale();
    source()["translations"]
        .get(locale.as_str())
        .and_then(|v| v.get(key))
        .or_else(|| source()["translations"]["en"].get(key))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| t(key))
}
fn valid_curve(value: &Value, low: f64, high: f64) -> bool {
    let Some(points) = value.as_array() else {
        return false;
    };
    if !(2..=20).contains(&points.len()) {
        return false;
    }
    let (mut temperature, mut speed) = (f64::NEG_INFINITY, 1000.);
    for point in points {
        let (Some(x), Some(y)) = (
            point["temperature"].as_f64(),
            point["fanSpeedValue"].as_f64(),
        ) else {
            return false;
        };
        if !x.is_finite()
            || !y.is_finite()
            || x < low
            || x > high
            || x <= temperature
            || y < speed
            || y > 2800.
        {
            return false;
        }
        (temperature, speed) = (x, y);
    }
    true
}

struct Geometry {
    min: f64,
    max: f64,
    right: f64,
}
impl Geometry {
    fn new(target: &str) -> Self {
        let ticks = source()["axes"][target].as_array().expect("source axis");
        Self {
            min: ticks[0].as_f64().unwrap(),
            max: ticks.last().unwrap().as_f64().unwrap(),
            right: ((820. - 8. - 75.) / (ticks.len() - 1) as f64).floor()
                * (ticks.len() - 1) as f64
                + 75.,
        }
    }
    fn x(&self, temperature: f64) -> f64 {
        75. + (temperature - self.min) / (self.max - self.min) * (self.right - 75.)
    }
    fn y(speed: f64) -> f64 {
        335. - (speed - 1000.) / 1800. * 324.
    }
    fn speed(y: f64) -> f64 {
        (1000. + (335. - y) / 324. * 1800.)
            .round()
            .clamp(1000., 2800.)
    }
}

impl AccessorySystemProductWorkspace {
    fn corex_choices(kind: &str) -> Vec<Choice> {
        let items: Vec<(&str, String)> = match kind {
            "preset" => ["quiet", "balanced", "performance"]
                .map(|s| (s, label(&format!("SYSTEM_FAN_PRESET_{}", s.to_uppercase()))))
                .into(),
            "target" => vec![("gpu", label("GPU")), ("chassis", label("CHASSIS"))],
            "temperature" => vec![("celsius", "°C".into()), ("fahrenheit", "°F".into())],
            _ => vec![("percentage", "%".into()), ("rpm", "RPM".into())],
        };
        items
            .into_iter()
            .map(|(id, label)| Choice::new(id, label))
            .collect()
    }
    pub(super) fn init_corex(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for kind in ["preset", "target", "temperature", "speed"] {
            let choices = Self::corex_choices(kind);
            let state = cx.new(|cx| SelectState::new(choices, None, window, cx));
            self.subscriptions.push(cx.subscribe_in(
                &state,
                window,
                move |this, _, event, window, cx| {
                    if this.syncing {
                        return;
                    }
                    let SelectEvent::Confirm(Some(value)) = event else {
                        return;
                    };
                    match kind {
                        "preset" => {
                            this.change("/fanCurve/activeManualPreset", json!(value), window, cx)
                        }
                        "target" => this.change(
                            &format!(
                                "/fanCurve/manualPresets/{}/smartFanCurve/activeTemperatureMode",
                                this.corex_preset_ix()
                            ),
                            json!(value),
                            window,
                            cx,
                        ),
                        "temperature" => {
                            this.celsius = value == "celsius";
                            cx.notify();
                        }
                        _ => {
                            this.percentage = value == "percentage";
                            cx.notify();
                        }
                    }
                },
            ));
            self.corex_graph.selects.insert(kind, state);
        }
    }
    pub(super) fn sync_corex(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let preset = self.draft["fanCurve"]["activeManualPreset"]
            .as_str()
            .unwrap_or("quiet");
        for (key, value) in [
            ("preset", preset),
            ("target", self.corex_target()),
            (
                "temperature",
                if self.celsius {
                    "celsius"
                } else {
                    "fahrenheit"
                },
            ),
            ("speed", if self.percentage { "percentage" } else { "rpm" }),
        ] {
            if let Some(state) = self.corex_graph.selects.get(key) {
                state.update(cx, |state, cx| {
                    state.set_selected_value(&value.to_owned(), window, cx)
                });
            }
        }
    }
    fn corex_select(&self, kind: &'static str, width: f32) -> AnyElement {
        surface::select(&self.corex_graph.selects[kind])
            .items(Self::corex_choices(kind))
            .w(surface::css(width))
            .into_any_element()
    }
    fn corex_preset_ix(&self) -> usize {
        ["quiet", "balanced", "performance"]
            .iter()
            .position(|p| self.draft["fanCurve"]["activeManualPreset"] == *p)
            .unwrap_or(0)
    }
    fn corex_target(&self) -> &str {
        self.draft["fanCurve"]["manualPresets"][self.corex_preset_ix()]["smartFanCurve"]["activeTemperatureMode"].as_str().unwrap_or("gpu")
    }
    fn corex_path(&self) -> String {
        format!(
            "/fanCurve/manualPresets/{}/smartFanCurve/{}",
            self.corex_preset_ix(),
            self.corex_target()
        )
    }
    fn corex_points(&self) -> Vec<(f64, f64)> {
        self.draft
            .pointer(&self.corex_path())
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|v| Some((v["temperature"].as_f64()?, v["fanSpeedValue"].as_f64()?)))
            .collect()
    }
    pub(super) fn restore_corex(&mut self, saved: &Value) {
        self.corex_graph.selected = None;
        self.corex_graph.dragging = false;
        if !["Auto", "Manual"]
            .iter()
            .any(|s| self.draft["fanCurve"]["fanMode"] == *s)
        {
            self.draft["fanCurve"]["fanMode"] = json!("Manual");
        }
        if !["quiet", "balanced", "performance"]
            .iter()
            .any(|s| self.draft["fanCurve"]["activeManualPreset"] == *s)
        {
            self.draft["fanCurve"]["activeManualPreset"] = json!("quiet");
        }
        for ix in 0..3 {
            self.draft["fanCurve"]["manualPresets"][ix]["mode"] =
                json!(["quiet", "balanced", "performance"][ix]);
            let root = format!("/fanCurve/manualPresets/{ix}/smartFanCurve");
            let target_path = format!("{root}/activeTemperatureMode");
            if !["gpu", "chassis"].contains(&self.string(&target_path)) {
                self.set_raw(&target_path, json!("gpu"));
            }
            for target in ["gpu", "chassis"] {
                let path = format!("{root}/{target}");
                let axis = Geometry::new(target);
                if let Some(value) = saved
                    .pointer(&path)
                    .filter(|v| valid_curve(v, axis.min, axis.max))
                {
                    self.set_raw(&path, value.clone());
                } else {
                    self.reset_value(&path);
                }
            }
        }
    }
    fn set_corex_points(&mut self, points: Vec<(f64, f64)>, cx: &mut Context<Self>) {
        self.set_raw(&self.corex_path(), json!(points.into_iter().map(|(temperature, speed)| json!({"temperature":temperature,"fanSpeedValue":speed})).collect::<Vec<_>>()));
        cx.notify();
    }
    fn corex_pointer(&self, position: Point<Pixels>) -> Option<(f64, f64)> {
        let bounds = self.corex_graph.bounds.get();
        let scale = f32::from(bounds.size.width) as f64 / 820.;
        (scale > 0.).then(|| {
            (
                f32::from(position.x - bounds.left()) as f64 / scale,
                f32::from(position.y - bounds.top()) as f64 / scale,
            )
        })
    }
    fn corex_press(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some((x, y)) = self.corex_pointer(position) else {
            return;
        };
        let geometry = Geometry::new(self.corex_target());
        let mut points = self.corex_points();
        if let Some(ix) = points.iter().position(|(t, rpm)| {
            (geometry.x(*t) - x).powi(2) + (Geometry::y(*rpm) - y).powi(2) <= 64.
        }) {
            self.corex_graph.selected = Some(ix);
            self.corex_graph.dragging = true;
        } else if points.len() < 20
            && x >= 75.
            && x <= geometry.right
            && !points.iter().any(|(t, _)| (geometry.x(*t) - x).abs() <= 8.)
        {
            let temperature =
                geometry.min + (x - 75.) / (geometry.right - 75.) * (geometry.max - geometry.min);
            let after = points
                .iter()
                .position(|p| p.0 > temperature)
                .unwrap_or(points.len());
            let before = if after == 0 {
                (geometry.min, 1000.)
            } else {
                points[after - 1]
            };
            let next = points
                .get(after)
                .copied()
                .unwrap_or((geometry.max, before.1));
            let rpm = if next.0 == before.0 {
                before.1
            } else {
                before.1 + (next.1 - before.1) * (temperature - before.0) / (next.0 - before.0)
            };
            if (Geometry::y(rpm) - y).abs() <= 10. {
                points.insert(after, (temperature, rpm.round()));
                self.corex_graph.selected = Some(after);
                self.set_corex_points(points, cx);
                cx.emit(AccessorySystemProductChanged);
            }
        } else {
            self.corex_graph.selected = None;
        }
        cx.notify();
    }
    fn corex_move(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if !self.corex_graph.dragging {
            return;
        }
        let (Some(ix), Some((_, y))) = (self.corex_graph.selected, self.corex_pointer(position))
        else {
            return;
        };
        let mut points = self.corex_points();
        if ix >= points.len() {
            return;
        }
        let min = ix.checked_sub(1).map_or(1000., |i| points[i].1);
        let max = points.get(ix + 1).map_or(2800., |p| p.1);
        points[ix].1 = Geometry::speed(y).clamp(min, max);
        self.set_corex_points(points, cx);
    }
    fn corex_release(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.corex_graph.dragging) {
            cx.emit(AccessorySystemProductChanged);
        }
    }
    fn corex_key(&mut self, key: &str, cx: &mut Context<Self>) {
        let mut points = self.corex_points();
        if points.is_empty() {
            return;
        }
        let ix = self.corex_graph.selected.unwrap_or(0).min(points.len() - 1);
        match key {
            "left" => self.corex_graph.selected = Some(ix.saturating_sub(1)),
            "right" => self.corex_graph.selected = Some((ix + 1).min(points.len() - 1)),
            "up" | "down" => {
                let min = ix.checked_sub(1).map_or(1000., |i| points[i].1);
                let max = points.get(ix + 1).map_or(2800., |p| p.1);
                points[ix].1 = (points[ix].1 + if key == "up" { 1. } else { -1. }).clamp(min, max);
                self.corex_graph.selected = Some(ix);
                self.set_corex_points(points, cx);
                cx.emit(AccessorySystemProductChanged);
            }
            "delete" | "backspace" if points.len() > 2 => {
                points.remove(ix);
                self.set_corex_points(points, cx);
                self.corex_graph.selected = None;
                cx.emit(AccessorySystemProductChanged);
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }
    pub(super) fn corex_fan(&self, cx: &mut Context<Self>) -> AnyElement {
        let smart = self.draft["fanCurve"]["fanMode"] == "Manual";
        let mut panel = surface::panel(label("SYSTEM_FAN_CONTROL"), cx).min_w(surface::css(900.));
        for (value, key) in [("Auto", "AUTO"), ("Manual", "SMART")] {
            panel = panel.child(
                v_flex()
                    .gap(surface::css(4.))
                    .mb(surface::css(10.))
                    .child(
                        Radio::new(SharedString::from(format!("corex-mode-{value}")))
                            .label(label(&format!("SYSTEM_FAN_CONTROL_{key}")))
                            .checked(self.draft["fanCurve"]["fanMode"] == value)
                            .on_change(cx.listener(move |this, _, window, cx| {
                                this.change("/fanCurve/fanMode", json!(value), window, cx)
                            })),
                    )
                    .child(div().ml(surface::css(30.)).child(surface::note(
                        label(&format!("SYSTEM_FAN_CONTROL_{key}_DESCRIPTION")),
                        cx,
                    ))),
            );
        }
        if !smart {
            return panel.into_any_element();
        }
        let preset_ix = self.corex_preset_ix();
        let target = self.corex_target().to_owned();
        let mode = ["quiet", "balanced", "performance"][preset_ix];
        let points = self.corex_points();
        let defaults = self.spec.presets[mode][&target]
            .as_array()
            .expect("source curve");
        let dirty = points.len() != defaults.len()
            || points
                .iter()
                .zip(defaults)
                .any(|(p, d)| (p.1 - d["fanSpeedValue"].as_f64().unwrap()).abs() > 0.1);
        let presets = h_flex()
            .gap_2()
            .child(self.corex_select("preset", 150.))
            .child(
                Button::new("corex-reset")
                    .label(t("RESET"))
                    .outline()
                    .disabled(!dirty)
                    .on_click(cx.listener(|this, _, _, cx| {
                        let ix = this.corex_preset_ix();
                        let mode = ["quiet", "balanced", "performance"][ix];
                        for target in ["gpu", "chassis"] {
                            this.draft["fanCurve"]["manualPresets"][ix]["smartFanCurve"][target] =
                                this.spec.presets[mode][target].clone();
                        }
                        this.corex_graph.selected = None;
                        cx.emit(AccessorySystemProductChanged);
                        cx.notify();
                    })),
            );
        panel = panel.child(presets).child(
            h_flex()
                .gap_2()
                .child(self.corex_select("temperature", 90.))
                .child(self.corex_select("speed", 90.)),
        );
        let geometry = Geometry::new(&target);
        let bounds_cell = self.corex_graph.bounds.clone();
        let paint_points = points.clone();
        let line = cx.theme().primary;
        let grid = cx.theme().border;
        let ticks = source()["axes"][&target].as_array().unwrap();
        let count = ticks.len();
        let right = geometry.right;
        let mut graph = div()
            .id("corex-curve")
            .relative()
            .w(surface::css(820.))
            .h(surface::css(410.))
            .child(
                canvas(
                    move |bounds, _, _| bounds_cell.set(bounds),
                    move |bounds, _, window, _| {
                        let scale = f32::from(bounds.size.width) / 820.;
                        let pos = |x: f64, y: f64| {
                            point(
                                bounds.left() + px(x as f32 * scale),
                                bounds.top() + px(y as f32 * scale),
                            )
                        };
                        let mut path = PathBuilder::stroke(px(scale));
                        for i in 0..count {
                            let x = 75. + (right - 75.) * i as f64 / (count - 1) as f64;
                            path.move_to(pos(x, 11.));
                            path.line_to(pos(x, 335.));
                        }
                        for i in 0..=4 {
                            let y = 335. - 81. * i as f64;
                            path.move_to(pos(75., y));
                            path.line_to(pos(right, y));
                        }
                        if let Ok(path) = path.build() {
                            window.paint_path(path, grid);
                        }
                        let mut path = PathBuilder::stroke(px(2. * scale));
                        if let Some((temp, rpm)) = paint_points
                            .first()
                            .filter(|(temp, _)| (geometry.x(*temp) - 75.).abs() < 1.)
                        {
                            path.move_to(pos(geometry.x(*temp), Geometry::y(*rpm)));
                        } else {
                            path.move_to(pos(75., 335.));
                        }
                        for (t, rpm) in &paint_points {
                            path.line_to(pos(geometry.x(*t), Geometry::y(*rpm)));
                        }
                        if let Some((_, rpm)) = paint_points.last() {
                            path.line_to(pos(right, Geometry::y(*rpm)));
                        }
                        if let Ok(path) = path.build() {
                            window.paint_path(path, line);
                        }
                    },
                )
                .size_full(),
            )
            .track_focus(&self.corex_graph.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                this.corex_key(&event.keystroke.key, cx)
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.corex_graph.focus.focus(window, cx);
                    this.corex_press(event.position, cx);
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                this.corex_move(event.position, cx)
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.corex_release(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.corex_release(cx)),
            );
        let geometry = Geometry::new(&target);
        for (ix, (temp, rpm)) in points.iter().enumerate() {
            graph = graph.child(
                div()
                    .absolute()
                    .left(surface::css(geometry.x(*temp) as f32 - 8.))
                    .top(surface::css(Geometry::y(*rpm) as f32 - 8.))
                    .size(surface::css(16.))
                    .rounded_full()
                    .bg(if self.corex_graph.selected == Some(ix) {
                        cx.theme().muted
                    } else {
                        line
                    })
                    .border_2()
                    .border_color(line),
            );
        }
        for (ix, tick) in ticks.iter().enumerate() {
            let temp = tick.as_f64().unwrap();
            graph = graph.child(
                div()
                    .absolute()
                    .left(surface::css(
                        (75. + (right - 75.) * ix as f64 / (count - 1) as f64 - 12.) as f32,
                    ))
                    .top(surface::css(345.))
                    .text_size(surface::css(10.))
                    .child(if self.celsius {
                        format!("{temp:.0}°C")
                    } else {
                        format!("{:.0}°F", temp * 1.8 + 32.)
                    }),
            );
        }
        for ix in 0..=4 {
            let rpm = 1000. + 450. * ix as f64;
            graph = graph.child(
                div()
                    .absolute()
                    .left(surface::css(10.))
                    .top(surface::css(327. - 81. * ix as f32))
                    .w(surface::css(55.))
                    .text_right()
                    .text_size(surface::css(10.))
                    .child(if self.percentage {
                        format!("{:.0}%", rpm / 2800. * 100.)
                    } else {
                        format!("{rpm:.0} RPM")
                    }),
            );
        }
        panel = panel.child(graph).child(
            h_flex()
                .gap_2()
                .child(label("SYSTEM_FAN_TEMPERATURE"))
                .child(self.corex_select("target", 150.)),
        );
        if let Some(ix) = self.corex_graph.selected.filter(|i| *i < points.len()) {
            let (temp, rpm) = points[ix];
            let temperature = if self.celsius {
                format!("{temp:.2}°C")
            } else {
                format!("{:.2}°F", temp * 1.8 + 32.)
            };
            let speed = if self.percentage {
                format!("{:.0}%", rpm / 2800. * 100.)
            } else {
                format!("{rpm:.0} RPM")
            };
            panel = panel.child(
                h_flex()
                    .gap_2()
                    .child(format!("{temperature} · {speed}"))
                    .child(
                        Button::new("corex-delete-point")
                            .label(t("DELETE"))
                            .outline()
                            .disabled(points.len() <= 2)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let mut points = this.corex_points();
                                if points.len() > 2 && ix < points.len() {
                                    points.remove(ix);
                                    this.set_corex_points(points, cx);
                                    this.corex_graph.selected = None;
                                    cx.emit(AccessorySystemProductChanged);
                                }
                            })),
                    ),
            );
        }
        panel.into_any_element()
    }
}
