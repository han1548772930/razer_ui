//! Native presentation of current Settings SVG class transitions and tooltips.
use gpui_kit::base::{
    Button, ElementExt as _, Link, Tooltip,
    motion::{self, Easing, Presence, Transition},
};
use gpui_kit::{component::ActiveTheme as _, prelude::FluentBuilder as _, *};
use razer_widgets::surface;
use razer_widgets::surface::css;
use std::{cell::Cell, rc::Rc, time::Duration};
#[path = "settings_window_theme.rs"]
mod theme;

#[derive(Default)]
struct Hover {
    hovered: bool,
    icon_hovered: bool,
    bounds: Rc<Cell<Bounds<Pixels>>>,
}

fn outline_contains(
    position: Point<Pixels>,
    bounds: Bounds<Pixels>,
    insider: bool,
    stroke: f32,
    unit: f32,
) -> bool {
    let x = f32::from(position.x - bounds.origin.x) / unit;
    let y = f32::from(position.y - bounds.origin.y) / unit;
    if !insider {
        return (x - 14.).powi(2) + (y - 14.).powi(2) <= (13. + stroke / 2.).powi(2);
    }
    let radius = 4. + stroke / 2.;
    let dx = (x - 135.).abs() - (134. + stroke / 2. - radius);
    let dy = (y - 25.).abs() - (24. + stroke / 2. - radius);
    dx.max(0.).powi(2) + dy.max(0.).powi(2) <= radius.powi(2)
}

fn tip(id: ElementId, label: SharedString, toolbar: bool) -> Tooltip {
    Tooltip::new(id)
        .font_family("Roboto")
        .text_size(css(14.))
        .line_height(css(if toolbar { 16. } else { 14. * 1.22 }))
        .whitespace_nowrap()
        .px(css(10.))
        .py(css(8.))
        .border_1()
        .border_color(theme::tooltip_border())
        .bg(theme::tooltip_surface())
        .text_color(theme::tooltip_text())
        .child(label)
}

#[derive(IntoElement)]
pub struct SocialLink {
    id: SharedString,
    name: SharedString,
    label: SharedString,
    href: SharedString,
    insider: bool,
}
impl SocialLink {
    pub fn new(
        name: impl Into<SharedString>,
        label: impl Into<SharedString>,
        href: impl Into<SharedString>,
    ) -> Self {
        let name = name.into();
        Self {
            id: format!("settings-social-{name}").into(),
            insider: name == "insider",
            name,
            label: label.into(),
            href: href.into(),
        }
    }
}
impl RenderOnce for SocialLink {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state =
            window.use_keyed_state((ElementId::from(self.id.clone()), "hover"), cx, |_, _| {
                Hover::default()
            });
        let hovered = state.read(cx).icon_hovered;
        let tooltip_hovered = state.read(cx).hovered;
        let bounds = state.read(cx).bounds.clone();
        let insider = self.insider;
        let transition = Transition::new(Duration::from_millis(200)).easing(Easing::Ease);
        let stroke = motion::transition(
            (ElementId::from(self.id.clone()), "stroke-width"),
            if hovered {
                1.5_f32
            } else if self.insider {
                0.
            } else {
                1.
            },
            transition,
            window,
            cx,
        );
        let color = surface::fade_color(
            self.id.clone(),
            if hovered || self.insider {
                theme::accent()
            } else {
                theme::social()
            },
            200,
            window,
            cx,
        );
        let (width, height) = if self.insider {
            (270., 50.)
        } else {
            (28., 28.)
        };
        // SVG strokes straddle their path. Expand the native border box by half
        // its width so the centerline stays at (14,14), r=13 (or rect x/y=1).
        let outline = div()
            .absolute()
            .left(css(1. - stroke / 2.))
            .top(css(1. - stroke / 2.))
            .w(css(width - 2. + stroke))
            .h(css(height - 2. + stroke))
            .rounded(css(if self.insider {
                4. + stroke / 2.
            } else {
                13. + stroke / 2.
            }))
            .border_t(css(stroke))
            .border_r(css(stroke))
            .border_b(css(stroke))
            .border_l(css(stroke))
            .border_color(color);
        let mut link = Link::new(self.id.clone())
            .href(self.href)
            .accessibility_label(self.label.clone())
            .open_with(|url, _, _, cx| cx.open_url(url))
            .relative()
            .flex_shrink_0()
            .w(css(width))
            .h(css(height))
            .cursor_default()
            .focus_visible(|s| s.bg(cx.theme().secondary_hover))
            .child(outline);
        if self.insider {
            for (class, color) in [
                ("razergreen", theme::accent()),
                ("grey", theme::insider_text()),
            ] {
                link = link.child(
                    svg()
                        .absolute()
                        .inset_0()
                        .size_full()
                        .path(SharedString::from(format!(
                            "synapse/settings-window-social-insider-{class}-mask.svg"
                        )))
                        .text_color(color),
                );
            }
        } else {
            link = link.child(
                svg()
                    .absolute()
                    .inset_0()
                    .size_full()
                    .path(SharedString::from(format!(
                        "synapse/settings-window-social-{}-social-mask.svg",
                        self.name
                    )))
                    .text_color(color),
            );
        }
        let mut wrapper = div()
            .id((ElementId::from(self.id.clone()), "item"))
            .relative()
            .flex_shrink_0()
            .w(css(width))
            .h(css(height))
            .on_prepaint(move |rect, _, _| bounds.set(rect))
            .on_hover(
                window.listener_for(&state, move |state, hovered, window, cx| {
                    state.hovered = *hovered;
                    state.icon_hovered = *hovered
                        && outline_contains(
                            window.mouse_position(),
                            state.bounds.get(),
                            insider,
                            stroke,
                            f32::from(window.rem_size()) / 16.,
                        );
                    cx.notify();
                }),
            )
            .on_mouse_move(window.listener_for(
                &state,
                move |state, event: &MouseMoveEvent, window, cx| {
                    let hovered = outline_contains(
                        event.position,
                        state.bounds.get(),
                        insider,
                        stroke,
                        f32::from(window.rem_size()) / 16.,
                    );
                    if state.icon_hovered != hovered {
                        state.icon_hovered = hovered;
                        cx.notify();
                    }
                },
            ))
            .child(link);
        if tooltip_hovered && !self.insider {
            let tooltip_id: ElementId = (ElementId::from(self.id), "tooltip").into();
            let mut measure = tip(tooltip_id.clone(), self.label.clone(), false).into_any_element();
            let bounds = measure.layout_as_root(
                size(AvailableSpace::MaxContent, AvailableSpace::MinContent),
                window,
                cx,
            );
            // 3414/be sets left=-clientWidth/2+14; clientWidth excludes borders.
            let client_width = bounds.width - window.rem_size() / 8.;
            wrapper = wrapper.child(
                tip(tooltip_id, self.label, false)
                    .absolute()
                    .bottom(css(37.))
                    .left(window.rem_size() * (14. / 16.) - client_width / 2.)
                    .w(bounds.width),
            );
        }
        wrapper
    }
}

pub fn toolbar_button(
    id: &'static str,
    label: SharedString,
    asset: &'static str,
    disabled: bool,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let state = window.use_keyed_state((ElementId::from(id), "hover"), cx, |_, _| Hover::default());
    let hovered = state.read(cx).hovered && !disabled;
    let opacity = Presence::new((ElementId::from(id), "tooltip-opacity"), hovered)
        .transition(Transition::new(Duration::from_millis(300)).easing(Easing::Linear))
        .sample(window, cx)
        .progress;
    let mut wrapper = div()
        .id((ElementId::from(id), "wrapper"))
        .relative()
        .w(css(40.))
        .h(css(38.))
        .flex_shrink_0()
        .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
            state.hovered = *hovered;
            cx.notify();
        }))
        .child(
            Button::new(id)
                .accessibility_label(label.clone())
                .disabled(disabled)
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p_0()
                .when(disabled, |button| button.opacity(0.3))
                .when(!disabled, |button| {
                    button.hover(|s| s.bg(theme::toolbar_hover()))
                })
                .focus_visible(|s| s.bg(theme::toolbar_hover()))
                .child(img(asset).size(css(20.)))
                .on_click(handler),
        );
    if hovered {
        wrapper = wrapper.child(
            deferred(
                tip((ElementId::from(id), "tooltip").into(), label, true)
                    .absolute()
                    .left_0()
                    .top(css(43.))
                    .opacity(opacity),
            )
            .with_priority(301),
        );
    }
    wrapper.into_any_element()
}
