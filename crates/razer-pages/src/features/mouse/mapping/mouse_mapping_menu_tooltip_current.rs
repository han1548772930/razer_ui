//! Actual row Tooltip.defaultProps bottom-right, with per-version coordinates.
use super::*;
use razer_widgets::theme::TooltipColors;

#[derive(Clone)]
pub(super) struct Tip {
    pub(super) text: String,
    pub(super) anchor: Rc<Cell<Bounds<Pixels>>>,
}
struct Layer {
    content: AnyElement,
    anchor: Rc<Cell<Bounds<Pixels>>>,
    offset: f32,
    clamp: bool,
}
pub(super) fn render(tip: &Tip, policy: &Policy, opacity: f32) -> AnyElement {
    razer_assets::deferred_resource(Layer {
        content: div()
            .w(surface::css(300.))
            .flex()
            .justify_start()
            .child(
                div()
                    .max_w(surface::css(300.))
                    .px(surface::css(10.))
                    .py(surface::css(8.))
                    .border_1()
                    .border_color(TooltipColors::border())
                    .bg(TooltipColors::background())
                    .text_color(Colors::text())
                    .text_size(surface::css(14.))
                    .line_height(surface::css(16.))
                    .whitespace_normal()
                    .opacity(opacity)
                    .child(tip.text.clone()),
            )
            .into_any_element(),
        anchor: tip.anchor.clone(),
        offset: policy.tooltip_offset,
        clamp: policy.tooltip_clamp,
    })
    .with_priority(1060)
    .into_any_element()
}
impl IntoElement for Layer {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for Layer {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let child = self.content.request_layout(window, cx);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    ..Style::default()
                },
                [child],
                cx,
            ),
            child,
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let anchor = self.anchor.get();
        let unit = window.rem_size() / 16.;
        let mut x = anchor.right() - unit * self.offset;
        if self.clamp {
            // handleReposition uses else-if: an overflowing right edge wins
            // even when the correction leaves the left edge below 8px.
            if x + unit * 300. > window.viewport_size().width - unit * 8. {
                x = window.viewport_size().width - unit * 308.;
            } else if x < unit * 8. {
                x = unit * 8.;
            }
        }
        let y = anchor.bottom() + unit * 5.;
        window.with_element_offset(point(x, y) - bounds.origin, |window| {
            self.content.prepaint(window, cx)
        });
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.content.paint(window, cx);
    }
}
