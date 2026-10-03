//! The right-hand LaunchSoundApp card is part of the actual 3946 Customize page.
use super::*;
impl Automation {
    pub(super) fn render_launch_sound(&self, cx: &Context<Self>) -> AnyElement {
        // 0: THX entitlement, not installed/unknown; 1: THX installed;
        // 2/3: source fallback 7.1 download/installed. Nonzero samples are preview-only.
        let thx = self.audio_mode < 2;
        let installed = self.audio_mode == 1 || self.audio_mode == 3;
        let heading = if thx {
            "THX_SPATIAL_AUDIO"
        } else {
            "SURROUND_SOUND_HEADER"
        };
        let action = match (thx, installed) {
            (true, true) => "THX_SPATIAL_AUDIO_LAUNCH",
            (true, false) => "THX_SPATIAL_AUDIO_DOWNLOAD",
            (false, true) => "SURROUND_SOUND_LAUNCH",
            (false, false) => "SURROUND_SOUND_DOWNLOAD",
        };
        let mut card = surface::panel(text(heading), cx)
            .text_color(Colors::foreground())
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .child(text(if thx {
                "THX_SPATIAL_AUDIO_DESC"
            } else {
                "SURROUND_SOUND_DESC_FOR_LAUNCH"
            }));
        if thx && !installed {
            card = card.child(text("THX_SPATIAL_AUDIO_DESC_LINE2"));
        }
        if !thx && !installed {
            card = card
                .child(text("SURROUND_SOUND_DESC_FOR_DOWNLOAD_V2").replace(
                    "{{url}}",
                    &text("SURROUND_SOUND_DESC_FOR_DOWNLOAD_V2_Partial"),
                ))
                .child(
                    Button::new("automation-register-audio")
                        .ghost()
                        .label(text("SURROUND_SOUND_DESC_FOR_DOWNLOAD_V2_Partial"))
                        .on_click(|_, _, cx| cx.open_url("https://razerid.razer.com/products")),
                );
        }
        card = card.child(
            h_flex()
                .justify_between()
                .mt(surface::css(20.))
                .child(
                    gpui_kit::base::Button::new("automation-launch-audio")
                        .accessibility_label(text(action))
                        .flex()
                        .items_center()
                        .gap(surface::css(10.))
                        .h(surface::css(50.))
                        .px(surface::css(16.))
                        .border_1()
                        .border_color(Colors::foreground())
                        .rounded(surface::css(3.))
                        .child(
                            img(if thx {
                                "synapse/automation-thx_spatial_audio_logo.svg"
                            } else {
                                "synapse/automation-logo-7.1.svg"
                            })
                            .size(surface::css(30.)),
                        )
                        .child(text(action).to_uppercase())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if installed {
                                this.alert = Some(if this.preview {
                                    format!("示例请求：{}", text(action))
                                } else {
                                    "暂时无法启动音频应用。".into()
                                });
                                cx.notify();
                            } else {
                                cx.open_url(if thx {
                                    "https://rzr.to/thx-spatial-audio"
                                } else {
                                    "https://www.razer.com/sg-en/71-surround-sound"
                                });
                            }
                        })),
                )
                .when(thx, |row| {
                    row.child(
                        Button::new("automation-activation-code")
                            .ghost()
                            .small()
                            .label(text("SHOW_ACTIVATION_CODE"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                let preview = this.preview;
                                window.open_dialog(cx, move |dialog, _, cx| {
                                    dialog
                                        .title(text("ACTIVATION_CODE_FOR_THX_SPATIAL_AUDIO"))
                                        .child(surface::note(
                                            if preview {
                                                "示例状态：激活码尚未返回。"
                                            } else {
                                                "暂时无法获取激活码，请稍后重试。"
                                            },
                                            cx,
                                        ))
                                });
                            })),
                    )
                }),
        );
        if installed {
            card = card.child(
                Button::new("automation-audio-learn")
                    .ghost()
                    .label(text("LEARN_MORE"))
                    .on_click(move |_, _, cx| {
                        cx.open_url(if thx {
                            "https://mysupport.razer.com/app/answers/detail/a_id/3779"
                        } else {
                            "https://mysupport.razer.com/app/answers/detail/a_id/3770"
                        })
                    }),
            );
        }
        card.into_any_element()
    }
}
