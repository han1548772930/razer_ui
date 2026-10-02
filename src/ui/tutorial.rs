//! 55 CSS's tutorial controls and indicator.b7ce7af4.svg's SMIL timeline.
use super::{surface::css, theme::MainPageColors};
use gpui_kit::base::{
    Button, TestSupportExt as _,
    motion::{self, Easing, Transition},
};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{prelude::FluentBuilder as _, *};

#[cfg(test)]
#[path = "tutorial_tests.rs"]
mod tests;

/// The source SVG has three filled, expanding circles, not stroked rings.
/// GPUI's static SVG decoder ignores SMIL, leaving Circle_1 as a solid disk.
#[derive(IntoElement)]
pub(crate) struct TutorialIndicator {
    id: ElementId,
    base: Stateful<Div>,
}

impl TutorialIndicator {
    pub(crate) fn new(id: impl Into<ElementId>) -> Self {
        let id = id.into();
        Self {
            base: div().id(id.clone()).relative().size(css(36.)),
            id,
        }
    }
}

impl Styled for TutorialIndicator {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

/// Circle_1 starts at 0 and Circle_2.end, Circle_2 at 1 and Circle_3.end,
/// Circle_3 at 2 and Circle_1.end. With 2s durations these start every 3s,
/// staggered by 1s. The initial hidden interval is not a wrapped steady state.
fn pulse(elapsed: f64, circle: usize) -> (f32, f32) {
    if elapsed < circle as f64 {
        return (0., 0.);
    }
    let progress = (((elapsed - circle as f64) % 3.) / 2.).min(1.) as f32;
    (36. * progress, 1. - progress)
}

impl RenderOnce for TutorialIndicator {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let now = cx.background_executor().now();
        let started = window.use_keyed_state((self.id, "smil-start"), cx, |_, _| now);
        let elapsed = now
            .saturating_duration_since(*started.read(cx))
            .as_secs_f64();
        let color = MainPageColors.tutorial_accent();
        let animated = !cx.reduce_motion();
        if animated {
            window.request_animation_frame();
        }
        self.base
            .test_support()
            // Retain SVG paint order (3, 2, 1, static center). These passive
            // shapes have no hitbox and cannot intercept the guided control.
            .children((0..3).rev().map(|circle| {
                let (diameter, opacity) = if animated {
                    pulse(elapsed, circle)
                } else {
                    (0., 0.)
                };
                div()
                    .id(SharedString::from(format!("tutorial-pulse-{}", circle + 1)))
                    .test_support()
                    .absolute()
                    .left(css((36. - diameter) / 2.))
                    .top(css((36. - diameter) / 2.))
                    .size(css(diameter))
                    .rounded_full()
                    .bg(color)
                    .opacity(opacity)
            }))
            .child(
                div()
                    .id("tutorial-indicator-center")
                    .test_support()
                    .absolute()
                    .left(css(14.4))
                    .top(css(14.4))
                    .size(css(7.2))
                    .rounded_full()
                    .bg(color),
            )
    }
}

/// Source buttons have a fixed border box, without Component size padding.
pub(crate) fn tutorial_button(
    id: &'static str,
    label: impl Into<SharedString>,
    primary: bool,
    disabled: bool,
    window: &mut Window,
    cx: &mut App,
) -> Button {
    let colors = MainPageColors;
    let label = label.into().to_uppercase();
    let opacity = motion::transition(
        (id, "tutorial-button-opacity"),
        if disabled { 0.5 } else { 1. },
        Transition::new(std::time::Duration::from_millis(300)).easing(Easing::Ease),
        window,
        cx,
    );
    Button::new(id)
        .accessibility_label(label.clone())
        .disabled(disabled)
        .w(css(100.))
        .h(css(27.))
        .flex_shrink_0()
        .p_0()
        .border_1()
        .border_color(colors.banner_shade())
        .rounded(css(3.))
        .text_size(css(12.))
        .line_height(css(14.))
        .text_color(if primary {
            colors.banner_shade()
        } else {
            colors.banner_heading()
        })
        .bg(if primary {
            colors.tutorial_accent()
        } else {
            colors.card_caption()
        })
        .opacity(opacity)
        .when(primary && !disabled, |button| {
            button.hover(|style| style.bg(colors.tutorial_hover()))
        })
        .when(disabled, |button| button.cursor_default())
        .when(!disabled, |button| button.cursor_pointer())
        .focus_visible(|style| style.border_color(cx.theme().primary))
        .child(label)
}
