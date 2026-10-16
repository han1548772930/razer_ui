//! Current 801 stage/mapping slider and 70130 BASIC DPI slider.
use super::*;
use gpui_kit::base::{Slider as BaseSlider, SliderIndicator, SliderTrack};
use razer_widgets::source_slider::source_thumb;
use razer_widgets::theme::{MouseMappingColors, SliderColors};
use std::{cell::Cell, rc::Rc};

struct Pointer {
    focus: FocusHandle,
    show_tip: bool,
    pressed: bool,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    tip_width: Rc<Cell<Pixels>>,
}
#[derive(IntoElement)]
pub(super) struct SensitivitySlider {
    slider: Entity<SliderState>,
    minimum: u32,
    maximum: u32,
    axis: Option<&'static str>,
    tags: bool,
    enabled: bool,
    stage_row: bool,
    single_dpi: bool,
}
impl SensitivitySlider {
    pub(super) fn new(
        slider: &Entity<SliderState>,
        minimum: u32,
        maximum: u32,
        axis: Option<&'static str>,
        tags: bool,
    ) -> Self {
        Self {
            slider: slider.clone(),
            minimum,
            maximum,
            axis,
            tags,
            enabled: true,
            stage_row: false,
            single_dpi: false,
        }
    }
    pub(super) fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    /// Current `.stage .slider-container` centers the 801 track and thumb
    /// inside a 20px row and positions its X/Y tip 25px above that row.
    pub(super) fn stage_row(mut self) -> Self {
        self.stage_row = true;
        self
    }
    /// PID199/208's mounted BASIC branch imports module 70130. Its value tip
    /// is always visible, its fill follows the thumb inset, and Home/End and
    /// arrows are prevented. It does not register keyboard/wheel step handlers.
    pub(super) fn single_dpi(mut self) -> Self {
        self.single_dpi = true;
        self
    }
}
impl RenderOnce for SensitivitySlider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let pointer = window.use_keyed_state(
            ("mouse-sensitivity-slider-pointer", self.slider.entity_id()),
            cx,
            |_, cx| Pointer {
                focus: cx.focus_handle().tab_stop(true),
                show_tip: false,
                pressed: false,
                bounds: Rc::default(),
                tip_width: Rc::default(),
            },
        );
        let state = pointer.read(cx);
        let bounds = state.bounds.clone();
        let tip_width = state.tip_width.clone();
        let show_tip = state.show_tip;
        let focus = state.focus.clone();
        let measured_tip_width = if tip_width.get() > px(0.) {
            tip_width.get()
        } else {
            surface::css(24.).to_pixels(window.rem_size())
        };
        let value = self.slider.read(cx).value().start();
        let progress =
            ((value - self.minimum as f32) / (self.maximum - self.minimum) as f32).clamp(0., 1.);
        let single_dpi = self.single_dpi;
        let enabled = self.enabled;
        let opacity = if single_dpi {
            surface::fade_opacity(
                ("mouse-single-dpi-opacity", self.slider.entity_id()),
                if enabled { 1. } else { 0.3 },
                300,
                window,
                cx,
            )
        } else {
            1.
        };
        let axis = self.axis;
        let observed_bounds = bounds.clone();
        let mut root = div()
            .id(("mouse-sensitivity-slider", self.slider.entity_id()))
            .relative()
            .w_full()
            .h(surface::css(if single_dpi {
                64.
            } else if self.stage_row {
                20.
            } else {
                36.
            }))
            .opacity(opacity)
            .when(enabled, |root| root.track_focus(&focus))
            .on_key_down(move |event, _, cx| {
                if matches!(
                    event.keystroke.key.as_str(),
                    "left" | "right" | "up" | "down"
                ) || single_dpi && matches!(event.keystroke.key.as_str(), "home" | "end")
                {
                    cx.stop_propagation();
                }
            })
            .on_mouse_move(window.listener_for(
                &pointer,
                move |state, event: &MouseMoveEvent, window, cx| {
                    if !enabled || axis.is_none() || state.pressed {
                        return;
                    }
                    let measured = state.bounds.get();
                    let width = if state.tip_width.get() > px(0.) {
                        state.tip_width.get()
                    } else {
                        surface::css(24.).to_pixels(window.rem_size())
                    };
                    let inset = surface::css(8.).to_pixels(window.rem_size());
                    let position =
                        (measured.size.width - inset * 2.) * progress - width / 2. + inset;
                    let x = event.position.x - measured.origin.x;
                    state.show_tip = x >= position + surface::css(3.).to_pixels(window.rem_size())
                        && x <= position + surface::css(19.).to_pixels(window.rem_size());
                    cx.notify();
                },
            ))
            .capture_any_mouse_down(window.listener_for(
                &pointer,
                move |state, event: &MouseDownEvent, window, cx| {
                    if enabled && event.button == MouseButton::Left {
                        state.focus.focus(window, cx);
                        if axis.is_some() {
                            state.pressed = true;
                            state.show_tip = true;
                            cx.notify();
                        }
                    }
                },
            ))
            .on_mouse_up(
                MouseButton::Left,
                window.listener_for(&pointer, |state, _, _, cx| {
                    state.pressed = false;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                window.listener_for(&pointer, |state, _, _, cx| {
                    state.pressed = false;
                    cx.notify();
                }),
            )
            .on_hover(window.listener_for(&pointer, |state, hovered, _, cx| {
                if !hovered {
                    state.show_tip = false;
                    cx.notify();
                }
            }))
            .child(
                canvas(
                    move |bounds, _, _| {
                        observed_bounds.set(bounds);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .bottom(surface::css(if self.stage_row { 7. } else { 25. }))
                    .w_full()
                    .h(surface::css(6.))
                    .rounded(surface::css(3.))
                    .bg(SliderColors::track()),
            );
        // Module 70130 uses `calc(8px + p*(100% - 16px))`; 801 uses 100*p%.
        root = root.child(if single_dpi {
            div()
                .absolute()
                .bottom(surface::css(25.))
                .w_full()
                .h(surface::css(6.))
                .child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, _, window, _| {
                            let inset = surface::css(8.).to_pixels(window.rem_size());
                            let width = inset + (bounds.size.width - inset * 2.) * progress;
                            window.paint_quad(PaintQuad {
                                corner_radii: Corners::all(
                                    surface::css(3.).to_pixels(window.rem_size()),
                                ),
                                ..fill(
                                    Bounds::new(bounds.origin, size(width, bounds.size.height)),
                                    SliderColors::fill(),
                                )
                            });
                        },
                    )
                    .size_full(),
                )
        } else {
            div()
                .absolute()
                .bottom(surface::css(if self.stage_row { 7. } else { 25. }))
                .w(relative(progress))
                .h(surface::css(6.))
                .rounded(surface::css(3.))
                .bg(SliderColors::fill())
        });
        if single_dpi {
            root = root
                .child(
                    h_flex()
                        .absolute()
                        .bottom(surface::css(-2.))
                        .w_full()
                        .justify_between()
                        .text_size(surface::css(12.))
                        .line_height(surface::css(14.))
                        .child(t("SLOW").to_uppercase())
                        .child(t("FAST").to_uppercase()),
                )
                .child(
                    div()
                        .absolute()
                        .bottom(surface::css(42.))
                        .left(relative(progress))
                        .ml(surface::css(8. - 16. * progress))
                        .w_0()
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .flex_shrink_0()
                                .px(surface::css(8.))
                                .py(surface::css(4.))
                                .rounded(surface::css(3.))
                                .text_size(surface::css(12.))
                                .line_height(surface::css(14.))
                                .bg(SliderColors::fill())
                                .text_color(SliderColors::tip_text())
                                .child((value as u32).to_string()),
                        ),
                );
        }
        if self.tags {
            root = root.child(
                h_flex()
                    .absolute()
                    .bottom(surface::css(-2.))
                    .w_full()
                    .justify_between()
                    .text_size(surface::css(14.))
                    .text_color(MouseMappingColors::description())
                    .child(self.minimum.to_string())
                    .child(self.maximum.to_string()),
            );
        }
        if let Some(axis) = axis {
            let tip_width_observer = tip_width.clone();
            root = root
                .child(
                    div()
                        .absolute()
                        .bottom(surface::css(if self.stage_row { 25. } else { 42. }))
                        .left(relative(progress))
                        .ml(surface::css(8. - 16. * progress))
                        .w_0()
                        .flex()
                        .justify_center()
                        .opacity(if show_tip { 1. } else { 0. })
                        .child(
                            div()
                                .flex_shrink_0()
                                .px(surface::css(8.))
                                .py(surface::css(4.))
                                .rounded(surface::css(3.))
                                .text_size(surface::css(12.))
                                .line_height(surface::css(14.))
                                .bg(SliderColors::fill())
                                .text_color(SliderColors::tip_text())
                                .relative()
                                .child(axis)
                                .child(
                                    canvas(
                                        move |bounds, _, _| {
                                            tip_width_observer.set(bounds.size.width);
                                        },
                                        |_, _, _, _| {},
                                    )
                                    .absolute()
                                    .size_full(),
                                ),
                        ),
                )
                .child(
                    div()
                        .absolute()
                        .top(surface::css(if self.stage_row { 2. } else { 0. }))
                        .left(relative(progress))
                        .ml(
                            surface::css(8. - 16. * progress + 3.5).to_pixels(window.rem_size())
                                - measured_tip_width / 2.,
                        )
                        .w(surface::css(16.))
                        .h(surface::css(16.))
                        .text_center()
                        .text_size(surface::css(12.))
                        .line_height(surface::css(16.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(SliderColors::tip_text())
                        .opacity(if show_tip { 0. } else { 1. })
                        .child(axis),
                );
        }
        root.child(
            BaseSlider::new(&self.slider)
                .disabled(!self.enabled)
                .absolute()
                .size_full()
                .child(
                    SliderTrack::new(&self.slider)
                        .disabled(!enabled)
                        .absolute()
                        .bottom(surface::css(if self.stage_row { 2. } else { 20. }))
                        .w_full()
                        .h(surface::css(16.))
                        .child(
                            SliderIndicator::new(&self.slider)
                                .absolute()
                                .left(surface::css(8.))
                                .right(surface::css(8.))
                                .h_full()
                                .child(
                                    source_thumb(&self.slider, enabled, window, cx)
                                        .absolute()
                                        .left(relative(progress))
                                        .ml(surface::css(-8.))
                                        .size(surface::css(16.))
                                        .rounded(surface::css(8.)),
                                ),
                        ),
                ),
        )
    }
}
