//! Position module 38162's menu in the current Macro web-content coordinates.
//! It uses a fixed 688 CSS-pixel threshold, not the native popup auto-flip.
use super::*;
use std::{cell::Cell, rc::Rc};

#[derive(Default)]
pub struct NestedDropdownState {
    pub expanded: bool,
    pub scroll: ScrollHandle,
    pub trigger: Rc<Cell<Bounds<Pixels>>>,
    pub menu: Rc<Cell<Bounds<Pixels>>>,
    pub upward: Rc<Cell<Option<bool>>>,
}

pub struct NestedMenuOverlay {
    pub content: AnyElement,
    pub trigger: Rc<Cell<Bounds<Pixels>>>,
    pub viewport: Rc<Cell<Bounds<Pixels>>>,
    pub upward: Rc<Cell<Option<bool>>>,
    pub final_height: Pixels,
}

impl IntoElement for NestedMenuOverlay {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for NestedMenuOverlay {
    type RequestLayoutState = gpui_kit::Size<Pixels>;
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
    ) -> (LayoutId, Self::RequestLayoutState) {
        let measured = self.content.layout_as_root(
            size(AvailableSpace::MaxContent, AvailableSpace::MinContent),
            window,
            cx,
        );
        // Deferred popup layout does not participate in the event row's size.
        let id = window.request_layout(
            Style {
                position: Position::Absolute,
                ..Default::default()
            },
            [],
            cx,
        );
        (id, measured)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        measured: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let trigger = self.trigger.get();
        let unit = window.rem_size() / 16.;
        let upward = self.upward.get().unwrap_or_else(|| {
            let upward =
                trigger.bottom() - self.viewport.get().top() + unit * 2. + self.final_height
                    >= unit * 688.;
            self.upward.set(Some(upward));
            upward
        });
        // `.s3-options`: down top:27 + margin-top:1; up bottom:28.
        // Side is fixed at opening; a growing up-menu stays against its trigger.
        let top = if upward {
            trigger.top() - unit - measured.height
        } else {
            trigger.bottom() + unit
        };
        self.content
            .prepaint_at(point(trigger.left(), top), window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.content.paint(window, cx);
    }
}
