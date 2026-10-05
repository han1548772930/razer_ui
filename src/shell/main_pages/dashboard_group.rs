//! 55 CSS's independent max-height, translation, rotation and overflow channels.
use crate::ui::surface;
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::base::motion::{self, Discrete, Easing, Transition};
use gpui_kit::component::Icon;
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::time::{Duration, Instant};

#[derive(IntoElement)]
pub(super) struct CollapseIcon {
    id: &'static str,
    collapsed: bool,
}

impl CollapseIcon {
    pub(super) fn new(id: &'static str, collapsed: bool) -> Self {
        Self { id, collapsed }
    }
}

impl RenderOnce for CollapseIcon {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let angle = motion::transition(
            (self.id, "collapse-arrow"),
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
            .size(surface::css(10.))
            .text_color(rgb(0x999999))
            .group_hover(self.id, |icon| icon.text_color(rgb(0xffffff)))
            .child(
                Icon::default()
                    .path("synapse/expand.svg")
                    .size(surface::css(10.))
                    .transform(Transformation::rotate(radians(angle))),
            )
    }
}

#[derive(IntoElement)]
pub(super) struct DashboardGroupContent {
    id: &'static str,
    collapsed: bool,
    height: f32,
    child: AnyElement,
}

struct OverflowPhase {
    collapsed: bool,
    started: Instant,
}

impl DashboardGroupContent {
    pub(super) fn new(
        id: &'static str,
        collapsed: bool,
        width: f32,
        count: usize,
        child: impl IntoElement,
    ) -> Self {
        // Current xi.moveItems fixes listRef height to 220 + (rows - 1) * 240.
        let columns = super::dashboard_columns(width);
        let rows = count.max(1).div_ceil(columns);
        Self {
            id,
            collapsed,
            height: 220. + (rows - 1) as f32 * 240.,
            child: child.into_any_element(),
        }
    }
}

impl RenderOnce for DashboardGroupContent {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let target = if self.collapsed { 0. } else { 1. };
        let max_height = motion::transition(
            (self.id, "collapse-height"),
            target,
            Transition::new(Duration::from_millis(300)).easing(Easing::EaseIn),
            window,
            cx,
        );
        let translation = motion::transition(
            (self.id, "collapse-translation"),
            target,
            Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
            window,
            cx,
        );
        let now = cx.background_executor().now();
        let phase = window.use_keyed_state(
            (ElementId::from(self.id), "collapse-overflow"),
            cx,
            |_, _| OverflowPhase {
                collapsed: self.collapsed,
                started: now,
            },
        );
        if phase.read(cx).collapsed != self.collapsed {
            phase.update(cx, |phase, _| {
                phase.collapsed = self.collapsed;
                phase.started = now;
            });
        }
        // @keyframes expand lasts 1s with the default CSS ease. Overflow is a
        // discrete property: switch at eased progress 0.5, not at the end.
        // Class changes restart the keyframe; only the transitions reverse.
        let elapsed = now
            .saturating_duration_since(phase.read(cx).started)
            .as_secs_f32()
            .min(1.);
        let clip = self.collapsed
            || (!cx.reduce_motion()
                && Discrete::new(true, false).sample(Easing::Ease.sample(elapsed)));
        if clip && !self.collapsed {
            window.request_animation_frame();
        }
        div()
            .id(SharedString::from(format!("{}-content", self.id)))
            .test_support()
            .w_full()
            .mt(surface::css(10.))
            .h(surface::css(self.height))
            .max_h(surface::css(2000. * max_height))
            .flex_shrink_0()
            .when(clip, |view| view.overflow_hidden())
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(surface::css(self.height))
                    .flex_shrink_0()
                    .top(surface::css(-self.height * (1. - translation)))
                    .child(self.child),
            )
    }
}

#[cfg(test)]
#[path = "dashboard_group_tests.rs"]
mod tests;
