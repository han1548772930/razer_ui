//! productId 182 鼠标性能页。
//!
//! 只保留 182 原模块已经证明的四类内容：DPI/XY、轮询率、抬升距离和阶段列表。
//! 其它共享 locale 或未命中的能力不在这里渲染。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::features::{LiftOffDistance, PollingRate};
use crate::shell::AppShell;
use crate::ui::widgets::{select_row, slider_row, widget_slot, EmptyState, PageLayout};

const STAGE_COLORS: [u32; 5] = [0xff1a1a, 0x24ff00, 0x006fff, 0x00edff, 0xfff700];

pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };
    if device.product_id != 182 {
        return EmptyState::new("性能页当前只为 productId 182 鼠标实现")
            .into_any_element();
    }

    let mut layout = PageLayout::new("性能", device.display_name()).without_product_banner();
    layout = layout.widget(widget_slot(dpi_panel(app, cx)));
    layout = layout.widget(widget_slot(polling_panel(app, cx)));
    if device.features.performance.lift_off.is_some() {
        layout = layout.widget(widget_slot(lift_off_panel(app, cx)));
    }
    layout.into_any_element()
}

fn panel(title: &'static str, cx: &mut Context<AppShell>) -> Div {
    v_flex()
        .w_full()
        .gap_3()
        .p(px(30.))
        .rounded(px(5.))
        .bg(cx.theme().group_box)
        .child(div().text_size(px(16.)).font_bold().child(title))
}

fn dpi_panel(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("DPI 数据不可用").into_any_element();
    };
    let (min, max, step) = device.dpi_bounds();
    let xy_supported = device.support_xy_dpi || device.product_id == 182;
    let stages = device
        .active_profile_obj()
        .and_then(|profile| profile.dpi_stages.as_ref())
        .map(|dpi| dpi.stages.clone())
        .unwrap_or_default();
    let Some(active) = stages.first().copied() else {
        return panel("DPI", cx)
            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(
                "当前配置文件没有可编辑的 DPI 阶段。",
            ))
            .into_any_element();
    };

    let mut content = panel("DPI", cx)
        .child(div().text_xs().text_color(cx.theme().muted_foreground).child(format!(
            "范围 {min}–{max} DPI，步进 {step}。XY 控件仅在设备声明 supportXYDPI 时显示。",
        )))
        .child(slider_row(
            "performance-dpi-x",
            "X",
            active.x as f32,
            min as f32,
            max as f32,
            step as f32,
            format!("{} DPI", active.x),
            true,
            cx,
            move |this, value, cx| {
                this.edit_device(cx, |device| {
                    let value = snap_dpi(value, min, max, step);
                    if let Some(stages) = device.dpi_stages_mut() {
                        if let Some(stage) = stages.stages.first_mut() {
                            stage.x = value;
                            if !xy_supported {
                                stage.y = value;
                            }
                        }
                    }
                });
            },
        ));

    if xy_supported {
        content = content.child(slider_row(
            "performance-dpi-y",
            "Y",
            active.y as f32,
            min as f32,
            max as f32,
            step as f32,
            format!("{} DPI", active.y),
            true,
            cx,
            move |this, value, cx| {
                this.edit_device(cx, |device| {
                    let value = snap_dpi(value, min, max, step);
                    if let Some(stages) = device.dpi_stages_mut() {
                        if let Some(stage) = stages.stages.first_mut() {
                            stage.y = value;
                        }
                    }
                });
            },
        ));
    }

    content = content
        .child(div().text_xs().text_color(cx.theme().muted_foreground).child("DPI 阶段"))
        .child(
            v_flex()
                .gap_1()
                .children(stages.into_iter().enumerate().map(|(index, stage)| {
                    let color = STAGE_COLORS[index % STAGE_COLORS.len()];
                    let value = if xy_supported && stage.x != stage.y {
                        format!("X: {}  Y: {}", stage.x, stage.y)
                    } else {
                        format!("{} DPI", stage.x)
                    };
                    h_flex()
                        .h(px(38.))
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .w(px(30.))
                                .h(px(30.))
                                .rounded_full()
                                .bg(rgb(color))
                                .text_color(cx.theme().primary_foreground)
                                .child(format!("{}", index + 1)),
                        )
                        .child(div().text_sm().child(value))
                        .into_any_element()
                })),
        )
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().foreground)
                .underline()
                .cursor_pointer()
                .child("配置灵敏度阶段"),
        );

    content.into_any_element()
}

fn polling_panel(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("轮询率数据不可用").into_any_element();
    };
    let selected = device.features.performance.polling_rate;
    let has_dongle = app.devices.iter().any(|device| device.product_id == 179);

    panel("轮询率", cx)
        .child(div().text_xs().text_color(cx.theme().muted_foreground).child(
            "轮询率按钮仅显示当前连接条件允许的档位。",
        ))
        .child(
            h_flex()
                .flex_wrap()
                .gap_2()
                .children(PollingRate::ALL.into_iter().map(|rate| {
                    let enabled = !rate.needs_hyperpolling() || has_dongle;
                    let active = rate == selected;
                    div()
                        .id(format!("polling-rate-{}", rate.hz()))
                        .flex()
                        .items_center()
                        .justify_center()
                        .h(px(27.))
                        .min_w(px(72.))
                        .when(!enabled, |this| this.opacity(0.3))
                        .rounded(px(3.))
                        .border_1()
                        .border_color(if active {
                            cx.theme().primary
                        } else {
                            cx.theme().border
                        })
                        .text_size(px(14.))
                        .child(rate.label().to_uppercase())
                        .when(enabled, |this| {
                            this.cursor_pointer().on_click(cx.listener(move |this, _, _, cx| {
                                this.set_polling_rate(rate, cx);
                            }))
                        })
                })),
        )
        .when(!has_dongle, |this| {
            this.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("2000 Hz 及以上需要已连接 HyperPolling 无线接收器。"),
            )
        })
        .into_any_element()
}

fn lift_off_panel(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("抬升距离数据不可用").into_any_element();
    };
    let Some(value) = device.features.performance.lift_off else {
        return EmptyState::new("该设备没有抬升距离设置").into_any_element();
    };

    panel("抬升距离", cx)
        .child(select_row(
            "lift-off-distance",
            "档位",
            value.zh().to_string(),
            &LiftOffDistance::LABELS,
            app.open_select.as_deref() == Some("lift-off-distance"),
            cx,
            |this, index, cx| {
                if let Some(value) = LiftOffDistance::ALL.get(index).copied() {
                    this.set_lift_off(value, cx);
                }
            },
        ))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("Smart Tracking、Asymmetric cut-off 等未被 182 当前页面状态证明的控件不在此渲染。"),
        )
        .into_any_element()
}

fn snap_dpi(value: f32, min: u32, max: u32, step: u32) -> u32 {
    let value = value.round().clamp(min as f32, max as f32) as u32;
    if step == 0 {
        value
    } else {
        (min + ((value.saturating_sub(min)) / step) * step).clamp(min, max)
    }
}
