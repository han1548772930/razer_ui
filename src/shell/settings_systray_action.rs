//! Current Settings 9762:fe and 6584 dropdown in the empty app-catalogue state.
use super::{settings_page::SettingsPage, settings_window::text};
use crate::{preferences::TrayDoubleClickAction, ui::surface::css};
use gpui_kit::base::{
    Button, Popover,
    motion::{self, Easing, Transition},
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::time::Duration;

struct Palette;
impl Palette {
    fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    fn selected() -> Hsla {
        rgb(0x44d62c).into()
    }
    fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    fn arrow() -> Hsla {
        rgb(0x999999).into()
    }
    fn background() -> Hsla {
        rgb(0x000000).into()
    }
    fn hover() -> Hsla {
        rgb(0x1a1a1a).into()
    }
}

pub(super) struct SystrayActionSelector {
    settings: Entity<SettingsPage>,
    mounted: bool,
    open: bool,
    active: bool,
    shown: bool,
    task: Option<Task<()>>,
}
impl SystrayActionSelector {
    pub(super) fn unmount(&mut self, cx: &mut Context<Self>) {
        self.task = None;
        self.open = false;
        self.active = false;
        self.shown = false;
        self.mounted = false;
        cx.notify();
    }
    pub(super) fn new(settings: Entity<SettingsPage>) -> Self {
        Self {
            settings,
            mounted: false,
            open: false,
            active: false,
            shown: false,
            task: None,
        }
    }
    fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if open == self.open {
            return;
        }
        self.open = open;
        self.active = open;
        self.mounted = true;
        self.shown = false;
        self.task = Some(cx.spawn(async move |owner, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let _ = owner.update(cx, |this, cx| {
                this.active = open;
                this.mounted = open;
                this.shown = open;
                cx.notify();
            });
        }));
        cx.notify();
    }
}
impl Render for SystrayActionSelector {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = text("DROPDOWN_SYSTRAY_1");
        let owner = cx.entity();
        let already_open = self.active;
        let selected =
            self.settings.read(cx).tray_double_click() == TrayDoubleClickAction::ShowMenu;
        let angle = motion::transition(
            "settings-tray-arrow",
            if self.active {
                std::f32::consts::PI
            } else {
                0.
            },
            Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut),
            window,
            cx,
        );
        let arrow = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let center = bounds.center();
                let mut path = PathBuilder::fill();
                for (index, (x, y)) in [(-0.5_f32, -0.5_f32), (0.5, -0.5), (0., 0.5)]
                    .into_iter()
                    .enumerate()
                {
                    let x = x * f32::from(bounds.size.width);
                    let y = y * f32::from(bounds.size.height);
                    let point = center
                        + point(
                            px(x * angle.cos() - y * angle.sin()),
                            px(x * angle.sin() + y * angle.cos()),
                        );
                    if index == 0 {
                        path.move_to(point);
                    } else {
                        path.line_to(point);
                    }
                }
                path.close();
                if let Ok(path) = path.build() {
                    window.paint_path(path, Palette::arrow());
                }
            },
        )
        .w(css(10.))
        .h(css(5.));
        let trigger = Button::new("settings-tray-action-trigger")
            .accessibility_label(title.clone())
            .relative()
            .w(css(200.))
            .h(css(27.))
            .pl(css(6.))
            .pr(css(30.))
            .py_0()
            .justify_start()
            .text_size(css(14.))
            .text_color(Palette::text())
            .bg(transparent_black())
            .border_1()
            .border_color(if self.active {
                Palette::selected()
            } else {
                Palette::border()
            })
            .hover(|style| style.border_color(Palette::selected()))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                if already_open {
                    cx.stop_propagation();
                }
            })
            .whitespace_nowrap()
            .overflow_hidden()
            .child(title.clone())
            .child(
                div()
                    .absolute()
                    .right(css(10.))
                    .top_0()
                    .bottom_0()
                    .flex()
                    .items_center()
                    .child(arrow),
            );
        let height = motion::transition(
            "settings-tray-popup-height",
            if self.shown { 14. * 1.36 + 8. + 2. } else { 0. },
            Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut),
            window,
            cx,
        );
        let shown = self.shown;
        let option = Button::new("settings-tray-show-menu")
            .accessibility_label(title.clone())
            .w_full()
            .justify_start()
            .px(css(6.))
            .py(css(4.))
            .text_size(css(14.))
            .line_height(relative(1.36))
            .whitespace_nowrap()
            .overflow_hidden()
            .bg(transparent_black())
            .text_color(if selected {
                Palette::selected()
            } else {
                Palette::text()
            })
            .hover(|style| style.bg(Palette::hover()))
            .child(title)
            .on_click(cx.listener(|this, _, _, cx| {
                this.settings.update(cx, |settings, cx| {
                    settings.set_tray_double_click(TrayDoubleClickAction::ShowMenu, cx)
                });
                this.set_open(false, cx);
            }));
        Popover::new("settings-tray-action")
            .open(self.mounted)
            .anchor(Anchor::TopLeft)
            .offset(css(2.).to_pixels(window.rem_size()))
            .mb(css(10.))
            .trigger(trigger)
            .on_open_change(move |open, _, cx| {
                owner.update(cx, |this, cx| {
                    if *open {
                        this.set_open(true, cx);
                    } else if this.open {
                        this.set_open(false, cx);
                        // 6584 outside dismissal calls onHidden only after the
                        // 100ms collapse; 9762 keeps the trigger active until then.
                        this.active = true;
                    }
                })
            })
            .content(move |_, _, _| {
                div()
                    .w(css(200.))
                    .h(css(height))
                    .overflow_hidden()
                    .bg(Palette::background())
                    .border_x_1()
                    .border_color(Palette::border())
                    .when(shown, |view| view.border_y_1())
                    .child(option)
            })
    }
}
