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

use crate::shell::AppShell;
use crate::domain::CalibrationState;
use crate::ui::widgets::{
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
    let surfaces: Vec<(String, bool, bool, String)> = calibration
        .surfaces
        .iter()
        .map(|s| {
            (
                s.name.clone(),
                s.builtin,
                s.calibrated,
                surface_element_id(&s.name, s.builtin),
            )
        })
        .collect();
    let can_remove = calibration.current().map(|s| !s.builtin).unwrap_or(false);
    let workflow_locked = state == CalibrationState::Running;
    let start_label = match state {
        CalibrationState::Idle => "开始校准",
        CalibrationState::Completed => "重新校准",
        CalibrationState::Failed => "重试校准",
        CalibrationState::Running => "校准进行中",
    };

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
                    |(index, (name, builtin, calibrated, element_id))| {
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
                                        btn(element_id, if selected == index { "已选中" } else { "选择" })
                                            .disabled(workflow_locked)
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
                                .disabled(workflow_locked)
                                .on_click(cx.listener(|this, _, _, cx| this.add_surface(cx))),
                        )
                        .child(
                            btn("surface-remove", "删除")
                                .disabled(!can_remove || workflow_locked)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if can_remove && !workflow_locked {
                                        this.remove_surface(cx);
                                    }
                                })),
                        ),
                )
                .child(div().text_xs().child(
                    "预置表面不可删除；「添加」对应雷云 ADD_MAT。校准进行中不能切换或删除表面。",
                )),
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
                            btn("cali-start", start_label)
                                .disabled(workflow_locked)
                                .on_click(cx.listener(|this, _, _, cx| this.start_calibration(cx))),
                        )
                )
                .child(match state {
                    CalibrationState::Idle => div()
                        .text_sm()
                        .child("尚未开始；选择表面后启动校准流程。"),
                    CalibrationState::Running => div().text_sm().child(
                        "校准进行中，等待设备服务返回阶段和进度；页面不会伪造完成结果。",
                    ),
                    CalibrationState::Completed => div().text_sm().child(
                        "校准成功；当前有效结果已保留。需要再次校准时可重新启动流程。",
                    ),
                    CalibrationState::Failed => div().text_sm().child(
                        "校准失败；上一次有效校准仍保留，可重试或更换表面。",
                    ),
                })
                .child(SettingRow::new(
                    "设备回传",
                    "完成、失败和进度由设备服务确认；当前前端不提供手动结束按钮。".to_string(),
                )),
        )
        .into_any_element()
}

fn surface_element_id(name: &str, builtin: bool) -> String {
    let prefix = if builtin { "surface-builtin" } else { "surface-custom" };
    let suffix: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    format!("{prefix}-{suffix}")
}
