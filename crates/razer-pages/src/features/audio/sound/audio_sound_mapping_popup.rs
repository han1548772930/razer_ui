//! Current 5064 calculateKeyConfigPopupLeft and dot-bg vertical positioning.
use gpui_kit::*;
use std::{cell::Cell, rc::Rc};

pub(super) fn place(
    content: AnyElement,
    root: Rc<Cell<Bounds<Pixels>>>,
    anchor: Rc<Cell<Bounds<Pixels>>>,
) -> AnyElement {
    razer_assets::deferred_resource(Popup {
        content,
        root,
        anchor,
    })
    .with_priority(106)
    .into_any_element()
}

struct Popup {
    content: AnyElement,
    root: Rc<Cell<Bounds<Pixels>>>,
    anchor: Rc<Cell<Bounds<Pixels>>>,
}
impl IntoElement for Popup {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for Popup {
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
        child: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let root = self.root.get();
        let anchor = self.anchor.get();
        let unit = window.rem_size() / 16.;
        // Source clientWidth excludes the two popup border pixels.
        let width = (window.layout_bounds(*child).size.width - unit * 2.).max(px(0.));
        let x = if anchor.right() + width < root.size.width - unit * 20. {
            anchor.right()
        } else {
            (anchor.left() - width).max(px(0.))
        };
        // The source dot-bg moves with the product body's vertical scroll.
        // The wrapper includes the heading above source body top = 36px.
        let target = point(x, root.top());
        window.with_element_offset(target - bounds.origin, |window| {
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
