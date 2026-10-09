//! Source-declared segmented DPI ranges: percentage geometry, previews and release commits.
use gpui_kit::base::{Slider as BaseSlider, SliderIndicator, SliderTrack};
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_widgets::source_slider::source_thumb;
use razer_widgets::surface;
use razer_widgets::theme::SliderColors;
use serde::Deserialize;
use std::{cell::Cell, rc::Rc, sync::OnceLock};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Segment {
    from: f64,
    to: f64,
    from_value: f64,
    to_value: f64,
    step: f64,
    #[serde(default)]
    tracks: Vec<f64>,
}
#[derive(Deserialize)]
pub(super) struct GridSpec {
    product_id: u32,
    min: f64,
    max: f64,
    step: f64,
    editing_enabled_default: bool,
    segments: Vec<Segment>,
}
pub(super) fn source_spec(product_id: u32) -> Option<&'static GridSpec> {
    static SPECS: OnceLock<Vec<GridSpec>> = OnceLock::new();
    SPECS
        .get_or_init(|| {
            serde_json::from_str(include_str!("mouse_dpi_grid_data.json"))
                .expect("audited segmented DPI capabilities")
        })
        .iter()
        .find(|spec| spec.product_id == product_id)
}
impl GridSpec {
    /// The current reducer's UI initial value, never a successful runtime query.
    pub(super) fn editing_enabled_default(&self) -> bool {
        self.editing_enabled_default
    }

    fn value_at_percent(&self, percent: f64) -> f32 {
        let percent = percent.clamp(0., 100.);
        let segment = self
            .segments
            .iter()
            .find(|segment| percent <= segment.to)
            .expect("audited DPI segments cover the percentage domain");
        (((percent - segment.from) / segment.step).round() * self.step + segment.from_value)
            .clamp(self.min, self.max) as f32
    }

    fn percent_at_value(&self, value: f32) -> (f32, f32) {
        let value = (value as f64).clamp(self.min, self.max);
        let segment = self
            .segments
            .iter()
            .find(|segment| value <= segment.to_value)
            .expect("audited DPI segments cover the product range");
        (
            ((value - segment.from_value) / self.step * segment.step + segment.from) as f32,
            segment.step as f32,
        )
    }
}
pub(super) enum Edited {
    Preview(f32),
    Commit(f32),
}
pub(super) struct GridState {
    spec: &'static GridSpec,
    position: Entity<SliderState>,
    focus: FocusHandle,
    value: f32,
    row_value: f32,
    expected_position: f32,
    pending: bool,
    pressed: bool,
    hovered: bool,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    tip_width: Rc<Cell<Pixels>>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<Edited> for GridState {}
impl GridState {
    pub(super) fn new(
        spec: &'static GridSpec,
        model: &Entity<SliderState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let model_state = model.read(cx);
        assert_eq!(
            (
                model_state.min_value(),
                model_state.max_value(),
                model_state.step_value()
            ),
            (spec.min as f32, spec.max as f32, spec.step as f32),
            "segmented DPI capability must match the product's numeric model"
        );
        let value = model_state.value().start();
        let (percent, step) = spec.percent_at_value(value);
        let position = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(100.)
                .step(step)
                .default_value(percent)
        });
        let subscriptions = vec![
            cx.observe_in(model, window, |this, model, window, cx| {
                let value = model.read(cx).value().start();
                if this.row_value != value {
                    this.row_value = value;
                    this.value = value;
                    this.sync_position(window, cx);
                    cx.notify();
                }
            }),
            cx.subscribe_in(&position, window, |this, position, event, window, cx| {
                match event {
                    SliderEvent::Change(percent) => {
                        this.pending = true;
                        let bounded = percent.start().clamp(0., 100.);
                        this.expected_position = bounded;
                        this.value = this.spec.value_at_percent(bounded as f64);
                        // The HTML control uses the previous step for this event,
                        // then Ft changes customStep for the newly entered segment.
                        let step = this
                            .spec
                            .segments
                            .iter()
                            .find(|segment| bounded as f64 <= segment.to)
                            .unwrap()
                            .step as f32;
                        position.update(cx, |position, cx| {
                            *position = std::mem::replace(position, SliderState::new()).step(step);
                            if bounded != percent.start() {
                                position.set_value(bounded, window, cx);
                            }
                        });
                        cx.emit(Edited::Preview(this.value));
                    }
                    SliderEvent::Release(_) => {
                        this.release(cx);
                    }
                }
                cx.notify();
            }),
            cx.observe_in(&position, window, |this, position, _, cx| {
                let value = position.read(cx).value().start();
                if value == this.expected_position {
                    return;
                }
                this.expected_position = value;
                if this.pending || this.pressed {
                    return;
                }
                // Base accessibility increment/decrement changes position and
                // notifies without a SliderEvent. Programmatic synchronization
                // records its expected position before set_value; only an
                // independent semantic adjustment reaches this commit path.
                this.value = this.spec.value_at_percent(value as f64);
                cx.emit(Edited::Commit(this.value));
                cx.notify();
            }),
        ];
        Self {
            spec,
            position,
            focus: cx.focus_handle(),
            value,
            row_value: value,
            expected_position: percent,
            pending: false,
            pressed: false,
            hovered: false,
            bounds: Rc::new(Cell::new(Bounds::default())),
            tip_width: Rc::new(Cell::new(px(0.))),
            _subscriptions: subscriptions,
        }
    }
    fn release(&mut self, cx: &mut Context<Self>) {
        self.pressed = false;
        if std::mem::take(&mut self.pending) {
            cx.emit(Edited::Commit(self.value));
        }
        cx.notify();
    }
    fn sync_position(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (percent, step) = self.spec.percent_at_value(self.value);
        self.expected_position = percent;
        self.position.update(cx, |position, cx| {
            *position = std::mem::replace(position, SliderState::new()).step(step);
            position.set_value(percent, window, cx);
        });
    }
    pub(super) fn reset(&mut self, value: f32, window: &mut Window, cx: &mut Context<Self>) {
        self.pending = false;
        self.pressed = false;
        self.hovered = false;
        self.value = value;
        self.row_value = value;
        self.sync_position(window, cx);
        cx.notify();
    }
    fn pointer(&mut self, x: Pixels, cx: &mut Context<Self>) {
        let bounds = self.bounds.get();
        let scale = bounds.size.width / 300.;
        let percent = self.position.read(cx).value().start() / 100.;
        let left =
            percent * (bounds.size.width - scale * 16.) - self.tip_width.get() / 2. + scale * 8.;
        let x = x - bounds.origin.x;
        let hovered = x >= left + scale * 3. && x <= left + scale * 19.;
        if hovered != self.hovered {
            self.hovered = hovered;
            cx.notify();
        }
    }
}

#[derive(IntoElement)]
pub(super) struct Grid {
    state: Entity<GridState>,
    enabled: bool,
    tag: Option<&'static str>,
}
impl Grid {
    pub(super) fn new(state: &Entity<GridState>, enabled: bool, tag: Option<&'static str>) -> Self {
        Self {
            state: state.clone(),
            enabled,
            tag,
        }
    }
}
impl RenderOnce for Grid {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let position = state.position.clone();
        let focus = state.focus.clone();
        let progress = position.read(cx).value().start() / 100.;
        let tip_visible = self.enabled && state.hovered;
        let bounds = state.bounds.clone();
        let tip_width = state.tip_width.clone();
        let enabled = self.enabled;
        let ticks = std::iter::once((0., Some(state.spec.min as u32))).chain(
            state.spec.segments.iter().flat_map(|segment| {
                segment
                    .tracks
                    .iter()
                    .map(|p| (*p, None))
                    .chain(std::iter::once((segment.to, Some(segment.to_value as u32))))
            }),
        );
        let mut root = BaseSlider::new(&position)
            .disabled(!enabled)
            .relative()
            .w(surface::css(300.))
            .h(surface::css(20.))
            .ml(surface::css(10.))
            .flex_shrink_0()
            .child(
                canvas(move |area, _, _| bounds.set(area), |_, _, _, _| ())
                    .absolute()
                    .size_full(),
            )
            .children(ticks.map(|(percent, label)| {
                div()
                    .absolute()
                    .left(relative(percent as f32 / 100.))
                    .ml(surface::css(-0.5))
                    .top(surface::css(2.))
                    .w(surface::css(1.))
                    .h(surface::css(16.))
                    .bg(razer_widgets::theme::DpiColors::tick())
                    .child(
                        div()
                            .absolute()
                            .top(surface::css(24.))
                            .w_0()
                            .flex()
                            .when(percent == 0., |view| view.justify_start())
                            .when(percent == 100., |view| view.justify_end())
                            .when(percent > 0. && percent < 100., |view| view.justify_center())
                            .children(label.map(|value| {
                                div()
                                    .flex_shrink_0()
                                    .text_size(surface::css(11.))
                                    .text_color(razer_widgets::theme::DpiColors::text())
                                    .child(value.to_string())
                            })),
                    )
            }));
        root = root
            .child(
                div()
                    .absolute()
                    .top(surface::css(7.))
                    .w_full()
                    .h(surface::css(6.))
                    .bg(SliderColors::track()),
            )
            .child(
                div()
                    .absolute()
                    .top(surface::css(7.))
                    .w(surface::css(8. + progress * 284.))
                    .h(surface::css(6.))
                    .bg(SliderColors::fill()),
            );
        let input = div()
            .id(("dpi-grid-input", position.entity_id()))
            .absolute()
            .top(surface::css(7.))
            .w_full()
            .h(surface::css(6.))
            .on_mouse_up(
                MouseButton::Left,
                window.listener_for(&self.state, |state, _, _, cx| state.release(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                window.listener_for(&self.state, |state, _, _, cx| state.release(cx)),
            )
            .on_hover(
                window.listener_for(&self.state, move |state, hovered, window, cx| {
                    if enabled && *hovered {
                        state.pointer(window.mouse_position().x, cx);
                    } else {
                        state.hovered = false;
                        cx.notify();
                    }
                }),
            )
            .child(
                SliderTrack::new(&position)
                    .disabled(!enabled)
                    .absolute()
                    .top_0()
                    .w_full()
                    .h(surface::css(6.))
                    .capture_any_mouse_down(window.listener_for(
                        &self.state,
                        move |state, event: &MouseDownEvent, window, cx| {
                            if enabled && event.button == MouseButton::Left {
                                state.focus.focus(window, cx);
                                state.pending = true;
                                state.pressed = true;
                                state.hovered = true;
                                cx.notify();
                            }
                        },
                    ))
                    .on_mouse_move(window.listener_for(
                        &self.state,
                        move |state, event: &MouseMoveEvent, _, cx| {
                            if enabled && !state.pressed {
                                state.pointer(event.position.x, cx);
                            }
                        },
                    ))
                    .child(
                        SliderIndicator::new(&position)
                            .absolute()
                            .left(surface::css(6.))
                            .right(surface::css(6.))
                            .h_full()
                            .child(
                                source_thumb(&position, enabled, window, cx)
                                    .absolute()
                                    .top(surface::css(-3.))
                                    .left(relative(progress))
                                    .ml(surface::css(-6.))
                                    .size(surface::css(12.))
                                    .rounded_full(),
                            ),
                    ),
            );
        root = root.child(input);
        if let Some(tag) = self.tag {
            root = root
                .child(
                    div()
                        .absolute()
                        .left(surface::css(3.1 + progress * 287.5))
                        .top_0()
                        .text_color(SliderColors::tip_text())
                        .text_size(surface::css(10.))
                        .font_weight(FontWeight::BOLD)
                        .line_height(surface::css(22.))
                        .opacity(if tip_visible { 0. } else { 1. })
                        .child(tag),
                )
                .child(
                    div()
                        .absolute()
                        .bottom(surface::css(25.))
                        .left(relative(progress))
                        .ml(surface::css(8. - 16. * progress))
                        .w_0()
                        .flex()
                        .justify_center()
                        .opacity(if tip_visible { 1. } else { 0. })
                        .child(
                            div()
                                .relative()
                                .flex_shrink_0()
                                .px(surface::css(8.))
                                .py(surface::css(4.))
                                .rounded(surface::css(3.))
                                .bg(SliderColors::fill())
                                .text_color(SliderColors::tip_text())
                                .text_size(surface::css(12.))
                                .line_height(surface::css(14.))
                                .child(tag)
                                .child(
                                    canvas(
                                        move |area, _, _| tip_width.set(area.size.width),
                                        |_, _, _, _| (),
                                    )
                                    .absolute()
                                    .size_full(),
                                ),
                        ),
                );
        }
        div()
            .id(("dpi-grid", position.entity_id()))
            .track_focus(&focus)
            .tab_stop(enabled)
            .flex_shrink_0()
            .on_key_down(|event, _, cx| {
                if matches!(
                    event.keystroke.key.as_str(),
                    "left" | "right" | "up" | "down"
                ) {
                    cx.stop_propagation();
                }
            })
            .child(root)
    }
}
