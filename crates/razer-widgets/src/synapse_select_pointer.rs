//! Source 190/229/239 module 7734 + option 7278. These mounted divs have no
//! keyboard/focus behavior; source placement uses final menu height and a 1px gap.
use super::*;
use gpui_kit::base::{Button as BaseButton, Positioner};
use gpui_kit::component::v_flex;

impl<I: SelectItem<Value = String> + 'static> SynapseSelectView<I> {
    pub(super) fn render_pointer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let options = self
            .list
            .read(cx)
            .delegate()
            .items
            .iter()
            .map(|item| (item.value().clone(), item.title(), item.disabled()))
            .collect::<Vec<_>>();
        let selected = self.state.read(cx).selected_value().cloned();
        let current = options
            .iter()
            .find(|(value, _, _)| Some(value) == selected.as_ref());
        let Some(title) = self
            .selected_text
            .clone()
            .or_else(|| current.map(|current| current.1.clone()))
        else {
            return div().into_any_element();
        };
        let disabled = self.disabled || options.len() <= 1;
        let open = self.open && !disabled;
        let colors = DropdownColors::new();
        let bounds = self.pointer_bounds.clone();
        let border = motion::transition(
            (self.id.clone(), "source-pointer-border"),
            if !disabled && (open || self.hovered) {
                cx.theme().primary
            } else {
                cx.theme().input
            },
            Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
            window,
            cx,
        );
        let arrow = motion::transition(
            (self.id.clone(), "source-pointer-arrow"),
            if open { std::f32::consts::PI } else { 0. },
            Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
            window,
            cx,
        );
        let click = self.on_trigger_click.clone();
        let trigger = BaseButton::new("source-pointer-trigger")
            .focusable(false)
            .disabled(disabled)
            .accessibility_label(self.label.clone().unwrap_or_else(|| title.clone()))
            .w_full()
            .h(css(27.))
            .border_1()
            .border_color(border)
            .text_size(css(14.))
            .line_height(css(17.))
            .text_color(cx.theme().foreground)
            .relative()
            .when(disabled, |button| button.opacity(0.3))
            .on_hover(cx.listener(|this, hovered, _, cx| {
                this.hovered = *hovered;
                cx.notify();
            }))
            .child(
                div()
                    .absolute()
                    .left(css(5.))
                    .top(css(4.))
                    .right(css(19.))
                    .truncate()
                    .child(title.clone()),
            )
            .child(
                div()
                    .absolute()
                    .right_0()
                    .top_0()
                    .w(css(29.))
                    .h(css(25.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::default()
                            .path("synapse/expand.svg")
                            .size(css(10.))
                            .transform(Transformation::rotate(radians(arrow))),
                    ),
            )
            .on_click(cx.listener(move |this, event, window, cx| {
                if !disabled {
                    this.set_open(!this.open, window, cx);
                }
                if let Some(click) = &click {
                    click(event, window, cx);
                }
            }));
        let menu_height = (options.len() as f32 * 25. + 2.).min(180.);
        let rows = options
            .iter()
            .map(|(value, title, disabled)| {
                let value = value.clone();
                let checked = selected.as_ref() == Some(&value);
                let disabled = *disabled;
                BaseButton::new(SharedString::from(format!("source-pointer-option-{value}")))
                    .focusable(false)
                    .disabled(disabled)
                    .w_full()
                    .h(css(25.))
                    .flex_shrink_0()
                    .p(css(4.))
                    .text_size(css(14.))
                    .line_height(css(17.))
                    .justify_start()
                    .text_color(if checked {
                        cx.theme().primary
                    } else {
                        cx.theme().foreground
                    })
                    .when(!disabled, |row| {
                        row.hover(|style| style.bg(colors.hover()))
                            .active(|style| style.text_color(cx.theme().primary))
                    })
                    .when(disabled, |row| row.opacity(0.3))
                    .child(div().w_full().truncate().child(title.clone()))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.confirm(value.clone(), window, cx)
                    }))
            })
            .collect::<Vec<_>>();
        let width = self.width;
        div()
            .id("source-pointer-dropdown")
            .relative()
            .w_full()
            .h(css(27.))
            .refine_style(&self.style)
            .on_prepaint({
                let owner = cx.entity().downgrade();
                let bounds = bounds.clone();
                move |current, _, cx| {
                    bounds.set(current);
                    let _ = owner.update(cx, |this, _| this.width = current.size.width);
                }
            })
            .child(trigger)
            .when(open, |root| {
                let reveal = Presence::new((self.id.clone(), "source-pointer-reveal"), true)
                    .transition(Transition::new(Duration::from_millis(200)).easing(Easing::Ease))
                    .sample(window, cx)
                    .progress;
                let content = div()
                    .id("source-pointer-options")
                    .w(width)
                    .h(css(menu_height))
                    .child(
                        v_flex()
                            .id("source-pointer-options-clip")
                            .w_full()
                            .h(css(menu_height.min(180. * reveal)))
                            .border_1()
                            .border_color(cx.theme().input)
                            .bg(colors.surface())
                            .overflow_hidden()
                            .on_mouse_up_out(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseUpEvent, window, cx| {
                                    // Source window.click ignores both trigger and option descendants.
                                    if !this.pointer_bounds.get().contains(&event.position) {
                                        this.set_open(false, window, cx);
                                    }
                                }),
                            )
                            .children(rows),
                    )
                    .into_any_element();
                root.child(
                    razer_assets::deferred_resource(PointerOptions {
                        bounds,
                        height: css(menu_height).to_pixels(window.rem_size()),
                        content: Some(content),
                    })
                    .with_priority(103),
                )
            })
            .into_any_element()
    }
}

/// Resolve after trigger prepaint. Base Positioner owns clipping and painting;
/// the source's 27/28px vertical placement does not add a default viewport inset.
struct PointerOptions {
    bounds: Rc<std::cell::Cell<Bounds<Pixels>>>,
    height: Pixels,
    content: Option<AnyElement>,
}
impl IntoElement for PointerOptions {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for PointerOptions {
    type RequestLayoutState = (Positioner, <Positioner as Element>::RequestLayoutState);
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
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut positioner = Positioner::corner(Anchor::TopLeft, Point::default())
            .margin(px(0.))
            .occlude()
            .child(self.content.take().unwrap());
        let (layout, state) = positioner.request_layout(None, None, window, cx);
        (layout, (positioner, state))
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let trigger = self.bounds.get();
        let gap = window.rem_size() / 16.;
        // Source uses bottom+2 for the decision; its actual CSS top27 plus
        // margin-top1 (or bottom28 above) leaves a single pixel at either side.
        let y = if trigger.bottom() + gap * 2. + self.height > window.viewport_size().height {
            trigger.top() - gap - self.height
        } else {
            trigger.bottom() + gap
        };
        state.0 = std::mem::replace(
            &mut state.0,
            Positioner::corner(Anchor::TopLeft, Point::default()),
        )
        .position(point(trigger.left(), y));
        state
            .0
            .prepaint(None, None, bounds, &mut state.1, window, cx);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        state
            .0
            .paint(None, None, bounds, &mut state.1, &mut (), window, cx);
    }
}
