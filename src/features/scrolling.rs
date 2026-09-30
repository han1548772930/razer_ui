//! 滚动页：滚动模式、滚轮触觉阶段、阻力、高分辨率和滚动加速。
//!
//! 页面只在设备声明 `features.scrolling` 时出现，并保持雷云设备页的
//! 250px 产品区与 600px widget 两列布局。没有滚动能力的设备使用真实
//! unavailable 状态，而不是渲染不可写的空设置。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::shell::AppShell;
use crate::ui::widgets::{
    btn, card, card_title, EmptyState, PageLayout, select_row, slider_row, stepper_row,
    toggle_button, SettingRow,
};

/// 渲染滚动页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    let device_name = device.display_name().to_string();
    let Some(scrolling) = device.features.scrolling.as_ref() else {
        return PageLayout::new("滚动", device_name.clone())
            .subtitle(format!("{device_name} · 该设备未声明滚动能力"))
            .widget(
                card()
                    .child(card_title("滚动能力"))
                    .child(EmptyState::new("该设备没有滚动设置。"))
                    .child(div().text_xs().child(
                        "滚动页仅由设备能力清单提供；耳机等没有滚轮的设备不会显示空控件。",
                    )),
            )
            .into_any_element();
    };

    let mode = scrolling.mode;
    let steps = scrolling.steps;
    let tension = scrolling.tension;
    let acceleration = scrolling.acceleration;
    let acceleration_level = scrolling.acceleration_level;
    let high_resolution = scrolling.high_resolution;
    let horizontal = scrolling.horizontal;
    let haptics_enabled = scrolling.haptics_enabled;
    let disabled_count = scrolling.disabled_count();
    let stages = scrolling
        .stages
        .iter()
        .map(|stage| (stage.index, stage.haptics, stage.disabled))
        .collect::<Vec<_>>();

    let stage_card = card()
        .child(card_title("滚轮触觉阶段"))
        .child(div().text_xs().child(format!(
            "每个阶段使用稳定业务编号；最多禁用 2 个阶段，当前已禁用 {disabled_count} 个。"
        )))
        .when(!haptics_enabled, |this| {
            this.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().warning)
                    .child("滚轮触觉已关闭；阶段配置会保留，重新启用后才会预览。"),
            )
        })
        .children(stages.into_iter().map(|(stage_id, haptics, disabled)| {
            let stage_index = stage_id.saturating_sub(1) as usize;
            let can_disable = disabled || disabled_count < 2;
            h_flex()
                .w_full()
                .min_h(px(68.))
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    v_flex()
                        .gap_1()
                        .child(div().text_sm().child(format!("阶段 {stage_id}")))
                        .child(div().text_xs().child(if disabled {
                            "已禁用"
                        } else {
                            "可用"
                        })),
                )
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(btn(
                            format!("scroll-stage-{stage_id}-dec"),
                            "−",
                        ).on_click(cx.listener(move |this, _, _, cx| {
                            this.adjust_scroll_stage_haptics(stage_index, -1, cx)
                        })))
                        .child(
                            div()
                                .min_w(px(90.))
                                .text_center()
                                .text_size(px(28.))
                                .child(format!("{haptics}%")),
                        )
                        .child(btn(
                            format!("scroll-stage-{stage_id}-inc"),
                            "+",
                        ).on_click(cx.listener(move |this, _, _, cx| {
                            this.adjust_scroll_stage_haptics(stage_index, 1, cx)
                        })))
                        .child(if can_disable {
                            btn(
                                format!("scroll-stage-{stage_id}-toggle"),
                                if disabled { "启用" } else { "禁用" },
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_scroll_stage_disabled(stage_index, cx)
                            }))
                            .into_any_element()
                        } else {
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("已达上限")
                                .into_any_element()
                        }),
                )
                .into_any_element()
        }));

    PageLayout::new("滚动", device_name.clone())
        .subtitle(format!("{device_name} · 更改会立即保存"))
        .widget(
            card()
                .child(card_title("滚动模式"))
                .child(select_row(
                    "scroll-mode",
                    "模式",
                    mode.zh(),
                    &crate::features::ScrollingMode::LABELS,
                    app.open_select.as_deref() == Some("scroll-mode"),
                    cx,
                    |this, index, cx| {
                        if let Some(picked) = crate::features::ScrollingMode::ALL.get(index).copied()
                        {
                            this.set_scrolling_mode(picked, cx);
                        }
                    },
                ))
                .child(div().text_xs().child(
                    "触觉模式强调精度，自由滚动模式强调速度；切换会立即预览并写入本地草稿。",
                )),
        )
        .widget(stage_card)
        .widget(
            card()
                .child(card_title("滚轮手感"))
                .child(stepper_row(
                    "每转级数",
                    format!("{steps}"),
                    cx,
                    |this, cx| this.adjust_scroll_steps(-1, cx),
                    |this, cx| this.adjust_scroll_steps(1, cx),
                ))
                .child(slider_row(
                    "scroll-tension",
                    "滚动阻力",
                    tension as f32,
                    0.,
                    100.,
                    5.,
                    format!("{tension}%"),
                    true,
                    cx,
                    |this, value, cx| this.set_scroll_tension(value, cx),
                ))
                .child(div().text_xs().child(
                    "降低阻力可使滚轮更顺滑，增加阻力可提升触觉反馈。",
                )),
        )
        .widget(
            card()
                .child(card_title("滚动选项"))
                .child(toggle_button(
                    "scroll-hires",
                    "高分辨率滚动",
                    high_resolution,
                    cx,
                    |this, cx| this.toggle_high_resolution_scrolling(cx),
                ))
                .child(toggle_button(
                    "scroll-horizontal",
                    "水平滚动",
                    horizontal,
                    cx,
                    |this, cx| this.toggle_horizontal_scrolling(cx),
                ))
                .child(toggle_button(
                    "scroll-haptics",
                    "滚轮触觉",
                    haptics_enabled,
                    cx,
                    |this, cx| this.toggle_scroll_haptics(cx),
                ))
                .child(div().text_xs().child(
                    "设备不支持的选项应由能力清单过滤；本页只渲染当前设备已声明的滚动模块。",
                )),
        )
        .widget(
            card()
                .child(card_title("滚动加速"))
                .child(toggle_button(
                    "scroll-acceleration",
                    "启用滚动加速",
                    acceleration,
                    cx,
                    |this, cx| this.toggle_scroll_acceleration(cx),
                ))
                .child(SettingRow::new("加速等级", format!("{acceleration_level}")))
                .child(div().text_xs().child(
                    "滚动加速主要在自由滚动模式下生效；设备确认前保留当前草稿。",
                )),
        )
        .into_any_element()
}
