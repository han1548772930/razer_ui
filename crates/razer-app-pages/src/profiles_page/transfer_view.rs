//! Exact current 43/Ia regions. Reference geometry is converted by css(rem).
use super::*;
use gpui_kit::base::{Dialog, DialogPopup};
use gpui_kit::component::{checkbox::Checkbox, select::Select};
use razer_widgets::theme::ProfilesTransferColors as Colors;

impl ProfileTransfer {
    fn button(
        id: &'static str,
        label: String,
        primary: bool,
        disabled: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> BaseButton {
        let pointer = controls::pointer(id.into(), window, cx);
        let state = pointer.read(cx);
        let opacity = motion::transition(
            (id, "opacity"),
            if disabled {
                0.3
            } else if state.pressed {
                0.6
            } else if state.hovered {
                0.8
            } else {
                1.0
            },
            Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
            window,
            cx,
        );
        controls::track(BaseButton::new(id), &pointer, window)
            .h(css(27.))
            .px(css(24.))
            .py_0()
            .border_1()
            .border_color(Colors::button_border())
            .rounded(css(3.))
            .text_size(css(12.))
            .bg(if primary {
                Colors::selected()
            } else {
                Colors::secondary()
            })
            .text_color(if primary {
                Colors::button_text()
            } else {
                Colors::secondary_text()
            })
            .disabled(disabled)
            .opacity(opacity)
            .focus_visible(|s| s.border_color(Colors::text()))
            .child(label.to_uppercase())
    }

    // Current CSS .warning .tip and .checkboxItemImgWrapper .tip use
    // immediate visibility with a 300 ms linear fade and fixed local offsets.
    fn source_tip(
        id: ElementId,
        label: String,
        warning: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        use razer_widgets::theme::TooltipColors;
        let pointer = controls::pointer(id.clone(), window, cx);
        let hovered = pointer.read(cx).hovered;
        let opacity = Presence::new((id.clone(), "opacity"), hovered)
            .transition(Transition::new(Duration::from_millis(300)).easing(Easing::Linear))
            .sample(window, cx)
            .progress;
        div()
            .id(id.clone())
            .relative()
            .w(css(if warning { 0. } else { 20. }))
            .h(css(if warning { 27. } else { 31. }))
            .flex_shrink_0()
            .on_hover(window.listener_for(&pointer, |pointer, hovered, _, cx| {
                pointer.hovered = *hovered;
                cx.notify();
            }))
            .child(
                div()
                    .id((id, "icon"))
                    .absolute()
                    .left(css(if warning { 10. } else { 0. }))
                    .top(css(if warning { 3.5 } else { 5.5 }))
                    .size(css(20.))
                    // The warning's CSS pseudo-element extends outside its
                    // zero-width flex item. Give that icon its own hitbox.
                    .on_hover(window.listener_for(&pointer, |pointer, hovered, _, cx| {
                        pointer.hovered = *hovered;
                        cx.notify();
                    }))
                    .child(
                        img(if warning {
                            "synapse/profiles-transfer-warning.svg"
                        } else {
                            "synapse/profiles-transfer-macro.svg"
                        })
                        .size_full(),
                    ),
            )
            .when(hovered, |v| {
                v.child(
                    deferred(
                        div()
                            .absolute()
                            .left(css(if warning { -113. } else { 0. }))
                            .top(css(if warning { -54. } else { 31. }))
                            .when(warning, |v| v.w(css(300.)))
                            .pt(css(8.))
                            .pb(css(10.))
                            .px(css(10.))
                            .border_1()
                            .border_color(TooltipColors::border())
                            .bg(TooltipColors::background())
                            .text_color(TooltipColors::foreground())
                            .text_size(css(14.))
                            .line_height(css(16.))
                            .opacity(opacity)
                            .child(label),
                    )
                    .with_priority(100),
                )
            })
            .into_any_element()
    }
    fn browse_row(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.cloud {
            return h_flex()
                .h(css(47.))
                .w_full()
                .flex_shrink_0()
                .bg(Colors::chrome())
                .child(div().ml(css(89.)).child(text("mYs")))
                .child(
                    Select::new(&self.cloud_devices)
                        .disabled(true)
                        .ml(css(10.))
                        .w(css(250.)),
                )
                .into_any_element();
        }
        h_flex()
            .h(css(47.))
            .w_full()
            .flex_shrink_0()
            .bg(Colors::chrome())
            .child(div().ml(css(100.)).child(text("M88")))
            .child(
                BaseButton::new("profiles-transfer-browse")
                    .accessibility_label(text("M88"))
                    .ml(css(10.))
                    .pl(css(6.))
                    .pr(css(4.))
                    .py_0()
                    .h(css(27.))
                    .w(css(250.))
                    .bg(Colors::panel())
                    .border_1()
                    .border_color(Colors::input_border())
                    .hover(|s| s.border_color(Colors::browse_hover()))
                    .disabled(self.busy)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .child(self.file_name.clone()),
                    )
                    .child(img("synapse/profiles-transfer-folder.svg").size(css(20.)))
                    .on_click(cx.listener(|this, _, _, cx| this.browse(cx))),
            )
            .into_any_element()
    }
    fn body(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.cloud {
            return h_flex()
                .flex_1()
                .min_h_0()
                .justify_center()
                .child(
                    h_flex()
                        .h(css(80.))
                        .px(css(20.))
                        .border_1()
                        .border_color(Colors::warning())
                        .rounded(css(3.))
                        .child(
                            img("synapse/profiles-transfer-cone.svg")
                                .size(css(40.))
                                .mr(css(10.)),
                        )
                        .child(text("mcz")),
                )
                .into_any_element();
        }
        if self.rows.is_empty() {
            return div()
                .flex_1()
                .min_h_0()
                .px(css(50.))
                .pt(css(20.))
                .text_center()
                .text_color(Colors::error())
                .children(self.error.clone())
                .into_any_element();
        }
        let rows =
            self.rows
                .iter()
                .map(|row| {
                    let key = row.key.clone();
                    let macros = row
                        .macros
                        .iter()
                        .enumerate()
                        .map(|(ix, item)| {
                            let key = row.key.clone();
                            let guid = item.guid.clone();
                            h_flex()
                                .h(css(31.))
                                .pl(css(32.))
                                .child(div().w(css(3.)).h_full().bg(Colors::secondary()))
                                .child(div().mx(css(10.)).w(css(20.)).when(ix == 0, |v| {
                                    v.child(Self::source_tip(
                                        SharedString::from(format!(
                                            "profiles-transfer-macro-tip-{}",
                                            row.key
                                        ))
                                        .into(),
                                        "Macro".into(),
                                        false,
                                        window,
                                        cx,
                                    ))
                                }))
                                .child(
                                    Checkbox::new(SharedString::from(format!(
                                        "profiles-transfer-macro-{}-{}",
                                        row.key, item.guid
                                    )))
                                    .label(item.name.clone())
                                    .checked(item.selected)
                                    .disabled(self.busy || !row.selected)
                                    .on_click(cx.listener(move |this, checked, _, cx| {
                                        if this.busy || this.closed {
                                            return;
                                        }
                                        if let Some(row) = this
                                            .rows
                                            .iter_mut()
                                            .find(|r| r.key == key && r.selected)
                                        {
                                            if let Some(item) =
                                                row.macros.iter_mut().find(|m| m.guid == guid)
                                            {
                                                item.selected = *checked;
                                                cx.notify();
                                            }
                                        }
                                    })),
                                )
                                .into_any_element()
                        })
                        .collect::<Vec<_>>();
                    v_flex()
                        .mb(css(10.))
                        .child(
                            Checkbox::new(SharedString::from(format!(
                                "profiles-transfer-profile-{}",
                                row.key
                            )))
                            .h(css(20.))
                            .label(row.name.clone())
                            .checked(row.selected)
                            .disabled(self.busy)
                            .on_click(cx.listener(
                                move |this, checked, _, cx| this.select(&key, *checked, cx),
                            )),
                        )
                        // .slide-off is display:none; .slide-on is display:block.
                        // Do not invent a 300 ms height tween between those states.
                        .when(row.selected && !macros.is_empty(), |v| {
                            v.child(v_flex().py(css(5.)).children(macros))
                        })
                        .into_any_element()
                })
                .collect::<Vec<_>>();
        v_flex()
            .flex_1()
            .min_h_0()
            .child(
                BaseButton::new("profiles-transfer-select-all")
                    .h(css(40.))
                    .w_full()
                    .pl(css(20.))
                    .justify_start()
                    .text_color(Colors::selected())
                    .disabled(self.busy)
                    .child(text(if self.select_all { "c2H" } else { "$w6" }))
                    .on_click(cx.listener(|this, _, _, cx| {
                        for row in &mut this.rows {
                            row.selected = this.select_all;
                        }
                        this.select_all = !this.select_all;
                        cx.notify();
                    })),
            )
            .child(
                v_flex()
                    .id("profiles-transfer-list")
                    .flex_1()
                    .min_h_0()
                    .scrollable_y()
                    .child(
                        v_flex()
                            .pl(css(20.))
                            .pr(css(27.))
                            .child(div().mb(css(6.)).child(self.device_name.clone()))
                            .children(rows),
                    ),
            )
            .into_any_element()
    }
}
impl Render for ProfileTransfer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = self.body(window, cx);
        let panel = v_flex()
            .id("profiles-transfer-dialog")
            .test_support()
            .aria_label(text("ov8"))
            .w(css(602.))
            .h(css(481.))
            .border_1()
            .border_color(Colors::border())
            .rounded(css(5.))
            .bg(Colors::panel())
            .font_family("Roboto")
            .text_size(css(14.))
            .line_height(css(17.))
            .text_color(Colors::text())
            .child(
                h_flex()
                    .relative()
                    .w_full()
                    .h(css(36.))
                    .flex_shrink_0()
                    .justify_center()
                    .bg(Colors::chrome())
                    .border_b_1()
                    .border_color(Colors::border())
                    .rounded_t(css(5.))
                    .text_color(Colors::title())
                    // Ia always uses TEXT_EXPORT_PROFILES, including its import mode.
                    .child(text("ov8").to_uppercase())
                    .child(
                        BaseButton::new("profiles-transfer-close")
                            .accessibility_label(i18n::t("CLOSE"))
                            .absolute()
                            .right(css(5.))
                            .size(css(20.))
                            .p_0()
                            .child(img("synapse/profiles-transfer-close.svg").size_full())
                            .on_click(cx.listener(|this, _, _, cx| this.dismiss(cx))),
                    ),
            )
            .when(self.mode == Mode::Import, |v| {
                v.child(
                    h_flex()
                        .justify_center()
                        .h(css(49.))
                        .flex_shrink_0()
                        .w_full()
                        .bg(Colors::chrome())
                        .pt(css(10.))
                        .items_start()
                        .child(
                            BaseButton::new("profiles-transfer-local-cloud")
                                .accessibility_label(format!("{} / {}", text("s2B"), text("GCq")))
                                .h(css(36.))
                                .p(css(5.))
                                .rounded(css(18.))
                                .bg(Colors::panel())
                                .border_1()
                                .border_color(Colors::input_border())
                                .hover(|s| s.border_color(Colors::selected()))
                                .children([false, true].map(|cloud| {
                                    div()
                                        .h(css(24.))
                                        .px(css(10.))
                                        .flex()
                                        .items_center()
                                        .rounded(css(12.))
                                        .when(!cloud, |v| v.mr(css(5.)))
                                        .when(cloud == self.cloud, |v| {
                                            v.bg(Colors::selected())
                                                .text_color(Colors::selected_text())
                                        })
                                        .child(text(if cloud { "GCq" } else { "s2B" }))
                                }))
                                .on_click(cx.listener(|this, _, _, cx| this.toggle_cloud(cx))),
                        ),
                )
                .child(self.browse_row(cx))
            })
            .child(body)
            .child(
                v_flex()
                    .w_full()
                    .flex_shrink_0()
                    .items_center()
                    .py(css(16.))
                    .border_t_1()
                    .border_color(Colors::border())
                    .rounded_b(css(5.))
                    .bg(Colors::chrome())
                    .when(self.mode == Mode::Export, |v| {
                        v.child(
                            div()
                                .mb(css(10.))
                                .px(css(10.))
                                .text_center()
                                .line_height(css(16.))
                                .child(text("QN3")),
                        )
                    })
                    .child(
                        h_flex()
                            .gap(css(10.))
                            .child(
                                Self::button(
                                    "profiles-transfer-cancel",
                                    text("bOp"),
                                    false,
                                    false,
                                    window,
                                    cx,
                                )
                                .on_click(cx.listener(|this, _, _, cx| this.dismiss(cx))),
                            )
                            .child(
                                Self::button(
                                    "profiles-transfer-submit",
                                    text(if self.mode == Mode::Import {
                                        "yE7"
                                    } else {
                                        "l1q"
                                    }),
                                    true,
                                    !self.enabled(),
                                    window,
                                    cx,
                                )
                                .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
                            )
                            .when(self.warning, |v| {
                                v.child(Self::source_tip(
                                    "profiles-transfer-warning".into(),
                                    text("BXZ"),
                                    true,
                                    window,
                                    cx,
                                ))
                            }),
                    ),
            );
        Dialog::new(cx)
            .layer(3, true)
            .focus_handle(self.focus.clone())
            .on_cancel(|_, _, _| false)
            .on_ok(|_, _, _| false)
            .close_on_backdrop_press(false)
            .backdrop(div().absolute().inset_0().bg(Colors::backdrop()).occlude())
            .popup(
                DialogPopup::new()
                    .absolute()
                    .left(
                        window.viewport_size().width / 2. - css(300.).to_pixels(window.rem_size()),
                    )
                    .top(css(104.))
                    .child(panel),
            )
    }
}
