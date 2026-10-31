//! 7003.fi/gi portals for command warnings and overflowing settings headings.
use super::*;
use gpui_kit::base::Tooltip;
use razer_widgets::surface;
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy)]
enum Placement {
    Command,
    Heading,
}

#[derive(Default)]
struct HoverState {
    hovered: bool,
    anchor: Rc<Cell<Bounds<Pixels>>>,
}

fn tooltip_origin(
    placement: Placement,
    anchor: Bounds<Pixels>,
    measured: Size<Pixels>,
    viewport: Size<Pixels>,
    unit: Pixels,
) -> Point<Pixels> {
    let margin = unit * 8.;
    let below = anchor.bottom() + unit * 5.;
    if below + measured.height > viewport.height - margin {
        return point(
            anchor.right() - measured.width,
            anchor.top() - measured.height - unit * 5.,
        );
    }
    let (left, width) = match placement {
        Placement::Command => (anchor.right() - unit * 320., unit * 300.),
        Placement::Heading => (anchor.left(), measured.width),
    };
    point(left.min(viewport.width - margin - width).max(margin), below)
}

struct Popup {
    content: AnyElement,
    anchor: Rc<Cell<Bounds<Pixels>>>,
    placement: Placement,
}

impl IntoElement for Popup {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for Popup {
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
    ) -> (LayoutId, Self::RequestLayoutState) {
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
        measured: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let origin = tooltip_origin(
            self.placement,
            self.anchor.get(),
            *measured,
            window.viewport_size(),
            window.rem_size() / 16.,
        );
        window.with_content_mask(None, |window| self.content.prepaint_at(origin, window, cx));
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_content_mask(None, |window| self.content.paint(window, cx));
    }
}

fn popup(
    id: ElementId,
    text: String,
    anchor: Rc<Cell<Bounds<Pixels>>>,
    placement: Placement,
    opacity: f32,
) -> impl IntoElement {
    razer_assets::deferred_resource(Popup {
        content: Tooltip::new((id, "popup"))
            .w_auto()
            .max_w(css(300.))
            .px(css(10.))
            .py(css(8.))
            .border_1()
            .border_color(rgb(0x5d5d5d))
            .bg(rgb(0))
            .text_color(rgb(0xcccccc))
            .font_family("Roboto")
            .text_size(css(14.))
            .line_height(css(16.))
            .whitespace_normal()
            .text_left()
            .opacity(opacity)
            .child(text)
            .into_any_element(),
        anchor,
        placement,
    })
    .priority(1060)
}

pub(super) fn command_warning(
    index: usize,
    disabled: bool,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let id: ElementId = ("macro-command-warning", index).into();
    let state = window.use_keyed_state(id.clone(), cx, |_, _| HoverState::default());
    let anchor = state.read(cx).anchor.clone();
    let hovered = state.read(cx).hovered && !disabled;
    let alpha = surface::fade_opacity(id.clone(), if hovered { 1. } else { 0. }, 100, window, cx);
    div()
        .id(id.clone())
        .size(css(20.))
        .mr(css(16.))
        .flex_shrink_0()
        .on_prepaint({
            let anchor = anchor.clone();
            move |bounds, _, _| anchor.set(bounds)
        })
        .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
            state.hovered = *hovered;
            cx.notify();
        }))
        .child(img("synapse/macro/warning.svg").size_full())
        .when(hovered, |view| {
            view.child(popup(
                id,
                tr("TEXT_TOOLTIP_COMMAND_WARNING"),
                anchor,
                Placement::Command,
                alpha,
            ))
        })
        .into_any_element()
}

pub(super) fn heading(index: usize, text: String, window: &mut Window, cx: &mut App) -> AnyElement {
    let id: ElementId = ("macro-record-heading", index).into();
    let state = window.use_keyed_state(id.clone(), cx, |_, _| HoverState::default());
    let anchor = state.read(cx).anchor.clone();
    let overflowing = surface::label_width(&text, 14., window) * f32::from(window.rem_size()) / 16.
        > f32::from(anchor.get().size.width);
    let hovered = state.read(cx).hovered && overflowing;
    let alpha = surface::fade_opacity(id.clone(), if hovered { 1. } else { 0. }, 100, window, cx);
    div()
        .id(id.clone())
        .flex_1()
        .min_w_0()
        .relative()
        .on_prepaint({
            let anchor = anchor.clone();
            move |bounds, _, _| anchor.set(bounds)
        })
        .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
            state.hovered = *hovered;
            cx.notify();
        }))
        .child(div().text_ellipsis().child(text.clone()))
        .when(hovered, |view| {
            view.child(popup(id, text, anchor, Placement::Heading, alpha))
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn command_and_heading_tips_use_the_source_offsets_and_eight_pixel_edge() {
        let viewport = size(px(800.), px(600.));
        let measured = size(px(250.), px(50.));
        let anchor = Bounds::new(point(px(500.), px(100.)), size(px(20.), px(20.)));
        assert_eq!(
            tooltip_origin(Placement::Command, anchor, measured, viewport, px(1.)),
            point(px(200.), px(125.))
        );
        assert_eq!(
            tooltip_origin(Placement::Heading, anchor, measured, viewport, px(1.)),
            point(px(500.), px(125.))
        );
        let near_bottom = Bounds::new(point(px(500.), px(522.)), size(px(20.), px(20.)));
        assert_eq!(
            tooltip_origin(Placement::Command, near_bottom, measured, viewport, px(1.)),
            point(px(270.), px(467.))
        );
        let near_left = Bounds::new(point(px(10.), px(100.)), size(px(20.), px(20.)));
        assert_eq!(
            tooltip_origin(Placement::Command, near_left, measured, viewport, px(1.)),
            point(px(8.), px(125.))
        );
        let near_right = Bounds::new(point(px(780.), px(100.)), size(px(20.), px(20.)));
        assert_eq!(
            tooltip_origin(Placement::Heading, near_right, measured, viewport, px(1.)),
            point(px(542.), px(125.))
        );
    }
}
