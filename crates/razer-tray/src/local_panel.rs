//! Local device widgets and settings, independent of account identity.
//! The account header and online notification feed are excluded by local-first scope.
//! Current systray-v2-current 554.2573b048 JS useRe (2465), Oe (2530),
//! and 554.7cdbd936 CSS: 360px panel, 60px identity header, 60px launcher.
//! Removing the excluded identity header subtracts 60px from measured bounds.
use super::widgets::widget_list_with_click;
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

    pub(super) fn local_navigation(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Source `.navbar` and its independent `.tabs` are flex rows with
        // default cross-axis stretch; centering the navbar shortens the gear.
        let navbar = div()
            .id("tray-navbar")
            .flex()
            .items_stretch()
            .relative()
            .w_full()
            .flex_shrink_0()
            .border_t_1()
            .border_color(TrayColors::launcher());
        let tabs = div()
            .id("tray-widgets-label")
            .px(surface::css(20.))
            .pt(surface::css(9.))
            .pb(surface::css(8.))
            .text_size(surface::css(12.))
            .line_height(relative(1.22))
            .text_color(TrayColors::selected_text())
            .child(text("popup", "TEXT_WIDGETS").to_uppercase());
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
        navbar
            .child(tabs)
            .child(
                BaseButton::new("tray-navbar-settings")
                    .relative()
                    .flex()
                    .items_center()
                    .flex_shrink_0()
                    .accessibility_label(text("popup", "TEXT_SETTINGS"))
                    .ml_auto()
                    .px(surface::css(23.))
                    .bg(settings_bg)
                    .on_hover(cx.listener(|this, hovered, _, cx| this.hover_settings(*hovered, cx)))
                    .on_click(cx.listener(|this, _, _, _| this.command("settings-quick-panel")))
                    .child(
                        razer_assets::resource_svg()
                            .path("synapse/tray-settings-current.svg")
                            .w(surface::css(14.026))
                            .h(surface::css(14.))
                            .text_color(settings_fg),
                    )
                    .when(self.settings_tip_mounted, |button| {
                        button.child(
                            razer_assets::deferred_resource(
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

    pub(super) fn local_body(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // Re marks the body `.taller` only when there are no launchers.
        // Removing the excluded 60px account header leaves 93px for the
        // navigation and launcher. Keep the local body and its scrolling.
        let popup = cx.entity().downgrade();
        let revision = self.widget_revision;
        let body = div()
            .on_children_prepainted(move |bounds, window, cx| {
                // Like 597/c's child.offsetHeight sum, measure the resolved
                // body children, not the clipped scroll viewport or spinner.
                let css_pixel = f32::from(surface::css(1.).to_pixels(window.rem_size()));
                let height = bounds
                    .iter()
                    .map(|child| (f32::from(child.size.height) / css_pixel).round())
                    .sum();
                let popup = popup.clone();
                // Release the render/paint stack before updating the entity.
                window.defer(cx, move |window, cx| {
                    let _ = popup.update(cx, |view, cx| {
                        if view.widget_revision == revision {
                            view.observe_widget_height(height, window, cx);
                        }
                    });
                });
            })
            .id("tray-body")
            .block()
            .relative()
            .w_full()
            .h(surface::css(547.))
            .when(self.launchers.is_empty(), |body| {
                body.min_h(surface::css(306.))
            })
            .max_h(
                (window.viewport_size().height - surface::css(93.).to_pixels(window.rem_size()))
                    .max(px(0.)),
            )
            .flex_shrink_0()
            .overflow_x_hidden()
            .overflow_y_scroll();
        let devices = self.sender.clone();
        let application = self.sender.clone();
        body.child(widget_list_with_click(
            &self.widget_devices,
            text("popup", "TEXT_WIDGETS_PLACEHOLDER"),
            move |device, _, _, _| {
                let _ = devices.try_send(Event::Device(device.id.clone()));
            },
            move |_, _, _| {
                let _ = application.try_send(Event::Menu("synapse".into()));
            },
        ))
        .into_any_element()
    }

    /// Preserve the existing child-height policy after removing the 60px
    /// account header. Layout measurements remain owned by prepaint.
    pub(super) fn requested_height(&self) -> f32 {
        (self.widget_height.unwrap_or(0.) + 93.).clamp(340., 640.)
    }

    fn observe_widget_height(&mut self, height: f32, window: &mut Window, cx: &mut Context<Self>) {
        self.widget_height = Some(height);
        #[cfg(target_os = "windows")]
        {
            let requested = self.requested_height();
            if native::visible(window)
                && self.placed_height != Some(requested)
                && let Some(placement) = native::popup_placement(window, self.anchor, requested)
            {
                self.placed_height = Some(requested);
                self.resize_task = Some(cx.spawn_in(window, async move |_, cx| {
                    // WM_SIZE may reenter GPUI: apply outside the entity update.
                    cx.background_spawn(async move {
                        placement.apply();
                    })
                    .await;
                }));
            }
        }
        #[cfg(not(target_os = "windows"))]
        let _ = (window, cx);
    }
}
