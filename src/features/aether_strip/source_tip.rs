//! Current 784 `.tip` and Fd help portal. Base owns placement and tooltip semantics.
use super::*;
use gpui_kit::base::{
    Positioner, Tooltip,
    motion::{self, Easing, Transition},
};
use std::{cell::Cell, rc::Rc, time::Duration};

type Click = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
#[derive(Clone, Copy)]
enum Kind {
    Icon,
    Help,
}
#[derive(IntoElement)]
pub(super) struct TipCommand {
    id: ElementId,
    name: &'static str,
    label: String,
    disabled: bool,
    kind: Kind,
    click: Option<Click>,
    style: StyleRefinement,
}
impl TipCommand {
    pub(super) fn icon(
        id: impl Into<ElementId>,
        name: &'static str,
        label: String,
        disabled: bool,
    ) -> Self {
        Self {
            id: id.into(),
            name,
            label,
            disabled,
            kind: Kind::Icon,
            click: None,
            style: StyleRefinement::default(),
        }
    }
    pub(super) fn help(label: String) -> Self {
        Self {
            id: "aether-layout-help".into(),
            name: "tooltip_questionmark",
            label,
            disabled: false,
            kind: Kind::Help,
            click: None,
            style: StyleRefinement::default(),
        }
    }
    pub(super) fn on_click(
        mut self,
        click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.click = Some(Box::new(click));
        self
    }
}
impl Styled for TipCommand {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
#[derive(Default)]
struct TipState {
    hover: bool,
    bounds: Rc<Cell<Bounds<Pixels>>>,
}
impl RenderOnce for TipCommand {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state((self.id.clone(), "tooltip-state"), cx, |_, _| {
            TipState::default()
        });
        let shown = state.read(cx).hover && !self.disabled;
        let bounds = state.read(cx).bounds.clone();
        let opacity = match self.kind {
            Kind::Help => {
                if shown {
                    1.
                } else {
                    0.
                }
            }
            Kind::Icon => motion::transition(
                (self.id.clone(), "tooltip-opacity"),
                if shown { 1. } else { 0. },
                Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
                window,
                cx,
            ),
        };
        let help = matches!(self.kind, Kind::Help);
        let mut button = gpui_kit::base::Button::new(self.id.clone())
            .disabled(self.disabled)
            .accessibility_label(self.label.clone())
            .size_full()
            .p_0()
            .focus_visible(|v| v.border_1().border_color(cx.theme().primary));
        if help {
            let color = motion::transition(
                (self.id.clone(), "help-background"),
                if shown {
                    cx.theme().button_foreground.opacity(0.3)
                } else {
                    crate::ui::theme::TooltipColors::help_background()
                },
                Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
                window,
                cx,
            );
            button = button
                .rounded(surface::css(7.))
                .bg(color)
                .child(img(asset(self.name)).size_full());
        } else {
            button = button.child(presentation::ActionIcon {
                id: self.id.clone(),
                name: self.name,
                disabled: self.disabled,
            });
        }
        if let Some(click) = self.click {
            button = button.on_click(move |event, window, cx| click(event, window, cx));
        }
        div()
            .id((self.id.clone(), "tip-trigger"))
            .relative()
            .size(surface::css(if help { 14. } else { 27. }))
            .refine_style(&self.style)
            .on_hover(window.listener_for(&state, |state, hover, _, cx| {
                state.hover = *hover;
                cx.notify();
            }))
            .on_prepaint({
                let bounds = bounds.clone();
                move |value, _, _| bounds.set(value)
            })
            .child(button)
            .child(
                deferred(TipLayer {
                    id: self.id,
                    label: self.label,
                    bounds,
                    kind: self.kind,
                    shown,
                    opacity,
                })
                .with_priority(200),
            )
    }
}
struct TipLayer {
    id: ElementId,
    label: String,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    kind: Kind,
    shown: bool,
    opacity: f32,
}
struct TipLayout {
    positioner: Option<Positioner>,
    layout: <Positioner as Element>::RequestLayoutState,
    size: gpui_kit::Size<Pixels>,
}
impl IntoElement for TipLayer {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}
impl Element for TipLayer {
    type RequestLayoutState = TipLayout;
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
    ) -> (LayoutId, TipLayout) {
        let width = window.rem_size() * (300. / 16.);
        let foreground = cx.theme().foreground;
        let surface = |id: ElementId| {
            Tooltip::new(id)
                .w(width)
                .px(surface::css(10.))
                .py(surface::css(8.))
                .border_1()
                .border_color(Colors::border())
                .bg(Colors::dialog())
                .text_color(foreground)
                .font_family("Roboto")
                .text_size(surface::css(14.))
                .line_height(surface::css(18.))
                .whitespace_normal()
                .child(self.label.clone())
        };
        let mut measure = surface((self.id.clone(), "tip-measure").into()).into_any_element();
        let size = measure.layout_as_root(
            size(AvailableSpace::Definite(width), AvailableSpace::MinContent),
            window,
            cx,
        );
        let content = div()
            .opacity(self.opacity)
            .when(!self.shown, |v| v.invisible())
            .child(surface((self.id.clone(), "tip-surface").into()));
        let mut positioner = Positioner::corner(Anchor::TopLeft, Point::default())
            .margin(px(0.))
            .child(content);
        let (id, layout) = positioner.request_layout(None, None, window, cx);
        (
            id,
            TipLayout {
                positioner: Some(positioner),
                layout,
                size,
            },
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut TipLayout,
        window: &mut Window,
        cx: &mut App,
    ) {
        let trigger = self.bounds.get();
        let scale = window.rem_size() / 16.;
        let mut point = match self.kind {
            Kind::Icon => trigger.origin + point(scale * 2., scale * 34.),
            // Fd: widget.right - 14 - tip.width, widget.top + 34.
            Kind::Help => point(
                trigger.right() - scale * 4. - layout.size.width,
                trigger.top() + scale * 24.,
            ),
        };
        if matches!(self.kind, Kind::Help) {
            let viewport = window.viewport_size();
            point.x = point
                .x
                .max(scale * 14.)
                .min((viewport.width - scale * 14. - layout.size.width).max(scale * 14.));
            if point.y + layout.size.height > viewport.height {
                point.x = trigger.right() + scale * 8.;
                point.y = trigger.top();
                if point.x + layout.size.width > viewport.width {
                    point.x = trigger.left() - scale * 8. - layout.size.width;
                }
                if point.y + layout.size.height > viewport.height {
                    point.y = (viewport.height - layout.size.height - scale * 10.).max(px(0.));
                }
            }
        }
        let mut positioner = layout.positioner.take().unwrap().position(point);
        positioner.prepaint(None, None, bounds, &mut layout.layout, window, cx);
        layout.positioner = Some(positioner);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut TipLayout,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        layout.positioner.as_mut().unwrap().paint(
            None,
            None,
            bounds,
            &mut layout.layout,
            &mut (),
            window,
            cx,
        );
    }
}
