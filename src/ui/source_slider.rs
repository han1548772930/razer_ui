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
//! `.thumb-tag` (the value bubble `OTA` swaps with `.slider-tip` while the thumb
//! is hovered or dragged) is only mounted when a caller passes `thumbTag`; no
//! current page does, so it is not rendered here.

use gpui_kit::base::{Slider as BaseSlider, SliderIndicator, SliderThumb, SliderTrack};
use gpui_kit::component::slider::SliderState;
use gpui_kit::{prelude::FluentBuilder as _, *};

use crate::ui::{surface, theme::SliderColors};

#[derive(IntoElement)]
pub(crate) struct SourceSlider {
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
}
impl SourceSlider {
    pub(crate) fn new(state: &Entity<SliderState>, progress: f32) -> Self {
        Self {
            state: state.clone(),
            progress: progress.clamp(0., 1.),
            tip: None,
            enabled: true,
        }
    }
    pub(crate) fn tip(mut self, tip: Option<impl Into<SharedString>>) -> Self {
        self.tip = tip.map(Into::into);
        self
    }
    pub(crate) fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}
impl RenderOnce for SourceSlider {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let progress = self.progress;
        let tip = self.tip;
        // The base slider root owns the a11y role and the release handling of
        // the drag; an inactive container is `.slider-container` without `.on`,
        // so it renders at 30% opacity and drops the pointer handlers that
        // mirror the source's `pointer-events:none`.
        let mut container = BaseSlider::new(&self.state)
            .disabled(!self.enabled)
            .relative()
            .w_full()
            .h(surface::css(if tip.is_some() { 64. } else { 36. }))
            .when(!self.enabled, |view| view.opacity(0.3));
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
                .disabled(!self.enabled)
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
                            SliderThumb::new(&self.state)
                                .disabled(!self.enabled)
                                .absolute()
                                .left(relative(progress))
                                .ml(surface::css(-8.))
                                .size(surface::css(16.))
                                .rounded(surface::css(8.))
                                .bg(SliderColors::thumb())
                                .when(self.enabled, |thumb| {
                                    thumb
                                        .hover(|style| {
                                            style
                                                .bg(SliderColors::thumb_hover())
                                                .border_2()
                                                .border_color(SliderColors::thumb_border())
                                        })
                                        .active(|style| {
                                            style
                                                .bg(SliderColors::thumb_active())
                                                .border_2()
                                                .border_color(SliderColors::thumb_border())
                                        })
                                }),
                        ),
                ),
        )
    }
}
