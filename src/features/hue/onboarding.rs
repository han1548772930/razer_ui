use super::*;

// Pi applies JavaScript parseInt, not IPv4 validation. Preserve prefix parsing,
// removal of leading zeros and the source's upper clamp (including its signed
// input behavior); pattern="[0-9]*" on a text input does not enforce digits.
pub(super) fn normalize_octet(value: &str) -> String {
    let value = value.trim_start();
    let (negative, digits) = if let Some(v) = value.strip_prefix('-') {
        (true, v)
    } else {
        (false, value.strip_prefix('+').unwrap_or(value))
    };
    let (radix, digits) = if digits.starts_with("0x") || digits.starts_with("0X") {
        (16, &digits[2..])
    } else {
        (10, digits)
    };
    let prefix = digits
        .chars()
        .take_while(|c| c.is_ascii() && c.is_digit(radix))
        .collect::<String>();
    let Some(number) = i32::from_str_radix(&prefix, radix).ok() else {
        return String::new();
    };
    (if negative { -number } else { number })
        .min(255)
        .to_string()
}
impl HueWorkspace {
    pub(super) fn onboarding(&self, cx: &Context<Self>) -> AnyElement {
        let mut body = v_flex()
            .items_center()
            .text_center()
            .child(
                div()
                    .font_family("RazerF5")
                    .text_size(surface::css(21.))
                    .text_color(cx.theme().primary)
                    .mb(surface::css(20.))
                    .child(text("TEXT_HUE_CHROMA_INTEGRATION")),
            )
            .child(text("TEXT_INTEGRATION_CONTENT_1"))
            .child(text("TEXT_INTEGRATION_CONTENT_2"))
            .child(
                logo(self.integration.pair_logo(), self.integration.loading())
                    .my(surface::css(20.)),
            );
        let unavailable = !self.preview;
        match self.integration {
            Integration::Init => {
                body = body.child(text("TEXT_CASE_SCAN_CONTENT")).child(
                    div()
                        .id("hue-scan-availability")
                        .mt(surface::css(10.))
                        .when(unavailable, |v| {
                            v.tooltip(|window, cx| {
                                tooltip::Tooltip::new("Hue 网桥扫描暂不可用").build(window, cx)
                            })
                        })
                        .child(
                            command("hue-scan", "TEXT_CASE_SCAN", true, unavailable, cx).on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.set_integration(Integration::Scanning, window, cx)
                                }),
                            ),
                        ),
                );
            }
            Integration::Scanning | Integration::ScanningIp => {
                body = body.child(text("TEXT_CASE_SCANNING_CONTENT")).child(
                    div().mt(surface::css(10.)).child(
                        command(
                            "hue-cancel-scan",
                            "TEXT_CASE_SCANNING",
                            false,
                            unavailable,
                            cx,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.set_integration(Integration::ScanCancel, window, cx)
                        })),
                    ),
                );
            }
            Integration::ScanFailed | Integration::ScanCancel | Integration::PairCancel => {
                body = body
                    .child(
                        div()
                            .text_color(Colors::error())
                            .child(text("TEXT_CASE_SCAN_AGAIN_CONTENT")),
                    )
                    .child(
                        div()
                            .text_color(Colors::error())
                            .child(text("TEXT_CASE_SCAN_AGAIN_FOLLOW_CONTENT")),
                    )
                    .child(
                        h_flex()
                            .gap(surface::css(10.))
                            .mt(surface::css(10.))
                            .justify_center()
                            .child(
                                command(
                                    "hue-rescan",
                                    "TEXT_CASE_SCAN_AGAIN",
                                    true,
                                    unavailable,
                                    cx,
                                )
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        this.set_integration(Integration::Scanning, window, cx)
                                    },
                                )),
                            ),
                    )
                    .child(
                        div()
                            .my(surface::css(10.))
                            .child(text("TEXT_CASE_SCAN_AGAIN_OR")),
                    )
                    .child(
                        h_flex()
                            .gap(surface::css(10.))
                            .justify_center()
                            .child(text("TEXT_CASE_SCAN_MANUAL_IP"))
                            .child(self.ip_editor(cx))
                            .child(
                                command(
                                    "hue-ip-search",
                                    "TEXT_CASE_SCAN_SEARCH",
                                    false,
                                    unavailable,
                                    cx,
                                )
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        this.set_integration(Integration::ScanningIp, window, cx)
                                    },
                                )),
                            ),
                    );
            }
            Integration::ScanSuccessful => {
                body = body.child(text("TEXT_CASE_PAIR_CONTENT")).child(
                    div().mt(surface::css(10.)).child(
                        command("hue-pair", "TEXT_CASE_PAIR", true, unavailable, cx).on_click(
                            cx.listener(|this, _, window, cx| {
                                this.set_integration(Integration::WaitUserClickPair, window, cx)
                            }),
                        ),
                    ),
                );
            }
            Integration::Pairing => {
                body = body.child(text("TEXT_CASE_PAIRING_CONTENT")).child(
                    div()
                        .relative()
                        .w(surface::css(300.))
                        .h(surface::css(20.))
                        .mt(surface::css(10.))
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .w_full()
                                .h(surface::css(5.))
                                .rounded(surface::css(2.5))
                                .bg(cx.theme().primary.opacity(0.3))
                                .overflow_hidden()
                                // Preview frame only. An elapsed timer cannot imply a bridge response.
                                .child(
                                    div()
                                        .w(surface::css(80.))
                                        .h_full()
                                        .rounded(surface::css(2.5))
                                        .bg(cx.theme().primary),
                                ),
                        )
                        .child(
                            gpui_kit::base::Button::new("hue-cancel-pair")
                                .absolute()
                                .left_full()
                                .ml(surface::css(10.))
                                .group("hue-cancel-pair")
                                .size(surface::css(20.))
                                .p_0()
                                .disabled(unavailable)
                                .accessibility_label(text("TEXT_CASE_SCANNING"))
                                .child(img("synapse/hue-icon_close_enclosed_1.svg").size_full())
                                .child(
                                    img("synapse/hue-icon_close_enclosed_1_hover.svg")
                                        .absolute()
                                        .inset_0()
                                        .size_full()
                                        .opacity(0.)
                                        .group_hover("hue-cancel-pair", |s| s.opacity(1.)),
                                )
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.set_integration(Integration::PairCancel, window, cx)
                                })),
                        ),
                );
            }
            Integration::PairFailed | Integration::RetryPair => {
                body = body
                    .child(
                        div()
                            .text_color(Colors::error())
                            .child(text("TEXT_CASE_CONNECT_FAIL_CONTENT")),
                    )
                    .child(
                        div()
                            .text_color(Colors::error())
                            .child(text("TEXT_CASE_CONNECT_FAIL_FOLLOW_CONTENT")),
                    )
                    .child(
                        h_flex()
                            .gap(surface::css(10.))
                            .mt(surface::css(10.))
                            .justify_center()
                            // vi renders these without onClick callbacks in the current
                            // source. Preserve that fact; never invent a retry command.
                            .child(command(
                                "hue-pair-retry",
                                "TEXT_CASE_CONNECT_FAIL_RETRY",
                                true,
                                unavailable || self.integration == Integration::RetryPair,
                                cx,
                            ))
                            .child(command(
                                "hue-pair-start-over",
                                "TEXT_CASE_CONNECT_FAIL_START_OVER",
                                false,
                                unavailable || self.integration == Integration::RetryPair,
                                cx,
                            )),
                    );
            }
            // Bi has no case for the locally requested WAIT_USER_CLICK_PAIR;
            // middleware must supply PAIRING. Keep its transient empty content.
            Integration::WaitUserClickPair => {}
        }
        div()
            .id("hue-onboarding")
            .w_full()
            .flex()
            .justify_center()
            .child(widget("", None, div(), cx).child(body))
            .into_any_element()
    }
    fn ip_editor(&self, cx: &Context<Self>) -> AnyElement {
        h_flex()
            .border_1()
            .border_color(Colors::ip_border())
            .py(surface::css(5.))
            .px(surface::css(6.))
            .children(self.ip.iter().enumerate().flat_map(|(octet, input)| {
                let editor = div()
                    .id(("hue-ip-octet", octet))
                    .track_focus(&input.focus_handle(cx))
                    .on_key_up(cx.listener(move |this, event: &KeyUpEvent, window, cx| {
                        let value = this.ip[octet].read(cx).value();
                        let target = if event.keystroke.key == "backspace" && value.is_empty() {
                            Some(octet.saturating_sub(1))
                        } else if value.len() == 3 {
                            Some((octet + 1).min(3))
                        } else {
                            None
                        };
                        if let Some(target) = target {
                            this.ip[target].focus_handle(cx).focus(window, cx);
                        }
                    }))
                    .child(
                        Input::new(input)
                            .id(("hue-ip-input", octet))
                            .aria_label(format!(
                                "{} {}",
                                text("TEXT_CASE_SCAN_MANUAL_IP"),
                                octet + 1
                            ))
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .w(surface::css(27.))
                            .h(surface::css(18.))
                            .p_0()
                            .text_center()
                            .text_size(surface::css(14.)),
                    )
                    .into_any_element();
                let mut children = vec![editor];
                if octet < 3 {
                    children.push(div().child(".").into_any_element());
                }
                children
            }))
            .into_any_element()
    }
}
