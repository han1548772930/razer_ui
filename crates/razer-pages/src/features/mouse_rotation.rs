//! Current rotation artwork: preserve its two source colors when rotating.
use gpui_kit::base::{Slider as BaseSlider, SliderIndicator, SliderTrack};
use gpui_kit::component::slider::SliderState;
use gpui_kit::*;
use razer_widgets::theme::DrawerColors;
use razer_widgets::theme::SliderColors;
use razer_widgets::{source_slider::source_thumb, surface};
use std::sync::{Arc, OnceLock};

/// GPUI's monochrome Svg transform would lose the #111 background cutout.
/// Nest the exact SVG in a larger color image, rotating around the source
/// CSS box center (70 x 108), without clipping its corners at +/-44 degrees.
pub(super) fn image(degrees: i32) -> Arc<Image> {
    static IMAGES: OnceLock<Vec<Arc<Image>>> = OnceLock::new();
    IMAGES.get_or_init(|| {
        let source = include_str!("../../../../assets/synapse/mouse-rotation.svg")
            .replacen("width=\"71\" height=\"110\"", "width=\"70\" height=\"108\"", 1);
        (-44..=44).map(|degrees| {
            Arc::new(Image::from_bytes(ImageFormat::Svg, format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"180\" height=\"180\" viewBox=\"0 0 180 180\"><g transform=\"translate(90 90) rotate({degrees}) translate(-35 -54)\">{source}</g></svg>"
            ).into_bytes()))
        }).collect()
    })[(degrees.clamp(-44, 44) + 44) as usize].clone()
}

pub(super) fn cross() -> Canvas<()> {
    // Original 165px hr with 2px #707070 dotted border; second hr rotates 90deg.
    canvas(
        |_, _, _| (),
        |area, _, window, _| {
            let scale = f32::from(window.rem_size()) / 16.;
            let center = area.center();
            let color = DrawerColors::new().muted();
            for offset in (-27..=27).map(|i| i as f32 * 3. * scale) {
                for origin in [
                    point(center.x + px(offset - scale), center.y - px(scale)),
                    point(center.x - px(scale), center.y + px(offset - scale)),
                ] {
                    window.paint_quad(fill(
                        Bounds::new(origin, size(px(2. * scale), px(2. * scale))),
                        color,
                    ));
                }
            }
        },
    )
}

#[derive(IntoElement)]
pub(super) struct RotationRange {
    pub state: Entity<SliderState>,
    pub enabled: bool,
}
impl RenderOnce for RotationRange {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let progress = (self.state.read(cx).value().start() + 44.) / 88.;
        let hovered = window.use_keyed_state(
            ("rotation-range-hover", self.state.entity_id()),
            cx,
            |_, _| false,
        );
        let show_tip = self.enabled && *hovered.read(cx);
        let value = self.state.read(cx).value().start() as i32;
        // DA.A disabledFill=true omits the green progress fill entirely.
        let slider = BaseSlider::new(&self.state)
            .disabled(!self.enabled)
            .relative()
            .w_full()
            .h(surface::css(64.))
            .opacity(if self.enabled { 1. } else { 0.3 })
            .child(
                div()
                    .absolute()
                    .bottom(surface::css(25.))
                    .w_full()
                    .h(surface::css(6.))
                    .rounded(surface::css(3.))
                    .bg(SliderColors::track()),
            )
            .child(
                div()
                    .absolute()
                    .bottom(surface::css(42.))
                    .left(relative(progress))
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
                            .bg(SliderColors::fill())
                            .text_color(SliderColors::tip_text())
                            .text_size(surface::css(12.))
                            .line_height(surface::css(14.))
                            .child(format!("{value}°")),
                    ),
            )
            .child(
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
                                source_thumb(&self.state, self.enabled, window, cx)
                                    .absolute()
                                    .left(relative(progress))
                                    .ml(surface::css(-8.))
                                    .size(surface::css(16.))
                                    .rounded(surface::css(8.)),
                            ),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .w_full()
                    .child(surface::slider_tags("-44°", None, "44°", None)),
            );
        div()
            .id("mouse-rotation-range-hover")
            .on_hover(window.listener_for(&hovered, |hovered, value, _, cx| {
                *hovered = *value;
                cx.notify();
            }))
            .child(slider)
    }
}
