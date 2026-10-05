//! Current 55 CSS: retained content, percentage transforms, and independent help.
use super::*;
use gpui_kit::base::motion::Discrete;
use std::time::Instant;

/// Source percentage transforms apply to the element's own measured box.
/// Deferred layers retain the containing scroll/banner clip and do not perform
/// Base Popup's viewport clamping, which is absent from 19388's marketing tree.
pub(super) struct SourceLayer {
    child: Option<AnyElement>,
    x: f32,
    y: f32,
    priority: Option<usize>,
}
impl SourceLayer {
    pub(super) fn new(child: impl IntoElement, x: f32, y: f32) -> Self {
        Self {
            child: Some(child.into_any_element()),
            x,
            y,
            priority: None,
        }
    }
    pub(super) fn priority(mut self, priority: usize) -> Self {
        self.priority = Some(priority);
        self
    }
}
impl IntoElement for SourceLayer {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for SourceLayer {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.as_mut().unwrap().request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let delta = point(bounds.size.width * self.x, bounds.size.height * self.y);
        if let Some(priority) = self.priority {
            let offset = window.element_offset() + delta;
            let mask = window.content_mask();
            window.defer_draw(self.child.take().unwrap(), offset, priority, Some(mask));
        } else {
            window.with_element_offset(delta, |window| {
                self.child.as_mut().unwrap().prepaint(window, cx);
            });
        }
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(child) = self.child.as_mut() {
            child.paint(window, cx);
        }
    }
}

#[derive(IntoElement)]
pub(super) struct CollapseArrow {
    pub(super) id: &'static str,
    pub(super) collapsed: bool,
}
impl RenderOnce for CollapseArrow {
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
            .size(surface::css(10.))
            .text_color(rgb(0x999999))
            .group_hover(self.id, |style| style.text_color(rgb(0xffffff)))
            .child(
                Icon::default()
                    .path("synapse/expand.svg")
                    .size(surface::css(10.))
                    .transform(Transformation::rotate(radians(angle))),
            )
    }
}

#[derive(IntoElement)]
pub(super) struct GroupContent {
    pub(super) id: &'static str,
    pub(super) collapsed: bool,
    pub(super) child: AnyElement,
}
struct OverflowPhase {
    collapsed: bool,
    started: Instant,
}
impl RenderOnce for GroupContent {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let target = if self.collapsed { 0. } else { 1. };
        let height = motion::transition(
            (self.id, "height"),
            target,
            Transition::new(Duration::from_millis(300)).easing(Easing::EaseIn),
            window,
            cx,
        );
        let translation = motion::transition(
            (self.id, "translation"),
            target,
            Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
            window,
            cx,
        );
        let now = cx.background_executor().now();
        let state = window.use_keyed_state((ElementId::from(self.id), "overflow"), cx, |_, _| {
            OverflowPhase {
                collapsed: self.collapsed,
                started: now,
            }
        });
        if state.read(cx).collapsed != self.collapsed {
            state.update(cx, |phase, _| {
                phase.collapsed = self.collapsed;
                phase.started = now;
            });
        }
        // `.expand .content`: @keyframes expand 1s forwards, default ease.
        let elapsed = now
            .saturating_duration_since(state.read(cx).started)
            .as_secs_f32()
            .min(1.);
        let clip = self.collapsed
            || (!cx.reduce_motion()
                && Discrete::new(true, false).sample(Easing::Ease.sample(elapsed)));
        if clip && !self.collapsed {
            window.request_animation_frame();
        }
        div()
            .id((ElementId::from(self.id), "content"))
            .w_full()
            .mt(surface::css(10.))
            .flex_shrink_0()
            .max_h(surface::css(2000. * height))
            .when(clip, |view| view.overflow_hidden())
            .child(SourceLayer::new(self.child, 0., -(1. - translation)))
    }
}

#[derive(IntoElement)]
pub(super) struct GroupHelp {
    pub(super) id: &'static str,
    pub(super) tip: &'static str,
}
impl RenderOnce for GroupHelp {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state((ElementId::from(self.id), "hover"), cx, |_, _| false);
        let hovered = *state.read(cx);
        let color_progress = motion::transition(
            (self.id, "background"),
            if hovered { 1. } else { 0. },
            Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
            window,
            cx,
        );
        // CSS interpolates premultiplied sRGB/alpha. The endpoints are
        // #4a4a4a and #ffffff4d; straight Hsla interpolation would differ here
        // because their alpha changes, despite both colors being achromatic.
        let alpha = 1. + (77. / 255. - 1.) * color_progress;
        let channel = (74. / 255. * (1. - color_progress) + 77. / 255. * color_progress) / alpha;
        let background = Rgba {
            r: channel,
            g: channel,
            b: channel,
            a: alpha,
        };
        let opacity = motion::transition(
            (self.id, "opacity"),
            if hovered { 1. } else { 0. },
            Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
            window,
            cx,
        );
        div()
            .id(self.id)
            .relative()
            .size(surface::css(14.))
            .ml(surface::css(5.))
            .on_hover(window.listener_for(&state, |hover, value, _, cx| {
                *hover = *value;
                cx.notify();
            }))
            .child(
                BaseButton::new((ElementId::from(self.id), "button"))
                    .accessibility_label(i18n::t(self.tip))
                    .size_full()
                    .p_0()
                    .rounded(surface::css(7.))
                    .bg(background)
                    .focus_visible(|style| style.border_1().border_color(cx.theme().primary))
                    .child(img("synapse/gr-help.svg").size_full()),
            )
            .child(
                SourceLayer::new(
                    gpui_kit::base::Tooltip::new((ElementId::from(self.id), "tip"))
                        .absolute()
                        .left_0()
                        .top(surface::css(20.))
                        .w(surface::css(300.))
                        .min_w(surface::css(300.))
                        .px(surface::css(10.))
                        .py(surface::css(8.))
                        .border_1()
                        .border_color(rgb(0x5d5d5d))
                        .bg(rgb(0))
                        .text_color(rgb(0xcccccc))
                        .text_size(surface::css(14.))
                        .line_height(surface::css(16.))
                        .whitespace_normal()
                        .opacity(opacity)
                        .when(!hovered, |view| view.invisible())
                        .child(i18n::t(self.tip)),
                    0.,
                    0.,
                )
                .priority(100),
            )
    }
}
