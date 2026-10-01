//! productId 182 鼠标表面校准页。
//!
//! 表面卡片和校准弹层状态来自 182 原模块；不把其它设备的校准文案混入此页。

use gpui_kit::assets::IconName;
use gpui_kit::component::*;
use gpui_kit::*;

use crate::domain::CalibrationState;
use crate::shell::AppShell;
use crate::ui::widgets::{btn, widget_slot, EmptyState, PageLayout};

const SURFACE_W: f32 = 290.0;
const SURFACE_H: f32 = 200.0;

pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };
    if device.product_id != 182 {
        return EmptyState::new("校准页当前只为 productId 182 鼠标实现").into_any_element();
    }
    if device.features.calibration.is_none() {
        return EmptyState::new("该设备没有表面校准设置").into_any_element();
    }

    PageLayout::new("校准", device.display_name())
        .without_product_banner()
        .widget(widget_slot(calibration_content(app, cx)))
        .into_any_element()
}

fn calibration_content(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("校准数据不可用").into_any_element();
    };
    let Some(calibration) = device.features.calibration.as_ref() else {
        return EmptyState::new("校准数据不可用").into_any_element();
    };

    let state = calibration.state;
    let selected = calibration.selected;
    let locked = state == CalibrationState::Running;
    let can_remove = calibration
        .current()
        .map(|surface| !surface.builtin)
        .unwrap_or(false);
    let selected_builtin = calibration.current().map(|surface| surface.builtin).unwrap_or(false);

    let mut surfaces = h_flex().w_full().flex_wrap().gap(px(20.));
    for (index, surface) in calibration.surfaces.iter().cloned().enumerate() {
        surfaces = surfaces.child(surface_card(
            index,
            surface.name,
            surface.builtin,
            surface.calibrated,
            index == selected,
            locked,
            cx,
        ));
    }
    surfaces = surfaces.child(add_surface_card(locked, cx));

    let start_label = match state {
        CalibrationState::Idle => "开始校准",
        CalibrationState::Running => "校准进行中",
        CalibrationState::Completed => "重新校准",
        CalibrationState::Failed => "重试校准",
    };

    let mut content = v_flex()
        .w_full()
        .gap_3()
        .p(px(30.))
        .rounded(px(5.))
        .bg(cx.theme().group_box)
        .child(div().text_size(px(16.)).font_bold().child("表面校准"))
        .child(
            div()
                .w_full()
                .max_w(px(600.))
                .mx_auto()
                .text_sm()
                .child(if selected_builtin {
                    "你的鼠标传感器需要进行微调，才能有效使用此预先校准的 Razer 表面配置文件。"
                } else {
                    "选择一个表面配置文件，然后开始校准。"
                }),
        )
        .child(surfaces)
        .child(
            h_flex()
                .gap_2()
                .child(
                    btn("calibration-start", start_label)
                        .disabled(locked || calibration.current().is_none())
                        .on_click(cx.listener(|this, _, _, cx| this.start_calibration(cx))),
                )
                .child(
                    btn("surface-add-inline", "添加表面")
                        .disabled(locked)
                        .on_click(cx.listener(|this, _, _, cx| this.add_surface(cx))),
                )
                .child(
                    btn("surface-remove-inline", "删除表面")
                        .disabled(locked || !can_remove)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if can_remove {
                                this.remove_surface(cx);
                            }
                        })),
                ),
        );

    content = content.child(state_message(state, cx));
    content.into_any_element()
}

fn surface_card(
    index: usize,
    name: String,
    builtin: bool,
    calibrated: bool,
    selected: bool,
    locked: bool,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    v_flex()
        .id(format!("surface-{index}"))
        .flex_shrink_0()
        .w(px(SURFACE_W))
        .h(px(SURFACE_H))
        .items_center()
        .gap_2()
        .p(px(8.))
        .rounded(px(5.))
        .bg(cx.theme().background)
        .border_1()
        .border_color(if selected {
            cx.theme().primary
        } else {
            cx.theme().background
        })
        .child(Icon::new(IconName::Mouse).w(px(64.)).h(px(64.)))
        .child(div().text_sm().child(format!("{}{}", name, if builtin { "（预置）" } else { "" })))
        .child(
            div()
                .text_xs()
                .text_color(if calibrated {
                    cx.theme().primary
                } else {
                    cx.theme().muted_foreground
                })
                .child(if calibrated { "已校准" } else { "未校准" }),
        )
        .child(
            btn(
                format!("surface-select-{index}"),
                if selected { "已选择" } else { "选择" },
            )
            .disabled(locked || selected)
            .on_click(cx.listener(move |this, _, _, cx| this.select_surface(index, cx))),
        )
        .into_any_element()
}

fn add_surface_card(locked: bool, cx: &mut Context<AppShell>) -> AnyElement {
    div()
        .id("surface-add")
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .w(px(SURFACE_W))
        .h(px(SURFACE_H))
        .gap_2()
        .rounded(px(5.))
        .border_1()
        .border_color(cx.theme().border)
        .text_sm()
        .child("创建自己的表面配置文件")
        .child(
            btn("surface-add-card", "添加")
                .disabled(locked)
                .on_click(cx.listener(|this, _, _, cx| this.add_surface(cx))),
        )
        .into_any_element()
}

fn state_message(state: CalibrationState, cx: &mut Context<AppShell>) -> AnyElement {
    let text = match state {
        CalibrationState::Idle => "校准尚未开始。单击左键并移动鼠标，然后按提示覆盖整个表面。",
        CalibrationState::Running => "校准进行中。请按照校准弹层的图示移动鼠标；结果等待设备服务回传。",
        CalibrationState::Completed => "校准已完成。当前表面已标记为已校准。",
        CalibrationState::Failed => "校准失败。请检查鼠标移动速度后重试。",
    };
    div()
        .w_full()
        .text_sm()
        .text_color(if state == CalibrationState::Failed {
            cx.theme().warning
        } else {
            cx.theme().muted_foreground
        })
        .child(text)
        .into_any_element()
}
