//! Current 16 color dropdown. Both fields anchor to the source .colors group.
use super::studio_color::{StudioColor, StudioColorEvent, checkered};
use super::studio_dropdown::{DropdownState, StudioDropdown, arrow};
use super::*;

pub(super) struct ColorDropdownChanged(pub(super) Option<u32>);
impl EventEmitter<ColorDropdownChanged> for StudioColorDropdown {}
pub(super) struct StudioColorDropdown {
    effect: String,
    value: Option<u32>,
    color: Entity<StudioColor>,
    dropdown: Entity<DropdownState>,
    enabled: bool,
    hidden: bool,
    shift: f32,
    _subscriptions: Vec<Subscription>,
}
impl StudioColorDropdown {
    pub(super) fn cancel_screen_color(&mut self, cx: &mut Context<Self>) {
        self.color
            .update(cx, |color, cx| color.cancel_screen_color(cx));
    }

    pub(super) fn new(shift: f32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let color = cx.new(|cx| StudioColor::new_dropdown(None, window, cx));
        let dropdown = cx.new(|_| DropdownState::new());
        let subscriptions = vec![
            cx.observe(&dropdown, |this, dropdown, cx| {
                if !dropdown.read(cx).is_open() {
                    this.color
                        .update(cx, |color, cx| color.cancel_screen_color(cx));
                }
                cx.notify();
            }),
            cx.observe(&color, |this, color, cx| {
                if this.enabled && this.value != color.read(cx).value() {
                    this.value = color.read(cx).value();
                    cx.notify();
                }
            }),
            cx.subscribe(&color, |this, _, event, cx| {
                if this.enabled {
                    let StudioColorEvent::Changed(value) = event;
                    this.value = *value;
                    cx.emit(ColorDropdownChanged(*value));
                    cx.notify();
                }
            }),
        ];
        Self {
            effect: String::new(),
            value: None,
            color,
            dropdown,
            enabled: false,
            hidden: false,
            shift,
            _subscriptions: subscriptions,
        }
    }
    pub(super) fn configure(
        &mut self,
        effect: &str,
        value: Option<u32>,
        enabled: bool,
        hidden: bool,
        replace: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let remount = self.effect != effect;
        self.effect = effect.into();
        self.enabled = enabled;
        self.hidden = hidden;
        if !enabled || remount {
            self.color
                .update(cx, |color, cx| color.cancel_screen_color(cx));
            self.dropdown
                .update(cx, |state, cx| state.set_open(false, cx));
        }
        if replace {
            self.value = value;
        }
        self.color.update(cx, |color, cx| {
            if replace {
                color.set_value(value, window, cx);
            }
            color.set_enabled(enabled, window, cx);
        });
        cx.notify();
    }
}
impl Render for StudioColorDropdown {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.dropdown.read(cx).is_open();
        let opacity = surface::fade_opacity(
            "studio-color-trigger-swatch",
            if self.hidden { 0. } else { 1. },
            100,
            window,
            cx,
        );
        let swatch = div()
            .size(surface::css(20.))
            .mt(surface::css(-1.))
            .overflow_hidden()
            .opacity(opacity)
            .border_1()
            .border_color(Colors::gradient_border())
            .rounded(surface::css(3.))
            .when_some(self.value, |view, color| view.bg(rgb(color)))
            .when(self.value.is_none(), |view| view.child(checkered()));
        let trigger = BaseButton::new("studio-color-dropdown-trigger")
            .accessibility_label("Selecting Color")
            .relative()
            .w(surface::css(53.))
            .h(surface::css(27.))
            .p_0()
            .pl(surface::css(2.))
            .bg(Colors::panel())
            .border_1()
            .border_color(if open {
                Colors::selected()
            } else {
                Colors::input_border()
            })
            .flex()
            .items_center()
            .justify_start()
            .when(self.enabled, |view| {
                view.hover(|style| style.border_color(Colors::selected()))
            })
            .child(swatch)
            .child(
                div()
                    .absolute()
                    .right(surface::css(10.))
                    .top(surface::css(10.))
                    .child(arrow(
                        open,
                        "studio-color-dropdown-arrow".into(),
                        window,
                        cx,
                    )),
            );
        StudioDropdown::new(
            "studio-color-dropdown",
            &self.dropdown,
            self.enabled,
            trigger,
            div()
                .p(surface::css(10.))
                .child(self.color.clone())
                .into_any_element(),
            230.,
            self.shift,
        )
    }
}

#[cfg(test)]
mod screen_color_tests {
    use super::{ColorDropdownChanged, StudioColorDropdown};
    use crate::features::screen_color::{ScreenColorUpdate, ScreenColorUpdateKind};
    use gpui_kit::component::Root;
    use gpui_kit::{AppContext as _, TestAppContext, px, size};
    use std::{cell::RefCell, rc::Rc};

    #[gpui_kit::test]
    fn studio_dropdown_previews_then_submits_and_closing_rejects_old_capture_events(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::init);
        let mut retained = None;
        let handle = cx.open_window(size(px(350.), px(600.)), |window, cx| {
            let view = cx.new(|cx| StudioColorDropdown::new(0., window, cx));
            view.update(cx, |view, cx| {
                view.configure("fire", Some(0x112233), true, false, true, window, cx);
                view.dropdown
                    .update(cx, |dropdown, cx| dropdown.set_open(true, cx));
            });
            retained = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = retained.unwrap();
        let changes = Rc::new(RefCell::new(Vec::new()));
        let history = changes.clone();
        let _subscription = cx.update(|cx| {
            cx.subscribe(&view, move |_, event: &ColorDropdownChanged, _| {
                history.borrow_mut().push(event.0)
            })
        });
        let capture = cx.update(|cx| view.read(cx).color.read(cx).screen_color_capture());
        cx.update(|cx| {
            capture.update(cx, |capture, cx| {
                cx.emit(ScreenColorUpdate::simulated(
                    capture,
                    ScreenColorUpdateKind::Color(0xabcdef),
                ));
            })
        });
        cx.update(|cx| assert_eq!(view.read(cx).value, Some(0xabcdef)));
        assert!(changes.borrow().is_empty());
        cx.update(|cx| {
            capture.update(cx, |capture, cx| {
                cx.emit(ScreenColorUpdate::simulated(
                    capture,
                    ScreenColorUpdateKind::Done,
                ));
            })
        });
        assert_eq!(*changes.borrow(), [Some(0xabcdef)]);
        let stale = cx.update(|cx| {
            ScreenColorUpdate::simulated(capture.read(cx), ScreenColorUpdateKind::Color(0xff0000))
        });
        cx.update_window(handle.into(), |_, _, cx| {
            let dropdown = view.read(cx).dropdown.clone();
            dropdown.update(cx, |dropdown, cx| dropdown.set_open(false, cx));
        })
        .unwrap();
        cx.update(|cx| capture.update(cx, |_, cx| cx.emit(stale)));
        cx.update(|cx| assert_eq!(view.read(cx).value, Some(0xabcdef)));
        assert_eq!(changes.borrow().len(), 1);
    }
}
