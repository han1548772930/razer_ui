//! ButtonPanel's mapped binding overflow callback and its independent tip.
//! Current generic source: keyboard-button-panel-current.json.gz.
//! Actual KeypadButtonPanel callers do not mount this parent tooltip.
use super::*;

#[derive(Default)]
pub(super) struct State {
    pub(super) data: String,
    pub(super) shown: bool,
    pub(super) position: Rc<Cell<Option<Pixels>>>,
}
#[derive(IntoElement)]
pub(super) struct Binding {
    pub(super) index: usize,
    pub(super) label: String,
    pub(super) raw: String,
    pub(super) mapped: bool,
    pub(super) row: Rc<Cell<Bounds<Pixels>>>,
    pub(super) owner: WeakEntity<KeyboardProductWorkspace>,
}
fn binding_surface() -> Div {
    div()
        .font_family("Roboto")
        .text_size(surface::css(14.))
        .line_height(surface::css(16.))
}
impl RenderOnce for Binding {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut measurement = binding_surface()
            .child(self.label.clone())
            .into_any_element();
        // Normal whitespace is retained: wrapped words do not cause horizontal
        // overflow. A word's min-content width can exceed the binding clientWidth.
        let minimum = measurement.layout_as_root(
            size(AvailableSpace::MinContent, AvailableSpace::MinContent),
            window,
            cx,
        );
        let overflow = Rc::new(Cell::new(false));
        let measured_overflow = overflow.clone();
        binding_surface()
            .id(("keyboard-panel-binding", self.index))
            .w_full()
            .overflow_hidden()
            .text_ellipsis()
            .on_prepaint(move |rect, _, _| {
                measured_overflow.set(minimum.width > rect.size.width);
            })
            .on_hover(move |entered, window, cx| {
                let _ = self.owner.update(cx, |this, cx| {
                    let state = &mut this.analog_gamepad.mapping.panel_value;
                    if *entered && self.mapped && overflow.get() {
                        state.shown = true;
                        // The panel tooltip prints the raw assignmentValue,
                        // while the binding itself uses the source translation.
                        state.data = self.raw.clone();
                        state.position.set(Some(
                            self.row.get().top() + surface::css(50.).to_pixels(window.rem_size()),
                        ));
                        cx.notify();
                    } else if !state.data.is_empty() {
                        state.shown = false;
                        state.data.clear();
                        state.position.set(None);
                        cx.notify();
                    }
                });
            })
            .child(self.label)
    }
}

#[derive(IntoElement)]
pub(super) struct Tip {
    pub(super) data: String,
    pub(super) shown: bool,
    pub(super) position: Rc<Cell<Option<Pixels>>>,
    pub(super) viewport: Rc<Cell<Bounds<Pixels>>>,
}
fn tip_surface() -> Div {
    div()
        .font_family("Roboto")
        .text_size(surface::css(14.))
        .line_height(surface::css(16.))
        .max_w(surface::css(358.))
        .px(surface::css(10.))
        .py(surface::css(8.))
        .bg(rgb(0x000000))
        .border_1()
        .border_color(rgb(0x5d5d5d))
        .text_color(rgb(0xcccccc))
        .text_left()
}
impl RenderOnce for Tip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut measure = tip_surface().child(self.data.clone()).into_any_element();
        let size = measure.layout_as_root(
            size(AvailableSpace::MaxContent, AvailableSpace::MinContent),
            window,
            cx,
        );
        let viewport = self.viewport.get();
        let available =
            (viewport.size.width - surface::css(61.).to_pixels(window.rem_size())).max(px(0.));
        let child = tip_surface()
            .w(size.width.min(available))
            .when(!self.shown, |tip| tip.invisible().opacity(0.))
            .child(self.data)
            .into_any_element();
        TipPosition {
            child,
            viewport: self.viewport,
            position: self.position,
            shown: self.shown,
        }
    }
}
struct TipPosition {
    child: AnyElement,
    viewport: Rc<Cell<Bounds<Pixels>>>,
    position: Rc<Cell<Option<Pixels>>>,
    shown: bool,
}
impl IntoElement for TipPosition {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for TipPosition {
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
        let mut top = self.position.get().unwrap_or(px(0.));
        let height = window.layout_bounds(*child).size.height;
        if self.shown && top + height > window.viewport_size().height {
            // componentDidUpdate changes the retained position itself. Repeated
            // overflow after later resizes therefore keeps the source subtraction.
            top -= height + surface::css(50.).to_pixels(window.rem_size());
            self.position.set(Some(top));
        }
        let origin = point(
            self.viewport.get().left() + surface::css(61.).to_pixels(window.rem_size()),
            top,
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
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}
