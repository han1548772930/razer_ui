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
use gpui_kit::component::slider::SliderState;
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::time::Duration;

use crate::surface;
use crate::theme::SliderColors;

#[derive(Default)]
struct ThumbPointer {
    hovered: bool,
    pressed: bool,
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
}
impl SourceSlider {
    pub fn new(state: &Entity<SliderState>, progress: f32) -> Self {
        Self {
            state: state.clone(),
            progress: progress.clamp(0., 1.),
            tip: None,
            enabled: true,
            interaction_enabled: true,
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
    pub fn interaction_enabled(mut self, enabled: bool) -> Self {
        self.interaction_enabled = enabled;
        self
    }
}
impl RenderOnce for SourceSlider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let progress = self.progress;
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
            .h(surface::css(if tip.is_some() { 64. } else { 36. }))
            .opacity(opacity);
        // `.track` (z-index 1), then `.left` (z-index 2) over it.
        container = container.child(
            div()
                .absolute()
                .bottom(surface::css(25.))
                .w_full()
                .h(surface::css(6.))
                .rounded(surface::css(3.))
                .bg(SliderColors::track()),
        );
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
                            let inset = window.rem_size() * 0.5;
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
        container.child(
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
        )
    }
}
