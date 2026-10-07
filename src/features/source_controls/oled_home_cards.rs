//! Current OLED `di -> Q -> G`: all seven cards, source order and hover actions.
use super::*;
use crate::ui::hover_tip::{SourceTipPlacement, source_hover_tip_element};
use crate::ui::source_tooltip::{SourceTooltip, SourceTooltipKind};

impl SourceControls {
    pub(in super::super) fn render_oled_presets(
        &self,
        disabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self
            .draft
            .pointer("/oled/homeScreenDisplay/selected")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        h_flex()
            .relative()
            .mt(surface::css(20.))
            .gap(surface::css(10.))
            .flex_wrap()
            .children(
                [
                    (0, "animation", "OLED_HOME_SCREEN_DISPLAY_TITLE_ANIMATION"),
                    (1, "image", "OLED_HOME_SCREEN_DISPLAY_TITLE_IMAGE"),
                    (4, "emote", "OLED_HOME_SCREEN_DISPLAY_TITLE_EMOTE"),
                    (2, "banner", "OLED_HOME_SCREEN_DISPLAY_TITLE_BANNER"),
                    (5, "media", "OLED_HOME_SCREEN_DISPLAY_TITLE_AUDIO_METER"),
                    (6, "system", "OLED_HOME_SCREEN_DISPLAY_TITLE_SYSTEM_INFO"),
                    (
                        3,
                        "keyboard",
                        "OLED_HOME_SCREEN_DISPLAY_TITLE_KEYBOARD_INFO",
                    ),
                ]
                .into_iter()
                .map(|(mode, name, title)| {
                    let ble_disabled = self.is_ble && matches!(mode, 5 | 6);
                    let active = selected == mode;
                    let group = SharedString::from(format!("oled-card-body-{name}"));
                    let hover = window.use_keyed_state(
                        (ElementId::from(group.clone()), "hover"),
                        cx,
                        |_, _| false,
                    );
                    let hovered = !disabled && !ble_disabled && *hover.read(cx);
                    // 源 OLED chunk：`require-synapse-icon-{id}` 上 `onMouseEnter`/`onMouseLeave`
                    // 直接翻转 `isMounted`，配 `xA position:"bottom-right" target="require-synapse-icon-{id}"`
                    // —— 即时挂载的 `.tooltip-razer.bottom-right` 提示。
                    let require_synapse_hover = window.use_keyed_state(
                        (
                            ElementId::from(format!("oled-require-synapse-hover-{name}")),
                            "hover",
                        ),
                        cx,
                        |_, _| false,
                    );
                    let require_synapse_hovered =
                        matches!(mode, 5 | 6) && !ble_disabled && *require_synapse_hover.read(cx);
                    let preview = match mode {
                        0 | 1 => {
                            let kind = if mode == 0 {
                                PresetKind::Animation
                            } else {
                                PresetKind::Image
                            };
                            let selection = self.preset_selection(kind);
                            let preset = selection.list.get(selection.selected_ix);
                            let source = preset
                                .and_then(|p| p.src.clone())
                                .unwrap_or_else(|| kind.asset(selection.selected_ix).to_string());
                            cropped_preview(
                                format!("oled-home-{name}").into(),
                                source.into(),
                                preset.and_then(|p| p.local_crop.as_ref()),
                            )
                        }
                        5 => OledMediaDraft::from_value(
                            self.draft.pointer("/oled/media").unwrap_or(&Value::Null),
                        )
                        .preview_content(),
                        6 => super::super::oled_system_editor::system_preview(
                            self.draft.pointer("/oled/system"),
                        ),
                        3 => img("synapse/oled-691-keyboard.png")
                            .w(surface::css(256.))
                            .h(surface::css(64.))
                            .grayscale(true)
                            .into_any_element(),
                        // Emote and Banner previews/editors are not implemented.
                        // Retain the source card geometry without invented
                        // telemetry, placeholder prose, or nonfunctional editor dialogs.
                        _ => div()
                            .w(surface::css(232.))
                            .h(surface::css(64.))
                            .into_any_element(),
                    };
                    let editable = mode != 3;
                    let edit_implemented = matches!(mode, 0 | 1 | 5 | 6);
                    let edit_disabled =
                        disabled || ble_disabled || self.is_ble || !edit_implemented;
                    let mut overlay = h_flex()
                        .absolute()
                        .left_0()
                        .top(surface::css(-1.))
                        .w(surface::css(236.))
                        .h(surface::css(68.))
                        .justify_around()
                        .bg(rgba(0x000000cc))
                        .border_2()
                        .border_color(rgba(0x44d62c4d));
                    if editable {
                        let edit = card_action(
                            format!("oled-edit-{name}"),
                            "EDIT",
                            "synapse/oled-691-edit.svg",
                            false,
                            edit_disabled,
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| match mode {
                                0 => this.open_oled_presets(PresetKind::Animation, window, cx),
                                1 => this.open_oled_presets(PresetKind::Image, window, cx),
                                5 => this.open_oled_media(window, cx),
                                6 => this.open_oled_system(window, cx),
                                _ => {}
                            },
                        ));
                        // 源 691/1383 的 OLED chunk：BLE 打开时卡片带
                        // `turn-off-ble-tooltip` 属性，CSS
                        // `[turn-off-ble-tooltip]:before{background:#000;border:1px solid #5d5d5d;
                        //  color:#ccc;font-size:14px;left:20px;line-height:16px;padding:8px 10px;
                        //  position:absolute;top:185px;transition:visibility 0s,opacity .3s linear}`
                        // 即卡片相对的 20px/185px 提示，300ms 线性淡入；本地已有该 kind。
                        let trigger = edit.into_any_element();
                        let edit = if self.is_ble {
                            SourceTooltip::new(
                                format!("oled-edit-tooltip-{name}"),
                                t("OLED_DISABLE_EDIT_BLE_MODE_TOOLTIP"),
                                300.,
                            )
                            .kind(SourceTooltipKind::OledBleDisabled)
                            .trigger(move |_, _, _| trigger)
                            .into_any_element()
                        } else {
                            trigger
                        };
                        overlay = overlay.child(edit);
                    }
                    overlay = overlay.child(
                        card_action(
                            format!("oled-apply-{name}"),
                            "APPLY",
                            "synapse/oled-691-apply.svg",
                            true,
                            disabled || ble_disabled || active,
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.edit("691:oled-home-mode", Value::from(mode), window, cx);
                            },
                        )),
                    );
                    let card = v_flex()
                        .id(SharedString::from(format!("oled-home-card-{name}")))
                        .w(surface::css(236.))
                        .flex_shrink_0()
                        .when(ble_disabled, |card| card.opacity(0.5))
                        .child(
                            h_flex()
                                .h(surface::css(19.))
                                .p(surface::css(2.))
                                .gap(surface::css(7.))
                                .text_size(surface::css(14.))
                                // OLED CSS declares title.hover after title.selected.
                                .text_color(if hovered {
                                    rgba(0x44d62c4d)
                                } else if active {
                                    rgba(0x44d62cff)
                                } else {
                                    rgba(0xccccccff)
                                })
                                .child(t(title).to_uppercase())
                                .when(matches!(mode, 5 | 6), |title| {
                                    title.child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "oled-require-synapse-{name}"
                                            )))
                                            .size(surface::css(15.))
                                            .when(!ble_disabled, |icon| {
                                                icon.on_hover(window.listener_for(
                                                    &require_synapse_hover,
                                                    |state, hovered, _, cx| {
                                                        *state = *hovered;
                                                        cx.notify();
                                                    },
                                                ))
                                            })
                                            .when(require_synapse_hovered, |icon| {
                                                icon.child(source_hover_tip_element(
                                                    format!("oled-require-synapse-tip-{name}"),
                                                    SourceTipPlacement::BottomRight,
                                                    t("OLED_REQUIRE_SYNAPSE_RUNNING_TOOLTIP"),
                                                ))
                                            })
                                            .child(
                                                img("synapse/oled-691-requires-synapse.svg")
                                                    .size_full(),
                                            ),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .id(group.clone())
                                .group(group.clone())
                                .on_hover(window.listener_for(&hover, |state, hovered, _, cx| {
                                    *state = *hovered;
                                    cx.notify();
                                }))
                                .relative()
                                .w(surface::css(236.))
                                .h(surface::css(68.))
                                .p(surface::css(1.))
                                .child(
                                    v_flex()
                                        .relative()
                                        .w(surface::css(234.))
                                        .h(surface::css(66.))
                                        .bg(rgb(0))
                                        .border_1()
                                        .border_color(rgb(0x5d5d5d))
                                        .items_center()
                                        .justify_center()
                                        .when(active, |body| {
                                            body.border_2()
                                                .w(surface::css(236.))
                                                .h(surface::css(68.))
                                                .m(surface::css(-1.))
                                                .border_color(rgb(0x44d62c))
                                        })
                                        .child(preview),
                                )
                                .when(!disabled && !ble_disabled, |body| {
                                    body.child(
                                        overlay.invisible().group_hover(group, |s| s.visible()),
                                    )
                                }),
                        )
                        .into_any_element();
                    if ble_disabled {
                        SourceTooltip::new(
                            format!("oled-ble-disabled-{name}"),
                            t("OLED_HOME_SCREEN_DISPLAY_TURN_OFF_BLE_MODE_TOOLTIP"),
                            300.,
                        )
                        .kind(SourceTooltipKind::OledBleDisabled)
                        .trigger(move |_, _, _| card)
                        .into_any_element()
                    } else {
                        card
                    }
                }),
            )
            .when(disabled, |cards| {
                cards.child(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(rgb(0x111111))
                        .opacity(0.8)
                        .occlude(),
                )
            })
            .into_any_element()
    }
}

fn card_action(
    id: String,
    label: &'static str,
    icon: &'static str,
    apply: bool,
    disabled: bool,
) -> gpui_kit::base::Button {
    let color = if disabled {
        rgb(0x707070)
    } else if apply {
        rgb(0x44d62c)
    } else {
        rgb(0xffffff)
    };
    gpui_kit::base::Button::new(SharedString::from(id))
        .accessibility_label(t(label))
        .disabled(disabled)
        .flex()
        .flex_col()
        .justify_center()
        .items_center()
        .p_0()
        .text_size(surface::css(12.))
        .line_height(surface::css(14.))
        .font_weight(FontWeight::NORMAL)
        .text_color(color)
        .child(
            svg()
                .path(icon)
                .size(surface::css(24.))
                .mb(surface::css(8.))
                .text_color(color),
        )
        .child(t(label).to_uppercase())
}
