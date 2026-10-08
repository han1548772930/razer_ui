//! Current systrayv2 554: oe/he/Oe/_e. See tray-account-current-evidence.json.
//! The real account channel and populated widgets/notifications remain pending.
use super::*;
use gpui_kit::base::Button as BaseButton;

impl TrayPopup {
    // 2554:de mounts immediately, adds `show` after 100ms and retains the
    // fading surface for 100ms on leave/blur. Re-entry cancels the old timer.
    pub(super) fn hover_settings(&mut self, hovered: bool, cx: &mut Context<Self>) {
        self.settings_hovered = hovered;
        self.settings_tip_task = None;
        if hovered {
            self.settings_tip_mounted = true;
        } else {
            self.settings_tip_visible = false;
        }
        self.settings_tip_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let _ = this.update(cx, |this, cx| {
                if hovered {
                    this.settings_tip_visible = true;
                } else {
                    this.settings_tip_mounted = false;
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn account_header(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let guest = self.session == TraySession::Guest;
        let command = if guest {
            "account-guest-logout"
        } else {
            "account-online"
        };
        let label = text(
            "popup",
            if guest {
                "TEXT_LOG_IN"
            } else {
                "TEXT_VIEW_ONLINE"
            },
        );
        let opacity = motion::transition(
            "tray-account-button-opacity",
            if self.account_hovered { 1_f32 } else { 0. },
            Transition::new(Duration::from_millis(100)).easing(Easing::EaseOut),
            window,
            cx,
        );
        let press = motion::transition(
            "tray-account-press-opacity",
            if self.account_pressed { 0.1_f32 } else { 0. },
            Transition::new(Duration::from_millis(100)).easing(Easing::Linear),
            window,
            cx,
        );
        let name = if self.account_name.is_empty() {
            text("popup", "TEXT_GUEST")
        } else {
            self.account_name.clone()
        };
        let avatar = self.account_avatar.clone().unwrap_or_else(|| {
            if guest {
                "synapse/tray-guest-current.svg"
            } else {
                "synapse/tray-user-current.svg"
            }
            .into()
        });
        h_flex()
            .id("tray-account-header")
            .relative()
            .w_full()
            .h(surface::css(60.))
            .flex_shrink_0()
            .items_center()
            .px(surface::css(20.))
            .bg(TrayColors::border())
            .on_hover(cx.listener(|this, hovered, _, cx| {
                this.account_hovered = *hovered;
                if !hovered {
                    this.account_pressed = false;
                }
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.account_pressed = true;
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.account_pressed = false;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.account_pressed = false;
                    cx.notify();
                }),
            )
            // ::before precedes children and has no pointer handlers.
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(TrayColors::border())
                    .opacity(press),
            )
            .child(
                BaseButton::new("tray-account-avatar")
                    .accessibility_label(label.clone())
                    .size(surface::css(40.))
                    .flex_shrink_0()
                    .mr(surface::css(10.))
                    .rounded_full()
                    .overflow_hidden()
                    .on_click(cx.listener(move |this, _, _, _| this.command(command)))
                    .child(img(avatar).size_full()),
            )
            .child(
                BaseButton::new("tray-account-name")
                    .min_w_0()
                    .mr(surface::css(10.))
                    .line_height(relative(1.22))
                    .text_color(TrayColors::selected_text())
                    .font_weight(FontWeight::BOLD)
                    .on_click(cx.listener(move |this, _, _, _| this.command(command)))
                    .child(div().truncate().child(name)),
            )
            .child(
                BaseButton::new("tray-view-online")
                    .ml_auto()
                    .flex_shrink_0()
                    .min_w(surface::css(90.))
                    .px(surface::css(7.))
                    .pt(surface::css(7.))
                    .pb(surface::css(6.))
                    .border_1()
                    .border_color(TrayColors::text())
                    .rounded(surface::css(2.))
                    .text_size(surface::css(12.))
                    .line_height(relative(1.))
                    .text_color(TrayColors::text())
                    .opacity(opacity)
                    .on_click(cx.listener(move |this, _, _, _| this.command(command)))
                    .child(label.to_uppercase()),
            )
            .into_any_element()
    }

    pub(super) fn account_navigation(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut tabs = h_flex()
            .id("tray-navbar")
            .relative()
            .w_full()
            .flex_shrink_0()
            .border_t_1()
            .border_color(TrayColors::launcher());
        for (section, id, key) in [
            (TraySection::Widgets, "tray-tab-widgets", "TEXT_WIDGETS"),
            (
                TraySection::Notifications,
                "tray-tab-notifications",
                "TEXT_NOTIFICATIONS",
            ),
        ] {
            let bg = motion::transition(
                (id, "background"),
                if self.hovered_tab == Some(section) {
                    TrayColors::launcher()
                } else {
                    TrayColors::surface()
                },
                Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut),
                window,
                cx,
            );
            let fg = motion::transition(
                (id, "foreground"),
                if self.section == section {
                    TrayColors::selected_text()
                } else {
                    TrayColors::muted()
                },
                Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut),
                window,
                cx,
            );
            tabs = tabs.child(
                BaseButton::new(id)
                    .px(surface::css(20.))
                    .pt(surface::css(9.))
                    .pb(surface::css(8.))
                    .text_size(surface::css(12.))
                    .line_height(relative(1.22))
                    .text_color(fg)
                    .bg(bg)
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        this.hovered_tab = (*hovered).then_some(section);
                        cx.notify();
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.section = section;
                        cx.notify();
                    }))
                    .child(text("popup", key).to_uppercase()),
            );
        }
        let settings_bg = motion::transition(
            "tray-settings-background",
            if self.settings_hovered {
                TrayColors::launcher()
            } else {
                TrayColors::surface()
            },
            Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut),
            window,
            cx,
        );
        let settings_fg = motion::transition(
            "tray-settings-fill",
            if self.settings_hovered {
                TrayColors::selected_text()
            } else {
                TrayColors::muted()
            },
            Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut),
            window,
            cx,
        );
        let tip_opacity = motion::transition(
            "tray-settings-tip-opacity",
            if self.settings_tip_visible { 1_f32 } else { 0. },
            Transition::new(Duration::from_millis(100)).easing(Easing::Linear),
            window,
            cx,
        );
        tabs.child(
            BaseButton::new("tray-navbar-settings")
                .relative()
                .accessibility_label(text("popup", "TEXT_SETTINGS"))
                .ml_auto()
                .px(surface::css(23.))
                .bg(settings_bg)
                .on_hover(cx.listener(|this, hovered, _, cx| this.hover_settings(*hovered, cx)))
                .on_click(cx.listener(|this, _, _, _| this.command("settings-quick-panel")))
                .child(
                    svg()
                        .path("synapse/tray-settings-current.svg")
                        .w(surface::css(14.026))
                        .h(surface::css(14.))
                        .text_color(settings_fg),
                )
                .when(self.settings_tip_mounted, |button| {
                    button.child(
                        deferred(
                            div()
                                .absolute()
                                .top_full()
                                .right_0()
                                .w(surface::css(300.))
                                .flex()
                                .justify_end()
                                .opacity(tip_opacity)
                                .child(
                                    gpui_kit::base::Tooltip::new("tray-settings-tip")
                                        .bg(TrayColors::border())
                                        .border_1()
                                        .border_color(TrayColors::tooltip_border())
                                        .text_color(TrayColors::text())
                                        .text_size(surface::css(14.))
                                        .line_height(relative(1.22))
                                        .text_left()
                                        .px(surface::css(8.))
                                        .py(surface::css(7.))
                                        .child(text("popup", "TEXT_SETTINGS")),
                                ),
                        )
                        .with_priority(1060),
                    )
                }),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .top_full()
                .w_full()
                .h(surface::css(2.))
                .bg(TrayColors::launcher()),
        )
        .into_any_element()
    }

    pub(super) fn account_body(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // c() supplies maxHeight = window height - 153. Source body remains
        // 547px; the native dynamic window placement still needs full auditing.
        let body = div()
            .id("tray-body")
            .relative()
            .w_full()
            .h(surface::css(547.))
            .max_h((window.viewport_size().height - px(153.)).max(px(0.)))
            .flex_shrink_0()
            .overflow_x_hidden()
            .overflow_y_scroll();
        if self.section == TraySection::Widgets {
            return body
                .child(self.empty_body(
                    "TEXT_WIDGETS_PLACEHOLDER",
                    "TEXT_CHANGE_wIDGETS_SETTINGS",
                    "settings-widgets",
                    window,
                    cx,
                ))
                .into_any_element();
        }
        let body = body.child(
            div()
                .w_full()
                .border_t_2()
                .border_color(TrayColors::launcher())
                .py(surface::css(6.))
                .text_size(surface::css(10.))
                .text_color(TrayColors::muted())
                .text_center()
                .child(text("popup", "TEXT_NOTIFICATIONS").to_uppercase()),
        );
        if self.notifications_loaded_empty {
            body.child(self.empty_body(
                "TEXT_NOTIFICATIONS_PLACEHOLDER",
                "TEXT_CHANGE_NOTIFICATION_SETTINGS",
                "settings-notifications",
                window,
                cx,
            ))
            .into_any_element()
        } else {
            // W.hasItems=false until a real getUserNotifications reply. A
            // failed/unconnected request does not turn into the empty result.
            body.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(LoadingSpinner),
            )
            .into_any_element()
        }
    }

    fn empty_body(
        &self,
        message: &str,
        label: &str,
        command: &'static str,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let sender = self.sender.clone();
        v_flex()
            .w_full()
            .p(surface::css(20.))
            .items_center()
            .text_center()
            .text_size(surface::css(14.))
            .text_color(TrayColors::text())
            .child(div().mb(surface::css(10.)).child(text("popup", message)))
            .child(
                text_button(command, window, cx)
                    .on_click(move |_, _, _| {
                        let _ = sender.try_send(Event::Menu(command.into()));
                    })
                    .child(div().underline().child(text("popup", label))),
            )
            .into_any_element()
    }
}

#[derive(Default)]
struct TextButtonState {
    hovered: bool,
    pressed: bool,
}

// 2554:ye delegates to .btn-text; its two properties have separate timings.
fn text_button(id: &'static str, window: &mut Window, cx: &mut App) -> BaseButton {
    let state = window.use_keyed_state((ElementId::from(id), "text-button"), cx, |_, _| {
        TextButtonState::default()
    });
    let current = state.read(cx);
    let color = motion::transition(
        (id, "text-color"),
        if current.hovered {
            TrayColors::accent()
        } else {
            TrayColors::text()
        },
        Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut),
        window,
        cx,
    );
    let pressed = state.read(cx).pressed;
    let opacity = motion::transition(
        (id, "press-opacity"),
        if pressed { 0.7_f32 } else { 1. },
        Transition::new(Duration::from_millis(100)).easing(Easing::Linear),
        window,
        cx,
    );
    BaseButton::new(id)
        .text_color(color)
        .opacity(opacity)
        .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
            state.hovered = *hovered;
            if !hovered {
                state.pressed = false;
            }
            cx.notify();
        }))
        .on_mouse_down(
            MouseButton::Left,
            window.listener_for(&state, |state, _, _, cx| {
                state.pressed = true;
                cx.notify();
            }),
        )
        .on_mouse_up(
            MouseButton::Left,
            window.listener_for(&state, |state, _, _, cx| {
                state.pressed = false;
                cx.notify();
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            window.listener_for(&state, |state, _, _, cx| {
                state.pressed = false;
                cx.notify();
            }),
        )
}

#[derive(IntoElement)]
struct LoadingSpinner;
impl RenderOnce for LoadingSpinner {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        if cx.reduce_motion() {
            spinner_frame(0.25).into_any_element()
        } else {
            div()
                .size(surface::css(31.))
                .with_animation(
                    "tray-notifications-spinner",
                    Animation::new(Duration::from_secs(2)).repeat(),
                    |view, phase| view.child(spinner_frame(phase)),
                )
                .into_any_element()
        }
    }
}

fn spinner_frame(phase: f32) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            // be's literal SVG: 100-unit viewBox, r30, stroke10, square caps;
            // rotation 0/180/720 and dash 10%/50%/10%, linear two-second period.
            let scale = f32::from(bounds.size.width) / 100.;
            let radius = 30. * scale;
            let center = bounds.center();
            let rotation = if phase <= 0.5 {
                phase * 360.
            } else {
                180. + (phase - 0.5) * 1080.
            };
            let fraction = if phase <= 0.5 {
                0.1 + phase * 0.8
            } else {
                0.5 - (phase - 0.5) * 0.8
            };
            for (start, length, alpha) in [
                (0., std::f32::consts::TAU, 0.3),
                (rotation.to_radians(), fraction * std::f32::consts::TAU, 1.),
            ] {
                let project = |a: f32| center + point(px(a.cos() * radius), px(a.sin() * radius));
                let mut path = PathBuilder::stroke(px(10. * scale));
                path.move_to(project(start));
                path.arc_to(
                    point(px(radius), px(radius)),
                    px(0.),
                    false,
                    true,
                    project(start + length / 2.),
                );
                path.arc_to(
                    point(px(radius), px(radius)),
                    px(0.),
                    false,
                    true,
                    project(start + length),
                );
                if let Ok(path) = path.build() {
                    window.paint_path(path, TrayColors::accent().opacity(alpha));
                }
                if alpha == 1. {
                    for (angle, direction) in [(start, -1.), (start + length, 1.)] {
                        let endpoint = project(angle);
                        let normal =
                            point(px(angle.cos() * 5. * scale), px(angle.sin() * 5. * scale));
                        let extension = point(
                            px(-angle.sin() * 5. * scale * direction),
                            px(angle.cos() * 5. * scale * direction),
                        );
                        let mut cap = PathBuilder::fill();
                        cap.move_to(endpoint + normal);
                        cap.line_to(endpoint - normal);
                        cap.line_to(endpoint - normal + extension);
                        cap.line_to(endpoint + normal + extension);
                        cap.close();
                        if let Ok(cap) = cap.build() {
                            window.paint_path(cap, TrayColors::accent());
                        }
                    }
                }
            }
        },
    )
    .size(surface::css(31.))
}
