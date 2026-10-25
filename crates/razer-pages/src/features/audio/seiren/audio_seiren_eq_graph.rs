//! Current EQ6007 Q/U graph and 3108 faders. Graph dragging previews until
//! mouseup; the only live spectrum and meter inputs are actual observations.
use super::*;
use gpui_kit::base::{Slider as BaseSlider, SliderIndicator, SliderThumb, SliderTrack};

const WIDTH: f32 = 840.;
const HEIGHT: f32 = 242.;

fn auxiliary(
    workspace: &AudioProductWorkspace,
    preset: &model::Preset,
) -> Vec<(&'static str, Band)> {
    let mut bands = Vec::new();
    for (id, frequency, kind) in [
        ("vocal_bass", 300., "lowshelf"),
        ("vocal_exciter", 6000., "highshelf"),
    ] {
        let effect = &workspace.draft["device"]["eqEffects"][id];
        if effect["isEnabled"] == true {
            bands.push((
                id,
                Band {
                    id: 0,
                    frequency,
                    gain: effect["value"].as_f64().unwrap_or(50.) / 100. * 12. - 6.,
                    q: 1.,
                    kind: kind.into(),
                    status: None,
                },
            ));
        }
    }
    if preset.high_pass.enabled {
        let frequency = model::HIGH_PASS
            .iter()
            .find(|(id, _)| *id == preset.high_pass.value)
            .map_or(80., |(_, value)| f64::from(*value));
        bands.push((
            "high_pass_filter",
            Band {
                id: 0,
                frequency,
                gain: 0.,
                q: 1.,
                kind: "highpass".into(),
                status: None,
            },
        ));
    }
    bands
}

fn pixel(bounds: Bounds<Pixels>, x: f64, y: f64) -> Point<Pixels> {
    bounds.origin + point(bounds.size.width * x as f32, bounds.size.height * y as f32)
}

fn line(window: &mut Window, a: Point<Pixels>, b: Point<Pixels>, color: Hsla, width: f32) {
    let mut path = PathBuilder::stroke(px(width));
    path.move_to(a);
    path.line_to(b);
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

fn curve(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    points: &[(f64, f64)],
    color: Hsla,
    width: f32,
    fill: bool,
    smooth: bool,
) {
    if points.is_empty() {
        return;
    }
    let mut path = if fill {
        PathBuilder::fill()
    } else {
        PathBuilder::stroke(px(width))
    };
    let mut previous: Option<(f64, f64)> = None;
    if fill && !smooth {
        path.move_to(pixel(bounds, 0., 0.5));
    }
    for &(x, y) in points {
        if !x.is_finite() || !y.is_finite() {
            if !fill {
                if let Some((px, py)) = previous.filter(|(_, py)| py.is_finite()) {
                    path.line_to(pixel(bounds, x, py));
                    path.line_to(pixel(
                        bounds,
                        x,
                        if y.is_sign_negative() { 1.1 } else { -0.1 },
                    ));
                    let _ = px;
                }
            }
            previous = Some((x, y));
            continue;
        }
        match previous {
            None if !(fill && !smooth) => path.move_to(pixel(bounds, x, y)),
            Some((last_x, last_y)) if smooth && last_y.is_finite() => {
                path.curve_to(
                    pixel(bounds, (last_x + x) / 2., (last_y + y) / 2.),
                    pixel(bounds, last_x, last_y),
                );
            }
            Some((_, last_y)) if !last_y.is_finite() && !fill => {
                path.move_to(pixel(
                    bounds,
                    x,
                    if last_y.is_sign_negative() { 1.1 } else { -0.1 },
                ));
                path.line_to(pixel(bounds, x, y));
            }
            _ => path.line_to(pixel(bounds, x, y)),
        }
        previous = Some((x, y));
    }
    if fill {
        if smooth {
            path.line_to(pixel(bounds, 1., previous.map_or(1., |(_, y)| y)));
            path.line_to(pixel(bounds, 1., 1.));
            path.line_to(pixel(bounds, 0., 1.));
        } else {
            path.line_to(pixel(bounds, 1., 0.5));
        }
        path.close();
    } else if smooth {
        path.line_to(pixel(bounds, 1., previous.map_or(1., |(_, y)| y)));
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

impl AudioProductWorkspace {
    pub(super) fn seiren_eq_graph(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(model) = self.seiren_model() else {
            return div().into_any_element();
        };
        let Some(preset) = model.active() else {
            return div().into_any_element();
        };
        let extra = auxiliary(self, preset);
        let all: Vec<Band> = preset
            .bands
            .iter()
            .cloned()
            .chain(extra.iter().map(|(_, band)| band.clone()))
            .collect();
        let sum = math::response_points(&all, self.seiren_eq.output_sample_rate);
        let hovered = self
            .seiren_eq
            .hovered_effect
            .or(self.seiren_eq.pressed_effect);
        let selected = self
            .seiren_eq
            .hovered_band
            .and_then(|id| preset.bands.iter().find(|band| band.id == id))
            .or_else(|| preset.selected());
        let fill_band = hovered
            .and_then(|id| {
                extra
                    .iter()
                    .find(|(effect, _)| *effect == id)
                    .map(|(_, band)| band)
            })
            .or(selected);
        let fill_points = fill_band
            .map(|band| {
                math::response_points(
                    std::slice::from_ref(band),
                    self.seiren_eq.output_sample_rate,
                )
            })
            .unwrap_or_default();
        let fill_color = if hovered.is_some() {
            C::white()
        } else {
            selected.map_or(C::white(), |band| C::band(band.id))
        };
        let spectrum = self
            .seiren_eq
            .capture_sample_rate
            .map(|rate| math::spectrum_points(&self.seiren_eq.bins, rate, f64::from(WIDTH)))
            .unwrap_or_default();
        let owner = cx.entity().downgrade();
        let drawing = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let scale = f32::from(bounds.size.width) / WIDTH;
                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                    curve(
                        window,
                        bounds,
                        &spectrum,
                        C::spectrum_fill(),
                        scale,
                        true,
                        true,
                    );
                    curve(
                        window,
                        bounds,
                        &spectrum,
                        C::spectrum_line(),
                        scale,
                        false,
                        true,
                    );
                    for index in 0..5 {
                        let y = f64::from(index) / 4.;
                        line(
                            window,
                            pixel(bounds, 0., y),
                            pixel(bounds, 1., y),
                            C::grid(),
                            scale,
                        );
                    }
                    for frequency in model::FREQUENCIES {
                        let x = model::log_position(frequency);
                        line(
                            window,
                            pixel(bounds, x, 0.),
                            pixel(bounds, x, 1.),
                            C::grid(),
                            scale,
                        );
                    }
                    curve(
                        window,
                        bounds,
                        &fill_points,
                        fill_color.opacity(0.3),
                        0.,
                        true,
                        false,
                    );
                    curve(
                        window,
                        bounds,
                        &sum,
                        C::response().opacity(0.7),
                        2. * scale,
                        false,
                        false,
                    );
                });
                let movement = owner.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if !phase.capture() {
                        return;
                    }
                    let _ = movement.update(cx, |this, cx| {
                        let Some(drag) = this.seiren_eq.drag else {
                            return;
                        };
                        let Some(mut model) = this.seiren_model() else {
                            return;
                        };
                        let (Drag::Band(id) | Drag::QHandle(id, _)) = drag;
                        let Some(band) = model
                            .active_mut()
                            .and_then(|preset| preset.bands.iter_mut().find(|band| band.id == id))
                        else {
                            return;
                        };
                        match drag {
                            Drag::Band(_) => {
                                *band = model::drag_band(
                                    band,
                                    f64::from(event.position.x - bounds.origin.x)
                                        / f64::from(bounds.size.width),
                                    f64::from(event.position.y - bounds.origin.y)
                                        / f64::from(bounds.size.height),
                                );
                            }
                            Drag::QHandle(_, right) => {
                                let centre =
                                    pixel(bounds, model::log_position(band.frequency), 0.).x;
                                let distance = f64::from(event.position.x - centre)
                                    / f64::from(scale)
                                    * if right { 1. } else { -1. };
                                band.q = model::q_from_handle_distance(distance);
                            }
                        }
                        this.seiren_set_model(model, false, window, cx);
                    });
                });
                let release = owner.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                    if !phase.capture() || event.button != MouseButton::Left {
                        return;
                    }
                    let _ = release.update(cx, |this, cx| {
                        let Some(drag) = this.seiren_eq.drag.take() else {
                            return;
                        };
                        let (Drag::Band(id) | Drag::QHandle(id, _)) = drag;
                        let Some(model) = this.seiren_model() else {
                            return;
                        };
                        let band = model
                            .active()
                            .and_then(|preset| preset.bands.iter().find(|band| band.id == id))
                            .cloned();
                        this.seiren_set_model(model, true, window, cx);
                        if let Some(band) = band {
                            this.seiren_submit_band(&band, cx);
                        }
                    });
                });
            },
        )
        .size_full();
        let mut plot = div()
            .relative()
            .w(css(WIDTH))
            .h(css(HEIGHT))
            .bg(C::black())
            .child(drawing);
        for band in &preset.bands {
            let id = band.id;
            let dragging =
                matches!(self.seiren_eq.drag, Some(Drag::Band(dragged)) if dragged == id);
            let diameter = if band.kind == "peaking" {
                if dragging { 14. } else { 12. }
            } else if dragging {
                16.
            } else {
                14.
            };
            let x = model::log_position(band.frequency) as f32;
            let y = if band.pass() {
                1. - (band.q as f32 - 0.3) / 14.7
            } else {
                1. - (band.gain as f32 + 12.) / 24.
            };
            let mut point = BaseButton::new(("seiren-eq-point", u64::from(id)))
                .accessibility_label(format!("{} {}", t("EQ_BAND"), id + 1))
                .absolute()
                .left(relative(x))
                .top(relative(y))
                .ml(css(-diameter / 2.))
                .mt(css(-diameter / 2.))
                .size(css(diameter))
                .p_0()
                .rounded_full()
                .bg(C::band(id))
                .cursor_grab()
                .when(band.kind == "lowpass", |point| {
                    point.rounded_tl(css(0.)).rounded_bl(css(0.))
                })
                .when(band.kind == "highpass", |point| {
                    point.rounded_tr(css(0.)).rounded_br(css(0.))
                })
                .when(band.kind == "lowshelf", |point| point.rounded_tl(css(0.)))
                .when(band.kind == "highshelf", |point| point.rounded_tr(css(0.)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, window, cx| {
                        this.seiren_select_band(id, window, cx);
                        this.seiren_eq.drag = Some(Drag::Band(id));
                        cx.notify();
                        cx.stop_propagation();
                    }),
                )
                .on_hover(cx.listener(move |this, hovered, _, cx| {
                    this.seiren_eq.hovered_band = if *hovered { Some(id) } else { None };
                    cx.notify();
                }));
            if preset.selected == id && band.kind == "peaking" {
                let distance = model::q_handle_distance(band.q) as f32;
                point = point.child(
                    div()
                        .absolute()
                        .left(relative(0.5))
                        .top(css(diameter / 2.))
                        .ml(css(-distance))
                        .w(css(distance * 2.))
                        .h(css(1.))
                        .bg(C::band(id)),
                );
                for right in [false, true] {
                    point = point.child(
                        BaseButton::new((
                            "seiren-eq-q-handle",
                            u64::from(id) * 2 + u64::from(right),
                        ))
                        .accessibility_label(format!("{} Q", t("EQ_BAND")))
                        .absolute()
                        .left(relative(0.5))
                        .ml(css(if right { distance - 3. } else { -distance - 3. }))
                        .top(css(diameter / 2. - 3.))
                        .size(css(6.))
                        .p_0()
                        .rounded_full()
                        .bg(C::band(id))
                        .cursor_ew_resize()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.seiren_eq.drag = Some(Drag::QHandle(id, right));
                                cx.notify();
                                cx.stop_propagation();
                            }),
                        ),
                    );
                }
            }
            plot = plot.child(point);
        }
        let mut gain_axis = div().relative().w(css(55.)).h(css(240.));
        for gain in [-12, -6, 0, 6, 12] {
            let label = if gain == 0 {
                "0".into()
            } else if gain > 0 {
                format!("+{gain}dB")
            } else {
                format!("{gain}dB")
            };
            gain_axis = gain_axis.child(
                div()
                    .absolute()
                    .left(css(5.))
                    .top(relative(1. - (gain as f32 + 12.) / 24.))
                    .mt(css(-7.))
                    .w(css(50.))
                    .text_size(css(12.))
                    .child(label),
            );
        }
        gain_axis = gain_axis.child(
            BaseButton::new("seiren-eq-to-flat")
                .accessibility_label(label("$Qz"))
                .absolute()
                .right_0()
                .top(css(110.))
                .size(css(20.))
                .p_0()
                .text_color(C::text())
                .hover(|style| style.text_color(C::selected()))
                .child(
                    razer_assets::resource_svg()
                        .path("synapse/seiren-eq-flat.svg")
                        .size_full(),
                )
                .on_click(cx.listener(|this, _, window, cx| this.seiren_reset(true, window, cx))),
        );
        let mut frequency_axis = div()
            .relative()
            .w(css(WIDTH))
            .h(css(20.))
            .text_size(css(12.));
        for frequency in model::FREQUENCIES {
            let label = if frequency >= 1000. {
                format!("{}kHz", frequency / 1000.)
            } else {
                format!("{frequency}Hz")
            };
            let width = if frequency >= 1000. { 40. } else { 32. };
            frequency_axis = frequency_axis.child(
                div()
                    .absolute()
                    .left(relative(model::log_position(frequency) as f32))
                    .ml(css(-width / 2.))
                    .w(css(width))
                    .text_center()
                    .child(label),
            );
        }
        v_flex()
            .child(
                h_flex()
                    .items_start()
                    .gap(css(10.))
                    .child(plot)
                    .child(gain_axis),
            )
            .child(frequency_axis)
            .into_any_element()
    }
}

fn meter(level: f32, peak: f32, muted: bool) -> Div {
    let level = if muted { 0. } else { level.clamp(0., 100.) / 100. };
    div()
        .relative()
        .w(css(10.))
        .h(css(220.))
        .bg(C::black())
        .overflow_hidden()
        .opacity(if muted { 0.3 } else { 1. })
        .children(
            [
                (0., 2. / 3., C::selected()),
                (2. / 3., 1. / 6., C::meter_yellow()),
                (5. / 6., 1. / 6., C::meter_red()),
            ]
            .into_iter()
            .filter_map(move |(bottom, height, color)| {
                let height = (level - bottom).clamp(0., height);
                (height > 0.).then(|| {
                    div()
                        .absolute()
                        .bottom(relative(bottom))
                        .h(relative(height))
                        .w_full()
                        .bg(color)
                })
            }),
        )
        .child(
            div()
                .absolute()
                .bottom(relative(peak.clamp(0., 100.) / 100.))
                .h(css(4.))
                .w_full()
                .bg(C::peak().opacity(0.7)),
        )
}

pub(super) fn fader(
    workspace: &AudioProductWorkspace,
    index: usize,
    cx: &mut Context<AudioProductWorkspace>,
) -> AnyElement {
    let Some(state) = workspace.seiren_eq.faders.get(index) else {
        return div().into_any_element();
    };
    let (name, _) = MONITORS[index];
    let unavailable = index < 2 && !workspace.seiren_endpoint_available(index);
    let volume = &workspace.seiren_eq.monitor_preview[index];
    let muted = volume["isEnabled"] != true;
    let progress = volume["value"].as_f64().unwrap_or(0.).clamp(0., 100.) as f32 / 100.;
    let agc = index == 0 && workspace.draft["device"]["micAgc"]["isEnabled"] == true;
    let disabled = unavailable
        || index == 2
            && workspace.draft["profile"]["streamMixerSettings"]["isStreamMixerEnabled"] == true;
    let thumb_height = if index == 2 { 10. } else { 7.33 };
    let slider = BaseSlider::new(state)
        .vertical()
        .disabled(muted || agc || disabled)
        .w(css(24.))
        .h(css(220.))
        .relative()
        .opacity(if muted { 0.3 } else { 1. })
        .when(index == 2, |slider| {
            slider.child(
                div()
                    .absolute()
                    .left(css(11.))
                    .w(css(2.))
                    .h_full()
                    .bg(C::track())
                    .child(
                        div()
                            .absolute()
                            .bottom_0()
                            .h(relative(progress))
                            .w_full()
                            .bg(C::text()),
                    ),
            )
        })
        .child(
            SliderTrack::new(state)
                .axis(Axis::Vertical)
                .disabled(muted || agc || disabled)
                .size_full()
                .child(SliderIndicator::new(state).size_full().relative().when(
                    !agc && !unavailable,
                    |track| {
                        track.child(
                            SliderThumb::new(state)
                                .axis(Axis::Vertical)
                                .disabled(muted || disabled)
                                .absolute()
                                .left(css(2.))
                                .bottom(relative(progress))
                                .mb(css(-thumb_height / 2.))
                                .w(css(20.))
                                .h(css(thumb_height))
                                .border_1()
                                .border_color(C::thumb_border())
                                .bg(C::thumb()),
                        )
                    },
                )),
        );
    let mut visual = div().relative().w(css(24.)).h(css(220.));
    if index < 2 {
        let [left, right] = workspace.seiren_eq.levels[index];
        let [left_peak, right_peak] = workspace.seiren_eq.peaks[index];
        visual = visual.child(
            h_flex()
                .gap(css(4.))
                .child(meter(left, left_peak, muted))
                .child(meter(
                    if index == 0 { left } else { right },
                    if index == 0 { left_peak } else { right_peak },
                    muted,
                )),
        );
    }
    visual = visual.child(div().absolute().inset_0().child(slider));
    let mut middle = h_flex()
        .items_center()
        .justify_center()
        .gap(css(5.))
        .mt(css(6.));
    if unavailable {
        middle = middle.child(div().text_color(C::text()).child("—"));
    } else if let Some(number) = workspace.seiren_eq.numbers.get(name) {
        middle = middle.child(number.clone());
    }
    if index == 0 {
        middle = middle.child(div().text_size(css(14.)).line_height(css(14.)).child("dB"));
    }
    let mut bottom = h_flex()
        .items_center()
        .justify_center()
        .gap(css(if index == 0 { 5. } else { 8. }))
        .mt(css(8.))
        .child(
            BaseButton::new(("seiren-eq-mute", index as u64))
                .p_0()
                .size(css(20.))
                .disabled(disabled)
                .text_color(C::text())
                .hover(|style| style.text_color(C::selected()))
                .accessibility_label(label(if index == 0 { "$Qz" } else { "Rp1" }))
                .child(
                    razer_assets::resource_svg()
                        .path(SharedString::from(format!(
                            "synapse/seiren-eq-{name}{}.svg",
                            if muted { "-off" } else { "" }
                        )))
                        .size_full(),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.seiren_monitor_mute(index, window, cx)
                })),
        );
    if index == 0 {
        bottom = bottom.child(
            source_button("seiren-eq-agc", agc)
                .accessibility_label("Automatic Gain Control")
                .size(css(26.))
                .p_0()
                .rounded(css(3.))
                .hover(|style| style.border_color(C::hover_border()))
                .child("A")
                .on_click(cx.listener(|this, _, window, cx| {
                    let enabled = this.draft["device"]["micAgc"]["isEnabled"] != true;
                    this.edit("/device/micAgc/isEnabled", json!(enabled), window, cx);
                    this.submit_seiren_eq_payload(
                        "ON_SET_MIC_AGC",
                        json!({"isEnabled":u8::from(enabled)}),
                        cx,
                    );
                })),
        );
    }
    let mut channel = v_flex()
        .id(("seiren-eq-fader", index as u64))
        .items_center()
        .relative()
        .child(
            v_flex()
                .items_center()
                .opacity(if disabled { 0.3 } else { 1. })
                .child(visual)
                .child(middle)
                .child(bottom),
        )
        .on_hover(cx.listener(move |this, hovered, _, cx| {
            if index == 2 {
                this.seiren_eq.sidetone_tooltip = if *hovered && disabled {
                    Some(Point::default())
                } else {
                    None
                };
                cx.notify();
            }
        }))
        .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
            if index == 2 && disabled {
                this.seiren_eq.sidetone_tooltip = Some(event.position);
                cx.notify();
            }
        }));
    if index == 2 && disabled {
        if let Some(position) = workspace.seiren_eq.sidetone_tooltip {
            channel = channel.child(
                razer_assets::deferred_resource(CursorTip {
                    position,
                    content: div()
                        .w(css(300.))
                        .px(css(10.))
                        .py(css(8.))
                        .bg(C::black())
                        .border_1()
                        .border_color(C::border())
                        .text_color(C::text())
                        .text_size(css(14.))
                        .line_height(css(16.))
                        .child(label("PxA"))
                        .into_any_element(),
                })
                .with_priority(1060),
            );
        }
    }
    channel.into_any_element()
}

/// R portal cursor-tooltip is fixed at clientX/clientY-clientHeight. It has
/// no window clamping and no pointer handlers.
struct CursorTip {
    position: Point<Pixels>,
    content: AnyElement,
}
impl IntoElement for CursorTip {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for CursorTip {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.content.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.content.prepaint_at(
            self.position - point(px(0.), bounds.size.height),
            window,
            cx,
        );
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.content.paint(window, cx);
    }
}
