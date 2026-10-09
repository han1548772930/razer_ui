//! 58190.Cr portal: anchored at help left + 15, top; MMT translates -100% Y.
use super::*;
use gpui_kit::Size;
use std::{cell::Cell, rc::Rc};
pub struct RecordHelp {
    pub content: AnyElement,
    pub anchor: Rc<Cell<Bounds<Pixels>>>,
    pub upward: bool,
}
impl IntoElement for RecordHelp {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for RecordHelp {
    type RequestLayoutState = Size<Pixels>;
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
    ) -> (LayoutId, Size<Pixels>) {
        let measured = self.content.layout_as_root(
            size(AvailableSpace::MaxContent, AvailableSpace::MinContent),
            window,
            cx,
        );
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    ..Default::default()
                },
                [],
                cx,
            ),
            measured,
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        measured: &mut Size<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let anchor = self.anchor.get();
        let origin = point(
            anchor.left() + window.rem_size() * (15. / 16.),
            anchor.top() - if self.upward { measured.height } else { px(0.) },
        );
        window.with_content_mask(None, |window| self.content.prepaint_at(origin, window, cx));
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Size<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_content_mask(None, |window| self.content.paint(window, cx));
    }
}
