//! Current common header's overflow-only tip; TwoTap does not mount it.
//! Source: keyboard-mapping-header-current.json.gz, all 72 products.
use super::*;

#[derive(Default)]
pub(super) struct State {
    pub(super) bounds: Rc<Cell<Bounds<Pixels>>>,
    pub(super) tip_bounds: Rc<Cell<Bounds<Pixels>>>,
    pub(super) hovered: bool,
}

#[derive(IntoElement)]
pub(super) struct Title {
    pub(super) label: String,
    pub(super) owner: WeakEntity<KeyboardProductWorkspace>,
    pub(super) bounds: Rc<Cell<Bounds<Pixels>>>,
    pub(super) tip_bounds: Rc<Cell<Bounds<Pixels>>>,
    pub(super) hovered: bool,
}

fn text_surface() -> Div {
    div()
        .font_family("Roboto")
        .text_size(surface::css(14.))
        .line_height(surface::css(17.))
        .whitespace_nowrap()
}

impl RenderOnce for Title {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut full = text_surface().child(self.label.clone()).into_any_element();
        let natural = full.layout_as_root(
            size(AvailableSpace::MaxContent, AvailableSpace::MinContent),
            window,
            cx,
        );
        // The source's span has max-width:200px; updateValue compares its
        // scrollWidth/clientWidth after mounting, visibility and language changes.
        let overflow = natural.width > surface::css(200.).to_pixels(window.rem_size());
        let label = text_surface()
            .max_w(surface::css(200.))
            .overflow_hidden()
            .text_ellipsis()
            .child(self.label.clone());
        let tip = (overflow && self.hovered).then(|| {
            let owner = self.owner;
            let tip_bounds = self.tip_bounds.clone();
            let tooltip = text_surface()
                .id("keyboard-mapping-header-tip")
                .w(natural.width + surface::css(20.).to_pixels(window.rem_size()) + px(2.))
                .p(surface::css(10.))
                .bg(rgb(0x000000))
                .border_1()
                .border_color(rgb(0x5d5d5d))
                .text_color(rgb(0x999999))
                .text_center()
                .on_prepaint(move |rect, _, _| tip_bounds.set(rect))
                .on_hover(move |hover, window, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        let header = &mut this.analog_gamepad.mapping.header;
                        let hovered =
                            *hover || header.bounds.get().contains(&window.mouse_position());
                        if header.hovered != hovered {
                            header.hovered = hovered;
                            cx.notify();
                        }
                    });
                })
                .child(self.label)
                .into_any_element();
            deferred(tip_position(
                self.bounds,
                point(
                    surface::css(-105.).to_pixels(window.rem_size()),
                    surface::css(35.).to_pixels(window.rem_size()),
                ),
                true,
                tooltip,
            ))
            .with_priority(106)
        });
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .justify_center()
            .child(label)
            .children(tip)
    }
}

// A source absolute descendant, without toolkit viewport fitting or clipping
// to the native popup's rounded body. Source left:calc(50% - 105px), top:35px.
pub(super) struct TipPosition {
    header: Rc<Cell<Bounds<Pixels>>>,
    offset: Point<Pixels>,
    centered: bool,
    child: AnyElement,
}
pub(super) fn tip_position(
    trigger: Rc<Cell<Bounds<Pixels>>>,
    offset: Point<Pixels>,
    centered: bool,
    child: AnyElement,
) -> TipPosition {
    TipPosition {
        header: trigger,
        offset,
        centered,
        child,
    }
}
impl IntoElement for TipPosition {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for TipPosition {
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
        let child = self.child.request_layout(window, cx);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    display: Display::Flex,
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
        let header = self.header.get();
        let origin = point(
            header.left()
                + if self.centered {
                    header.size.width / 2.
                } else {
                    px(0.)
                }
                + self.offset.x,
            header.top() + self.offset.y,
        );
        window.with_element_offset(origin - bounds.origin, |window| {
            self.child.prepaint(window, cx)
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
        self.child.paint(window, cx);
    }
}
