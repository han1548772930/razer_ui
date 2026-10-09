//! 1383 `58837:p/h`: portal placement, including the source horizontal 8px guard.
use super::*;
use gpui_kit::base::{ElementExt as _, Tooltip};
use std::{cell::Cell, rc::Rc, time::Duration};

#[derive(Default)]
struct TipState {
    hovered: bool,
    anchor: Rc<Cell<Bounds<Pixels>>>,
}

pub(super) fn requires_synapse(id: String, window: &mut Window, cx: &mut App) -> AnyElement {
    trigger(
        id,
        label("vs6"),
        img("synapse/audio-oled-1383-requires-synapse.svg")
            .size_full()
            .into_any_element(),
        TipKind::Requires,
        window,
        cx,
    )
}

pub(super) fn artwork(
    id: String,
    text: String,
    button: BaseButton,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    trigger(
        id,
        text,
        button.into_any_element(),
        TipKind::Artwork,
        window,
        cx,
    )
}

#[derive(Clone, Copy, PartialEq)]
enum TipKind {
    Requires,
    Artwork,
}

fn trigger(
    id: String,
    text: String,
    content: AnyElement,
    kind: TipKind,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let id = ElementId::from(id);
    let state = window.use_keyed_state((id.clone(), "hover-tip"), cx, |_, _| TipState::default());
    let hovered = state.read(cx).hovered;
    let anchor = state.read(cx).anchor.clone();
    div()
        .id((id.clone(), "trigger"))
        .relative()
        .w(surface::css(if kind == TipKind::Artwork {
            28.
        } else {
            15.
        }))
        .h(surface::css(if kind == TipKind::Artwork {
            27.
        } else {
            15.
        }))
        .when(kind == TipKind::Artwork, |el| el.m(surface::css(2.)))
        .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
            state.hovered = *hovered;
            cx.notify();
        }))
        .on_prepaint({
            let anchor = anchor.clone();
            move |bounds, _, _| anchor.set(bounds)
        })
        .child(content)
        .when(hovered, |el| {
            el.child(
                deferred(SourceOverlay {
                    id,
                    anchor,
                    text,
                    kind,
                    content: None,
                })
                .with_priority(1060),
            )
        })
        .into_any_element()
}

struct SourceOverlay {
    id: ElementId,
    anchor: Rc<Cell<Bounds<Pixels>>>,
    content: Option<AnyElement>,
    text: String,
    kind: TipKind,
}
impl IntoElement for SourceOverlay {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for SourceOverlay {
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
        let mut content = div()
            .w(surface::css(300.))
            .flex()
            .justify_start()
            .child(
                Tooltip::new((self.id.clone(), "surface"))
                    .px(surface::css(10.))
                    .py(surface::css(8.))
                    .border_1()
                    .border_color(Colors::border())
                    .bg(Colors::black())
                    .text_color(Colors::text())
                    .font_family("Roboto")
                    .text_size(surface::css(14.))
                    .line_height(surface::css(16.))
                    .text_left()
                    .whitespace_normal()
                    .child(self.text.clone()),
            )
            .with_animation(
                (self.id.clone(), "fade"),
                Animation::new(Duration::from_millis(100)),
                |el, delta| el.opacity(delta),
            )
            .into_any_element();
        let child = content.request_layout(window, cx);
        self.content = Some(content);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    ..Style::default()
                },
                [child],
                cx,
            ),
            (),
        )
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
        let anchor = self.anchor.get();
        let unit = window.rem_size() / 16.;
        let width = unit * 300.;
        // p(bottom-right) + each caller's own CSS margin-left.
        let margin = if self.kind == TipKind::Artwork {
            270.
        } else {
            285.
        };
        let mut left = anchor.left() - unit * 320. + anchor.size.width + unit * margin;
        let right_limit = window.viewport_size().width - unit * 8.;
        // h checks the right edge before the left edge; it does not flip vertically.
        if left + width > right_limit {
            left = right_limit - width;
        } else if left < unit * 8. {
            left = unit * 8.;
        }
        let top = anchor.bottom()
            + unit
                * if self.kind == TipKind::Artwork {
                    5.
                } else {
                    2.
                };
        // Base Positioner always clamps both axes. This narrow source adapter
        // delegates layout/semantics to GPUI/Tooltip and preserves h's x-only rule.
        window.with_element_offset(point(left, top) - bounds.origin, |window| {
            self.content.as_mut().unwrap().prepaint(window, cx);
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
        self.content.as_mut().unwrap().paint(window, cx);
    }
}
