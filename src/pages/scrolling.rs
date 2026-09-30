//! 滚动页。
//!
//! 布局依据 `docs/screens/06-scrolling.md`（从设备模块 CSS 的语义类名还原）：
//!
//! | 分区 | 雷云真实类名 |
//! |---|---|
//! | ① 滚动模式 | `SCROLL_MODE` / `FREE_SPIN` / `SCROLL_MODE_SWITCH` |
//! | ② 滚轮触觉等级 | `CONFIGURE_SCROLL_WHEEL_STAGES_SETTINGS` / `CYCLE_UP_SCROLL_WHEEL_STAGES` |
//! | ③ 每转级数 / 阻力 | `SCROLL_STEPS_TOOLTIP` / `SCROLL_TENSION_TOOLTIP` |
//! | ④ 高精度与水平滚动 | `HIGH_RESOLUTION_SCROLLING` / `HORIZONTAL_SCROLLING` |
//! | ⑤ 滚动加速 | `SCROLL_ACCELERATION` / `SCROLL_ACCELERATION_WITH_LEVEL` |
//!
//! 该页只在**鼠标与键盘**上出现（实测：鼠标 182 有、键盘 653 有、耳机 777 无）。

use gpui_kit::component::*;
use gpui_kit::*;

use crate::app::AppShell;
use crate::pages::widgets::{card_title, select_row, PageLayout, card, EmptyState, PageHeader, SettingRow, stepper_row, toggle_button, btn};

/// 渲染滚动页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    let Some(scrolling) = device.features.scrolling.as_ref() else {
        return v_flex()
            .size_full()
            .gap_4()
            .child(PageHeader::new(
                "滚动",
                format!("{} · 该设备没有滚轮设置", device.display_name()),
            ))
            .child(EmptyState::new(
"该设备没有滚动设置。实测：鼠标（182）与键盘（653）有，耳机（777）没有。",
            ))
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
    let stages: Vec<(u8, u8, bool)> = scrolling
        .stages
        .iter()
        .map(|stage| (stage.index, stage.haptics, stage.disabled))
        .collect();

    PageLayout::new("滚动", device.display_name())
        // ① 滚动模式
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
                        if let Some(picked) =
                            crate::features::ScrollingMode::ALL.get(index).copied()
                        {
                            this.set_scrolling_mode(picked, cx);
                        }
                    },
                ))
                .child(div().text_xs().child(
                    "雷云原文：若追求精度，请用触觉滚动模式；若追求速度，请用自由滚动模式。",
                )),
        )
        // ② 滚轮触觉等级
        .widget(
            card()
                .child(card_title("配置滚轮触觉等级"))
                .child(div().text_xs().child(format!(
                    "雷云最多允许禁用 2 个等级；当前已禁用 {disabled_count} 个"
                )))
                .children(stages.into_iter().enumerate().map(
                    |(index, (number, haptics, disabled))| {
                        h_flex()
                            .w_full()
                            .justify_between()
                            .items_center()
                            .gap_3()
                            .child(div().text_sm().child(format!("等级 {number}")))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        btn(format!("scroll-dec-{index}"), "◀")
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.adjust_scroll_stage_haptics(index, -1, cx)
                                            })),
                                    )
                                    .child(
                                        div()
                                            .min_w(px(90.))
                                            .text_center()
                                            .text_sm()
                                            .child(format!("{haptics}")),
                                    )
                                    .child(
                                        btn(format!("scroll-inc-{index}"), "▶")
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.adjust_scroll_stage_haptics(index, 1, cx)
                                            })),
                                    )
                                    .child(toggle_button(
                                        "scroll-stage-off",
                                        "禁用",
                                        disabled,
                                        cx,
                                        move |this, cx| this.toggle_scroll_stage_disabled(index, cx),
                                    )),
                            )
                            .into_any_element()
                    },
                )),
        )
        // ③ 每转级数 / 阻力
        .widget(
            card()
                .child(card_title("手感"))
                .child(stepper_row(
                    "每转级数",
                    format!("{steps}"),
                    cx,
                    |this, cx| this.adjust_scroll_steps(-1, cx),
                    |this, cx| this.adjust_scroll_steps(1, cx),
                ))
                .child(stepper_row(
                    "滚动阻力",
                    format!("{tension}%"),
                    cx,
                    |this, cx| this.adjust_scroll_tension(-1, cx),
                    |this, cx| this.adjust_scroll_tension(1, cx),
                ))
                .child(div().text_xs().child(
                    "雷云原文：调整滚轮每转的级数，以获得你喜欢的触感；降低滚动阻力以使滚轮平滑滚动，或增加阻力以提升触感。",
                )),
        )
        // ④ 高精度 / 水平 / 触觉
        .widget(
            card()
                .child(card_title("其它"))
                .child(
                    h_flex()
                        .gap_4()
                        .flex_wrap()
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
                        )),
                ),
        )
        // ⑤ 滚动加速
        .widget(
            card()
                .child(card_title("滚动加速"))
                .child(
                    h_flex()
                        .gap_4()
                        .child(toggle_button(
                            "scroll-accel",
                            "启用滚动加速",
                            acceleration,
                            cx,
                            |this, cx| this.toggle_scroll_acceleration(cx),
                        )),
                )
                .child(SettingRow::new("加速等级", format!("{acceleration_level}")))
                .child(div().text_xs().child(
                    "雷云原文：提升你滑动滚轮时的滚动速度（自由滚动模式下生效）。",
                )),
        )
        // 该设备显示该页的证据
        .widget(
            card()
                .child(card_title("为什么这台设备有这个页面"))
                .child(SettingRow::new("标签页 key", "TAB_SCROLLING".to_string()))
                .child(SettingRow::new(
                    "判定依据",
                    "设备模块常量块声明了该标签页".to_string(),
                ))
                .child(SettingRow::new(
                    "实测分布",
                    "鼠标 182 有 · 键盘 653 有 · 耳机 777 无".to_string(),
                )),
        )
        .into_any_element()
}
