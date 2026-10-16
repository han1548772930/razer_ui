//! Shared drawing layer for the current source `OTA` slider (`.slider-container`).
//!
//! `STA` and the other `#multipleBrightness` controls all mount `OTA`, whose
//! audited markup is `.thumb-tag`, the `.foot` range tags, `.left`, `.track`,
//! `.slider-tip` and an `<input type=range class="slider">`.  The declarations
//! this layer reproduces verbatim:
//!
//! ```css
//! .slider-container{height:64px;opacity:.3;pointer-events:none;position:relative}
//! .slider-container.on{opacity:1;pointer-events:auto}
//! .slider-container.no-tip{height:36px}
//! .slider{background:#0000;border-radius:3px;bottom:25px;height:6px;width:100%}
//! .slider::-webkit-slider-thumb{background:#44d62c;border-radius:8px;height:16px;width:16px}
//! .slider-container.on .slider::-webkit-slider-thumb:hover{background:#5d5d5d;border:2px solid #44d62c}
//! .slider-container.on .slider::-webkit-slider-thumb:active{background:#383838;border:2px solid #44d62c}
//! .slider-container .left{background:#44d62c;border-radius:3px;bottom:25px;height:6px;position:absolute}
//! .slider-container .track{background:#44d62c4d;border-radius:3px;bottom:25px;height:6px;position:absolute;width:100%}
//! .slider-tip{background-color:#44d62c;border-radius:3px;bottom:42px;color:#212121;font-size:12px;
//!   line-height:14px;padding:4px 8px;width:max-content}
//! ```
//!
//! `OTA.updateValue()` places the fill at `calc(8px + p*(100% - 16px))` and the
//! tip at `p*(W - 16) - tipW/2 + 8`, i.e. centered on the thumb, whose center
//! travels between the 8px insets.  Pointer dragging and focus stay with the
//! gpui-kit base slider; this component only paints the source appearance.
//!
//! The source also mounts an empty `.thumb-tag` unless a caller supplies its
//! content. The local call sites use the ordinary value tip or `noTip` and do
//! not supply that alternate bubble. Its empty DOM box is not drawn here.
//!
//! `source-slider-current-evidence.json` binds the range components and CSS to
//! each current manifest. Container opacity and thumb background change over
//! 300ms CSS ease; pointer disabling and the hover/active border are immediate.

use gpui_kit::base::motion::{self, Easing, Interpolate, Transition};
use gpui_kit::base::{Slider as BaseSlider, SliderIndicator, SliderThumb, SliderTrack};
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::time::Duration;

use crate::surface;
use crate::theme::SliderColors;

#[derive(Default)]
struct ThumbPointer {
    hovered: bool,
    pressed: bool,
}

// Current 3594 FM/WM register window key/wheel listeners on click and remove
// them on blur. Keyboard focus alone does not register those listeners.
struct SourceSliderInput {
    focus: FocusHandle,
    registered: bool,
    _blur: Subscription,
}

fn source_slider_step(
    current: f32,
    min: f32,
    max: f32,
    step: f32,
    values: &[f32],
    increase: bool,
) -> f32 {
    let next = if values.is_empty() {
        current + if increase { step } else { -step }
    } else if let Some(index) = values.iter().position(|value| *value == current) {
        let next = if increase {
            index.saturating_add(1)
        } else {
            index.saturating_sub(1)
        };
        values.get(next).copied().unwrap_or(current)
    } else {
        current
    };
    // FM/WM round fractional keyboard/wheel values with toFixed(1).
    ((next.clamp(min, max) * 10.).round() / 10.).clamp(min, max)
}

// CSS interpolates the green/gray background channels, not their HSL hues.
// Base's Hsla interpolation is intended for near-grayscale endpoints only.
#[derive(Clone, PartialEq)]
struct ThumbBackground(Rgba);
impl Interpolate for ThumbBackground {
    fn interpolate(&self, target: &Self, progress: f32) -> Self {
        let mix = |a: f32, b: f32| a + (b - a) * progress;
        Self(Rgba {
            r: mix(self.0.r, target.0.r),
            g: mix(self.0.g, target.0.g),
            b: mix(self.0.b, target.0.b),
            a: mix(self.0.a, target.0.a),
        })
    }
}

/// Current `.slider::-webkit-slider-thumb`: only background has a changing
/// 300ms ease transition; the 2px green hover/active border changes immediately.
/// Base retains drag/focus ownership. Geometry belongs to the calling slider.
pub fn source_thumb(
    state: &Entity<SliderState>,
    enabled: bool,
    window: &mut Window,
    cx: &mut App,
) -> SliderThumb {
    let id = ElementId::from(("source-slider-thumb", state.entity_id()));
    let pointer =
        window.use_keyed_state((id.clone(), "pointer"), cx, |_, _| ThumbPointer::default());
    let interaction = pointer.read(cx);
    let hovered = enabled && interaction.hovered;
    let pressed = enabled && interaction.pressed;
    let target = if pressed {
        SliderColors::thumb_active()
    } else if hovered {
        SliderColors::thumb_hover()
    } else {
        SliderColors::thumb()
    };
    let background = motion::transition(
        (id, "background"),
        ThumbBackground(target.into()),
        Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
        window,
        cx,
    );
    SliderThumb::new(state)
        .disabled(!enabled)
        .bg(background.0)
        .when(hovered || pressed, |thumb| {
            thumb.border_2().border_color(SliderColors::thumb_border())
        })
        .on_hover(window.listener_for(&pointer, |pointer, hovered, _, cx| {
            if pointer.hovered != *hovered {
                pointer.hovered = *hovered;
                cx.notify();
            }
        }))
        // Thumb's base handler stops bubbling to prevent track repositioning.
        // Observe capture without consuming it or interfering with that handler.
        .capture_any_mouse_down(window.listener_for(
            &pointer,
            move |pointer, event: &MouseDownEvent, _, cx| {
                if enabled && event.button == MouseButton::Left && !pointer.pressed {
                    pointer.pressed = true;
                    cx.notify();
                }
            },
        ))
        .on_mouse_up(
            MouseButton::Left,
            window.listener_for(&pointer, |pointer, _, _, cx| {
                if pointer.pressed {
                    pointer.pressed = false;
                    cx.notify();
                }
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            window.listener_for(&pointer, |pointer, _, _, cx| {
                if pointer.pressed {
                    pointer.pressed = false;
                    cx.notify();
                }
            }),
        )
}

#[derive(IntoElement)]
pub struct SourceSlider {
    /// The base slider derives its element id from this entity, so every
    /// rendered slider keeps a stable id without a second identifier.
    state: Entity<SliderState>,
    /// `OTA.getPercent()`: the value's share of `min..max`.
    progress: f32,
    /// `.slider-tip` content; its presence is what keeps the container 64px
    /// tall instead of `.no-tip`'s 36px.
    tip: Option<SharedString>,
    /// `.slider-container.on`: 100% opacity and pointer events, else `.3` and none.
    enabled: bool,
    /// An ancestor may block input while leaving `.slider-container.on` intact.
    interaction_enabled: bool,
    range_labels: Option<(SharedString, SharedString, Hsla)>,
    track_gradient: Option<(Hsla, Hsla)>,
    fill_inset: bool,
    keyboard_steps: Option<Vec<f32>>,
    container_height: Option<f32>,
}
impl SourceSlider {
    pub fn new(state: &Entity<SliderState>, progress: f32) -> Self {
        Self {
            state: state.clone(),
            progress: progress.clamp(0., 1.),
            tip: None,
            enabled: true,
            interaction_enabled: true,
            range_labels: None,
            track_gradient: None,
            fill_inset: true,
            keyboard_steps: None,
            container_height: None,
        }
    }
    pub fn tip(mut self, tip: Option<impl Into<SharedString>>) -> Self {
        self.tip = tip.map(Into::into);
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    /// Source-specific CSS override, e.g. current PID1342
    /// `.wrapper-MixerVolumn .option-mic .slider-container { height:55px }`.
    pub fn container_height(mut self, css_pixels: f32) -> Self {
        self.container_height = Some(css_pixels);
        self
    }
    pub fn interaction_enabled(mut self, enabled: bool) -> Self {
        self.interaction_enabled = enabled;
        self
    }
    /// Source `.foot.min/.max`, kept inside the slider's disabled opacity.
    pub fn range_labels(
        mut self,
        minimum: impl Into<SharedString>,
        maximum: impl Into<SharedString>,
        color: Hsla,
    ) -> Self {
        self.range_labels = Some((minimum.into(), maximum.into(), color));
        self
    }
    /// Source `.gradient_slider`: a full track gradient with no green fill.
    pub fn track_gradient(mut self, start: Hsla, end: Hsla) -> Self {
        self.track_gradient = Some((start, end));
        self
    }
    /// Camera FM/WM paints `percent * 100%`; OTA instead follows the thumb's
    /// inset center. Keep the two source implementations distinct.
    pub fn proportional_fill(mut self) -> Self {
        self.fill_inset = false;
        self
    }
    /// Source FM steps normally; WM uses adjacent originData values on arrow
    /// keys. An empty list selects the slider state's ordinary step size.
    pub fn keyboard_steps(mut self, values: Vec<f32>) -> Self {
        self.keyboard_steps = Some(values);
        self
    }
}
impl RenderOnce for SourceSlider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let progress = self.progress;
        let fill_inset = self.fill_inset;
        let tip = self.tip;
        let interaction_enabled = self.enabled && self.interaction_enabled;
        let opacity = surface::fade_opacity(
            ("source-slider-opacity", self.state.entity_id()),
            if self.enabled { 1. } else { 0.3 },
            300,
            window,
            cx,
        );
        // The base slider root owns the a11y role and the release handling of
        // the drag; an inactive container is `.slider-container` without `.on`,
        // so it renders at 30% opacity and drops the pointer handlers that
        // mirror the source's `pointer-events:none`.
        let mut container = BaseSlider::new(&self.state)
            .disabled(!interaction_enabled)
            .relative()
            .w_full()
            .h(surface::css(
                self.container_height
                    .unwrap_or(if tip.is_some() { 64. } else { 36. }),
            ))
            .opacity(opacity);
        // `.track` (z-index 1), then `.left` (z-index 2) over it.
        container = container.child(
            div()
                .absolute()
                .bottom(surface::css(25.))
                .w_full()
                .h(surface::css(6.))
                .rounded(surface::css(3.))
                .bg(SliderColors::track())
                .when_some(self.track_gradient, |track, (start, end)| {
                    track.bg(linear_gradient(
                        90.,
                        linear_color_stop(start, 0.),
                        linear_color_stop(end, 1.),
                    ))
                }),
        );
        if self.track_gradient.is_none() {
            container = container.child(
                div()
                    .absolute()
                    .bottom(surface::css(25.))
                    .w_full()
                    .h(surface::css(6.))
                    .rounded(surface::css(3.))
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                // `updateValue()`: `calc(8px + p*(100% - 16px))`.
                                let inset = if fill_inset {
                                    window.rem_size() * 0.5
                                } else {
                                    px(0.)
                                };
                                let width = inset + (bounds.size.width - inset * 2.) * progress;
                                window.paint_quad(PaintQuad {
                                    corner_radii: Corners::all(window.rem_size() * (3. / 16.)),
                                    ..fill(
                                        Bounds::new(bounds.origin, size(width, bounds.size.height)),
                                        SliderColors::fill(),
                                    )
                                });
                            },
                        )
                        .size_full(),
                    ),
            );
        }
        if let Some((minimum, maximum, color)) = self.range_labels {
            container =
                container.children([(false, minimum), (true, maximum)].map(|(right, text)| {
                    div()
                        .absolute()
                        .bottom(surface::css(-2.))
                        .when(right, |label| label.right_0())
                        .when(!right, |label| label.left_0())
                        .font_family("Roboto")
                        .text_size(surface::css(12.))
                        .line_height(surface::css(14.))
                        .text_color(color)
                        .child(text.to_uppercase())
                }));
        }
        // `.slider-tip` (bottom 42px) is absolutely placed by the source JS at
        // `p*(W - 16) - tipW/2 + 8`, i.e. centred on the thumb, whose centre
        // travels between the two 8px insets.
        if let Some(tip) = tip {
            container = container.child(
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
                            .bg(SliderColors::fill())
                            .text_color(SliderColors::tip_text())
                            .text_size(surface::css(12.))
                            .line_height(surface::css(14.))
                            .child(tip),
                    ),
            );
        }
        // `.slider` (z-index 3): the base slider owns the drag/focus behaviour,
        // the thumb keeps the source's 16px `#44d62c` square and its two
        // pointer-state colors.
        let container = container.child(
            SliderTrack::new(&self.state)
                .disabled(!interaction_enabled)
                .absolute()
                .bottom(surface::css(20.))
                .w_full()
                .h(surface::css(16.))
                .child(
                    SliderIndicator::new(&self.state)
                        .absolute()
                        .left(surface::css(8.))
                        .right(surface::css(8.))
                        .h_full()
                        .child(
                            source_thumb(&self.state, interaction_enabled, window, cx)
                                .absolute()
                                .left(relative(progress))
                                .ml(surface::css(-8.))
                                .size(surface::css(16.))
                                .rounded(surface::css(8.)),
                        ),
                ),
        );
        let Some(steps) = self.keyboard_steps else {
            return container.into_any_element();
        };
        let input = window.use_keyed_state(
            ("source-slider-keyboard", self.state.entity_id()),
            cx,
            |window, cx: &mut Context<SourceSliderInput>| {
                let focus = cx.focus_handle().tab_stop(true);
                let blur = cx.on_blur(&focus, window, |input, _, cx| {
                    input.registered = false;
                    cx.notify();
                });
                SourceSliderInput {
                    focus,
                    registered: false,
                    _blur: blur,
                }
            },
        );
        let focus = input.read(cx).focus.clone();
        let click_input = input.clone();
        let key_input = input.clone();
        let release_input = input.clone();
        let release_state = self.state.clone();
        let wheel_input = input.clone();
        let wheel_state = self.state.clone();
        let wheel_steps = steps.clone();
        div()
            .id(("source-slider-input", self.state.entity_id()))
            .track_focus(&focus)
            .when(interaction_enabled, |wrapper| {
                wrapper
                    .capture_any_mouse_down(move |event, window, cx| {
                        if event.button == MouseButton::Left {
                            focus.focus(window, cx);
                        }
                    })
                    .on_click(move |_, _, cx| {
                        click_input.update(cx, |input, cx| {
                            input.registered = true;
                            cx.notify();
                        });
                    })
                    .on_key_down(move |event, window, cx| {
                        // FM/WM prevent the range input's Home/End default,
                        // but their custom handler leaves the value unchanged.
                        let increase = match event.keystroke.key.as_str() {
                            "home" | "end" => {
                                cx.stop_propagation();
                                return;
                            }
                            "left" | "down" => false,
                            "right" | "up" => true,
                            _ => return,
                        };
                        cx.stop_propagation();
                        if !key_input.read(cx).registered {
                            return;
                        }
                        let state = self.state.read(cx);
                        let next = source_slider_step(
                            state.value().start(),
                            state.min_value(),
                            state.max_value(),
                            state.step_value(),
                            &steps,
                            increase,
                        );
                        self.state.update(cx, |state, cx| {
                            state.set_value(next, window, cx);
                            // Locked 0.7.3 set_value only notifies. User changes
                            // must emit Change so the owning page submits them.
                            cx.emit(SliderEvent::Change(state.value()));
                        });
                    })
                    .on_key_up(move |_, _, cx| {
                        if release_input.read(cx).registered {
                            release_state.update(cx, |state, cx| {
                                cx.emit(SliderEvent::Release(state.value()))
                            });
                        }
                    })
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |_, _, window, _| {
                                // Source registers on window, so a wheel event keeps
                                // changing the clicked slider even after the pointer
                                // leaves its bounds. The keyed blur subscription ends
                                // this behavior, and unmount removes the paint listener.
                                window.on_mouse_event(
                                    move |event: &ScrollWheelEvent, phase, window, cx| {
                                        if !phase.bubble() || !wheel_input.read(cx).registered {
                                            return;
                                        }
                                        let delta = event.delta.pixel_delta(px(1.)).y;
                                        if delta == px(0.) {
                                            return;
                                        }
                                        let state = wheel_state.read(cx);
                                        // FM uses deltaY, WM uses the opposite wheelDelta.
                                        let increase = if wheel_steps.is_empty() {
                                            delta < px(0.)
                                        } else {
                                            delta > px(0.)
                                        };
                                        let next = source_slider_step(
                                            state.value().start(),
                                            state.min_value(),
                                            state.max_value(),
                                            state.step_value(),
                                            &wheel_steps,
                                            increase,
                                        );
                                        cx.stop_propagation();
                                        wheel_state.update(cx, |state, cx| {
                                            state.set_value(next, window, cx);
                                            cx.emit(SliderEvent::Change(state.value()));
                                        });
                                    },
                                );
                            },
                        )
                        .absolute()
                        .size_0(),
                    )
            })
            .child(container)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::source_slider_step;

    #[test]
    fn camera_shutter_arrows_follow_origin_data_instead_of_uniform_time_steps() {
        // Current WM uses getIncreaseValue/getDecreaseValue over originData;
        // these are consecutive current 3594 exposure times in microseconds.
        let times = [500., 625., 781., 1000.];
        assert_eq!(
            source_slider_step(625., 500., 1000., 1., &times, true),
            781.
        );
        assert_eq!(
            source_slider_step(781., 500., 1000., 1., &times, false),
            625.
        );
        assert_eq!(
            source_slider_step(500., 500., 1000., 1., &times, false),
            500.
        );
        assert_eq!(
            source_slider_step(1000., 500., 1000., 1., &times, true),
            1000.
        );
    }

    #[test]
    fn camera_compensation_arrows_keep_source_decimal_precision_and_endpoints() {
        assert_eq!(source_slider_step(-0.3, -3., 3., 0.1, &[], true), -0.2);
        assert_eq!(source_slider_step(0.3, -3., 3., 0.1, &[], false), 0.2);
        assert_eq!(source_slider_step(-3., -3., 3., 0.1, &[], false), -3.);
        assert_eq!(source_slider_step(3., -3., 3., 0.1, &[], true), 3.);
    }
}
