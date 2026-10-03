use super::*;

impl HueWorkspace {
    pub(super) fn bridge_card(&self, cx: &Context<Self>) -> AnyElement {
        let switch = surface::SynapseSwitch::new("hue-bridge-enable")
            .checked(self.bridge.bridge_enabled)
            .accessibility_label(text("BRIDGE"))
            .disabled(!self.preview)
            .on_change(cx.listener(|this, enabled, _, cx| {
                if !this.preview {
                    return;
                }
                if *enabled {
                    this.alert = Some(BridgeAlert::Enable);
                } else {
                    this.bridge.bridge_enabled = false;
                    this.bridge.is_loading = false;
                    this.last_command = Some("ON_SET_BRIDGE_ENABLE: false".into());
                }
                cx.notify();
            }));
        let mut card = widget("BRIDGE", Some("BRIDGE_TIP"), switch, cx);
        let mut icon = div()
            .id("hue-bridge-icon")
            .group("hue-bridge-icon")
            .relative()
            .m(surface::css(10.))
            .child(logo(false, self.bridge.is_loading));
        if !self.bridge.is_loading && self.alert != Some(BridgeAlert::Remove) {
            if self.bridge.bridge_enabled {
                icon = icon.child(
                    gpui_kit::base::Button::new("hue-refresh-bridge")
                        .absolute()
                        .top(surface::css(25.))
                        .left(surface::css(25.))
                        .size(surface::css(30.))
                        .p_0()
                        .rounded_full()
                        .border_2()
                        .border_color(Colors::icon_border())
                        .opacity(0.)
                        .group_hover("hue-bridge-icon", |s| s.opacity(1.))
                        .focus_visible(|s| s.opacity(1.))
                        .hover(|s| s.bg(cx.theme().primary))
                        .disabled(!self.preview)
                        .accessibility_label(i18n::t("REFRESH"))
                        .child(img("synapse/hue-bridge-refresh.svg").size(surface::css(20.)))
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.preview {
                                this.bridge.is_loading = true;
                                this.last_command = Some("ON_REFRESH_BRIDGE".into());
                                cx.notify();
                            }
                        })),
                );
            }
            icon = icon.child(
                gpui_kit::base::Button::new("hue-remove-bridge")
                    .absolute()
                    .left(surface::css(-15.))
                    .top(surface::css(65.))
                    .size(surface::css(30.))
                    .p_0()
                    .rounded_full()
                    .border_2()
                    .border_color(Colors::icon_border())
                    .opacity(0.)
                    .group_hover("hue-bridge-icon", |s| s.opacity(1.))
                    .focus_visible(|s| s.opacity(1.))
                    .hover(|s| s.bg(Colors::error()))
                    .disabled(!self.preview)
                    .accessibility_label(text("DELETE_BRIDGE"))
                    .child(img("synapse/hue-bridge-remove.svg").size(surface::css(20.)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.alert = Some(BridgeAlert::Remove);
                        cx.notify();
                    })),
            );
        }
        let mut details = v_flex().flex_1().min_w_0();
        if self.bridge.bridge_enabled {
            details = details
                .child(
                    div().mb(surface::css(20.)).child(
                        surface::select(&self.groups)
                            .disabled(!self.preview)
                            .w_full(),
                    ),
                )
                .child(text("BRIDGE_DESC"));
        } else {
            let (asset, title, description) = if self.bridge.is_control {
                (
                    "synapse/hue-icon_wifi_no.svg",
                    "NOT_CONNECTED",
                    "BRIDGE_NOT_CONNECTED",
                )
            } else {
                (
                    "synapse/hue-icon_wifi_error.svg",
                    "STREAM_BUSY",
                    "STREAM_BUSY_DESC",
                )
            };
            details = details
                .child(
                    h_flex()
                        .gap(surface::css(10.))
                        .mb(surface::css(10.))
                        .when(!self.bridge.is_control, |v| v.text_color(Colors::error()))
                        .child(img(asset).size(surface::css(20.)))
                        .child(text(title)),
                )
                .child(text(description));
        }
        card = card.child(
            h_flex()
                .items_start()
                .gap(surface::css(20.))
                .child(
                    v_flex().flex_shrink_0().child(icon).child(
                        div()
                            .mt(surface::css(20.))
                            .text_center()
                            .child("Philips Hue"),
                    ),
                )
                .child(details),
        );
        // Inline source alert geometry, with Base's Escape / outside click and
        // focus restoration. Anchor it to the card, not the centered window.
        let owner = cx.entity().downgrade();
        let alert = self.alert;
        card.child(
            div()
                .absolute()
                .left(surface::css(40.))
                .top(surface::css(if alert == Some(BridgeAlert::Remove) {
                    180.
                } else {
                    60.
                }))
                .child(
                    gpui_kit::base::Popover::new("hue-bridge-alert")
                        .anchor(Anchor::TopLeft)
                        .open(alert.is_some())
                        .trigger_with(|_, _, _| div().size_0().into_any_element())
                        .on_open_change(cx.listener(|this, open: &bool, _, cx| {
                            if !*open {
                                this.alert = None;
                                cx.notify();
                            }
                        }))
                        .content(move |_, _, cx| {
                            owner
                                .update(cx, |this, cx| this.bridge_alert(cx))
                                .unwrap_or_else(|_| div().into_any_element())
                        }),
                ),
        )
        .into_any_element()
    }
    fn bridge_alert(&self, cx: &Context<Self>) -> AnyElement {
        let Some(alert) = self.alert else {
            return div().into_any_element();
        };
        let enable = alert == BridgeAlert::Enable;
        v_flex()
            .w(surface::css(300.))
            .py(surface::css(if enable { 15. } else { 20. }))
            .px(surface::css(20.))
            .border_1()
            .border_color(if enable {
                Colors::warning()
            } else {
                Colors::remove()
            })
            .rounded(surface::css(3.))
            .bg(cx.theme().group_box)
            .text_size(surface::css(14.))
            .line_height(surface::css(20.))
            .text_center()
            .when(enable, |v| {
                v.child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .text_color(Colors::warning())
                        .mb(surface::css(10.))
                        .child(text("BRIDGE_CONTROL")),
                )
            })
            .child(div().mb(surface::css(10.)).child(text(if enable {
                "BRIDGE_CONTROL_DES"
            } else {
                "DELETE_BRIDGE"
            })))
            .child(
                div().flex().justify_center().child(
                    command(
                        "hue-bridge-confirm",
                        if enable { "ENABLE" } else { "DELETE" },
                        true,
                        !self.preview,
                        cx,
                    )
                    .when(!enable, |b| b.bg(Colors::remove()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.preview {
                            return;
                        }
                        if enable {
                            this.bridge.bridge_enabled = true;
                            this.bridge.is_loading = true;
                            this.last_command = Some("ON_SET_BRIDGE_ENABLE: true".into());
                        } else {
                            // Source sends a command and waits for MW_SET_IS_PAIRED.
                            // A preview does not fabricate that asynchronous reply.
                            this.last_command = Some("ON_REMOVE_BRIDGE".into());
                        }
                        this.alert = None;
                        cx.notify();
                    })),
                ),
            )
            .into_any_element()
    }
}
