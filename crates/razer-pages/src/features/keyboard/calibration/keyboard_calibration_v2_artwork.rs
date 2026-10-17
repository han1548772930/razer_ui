//! Native painting of 756's source SVG masks, CSS animation and selection zone.
//! No SMIL playback is assumed and animation never fabricates a device result.
use super::*;
use crate::keyboard_geometry::KeyboardKeyExt;
use gpui_kit::base::motion::Easing;
use std::{cell::Cell, rc::Rc, time::Instant};

#[derive(Default)]
pub(super) struct Artwork {
    bounds: Rc<Cell<Bounds<Pixels>>>,
    drag: Option<Drag>,
    suppress_click: bool,
    hovered: Option<String>,
    press: Option<(String, Instant)>,
    pulse_started: Option<(String, Instant)>,
    failures: BTreeMap<String, Instant>,
    save_wave: Option<Wave>,
    reset_wave: Option<Wave>,
    pub(super) feedback_lock: bool,
    feedback_task: Option<Task<()>>,
    animation_frames: Option<Task<()>>,
    toast: Option<(Instant, usize)>,
    generation: u64,
}
struct Drag {
    start: Point<Pixels>,
    current: Point<Pixels>,
    initial: BTreeSet<String>,
    additive: bool,
    moved: bool,
}
#[derive(Clone)]
struct Wave {
    started: Instant,
    keys: BTreeSet<String>,
    reset: bool,
}
impl Wave {
    fn progress(&self, reduce_motion: bool) -> Option<f32> {
        let elapsed = self.started.elapsed().as_secs_f32();
        (elapsed < 4.5 && !reduce_motion).then_some(elapsed / 4.5)
    }
}
impl Artwork {
    pub(super) fn target_changed(&mut self, target: Option<&str>) {
        if self.pulse_started.as_ref().map(|(key, _)| key.as_str()) != target {
            self.pulse_started = target.map(|key| (key.to_owned(), Instant::now()));
        }
    }
    pub(super) fn keyboard_bounds(&self) -> Bounds<Pixels> {
        self.bounds.get()
    }
    pub(super) fn unmount(&mut self) {
        let generation = self.generation.wrapping_add(1);
        *self = Self::default();
        self.generation = generation;
    }
    pub(super) fn observe_key(&mut self, payload: &Value) {
        let Some(id) = payload["inputId"].as_str() else {
            return;
        };
        match payload["status"].as_str() {
            Some("calibrationStart") => {
                self.press = Some((id.to_owned(), Instant::now()));
                self.failures.remove(id);
            }
            Some("calibrationFailed") => {
                self.failures
                    .entry(id.to_owned())
                    .or_insert_with(Instant::now);
                if self.press.as_ref().is_some_and(|(key, _)| key == id) {
                    self.press = None;
                }
            }
            Some("calibrationSuccess" | "calibrationAborted") => {
                if self.press.as_ref().is_some_and(|(key, _)| key == id) {
                    self.press = None;
                }
                self.failures.remove(id);
            }
            _ => {}
        }
    }
    pub(super) fn reset(&mut self, keys: BTreeSet<String>) {
        self.reset_wave = Some(Wave {
            started: Instant::now(),
            keys,
            reset: true,
        });
        self.press = None;
    }
}
fn zone(start: Point<Pixels>, end: Point<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        point(start.x.min(end.x), start.y.min(end.y)),
        size((start.x - end.x).abs(), (start.y - end.y).abs()),
    )
}
fn intersects(a: Bounds<Pixels>, b: Bounds<Pixels>) -> bool {
    a.right() >= b.left() && a.left() <= b.right() && a.bottom() >= b.top() && a.top() <= b.bottom()
}
fn pulse(elapsed: f32, period: f32, low: f32) -> f32 {
    let p = (elapsed / period).fract();
    let half = if p < 0.5 { p * 2. } else { (1. - p) * 2. };
    1. - (1. - low) * Easing::EaseInOut.sample(half)
}
impl KeyboardProductWorkspace {
    pub(super) fn start_calibration_v2_artwork_clock(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.calibration_v2.artwork.animation_frames.is_some() {
            return;
        }
        self.calibration_v2.artwork.animation_frames =
            Some(cx.spawn_in(window, async move |view, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(16))
                        .await;
                    let active = view
                        .update_in(cx, |this, _, cx| {
                            let state = &this.calibration_v2;
                            let visual = &state.artwork;
                            let active = this.page == "TAB_CALIBRATION"
                                && (state.phase == Phase::Calibrating
                                    || visual.feedback_lock
                                    || visual.save_wave.as_ref().is_some_and(|wave| {
                                        wave.started.elapsed().as_secs_f32() < 4.5
                                    })
                                    || visual.reset_wave.as_ref().is_some_and(|wave| {
                                        wave.started.elapsed().as_secs_f32() < 4.5
                                    })
                                    || visual.toast.is_some_and(|(started, _)| {
                                        started.elapsed().as_secs_f32() < 3.
                                    }));
                            if !active {
                                this.calibration_v2.artwork.animation_frames = None;
                            }
                            cx.notify();
                            active
                        })
                        .unwrap_or(false);
                    if !active {
                        break;
                    }
                }
            }));
    }
    pub(super) fn calibration_v2_save_feedback(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keys = self.calibration_v2.calibrated.clone();
        let count = keys.len();
        let reduced = cx.reduce_motion();
        self.calibration_v2.selected.clear();
        let state = &mut self.calibration_v2.artwork;
        state.press = None;
        state.feedback_lock = true;
        state.generation = state.generation.wrapping_add(1);
        let generation = state.generation;
        state.feedback_task = Some(cx.spawn_in(window, async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let active = view
                .update_in(cx, |this, _, cx| {
                    let state = &mut this.calibration_v2.artwork;
                    if state.generation != generation {
                        return false;
                    }
                    if count > 0 {
                        state.save_wave = Some(Wave {
                            started: Instant::now(),
                            keys,
                            reset: false,
                        });
                    }
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !active {
                return;
            }
            if count > 0 && !reduced {
                cx.background_executor()
                    .timer(Duration::from_millis(4500))
                    .await;
            }
            let active = view
                .update_in(cx, |this, _, cx| {
                    let state = &mut this.calibration_v2.artwork;
                    if state.generation != generation {
                        return false;
                    }
                    state.feedback_lock = false;
                    state.save_wave = None;
                    state.toast = Some((Instant::now(), count));
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !active {
                return;
            }
            cx.background_executor().timer(Duration::from_secs(3)).await;
            let _ = view.update_in(cx, |this, _, cx| {
                if this.calibration_v2.artwork.generation == generation {
                    this.calibration_v2.artwork.toast = None;
                    cx.notify();
                }
            });
        }));
        self.start_calibration_v2_artwork_clock(window, cx);
    }
    fn move_v2_zone(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let state = &mut self.calibration_v2;
        let bounds = state.artwork.bounds.get();
        let Some(drag) = &mut state.artwork.drag else {
            return;
        };
        if !drag.moved
            && (position.x - drag.start.x).abs() < px(4.)
            && (position.y - drag.start.y).abs() < px(4.)
        {
            return;
        }
        drag.moved = true;
        drag.current = point(
            position.x.clamp(bounds.left(), bounds.right()),
            position.y.clamp(bounds.top(), bounds.bottom()),
        );
        let area = zone(drag.start, drag.current);
        let mut selected = if drag.additive {
            drag.initial.clone()
        } else {
            BTreeSet::new()
        };
        for (key, rect) in self.analog_gamepad.floating.key_bounds.borrow().values() {
            if intersects(*rect, area)
                && self
                    .spec
                    .keys
                    .iter()
                    .any(|row| row["inputID"] == key.id && row["inputType"] == "AnalogInput")
            {
                selected.insert(key.id.clone());
            }
        }
        state.selected = selected;
        state.artwork.hovered = None;
        cx.notify();
    }
    fn finish_v2_zone(&mut self, cx: &mut Context<Self>) {
        if let Some(drag) = self.calibration_v2.artwork.drag.take() {
            self.calibration_v2.artwork.suppress_click = drag.moved;
            let owner = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = owner.update(cx, |this, _| {
                    this.calibration_v2.artwork.suppress_click = false
                });
            });
            cx.notify();
        }
    }
    pub(in super::super) fn calibration_v2_keyboard(
        &self,
        keyboard: Div,
        cx: &Context<Self>,
    ) -> Div {
        let state = &self.calibration_v2;
        let visual = &state.artwork;
        let measuring = visual.bounds.clone();
        let owner = cx.weak_entity();
        let dragging = visual.drag.is_some();
        let drag_area = visual
            .drag
            .as_ref()
            .filter(|drag| drag.moved)
            .map(|drag| zone(drag.start, drag.current));
        let keyboard_bounds = visual.bounds.get();
        let mut keyboard = keyboard
            .on_prepaint(move |bounds, _, _| measuring.set(bounds))
            .capture_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, window, cx| {
                if event.button != MouseButton::Left
                    || this.calibration_v2.phase != Phase::Idle
                    || this.calibration_v2.artwork.feedback_lock
                    || this.factory_default_profile && this.calibration_v2.normal_mode
                {
                    return;
                }
                let state = &mut this.calibration_v2;
                state.artwork.suppress_click = false;
                state.artwork.drag = Some(Drag {
                    start: event.position,
                    current: event.position,
                    initial: state.selected.clone(),
                    additive: event.modifiers.control || event.modifiers.platform,
                    moved: false,
                });
                window.prevent_default();
                cx.notify();
            }))
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        if !dragging {
                            return;
                        }
                        let owner_move = owner.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture {
                                let _ = owner_move
                                    .update(cx, |this, cx| this.move_v2_zone(event.position, cx));
                            }
                        });
                        let owner_up = owner.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture && event.button == MouseButton::Left
                            {
                                let _ = owner_up.update(cx, |this, cx| this.finish_v2_zone(cx));
                            }
                        });
                    },
                )
                .absolute()
                .inset_0(),
            )
            .when(
                state.phase == Phase::Saving
                    && visual
                        .reset_wave
                        .as_ref()
                        .and_then(|wave| wave.progress(cx.reduce_motion()))
                        .is_none(),
                |keyboard| keyboard.opacity(0.3),
            );
        if let Some(area) = drag_area {
            keyboard = keyboard.cursor(CursorStyle::Crosshair).child(
                div()
                    .absolute()
                    .left(area.left() - keyboard_bounds.left())
                    .top(area.top() - keyboard_bounds.top())
                    .w(area.size.width)
                    .h(area.size.height)
                    .bg(rgba(0x44d62c26))
                    .border_1()
                    .border_color(rgb(0x44d62c)),
            );
        }
        if let Some(id) = &visual.hovered
            && let Some((key, bounds)) = self
                .analog_gamepad
                .floating
                .key_bounds
                .borrow()
                .values()
                .find(|(key, _)| &key.id == id)
        {
            let switch = state.switches.get(id);
            let index = switch.and_then(|s| s["switchIndex"].as_u64()).unwrap_or(1);
            let definition = data()
                .switches
                .iter()
                .find(|s| s.switch_index as u64 == index)
                .unwrap_or(&data().switches[0]);
            let travel = switch.and_then(|s| s["keyTravel"].as_f64()).unwrap_or(40.);
            keyboard = keyboard.child(
                razer_assets::deferred_resource(
                    anchored()
                        .anchor(Anchor::BottomCenter)
                        .position(point(
                            bounds.left() + bounds.size.width / 2.,
                            bounds.top() - px(5. * f32::from(bounds.size.width) / key.bounds[2]),
                        ))
                        .child(
                            v_flex()
                                .min_w(surface::css(130.))
                                .border_1()
                                .border_color(rgb(0x5d5d5d))
                                .bg(rgb(0))
                                .text_center()
                                .child(
                                    v_flex()
                                        .items_center()
                                        .gap(surface::css(5.))
                                        .px(surface::css(10.))
                                        .py(surface::css(5.))
                                        .child(
                                            div()
                                                .text_size(surface::css(14.))
                                                .line_height(surface::css(16.))
                                                .text_color(rgb(0x707070))
                                                .child(
                                                    self.spec
                                                        .keys
                                                        .iter()
                                                        .find(|row| row["inputID"] == key.id)
                                                        .and_then(|row| {
                                                            row["defaultValue"].as_str()
                                                        })
                                                        .unwrap_or(&key.label)
                                                        .to_uppercase(),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_size(surface::css(12.))
                                                .line_height(surface::css(14.))
                                                .text_color(if index == 1 {
                                                    rgb(0xcccccc)
                                                } else {
                                                    rgb(0xfd8611)
                                                })
                                                .child(t(&definition.name).to_uppercase()),
                                        )
                                        .child(
                                            div()
                                                .text_size(surface::css(18.))
                                                .text_color(rgb(0xcccccc))
                                                .child(format!("{:.1}mm", travel / 10.)),
                                        ),
                                )
                                .children(state.calibrated.contains(id).then(|| {
                                    div()
                                        .w_full()
                                        .px(surface::css(10.))
                                        .py(surface::css(3.))
                                        .bg(rgb(0x5d5d5d))
                                        .text_color(rgb(0xffffff))
                                        .text_size(surface::css(12.))
                                        .child(label("xbB"))
                                })),
                        ),
                )
                .with_priority(3),
            );
        }
        if let Some((started, count)) = visual.toast {
            let elapsed = started.elapsed().as_secs_f32();
            let opacity = if cx.reduce_motion() || elapsed <= 2.64 {
                1.
            } else {
                1. - Easing::Ease.sample(((elapsed - 2.64) / 0.36).clamp(0., 1.))
            };
            keyboard = keyboard.child(
                div()
                    .absolute()
                    .top(surface::css(20.))
                    .left(relative(0.25))
                    .min_w(surface::css(338.))
                    .px(surface::css(18.))
                    .py(surface::css(14.))
                    .border_1()
                    .border_color(if count == 0 { rgb(0xfd8611) } else { rgb(0x44d62c) })
                    .rounded(surface::css(3.))
                    .bg(rgb(0x111111))
                    .text_center()
                    .opacity(opacity)
                    .child(if count == 0 {
                        label("wAn")
                    } else {
                        formatted("$Nd", &[("count", count.to_string())])
                    })
                    .child(
                        canvas(
                            |_, _, _| (),
                            |_, _, window, _| window.request_animation_frame(),
                        )
                        .absolute()
                        .inset_0(),
                    ),
            );
        }
        keyboard
    }
    pub(in super::super) fn calibration_v2_key(
        &self,
        key: &'static razer_assets::KeyboardKey,
        cx: &Context<Self>,
    ) -> AnyElement {
        let state = &self.calibration_v2;
        let visual = &state.artwork;
        let selected = state.selected.contains(&key.id);
        let hovered = visual.hovered.as_deref() == Some(&key.id);
        let status = state
            .measuring
            .iter()
            .find(|row| row["inputId"] == key.id)
            .and_then(|row| row["status"].as_str());
        let complete = status == Some("calibrationSuccess");
        let target = state.target.as_deref() == Some(&key.id) && !complete;
        let failed = target && status == Some("calibrationFailed");
        let pressing = visual
            .press
            .as_ref()
            .filter(|(id, _)| id == &key.id && target)
            .map(|(_, started)| started.elapsed().as_secs_f32());
        let calibrated = state.calibrated.contains(&key.id);
        let reduced = cx.reduce_motion();
        let waves = [&visual.save_wave, &visual.reset_wave]
            .into_iter()
            .flatten()
            .filter_map(|wave| wave.progress(reduced).map(|p| (wave.clone(), p)))
            .collect::<Vec<_>>();
        let wave_active = !waves.is_empty();
        let fill = if wave_active || pressing.is_some() || target {
            rgba(0)
        } else if complete || calibrated && selected {
            rgba(0x44d62c4d)
        } else {
            rgba(0)
        };
        let (color, width) = if complete {
            (rgb(0x666666), 1.)
        } else if failed {
            (rgb(0xfd8611), 2.)
        } else if target || selected || hovered {
            (rgb(0x44d62c), 2.)
        } else if calibrated {
            (rgba(0x44d62c4d), 2.)
        } else {
            (rgba(0), 1.)
        };
        let mut stroke: Hsla = color.into();
        let mut animate = false;
        if !reduced {
            if failed && let Some(started) = visual.failures.get(&key.id) {
                let elapsed = started.elapsed().as_secs_f32();
                if elapsed < 1.05 {
                    stroke.a *= pulse(elapsed, 0.35, 0.25);
                    animate = true;
                }
            } else if target && pressing.is_none() {
                stroke.a *= pulse(
                    visual
                        .pulse_started
                        .as_ref()
                        .map_or(0., |(_, at)| at.elapsed().as_secs_f32()),
                    0.8,
                    0.2,
                );
                animate = true;
            }
        }
        let enabled = self.calibration_v2_key_enabled(&key.id) && !visual.feedback_lock;
        let bounds = Rc::new(Cell::new(Bounds::default()));
        let measured = bounds.clone();
        let click_bounds = bounds.clone();
        let hover_bounds = bounds.clone();
        let key_id = key.id.clone();
        let hover_id = key.id.clone();
        let leave_id = key.id.clone();
        let keyboard_bounds = visual.bounds.get();
        div()
            .relative()
            .size_full()
            .on_prepaint(move |rect, _, _| measured.set(rect))
            .child(
                canvas(
                    |_, _, _| (),
                    move |rect, _, window, _| {
                        if fill.a > 0.
                            && let Ok(path) = key.path(rect, false).build()
                        {
                            window.paint_path(path, fill);
                        }
                        if let Some(elapsed) = pressing {
                            let progress = if reduced { 1. } else { (elapsed / 0.5).min(1.) };
                            let mask = ContentMask {
                                bounds: Bounds::new(
                                    rect.origin,
                                    size(rect.size.width, rect.size.height * progress),
                                ),
                            };
                            window.with_content_mask(Some(mask), |window| {
                                if let Ok(path) = key.path(rect, false).build() {
                                    window.paint_path(path, rgba(0x44d62c4d));
                                }
                            });
                            if progress < 1. {
                                window.request_animation_frame();
                            }
                        }
                        for (wave, progress) in &waves {
                            if wave.keys.contains(&key.id) {
                                paint_wave(
                                    key,
                                    rect,
                                    keyboard_bounds,
                                    *progress,
                                    wave.reset,
                                    window,
                                );
                            }
                        }
                        if stroke.a > 0. {
                            let options = StrokeOptions::default()
                                .with_line_width(width * f32::from(rect.size.width) / key.bounds[2])
                                .with_line_join(lyon_tessellation::LineJoin::Round);
                            if let Ok(path) = key
                                .path(rect, true)
                                .with_style(PathStyle::Stroke(options))
                                .build()
                            {
                                window.paint_path(path, stroke);
                            }
                        }
                        if animate || !waves.is_empty() {
                            window.request_animation_frame();
                        }
                    },
                )
                .absolute()
                .inset_0(),
            )
            .child(
                BaseButton::new(SharedString::from(format!("v2-calibration-key-{}", key.id)))
                    .size_full()
                    .accessibility_label(key.label.clone())
                    .disabled(!enabled)
                    .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                        if this.calibration_v2.artwork.suppress_click {
                            return;
                        }
                        if key.accepts_click(event, click_bounds.get()) {
                            this.select_calibration_v2_key(
                                &key_id,
                                event.modifiers().control || event.modifiers().platform,
                                cx,
                            );
                            cx.stop_propagation();
                        }
                    }))
                    .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                        let next = (enabled
                            && key.contains_window_point(event.position, hover_bounds.get()))
                        .then(|| hover_id.clone());
                        if this.calibration_v2.artwork.hovered != next {
                            this.calibration_v2.artwork.hovered = next;
                            cx.notify();
                        }
                    }))
                    .on_hover(cx.listener(move |this, inside: &bool, _, cx| {
                        if !inside
                            && this.calibration_v2.artwork.hovered.as_ref() == Some(&leave_id)
                        {
                            this.calibration_v2.artwork.hovered = None;
                            cx.notify();
                        }
                    })),
            )
            .into_any_element()
    }
}
fn paint_wave(
    key: &razer_assets::KeyboardKey,
    rect: Bounds<Pixels>,
    keyboard: Bounds<Pixels>,
    progress: f32,
    reset: bool,
    window: &mut Window,
) {
    let width = f32::from(keyboard.size.width);
    let wave_width = 1.4 * width;
    let distance = 1.3 * width;
    let translate = -wave_width + (width + 2. * distance + wave_width) * progress;
    let stops = [
        (0., 0.),
        (0.2, 0.1),
        (0.35, 0.38),
        (0.5, 0.5),
        (0.65, 0.38),
        (0.8, 0.1),
        (1., 0.),
    ];
    let color: Hsla = if reset {
        rgb(0xfd4949).into()
    } else {
        rgb(0x44d62c).into()
    };
    for offset in [-2. * distance, -distance, 0.] {
        let start = keyboard.left() + px(translate + offset);
        for pair in stops.windows(2) {
            let a = start + px(wave_width * pair[0].0);
            let b = start + px(wave_width * pair[1].0);
            let left = a.max(rect.left());
            let right = b.min(rect.right());
            if right <= left {
                continue;
            }
            let interpolate = |x: Pixels| {
                pair[0].1 + (pair[1].1 - pair[0].1) * f32::from(x - a) / f32::from(b - a)
            };
            let mut first = color;
            first.a = interpolate(left);
            let mut last = color;
            last.a = interpolate(right);
            let gradient = linear_gradient(
                90.,
                linear_color_stop(
                    first,
                    f32::from(left - rect.left()) / f32::from(rect.size.width),
                ),
                linear_color_stop(
                    last,
                    f32::from(right - rect.left()) / f32::from(rect.size.width),
                ),
            );
            window.with_content_mask(
                Some(ContentMask {
                    bounds: Bounds::new(
                        point(left, rect.top()),
                        size(right - left, rect.size.height),
                    ),
                }),
                |window| {
                    if let Ok(path) = key.path(rect, false).build() {
                        window.paint_path(path, gradient);
                    }
                },
            );
        }
    }
}

/// CSS saving border spinner and SVG inactivity dash ring, painted natively.
pub(super) fn status_ring(
    saving: bool,
    started: Option<Instant>,
    seconds: u32,
    reduced: bool,
) -> AnyElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let elapsed = started.map_or(0., |at| at.elapsed().as_secs_f32());
            let turn = if saving {
                if reduced { 0. } else { elapsed.fract() }
            } else {
                0.
            };
            let fraction = if saving {
                0.25
            } else {
                ((seconds as f32 - elapsed) / 15.).clamp(0., 1.)
            };
            let scale = f32::from(bounds.size.width) / 24.;
            let center = bounds.center();
            let radius = if saving { 10.5 } else { 10. } * scale;
            for (from, length, color) in [
                (0., 1., rgba(0x44d62c4d)),
                (
                    turn - if saving { 0.375 } else { 0.25 },
                    fraction,
                    rgba(0x44d62cff),
                ),
            ] {
                let mut path = PathBuilder::stroke(px(3. * scale));
                let count = (length * 128.).ceil().max(1.) as usize;
                for index in 0..=count {
                    let angle =
                        (from + length * index as f32 / count as f32) * std::f32::consts::TAU;
                    let point = center + point(px(angle.cos() * radius), px(angle.sin() * radius));
                    if index == 0 {
                        path.move_to(point);
                    } else {
                        path.line_to(point);
                    }
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            }
            if !reduced {
                window.request_animation_frame();
            }
        },
    )
    .size(surface::css(24.))
    .flex_shrink_0()
    .into_any_element()
}
