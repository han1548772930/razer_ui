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

fn left(root: Bounds<Pixels>, anchor: Bounds<Pixels>, width: Pixels, margin: Pixels) -> Pixels {
    let anchor_left = anchor.left() - root.left();
    let anchor_right = anchor.right() - root.left();
    root.left()
        + if anchor_right + width < root.size.width - margin {
            anchor_right
        } else {
            (anchor_left - width).max(px(0.))
        }
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
        let x = left(root, anchor, width, unit * 20.);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn area(left: f32, width: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(left), px(0.)), size(px(width), px(100.)))
    }

    #[::core::prelude::v1::test]
    fn shifted_body_does_not_change_the_available_side() {
        // There is 30px of room after the popup in the source body.
        assert_eq!(
            left(area(80., 600.), area(310., 40.), px(300.), px(20.)),
            px(350.)
        );
        assert_eq!(
            left(area(0., 600.), area(230., 40.), px(300.), px(20.)),
            px(270.)
        );
    }

    #[::core::prelude::v1::test]
    fn flipped_popup_clamps_to_the_product_body_origin() {
        assert_eq!(
            left(area(80., 500.), area(310., 40.), px(300.), px(20.)),
            px(80.)
        );
        assert_eq!(
            left(area(80., 600.), area(470., 40.), px(300.), px(20.)),
            px(170.)
        );
    }

    #[::core::prelude::v1::test]
    fn source_strict_margin_is_preserved_under_scale() {
        assert_eq!(
            left(area(25., 600.), area(275., 40.), px(290.), px(20.)),
            px(25.)
        );
        assert_eq!(
            left(area(25., 750.), area(337.5, 50.), px(362.5), px(25.)),
            px(25.)
        );
    }
}
