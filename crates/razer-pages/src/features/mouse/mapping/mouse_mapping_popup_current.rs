//! Source calculateKeyConfigPopupLeft and ordinary dot-bg position observer.
use super::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub(super) struct Popup {
    content: AnyElement,
    root: Rc<Cell<Bounds<Pixels>>>,
    drawer: Rc<Cell<Bounds<Pixels>>>,
    anchors: Rc<RefCell<BTreeMap<String, Bounds<Pixels>>>>,
    input: String,
    from_panel: bool,
    panel_open: bool,
    scroll: ScrollHandle,
    babel_162: bool,
    global_shortcut: bool,
}
fn left(
    anchor: Bounds<Pixels>,
    body: Bounds<Pixels>,
    scroll_left: Pixels,
    popup_width: Pixels,
    margin: Pixels,
    drawer_width: Pixels,
    drawer_clamp: Pixels,
    panel_open: bool,
    from_panel: bool,
) -> Pixels {
    if panel_open && from_panel {
        return body.left() + drawer_width;
    }
    let anchor_left = anchor.left() - body.left();
    let anchor_right = anchor.right() - body.left();
    let candidate = if anchor_right + scroll_left + popup_width < body.size.width - margin {
        anchor_right
    } else {
        anchor_left - popup_width
    };
    body.left()
        + if panel_open && candidate < drawer_width {
            drawer_clamp
        } else {
            candidate.max(px(0.))
        }
}
impl MouseProductWorkspace {
    pub(in super::super) fn current_mouse_mapping_popup_layer(
        &self,
        content: impl IntoElement,
    ) -> AnyElement {
        let state = &self.mapping_menu_current_state;
        razer_assets::deferred_resource(Popup {
            content: content.into_any_element(),
            root: state.root.clone(),
            drawer: state.drawer.clone(),
            anchors: state.anchors.clone(),
            input: self.mapping_input.clone().unwrap_or_default(),
            from_panel: self.current_mouse_mapping_from_panel(),
            panel_open: self.current_mouse_mapping_panel_open(),
            scroll: self.current_mouse_mapping_body_scroll(),
            babel_162: self.spec.product_id == 162,
            global_shortcut: self.mouse_customize_observation["customizeReducer"]["displayMode"]
                == "global-shortcuts",
        })
        .with_priority(103)
        .into_any_element()
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
        let unit = window.rem_size() / 16.;
        let anchor = self
            .anchors
            .borrow()
            .get(&self.input)
            .copied()
            .unwrap_or(root);
        let width = (window.layout_bounds(*child).size.width - unit * 2.).max(px(0.));
        let drawer = if self.panel_open {
            self.drawer.get().size.width
        } else {
            px(0.)
        };
        let scroll_left = -self.scroll.offset().x;
        // GPUI measurements are in window coordinates. The source popup's
        // containing block starts at zero, so normalize against the body once.
        let x = left(
            anchor,
            root,
            scroll_left,
            width,
            unit * if self.babel_162 { 0. } else { 20. },
            drawer,
            unit * 230.,
            self.panel_open,
            self.from_panel,
        );
        let y = root.top()
            + if self.global_shortcut {
                unit * 132.
            } else {
                unit * 36.
                    + if self.from_panel {
                        px(0.)
                    } else {
                        self.scroll.offset().y
                    }
            };
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
#[cfg(test)]
mod tests {
    use super::*;

    fn area(left: f32, width: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(left), px(0.)), size(px(width), px(30.)))
    }

    #[::core::prelude::v1::test]
    fn source_margin_and_drawer_protection_change_placement() {
        let anchor = Bounds::new(point(px(20.), px(0.)), size(px(30.), px(30.)));
        assert_eq!(
            left(
                anchor,
                area(0., 360.),
                px(0.),
                px(290.),
                px(0.),
                px(0.),
                px(230.),
                false,
                false
            ),
            px(50.)
        );
        assert_eq!(
            left(
                anchor,
                area(0., 360.),
                px(0.),
                px(290.),
                px(20.),
                px(0.),
                px(230.),
                false,
                false
            ),
            px(0.)
        );
        assert_eq!(
            left(
                anchor,
                area(0., 360.),
                px(0.),
                px(290.),
                px(20.),
                px(230.),
                px(230.),
                true,
                false
            ),
            px(230.)
        );
        assert_eq!(
            left(
                anchor,
                area(0., 360.),
                px(0.),
                px(290.),
                px(20.),
                px(218.),
                px(230.),
                true,
                true
            ),
            px(218.)
        );
    }

    #[::core::prelude::v1::test]
    fn shifted_body_preserves_the_source_side_and_applies_its_origin_once() {
        assert_eq!(
            left(
                area(310., 40.),
                area(80., 600.),
                px(0.),
                px(300.),
                px(20.),
                px(0.),
                px(230.),
                false,
                false
            ),
            px(350.)
        );
        assert_eq!(
            left(
                area(310., 40.),
                area(80., 500.),
                px(0.),
                px(300.),
                px(20.),
                px(0.),
                px(230.),
                false,
                false
            ),
            px(80.)
        );
    }

    #[::core::prelude::v1::test]
    fn horizontal_scroll_changes_fit_without_moving_the_measured_anchor() {
        assert_eq!(
            left(
                area(230., 40.),
                area(0., 600.),
                px(20.),
                px(300.),
                px(20.),
                px(0.),
                px(230.),
                false,
                false
            ),
            px(0.)
        );
        assert_eq!(
            left(
                area(230., 40.),
                area(0., 600.),
                px(0.),
                px(300.),
                px(20.),
                px(0.),
                px(230.),
                false,
                false
            ),
            px(270.)
        );
    }

    #[::core::prelude::v1::test]
    fn drawer_click_uses_actual_width_but_image_click_uses_source_clamp() {
        let body = area(25., 640.);
        let anchor = area(65., 40.);
        assert_eq!(
            left(
                anchor,
                body,
                px(0.),
                px(362.5),
                px(25.),
                px(260.),
                px(287.5),
                true,
                true
            ),
            px(285.)
        );
        assert_eq!(
            left(
                anchor,
                body,
                px(0.),
                px(362.5),
                px(25.),
                px(260.),
                px(287.5),
                true,
                false
            ),
            px(312.5)
        );
    }
}
