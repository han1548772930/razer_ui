//! Current 179 Te parent widget: empty, temporary and confirmed binding rows.
use super::pairing_view::t;
use super::*;
use crate::ui::theme::{DockPairingColors as Colors, SliderColors};
use gpui_kit::base::Button as BaseButton;

fn title_case(value: &str) -> String {
    // Te lowercases then uppercases ASCII word starts (JavaScript /\b\w/g).
    let mut previous_word = false;
    value
        .to_lowercase()
        .chars()
        .map(|character| {
            let word = character.is_ascii_alphanumeric() || character == '_';
            let result = if word && !previous_word {
                character.to_ascii_uppercase()
            } else {
                character
            };
            previous_word = word;
            result
        })
        .collect()
}

fn connection_skeleton(cx: &App) -> AnyElement {
    let frame = move |phase: f32| {
        div()
            .relative()
            .overflow_hidden()
            .w(surface::css(80.))
            .h(surface::css(16.))
            .flex_shrink_0()
            .child(img("synapse/receiver/connection-skeleton-base.svg").size_full())
            .child(
                img("synapse/receiver/connection-skeleton-sweep.svg")
                    .absolute()
                    .top_0()
                    .left(surface::css(-80. + 160. * (phase / 0.75).min(1.)))
                    .w(surface::css(80.))
                    .h(surface::css(16.)),
            )
    };
    if cx.reduce_motion() {
        frame(0.).into_any_element()
    } else {
        div()
            .with_animation(
                "receiver-connection-skeleton",
                Animation::new(Duration::from_secs(2))
                    .repeat()
                    .with_easing(linear),
                move |view, phase| view.child(frame(phase)),
            )
            .into_any_element()
    }
}

impl SourceControls {
    pub(super) fn receiver_pairing_widget(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.receiver.page;
        let icon = || {
            img("synapse/hyperpolling-icon-multidevicepairing2.svg")
                .size(surface::css(44.))
                .flex_shrink_0()
        };
        let body = if let Some(peer) = state.peer() {
            let temporary = state.temporary();
            let connected = state.connected();
            let name = title_case(&peer.label(&crate::i18n::locale()));
            let name_enabled = temporary || connected == Some(true);
            let description = match connected {
                Some(true) => t("CONFIGURE_4000_POLLING_RATE"),
                Some(false) => t("CONFIGURE_POLLING_RATE_DEVICE_DISCONNECTED_TEXT"),
                None => crate::i18n::t_or(
                    "RECEIVER_CONNECTION_UNKNOWN",
                    "Device connection status is unavailable.",
                ),
            };
            let can_navigate = state.navigation().is_some();
            let details = v_flex()
                .id("receiver-paired-description")
                .test_support()
                .w(surface::css(369.))
                .flex_shrink_0()
                .font_family("Roboto")
                .text_color(Colors::dialog_warning_text())
                .when(
                    !temporary && !crate::i18n::locale().eq_ignore_ascii_case("en"),
                    |view| view.pl(surface::css(10.)),
                )
                .child(
                    h_flex()
                        .items_start()
                        .gap(surface::css(5.))
                        .text_size(surface::css(14.))
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .min_w_0()
                                .child(format!("{} ", t("PAIRED_WITH")))
                                .child(
                                    BaseButton::new("receiver-paired-name")
                                        .accessibility_label(name.clone())
                                        .disabled(!name_enabled)
                                        .text_color(if name_enabled {
                                            Colors::dialog_warning_text()
                                        } else {
                                            Colors::warning_text()
                                        })
                                        .underline()
                                        .focus_visible(|style| {
                                            style.border_1().border_color(SliderColors::thumb())
                                        })
                                        .when(name_enabled, |button| {
                                            button.hover(|style| {
                                                style.text_color(SliderColors::thumb())
                                            })
                                        })
                                        .child(name)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.open_receiver_pairing(window, cx)
                                        })),
                                ),
                        )
                        .when(temporary && state.loading, |title| {
                            title.child(
                                div()
                                    .id("receiver-parent-loading")
                                    .test_support()
                                    .role(Role::Status)
                                    .aria_label(t("LOADING"))
                                    .child(connection_skeleton(cx)),
                            )
                        }),
                )
                .child(
                    div()
                        .mt(surface::css(10.))
                        .text_size(surface::css(12.))
                        .text_color(Colors::warning_text())
                        .child(
                            BaseButton::new("receiver-configure-device")
                                .accessibility_label(description.clone())
                                .disabled(!can_navigate)
                                .text_left()
                                .when(can_navigate, |button| {
                                    button.hover(|style| style.text_color(SliderColors::thumb()))
                                })
                                .focus_visible(|style| {
                                    style.border_1().border_color(SliderColors::thumb())
                                })
                                .child(description)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(event) = this.receiver.page.navigation() {
                                        cx.emit(event);
                                    }
                                })),
                        ),
                );
            h_flex()
                .id("receiver-parent-binding")
                .test_support()
                .items_start()
                .justify_between()
                .mb(surface::css(20.))
                .child(icon())
                .child(details)
                .child(
                    BaseButton::new("receiver-parent-unpair")
                        .accessibility_label(t("UNPAIR"))
                        .disabled(temporary && state.loading)
                        .h(surface::css(if temporary && state.loading {
                            28.
                        } else {
                            27.
                        }))
                        .line_height(surface::css(if temporary && state.loading {
                            28.
                        } else {
                            27.
                        }))
                        .px(surface::css(5.))
                        .text_size(surface::css(12.))
                        .whitespace_nowrap()
                        .text_center()
                        .font_family("Roboto")
                        .text_color(Colors::dialog_warning_text())
                        .bg(Colors::card())
                        .border_1()
                        .border_color(if temporary && state.loading {
                            Colors::border()
                        } else {
                            Colors::receiver_unpair_border()
                        })
                        .rounded(surface::css(3.))
                        .when(
                            !temporary && crate::i18n::locale().eq_ignore_ascii_case("en"),
                            |button| button.w(surface::css(90.)),
                        )
                        .when(temporary && state.loading, |button| button.opacity(0.8))
                        .focus_visible(|style| style.border_1().border_color(SliderColors::thumb()))
                        .child(t("UNPAIR"))
                        // Te's Unpair button opens the utility, not a device write.
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_receiver_pairing(window, cx)
                        })),
                )
                .into_any_element()
        } else {
            h_flex()
                .id("receiver-parent-empty")
                .test_support()
                .gap(surface::css(10.))
                .child(icon())
                .child(
                    BaseButton::new("receiver-open-pairing")
                        .accessibility_label(t("OPEN_PAIRING_UTILITY"))
                        .h(surface::css(44.))
                        .text_size(surface::css(14.))
                        .line_height(surface::css(44.))
                        .text_color(Colors::dialog_warning_text())
                        .underline()
                        .hover(|style| style.text_color(SliderColors::thumb()))
                        .focus_visible(|style| style.border_1().border_color(SliderColors::thumb()))
                        .child(t("OPEN_PAIRING_UTILITY"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_receiver_pairing(window, cx)
                        })),
                )
                .into_any_element()
        };
        surface::panel(t("HYPERPOLLING_WIRELESS"), cx)
            .id("receiver-pairing-widget")
            .test_support()
            .relative()
            .w(surface::css(600.))
            .pr(surface::css(30.))
            .child(
                div()
                    .absolute()
                    .right(surface::css(10.))
                    .top(surface::css(10.))
                    .child(surface::receiver_help_control(
                        "receiver-pairing-help",
                        t("MULTI__DUALINK_PROPERTIES_TOOLTIP"),
                    )),
            )
            .child(body)
            .when(state.failed, |panel| {
                panel.child(
                    h_flex()
                        .gap_2()
                        .mt_2()
                        .child(
                            div()
                                .id("receiver-parent-read-error")
                                .test_support()
                                .role(Role::Status)
                                .child(crate::i18n::t_or(
                                    "RECEIVER_BINDING_READ_FAILED",
                                    "Unable to read the receiver binding.",
                                )),
                        )
                        .child(
                            BaseButton::new("receiver-parent-retry")
                                .disabled(!state.active)
                                .accessibility_label(crate::i18n::t_or("RETRY", "Retry"))
                                .underline()
                                .child(crate::i18n::t_or("RETRY", "Retry"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(event) = this.receiver.page.begin() {
                                        cx.emit(event);
                                    }
                                    this.sync_receiver_page_retry(cx);
                                    cx.notify();
                                })),
                        ),
                )
            })
            .into_any_element()
    }
}
