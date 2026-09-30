//! 校准页。
//!
//! 布局依据 `docs/screens/04-calibration.md`：
//!
//! | 分区 | 雷云真实文案 |
//! |---|---|
//! | ① 校准信息 | `CALIBRATION_INFORMATION` = 校准信息 |
//! | ② 表面配置文件列表 | `ADD_MAT` = 添加、`CREATE_OWN_SURFACE_PROFILE` |
//! | ③ 校准步骤 | `CALIBRATE_STEP1` = 单击鼠标左键，并移动鼠标。 |
//! | ④ 状态与结果 | `CALIBRATING` 校准中 / `CALIBRATION_COMPLETED` / `CALIBRATION_FAILED` |
//!
//! 该页在**鼠标与耳机**上出现（实测：182 有、777 有、653 无）。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::AppShell;
use crate::features::CalibrationState;
use crate::pages::widgets::{
    EmptyState, PageHeader, PageLayout, SettingRow, btn, card, card_title,
};

/// 渲染校准页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    let Some(calibration) = device.features.calibration.as_ref() else {
        return v_flex()
            .size_full()
            .gap_4()
            .child(PageHeader::new(
                "校准",
                format!("{} · 该设备无需校准", device.display_name()),
            ))
            .child(EmptyState::new(
                "该设备没有校准设置。实测：鼠标（182）与耳机（777）有，键盘（653）没有。",
            ))
            .into_any_element();
    };

    let state = calibration.state;
    let selected = calibration.selected;
    let surfaces: Vec<(String, bool, bool)> = calibration
        .surfaces
        .iter()
        .map(|s| (s.name.clone(), s.builtin, s.calibrated))
        .collect();
    let can_remove = calibration.current().map(|s| !s.builtin).unwrap_or(false);

    PageLayout::new("校准", device.display_name())
        // ① 校准信息
        .widget(
            card()
                .child(card_title("校准信息"))
                .child(div().text_sm().child(
                    "雷云原文：你的鼠标传感器需要进行微调，才能有效使用此预先校准的 Razer 雷蛇表面配置文件。",
                ))
                .child(SettingRow::new("当前状态", state.zh())),
        )
        // ② 表面配置文件
        .widget(
            card()
                .child(card_title("表面配置文件"))
                .children(surfaces.into_iter().enumerate().map(
                    |(index, (name, builtin, calibrated))| {
                        h_flex()
                            .w_full()
                            .justify_between()
                            .items_center()
                            .gap_3()
                            .child(
                                div().text_sm().child(format!(
                                    "{}{}",
                                    name,
                                    if builtin { "（预置）" } else { "" }
                                )),
                            )
                            .child(
                                h_flex()
                                    .gap_3()
                                    .items_center()
                                    .child(div().text_xs().child(if calibrated {
                                        "已校准"
                                    } else {
                                        "未校准"
                                    }))
                                    .child(
                                        btn(format!("surface-{index}"), if selected == index { "已选中" } else { "选择" })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.select_surface(index, cx)
                                            })),
                                    ),
                            )
                            .into_any_element()
                    },
                ))
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            btn("surface-add", "添加")
                                .on_click(cx.listener(|this, _, _, cx| this.add_surface(cx))),
                        )
                        .child(
                            btn("surface-remove", "删除")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if can_remove {
                                        this.remove_surface(cx);
                                    }
                                })),
                        ),
                )
                .child(div().text_xs().child("预置表面不可删除；「添加」对应雷云 ADD_MAT。")),
        )
        // ③ 校准步骤
        .widget(
            card()
                .child(card_title("校准步骤"))
                .child(SettingRow::new(
                    "第 1 步",
                    "单击鼠标左键，并移动鼠标。".to_string(),
                ))
                .child(SettingRow::new(
                    "第 2 步",
                    "以 Z 字形方式移动鼠标，并覆盖整个鼠标垫表面。".to_string(),
                ))
                .child(SettingRow::new(
                    "提示",
                    "鼠标移动过快会中断校准（雷云会提示「鼠标移动过快！」）".to_string(),
                )),
        )
        // ④ 操作与结果
        .widget(
            card()
                .child(card_title("开始校准"))
                .child(
                    h_flex()
                        .gap_2()
                        .flex_wrap()
                        .child(
                            btn("cali-start", "开始")
                                .on_click(cx.listener(|this, _, _, cx| this.start_calibration(cx))),
                        )
                        .child(
                            btn("cali-ok", "完成")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.finish_calibration(true, cx)
                                })),
                        )
                        .child(
                            btn("cali-fail", "标记失败")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.finish_calibration(false, cx)
                                })),
                        ),
                )
                .when(state == CalibrationState::Completed, |this| {
                    this.child(div().text_sm().child("校准完成。"))
                })
                .when(state == CalibrationState::Failed, |this| {
                    this.child(div().text_sm().child("校准失败。请重新开始校准。"))
                })
                .when(state == CalibrationState::Running, |this| {
                    this.child(div().text_sm().child("校准中..."))
                }),
        )
        .widget(
            card()
                .child(card_title("为什么这台设备有这个页面"))
                .child(SettingRow::new("标签页 key", "TAB_CALIBRATION".to_string()))
                .child(SettingRow::new(
                    "实测分布",
                    "鼠标 182 有 · 耳机 777 有 · 键盘 653 无".to_string(),
                )),
        )
        .into_any_element()
}
