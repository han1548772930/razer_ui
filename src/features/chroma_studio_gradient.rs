//! Current Spectrum 8552:b and shared gradient selector 3690/99/1592.
use super::studio_color::{StudioColor, StudioColorEvent, checkered};
use super::studio_gradient_data::{Stop, bar, pack, sample, serialize};
use super::*;
use gpui_kit::base::Popover;
use std::{cell::Cell, rc::Rc, time::Duration};

pub(super) struct GradientChanged {
    pub(super) stops: Value,
    pub(super) custom: bool,
}
impl EventEmitter<GradientChanged> for StudioGradient {}

pub(super) struct StudioGradient {
    stops: Vec<Stop>,
    custom: bool,
    custom_cache: Option<Vec<Stop>>,
    selected: Option<u64>,
    next_id: u64,
    enabled: bool,
    open: bool,
    revealed: bool,
    closing: bool,
    delay: Option<Task<()>>,
    drag: Option<(u64, f32, f32)>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    color: Entity<StudioColor>,
    _subscriptions: Vec<Subscription>,
}

impl StudioGradient {
    pub(super) fn custom_cache(&self) -> Option<Vec<Stop>> {
        self.custom_cache.clone()
    }
    pub(super) fn restore_cache(&mut self, cache: Option<Vec<Stop>>) {
        if let Some(stops) = &cache {
            self.next_id = stops
                .iter()
                .map(|stop| stop.id)
                .max()
                .map(|id| id + 1)
                .unwrap_or(0);
        }
        self.custom_cache = cache;
    }
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let color = cx.new(|cx| StudioColor::new_dropdown(None, window, cx));
        let subscriptions = vec![
            cx.observe(&color, |this, color, cx| {
                if !this.enabled || !this.open || this.closing {
                    return;
                }
                let value = color.read(cx).value();
                let Some(stop) = this
                    .stops
                    .iter_mut()
                    .find(|stop| Some(stop.id) == this.selected)
                else {
                    return;
                };
                // Owner sync changes the stop first, so it cannot feed back as
                // a user edit. Continuous color changes only affect the draft.
                if stop.color != value {
                    stop.color = value;
                    this.remember_custom();
                    cx.notify();
                }
            }),
            cx.subscribe(&color, |this, _, event, cx| {
                if !this.enabled || !this.open || this.closing {
                    return;
                }
                let StudioColorEvent::Changed(value) = event;
                if let Some(stop) = this
                    .stops
                    .iter_mut()
                    .find(|stop| Some(stop.id) == this.selected)
                {
                    stop.color = *value;
                    this.remember_custom();
                    this.commit(cx);
                }
            }),
            cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    // 1592 showOnBlur keeps the picker; 3690 clears dragging.
                    this.drag = None;
                    cx.notify();
                }
            }),
        ];
        Self {
            stops: Vec::new(),
            custom: false,
            custom_cache: None,
            selected: None,
            next_id: 0,
            enabled: false,
            open: false,
            revealed: false,
            closing: false,
            delay: None,
            drag: None,
            bounds: Rc::new(Cell::new(Bounds::default())),
            color,
            _subscriptions: subscriptions,
        }
    }

    pub(super) fn configure(
        &mut self,
        value: &Value,
        custom: bool,
        enabled: bool,
        replace: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.enabled = enabled;
        if replace {
            self.stops.clear();
            if let Some(values) = value.as_array() {
                for value in values {
                    let Some(position) = value["Stop"].as_f64() else {
                        continue;
                    };
                    self.stops.push(Stop {
                        id: self.next_id,
                        position: position as f32 / 100.,
                        color: value["Color"].as_u64().and_then(|c| u32::try_from(c).ok()),
                    });
                    self.next_id += 1;
                }
            }
            self.stops.sort_by(|a, b| a.position.total_cmp(&b.position));
            self.custom = custom;
            self.selected = self.stops.first().map(|stop| stop.id);
            self.drag = None;
        }
        if !enabled {
            self.set_open(false, window, cx);
        }
        self.sync_color(window, cx);
        self.color
            .update(cx, |color, cx| color.set_enabled(enabled, window, cx));
        cx.notify();
    }

    fn sync_color(&self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self
            .stops
            .iter()
            .find(|stop| Some(stop.id) == self.selected)
            .and_then(|stop| stop.color);
        self.color
            .update(cx, |color, cx| color.set_value(value, window, cx));
    }

    fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        if open && !self.enabled {
            return;
        }
        if open {
            if self.open && !self.closing {
                return;
            }
            self.open = true;
            self.selected = self.stops.first().map(|stop| stop.id);
            self.sync_color(window, cx);
        } else if !self.open {
            return;
        }
        self.closing = !open;
        self.revealed = false;
        self.drag = None;
        // 1592 mounts at height zero, reveals after 100ms, and retains a
        // zero-height closing surface for another 100ms before unmounting.
        self.delay = Some(cx.spawn(async move |owner, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let _ = owner.update(cx, |this, cx| {
                this.open = open;
                this.revealed = open;
                this.closing = false;
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn remember_custom(&mut self) {
        self.custom = true;
        self.custom_cache = Some(self.stops.clone());
    }
    fn commit(&self, cx: &mut Context<Self>) {
        cx.emit(GradientChanged {
            stops: serialize(&self.stops),
            custom: self.custom,
        });
        cx.notify();
    }

    fn preset(&mut self, index: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled {
            return;
        }
        if let Some(index) = index {
            self.stops = source().gradients["spectrum"].presets[index]
                .iter()
                .map(|point| {
                    let id = self.next_id;
                    self.next_id += 1;
                    Stop {
                        id,
                        position: point.stop,
                        color: point.rgb.map(pack),
                    }
                })
                .collect();
            self.custom = false;
        } else if let Some(stops) = &self.custom_cache {
            self.stops = stops.clone();
            self.custom = true;
        }
        self.selected = self.stops.first().map(|stop| stop.id);
        self.sync_color(window, cx);
        self.commit(cx);
    }

    fn add(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled
            || self.stops.is_empty()
            || self.stops.len() >= source().gradients["spectrum"].max_stops
        {
            return;
        }
        let bounds = self.bounds.get();
        let x = (f32::from(position.x - bounds.left()) / f32::from(bounds.size.width) * 180.)
            .clamp(0., 179.999);
        let rgba = sample(&self.stops, (x.floor() + 0.5) / 180.);
        let color = pack([rgba.r, rgba.g, rgba.b].map(|c| (c * 255.).round() as u8));
        let id = self.next_id;
        self.next_id += 1;
        self.stops.push(Stop {
            id,
            position: (x / 180. * 100.).round() / 100.,
            color: Some(color),
        });
        self.stops.sort_by(|a, b| a.position.total_cmp(&b.position));
        self.selected = Some(id);
        self.remember_custom();
        self.sync_color(window, cx);
        self.commit(cx);
    }

    fn remove(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled || self.stops.len() <= source().gradients["spectrum"].min_stops {
            return;
        }
        self.stops.retain(|stop| Some(stop.id) != self.selected);
        self.selected = self.stops.last().map(|stop| stop.id);
        self.remember_custom();
        self.sync_color(window, cx);
        self.commit(cx);
    }

    fn begin_drag(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled {
            return;
        }
        let Some(index) = self.stops.iter().position(|stop| stop.id == id) else {
            return;
        };
        self.selected = Some(id);
        let gap = (6_f32 / (180. - 6.) * 100.).ceil() / 100.;
        let min = index
            .checked_sub(1)
            .map(|i| self.stops[i].position + gap)
            .unwrap_or(0.);
        let max = self
            .stops
            .get(index + 1)
            .map(|stop| stop.position - gap)
            .unwrap_or(1.);
        self.drag = Some((id, min, max));
        self.sync_color(window, cx);
        cx.notify();
    }
    fn move_drag(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if !self.enabled {
            return;
        }
        let Some((id, min, max)) = self.drag else {
            return;
        };
        let bounds = self.bounds.get();
        let normalized =
            (f32::from(position.x - bounds.left()) / f32::from(bounds.size.width)).clamp(0., 1.);
        let position = ((normalized * 100.).round() / 100.).max(min).min(max);
        if let Some(stop) = self.stops.iter_mut().find(|stop| stop.id == id) {
            stop.position = position;
            self.remember_custom();
            cx.notify();
        }
    }
    fn finish_drag(&mut self, cx: &mut Context<Self>) {
        if self.drag.take().is_some() && self.enabled {
            self.custom = true;
            self.commit(cx);
        }
    }

    fn strip(stops: Vec<Stop>) -> Div {
        div()
            .relative()
            .size_full()
            .child(checkered())
            .child(div().absolute().inset_0().child(bar(stops)))
    }

    fn editor(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let definition = &source().gradients["spectrum"];
        let mut patterns: Vec<_> = definition
            .presets
            .iter()
            .enumerate()
            .map(|(index, points)| {
                let stops = points
                    .iter()
                    .enumerate()
                    .map(|(id, p)| Stop {
                        id: id as u64,
                        position: p.stop,
                        color: p.rgb.map(pack),
                    })
                    .collect::<Vec<_>>();
                let active = !self.custom && serialize(&stops) == serialize(&self.stops);
                (Some(index), stops, active)
            })
            .collect();
        if let Some(custom) = &self.custom_cache {
            patterns.push((None, custom.clone(), self.custom));
        }
        let pattern_count = patterns.len();
        let presets =
            div()
                .flex()
                .mb(surface::css(10.))
                .children(
                    patterns
                        .into_iter()
                        .enumerate()
                        .map(|(row, (index, stops, active))| {
                            let name = index
                                .map(|i| format!("Pattern {} Selected", i + 1))
                                .unwrap_or_else(|| "Custom Pattern Selected".into());
                            div()
                                .flex_shrink_0()
                                .when(row + 1 < pattern_count, |view| view.mr(surface::css(10.)))
                                .child(super::studio_gradient_preset::GradientPreset::new(
                                    ("studio-gradient-preset", index.unwrap_or(usize::MAX)).into(),
                                    name,
                                    stops,
                                    active,
                                    self.enabled,
                                    cx.listener(move |this, _, window, cx| {
                                        this.preset(index, window, cx)
                                    }),
                                ))
                        }),
                );
        let bounds = self.bounds.clone();
        let owner = cx.weak_entity();
        let dragging = self.drag.is_some();
        let mut track = div()
            .id("studio-gradient-track")
            .relative()
            .w(surface::css(180.))
            .h(surface::css(16.))
            .child(Self::strip(self.stops.clone()))
            .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                this.add(event.position(), window, cx)
            }))
            .child(
                canvas(
                    move |rect, _, _| bounds.set(rect),
                    move |_, _, window, _| {
                        if !dragging {
                            return;
                        }
                        let moving = owner.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                            if phase == DispatchPhase::Bubble {
                                let _ = moving.update(cx, |this, cx| {
                                    if event.dragging() {
                                        this.move_drag(event.position, cx);
                                    } else {
                                        this.finish_drag(cx);
                                    }
                                });
                            }
                        });
                        let release = owner.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                            if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                                let _ = release.update(cx, |this, cx| this.finish_drag(cx));
                            }
                        });
                    },
                )
                .absolute()
                .inset_0(),
            );
        let mut previous = None;
        for stop in &self.stops {
            let id = stop.id;
            let mut left = stop.position * 174.;
            if let Some(prior) = previous {
                if left - prior - 6. < 1. {
                    left = prior + 7.;
                }
            }
            previous = Some(left);
            track = track.child(
                BaseButton::new(("studio-gradient-stop", id))
                    .accessibility_label(format!(
                        "Gradient stop {}%",
                        (stop.position * 100.).round()
                    ))
                    .absolute()
                    .left(surface::css(left))
                    .top_0()
                    .p_0()
                    .w(surface::css(6.))
                    .h(surface::css(16.))
                    .border_1()
                    .border_color(Colors::white())
                    .bg(transparent_black())
                    .shadow(vec![BoxShadow {
                        inset: false,
                        color: Colors::black(),
                        offset: point(px(0.), px(0.)),
                        blur_radius: px(0.),
                        spread_radius: surface::css(1.).to_pixels(window.rem_size()),
                    }])
                    .when(self.selected == Some(id), |view| {
                        view.child(
                            div()
                                .absolute()
                                .left(surface::css(-2.))
                                .top(surface::css(-7.))
                                .w(surface::css(8.))
                                .h(surface::css(4.))
                                .child(
                                    svg()
                                        .path("synapse/chroma-studio-brightness-pointer.svg")
                                        .size_full()
                                        .text_color(Colors::white()),
                                ),
                        )
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.begin_drag(id, window, cx);
                        }),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.selected = Some(id);
                        this.sync_color(window, cx);
                        cx.notify();
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if !this.enabled {
                            return;
                        }
                        match event.keystroke.key.as_str() {
                            "delete" | "backspace" => {
                                this.selected = Some(id);
                                this.remove(window, cx);
                            }
                            "left" | "right" | "home" | "end" => {
                                this.begin_drag(id, window, cx);
                                if let Some((_, min, max)) = this.drag {
                                    if let Some(stop) =
                                        this.stops.iter_mut().find(|stop| stop.id == id)
                                    {
                                        let next = match event.keystroke.key.as_str() {
                                            "left" => stop.position - 0.01,
                                            "right" => stop.position + 0.01,
                                            "home" => min,
                                            _ => max,
                                        };
                                        stop.position = next.max(min).min(max);
                                        this.remember_custom();
                                    }
                                    this.finish_drag(cx);
                                }
                            }
                            _ => return,
                        }
                        window.prevent_default();
                        cx.stop_propagation();
                    })),
            );
        }
        div()
            .flex()
            .flex_col()
            .p(surface::css(10.))
            .child(div().mb(surface::css(6.)).child(label("TEXT_PATTERNS")))
            .child(presets)
            .when(!self.stops.is_empty(), |view| {
                view.child(
                    div()
                        .h(surface::css(1.))
                        .my(surface::css(10.))
                        .bg(Colors::input_border()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .mb(surface::css(10.))
                        .child(
                            div()
                                .border_1()
                                .border_color(Colors::gradient_border())
                                .rounded(surface::css(3.))
                                .child(track),
                        )
                        .child(
                            button("studio-gradient-remove", "DELETE")
                                .disabled(self.stops.len() <= definition.min_stops)
                                .size(surface::css(20.))
                                .bg(transparent_black())
                                .hover(|style| {
                                    style.bg(transparent_black()).text_color(Colors::selected())
                                })
                                .child(
                                    svg()
                                        .path("synapse/chroma-studio-trash-white.svg")
                                        .size_full(),
                                )
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.remove(window, cx)),
                                ),
                        ),
                )
                .child(self.color.clone())
            })
            .into_any_element()
    }
}

impl Render for StudioGradient {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = self.editor(window, cx);
        let owner = cx.weak_entity();
        let revealed = self.revealed;
        let trigger = BaseButton::new("studio-gradient-trigger")
            .accessibility_label("Gradient Selector")
            .disabled(!self.enabled)
            .w_full()
            .h(surface::css(27.))
            .p(surface::css(4.))
            .bg(Colors::panel())
            .border_1()
            .border_color(if self.open {
                Colors::selected()
            } else {
                Colors::input_border()
            })
            .when(self.enabled, |view| {
                view.hover(|style| style.border_color(Colors::selected()))
                    .focus_visible(|style| style.border_color(Colors::selected()))
            })
            .child(
                div()
                    .w_full()
                    .h(surface::css(17.))
                    .border_1()
                    .border_color(Colors::gradient_border())
                    .rounded(surface::css(3.))
                    .overflow_hidden()
                    .child(Self::strip(self.stops.clone())),
            );
        Popover::new("studio-gradient-popover")
            .w_full()
            .mb(surface::css(10.))
            .anchor(Anchor::TopLeft)
            .offset(surface::css(2.).to_pixels(window.rem_size()))
            .open(self.open)
            .trigger(trigger)
            .on_open_change(move |open, window, cx| {
                let _ = owner.update(cx, |this, cx| this.set_open(*open, window, cx));
            })
            .content(move |_, _, _| {
                div()
                    .w(surface::css(230.))
                    .bg(Colors::black())
                    .border_color(Colors::input_border())
                    .border_x_1()
                    .when(revealed, |view| view.border_y_1())
                    .when(!revealed, |view| view.h_0().overflow_hidden())
                    .child(body)
            })
    }
}
