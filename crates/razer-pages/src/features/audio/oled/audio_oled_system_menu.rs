//! Keep 1383 `hv` in its inline CSS layout while honoring its ancestor clip.
use gpui_kit::*;
use std::{cell::Cell, rc::Rc};

pub(super) struct ClippedMenu {
    content: AnyElement,
    mask: Rc<Cell<ContentMask<Pixels>>>,
}

impl ClippedMenu {
    pub(super) fn new(content: AnyElement, mask: Rc<Cell<ContentMask<Pixels>>>) -> Self {
        Self { content, mask }
    }
}

impl IntoElement for ClippedMenu {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for ClippedMenu {
    type RequestLayoutState = ();
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
    ) -> (LayoutId, ()) {
        // Return the original absolute child's layout to the slide. No popup
        // positioner, window clamping, re-anchoring, or additional layout node.
        (self.content.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        // Deferred does not inherit the ancestor mask. Apply it before hit
        // testing as well as paint so clipped menu items cannot intercept input.
        window.with_content_mask(Some(self.mask.get()), |window| {
            self.content.prepaint(window, cx);
        });
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
        window.with_content_mask(Some(self.mask.get()), |window| {
            self.content.paint(window, cx);
        });
    }
}
