//! 332.720c66b0 CSS: independent Chroma navigation and collapse timelines.
use crate::ui::surface::{self, css};
use gpui_kit::base::{
    Button as BaseButton,
    motion::{self, Easing, Transition},
};
use gpui_kit::component::*;
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::time::{Duration, Instant};

#[derive(IntoElement)]
pub(super) struct NavigationButton {
    pub(super) id: &'static str,
    pub(super) selected: bool,
    pub(super) button: BaseButton,
}
impl RenderOnce for NavigationButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = surface::pointer_state(self.id, window, cx);
        let (hovered, pressed) = state.read(cx).sample();
        let background: Hsla = if self.selected {
            rgb(0x44d62c).into()
        } else if hovered && pressed {
            rgb(0x3cbf27).into()
        } else if hovered {
            rgb(0x2d2d2d).into()
        } else {
            rgba(0x00000000).into()
        };
        let color: Hsla = if self.selected || hovered && pressed {
            rgb(0x111111).into()
        } else if hovered {
            rgb(0xcccccc).into()
        } else {
            rgb(0x999999).into()
        };
        let background = motion::transition(
            (self.id, "background"),
            background,
            Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
            window,
            cx,
        );
        let color = motion::transition(
            (self.id, "color"),
            color,
            Transition::new(Duration::from_millis(100)).easing(Easing::Ease),
            window,
            cx,
        );
        surface::track_pointer(
            self.button
                .h(css(28.))
                .px(css(10.))
                .py(css(7.))
                .flex_shrink_0()
                .rounded(css(14.))
                .text_size(css(12.))
                .line_height(css(14.))
                .bg(background)
                .text_color(color),
            &state,
            window,
        )
    }
}

#[derive(IntoElement)]
pub(super) struct CollapseIcon {
    pub(super) id: &'static str,
    pub(super) collapsed: bool,
}
impl RenderOnce for CollapseIcon {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let angle = motion::transition(
            (self.id, "arrow"),
            if self.collapsed {
                -std::f32::consts::FRAC_PI_2
            } else {
                0.
            },
            Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
            window,
            cx,
        );
        div()
            .size(css(10.))
            .text_color(rgb(0x999999))
            .group_hover(self.id, |s| s.text_color(rgb(0xffffff)))
            .child(
                Icon::default()
                    .path("synapse/expand.svg")
                    .size(css(10.))
                    .transform(Transformation::rotate(radians(angle))),
            )
    }
}

#[derive(IntoElement)]
pub(super) struct GroupContent {
    pub(super) id: &'static str,
    pub(super) collapsed: bool,
    pub(super) height: f32,
    pub(super) children: AnyElement,
}
struct OverflowPhase {
    collapsed: bool,
    changed: Instant,
}
impl RenderOnce for GroupContent {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let target = if self.collapsed { 0. } else { 1. };
        let progress = motion::transition(
            (self.id, "expand"),
            target,
            Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
            window,
            cx,
        );
        let now = cx.background_executor().now();
        let phase = window.use_keyed_state((ElementId::from(self.id), "overflow"), cx, |_, _| {
            OverflowPhase {
                collapsed: self.collapsed,
                changed: now,
            }
        });
        if phase.read(cx).collapsed != self.collapsed {
            phase.update(cx, |phase, _| {
                phase.collapsed = self.collapsed;
                phase.changed = now;
            });
        }
        // Ss and Kt retain children; show-overflow is set after 300ms.
        let clip = self.collapsed
            || now.saturating_duration_since(phase.read(cx).changed) < Duration::from_millis(300);
        if clip && !self.collapsed {
            window.request_animation_frame();
        }
        div()
            .w_full()
            .mt(css(10.))
            .h(css(self.height))
            .max_h(css(2000. * progress))
            .flex_shrink_0()
            .when(clip, |v| v.overflow_hidden())
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(css(self.height))
                    .flex_shrink_0()
                    .top(css(-self.height * (1. - progress)))
                    .child(self.children),
            )
    }
}

pub(super) fn grid_height(
    width: f32,
    count: usize,
    card_width: f32,
    card_height: f32,
    gap: f32,
) -> f32 {
    let columns = ((width + gap) / (card_width + gap)).floor().max(1.) as usize;
    let rows = count.max(1).div_ceil(columns);
    card_height + (rows - 1) as f32 * (card_height + gap)
}
