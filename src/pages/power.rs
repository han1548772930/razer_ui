//! 电源页：电量、休眠、低功耗、降亮度、电池健康优化。
//!
//! 仅无线/带电池设备有这些设置。雷云实测本机只有
//! DeathAdder V3 Pro 的 `hasBattery` 为 true（电量 47%）。
//!
//! # 布局
//!
//! 真实布局：顶部 `.widget-prod` 产品图区（250px）
//! ＋ 下方 `.body-widgets` 里固定 600px 宽的 `.widget` 两列换行。
//! 见 [`docs/screens/05-power.md`](../../docs/screens/05-power.md)。
//!
//! # 新增项文案依据
//!
//! | key | 中文 |
//! |---|---|
//! | `BATTERY_HEALTH_OPTIMIZER` | 电池健康优化功能 |
//! | `BATTERY_HEALTH_OPTIMIZER_MOUSEMAT_DESCRIPTION` | 当达到设定的百分比时，鼠标垫将停止为设备充电。 |
//! | `BRIGHTNESS_WHEN_INACTIVE` | 启用时的亮度 |
//! | `LIGHTING_ON_BATTERY` | （电池供电时的灯光） |

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::AppShell;
use crate::pages::widgets::{card_title, 
    body_widgets, card, EmptyState, PageHeader, ProductBanner, SettingRow, stepper_row, toggle_button,
    widget_card,
};

/// 渲染电源页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };
    let device_name = device.display_name().to_string();

    let Some(power) = device.features.power.as_ref() else {
        return v_flex()
            .size_full()
            .gap_2()
            .child(PageHeader::new(
                "电源",
                format!("{device_name} · 该设备为有线供电"),
            ))
            .child(ProductBanner::new(device_name.clone()))
            .child(
                body_widgets().child(widget_card(
                    card().child(div().text_sm().child(
                        "该设备没有电源管理设置。雷云实测本机仅 DeathAdder V3 Pro 带电池。",
                    )),
                )),
            )
            .into_any_element();
    };

    let sleep = power.sleep_after_min;
    let dim = power.dim_after_min;
    let low_power = power.low_power_mode;
    let health = power.battery_health_optimizer;
    let threshold = power.battery_health_threshold;
    let brightness = power.brightness_when_active;
    let lighting_on_battery = power.lighting_on_battery;
    let battery = device.power_status.as_ref().map(|status| {
        (
            status.level,
            status.label_zh().to_string(),
            status.charging_status.clone(),
        )
    });
    let has_battery_status = device.power_status.is_some();

    v_flex()
        .size_full()
        .gap_2()
        .child(PageHeader::new(
            "电源",
            format!("{device_name} · 更改会立即保存"),
        ))
        .child(ProductBanner::new(device_name.clone()))
        .child(
            body_widgets()
                // 电池
                .child(widget_card(
                    card()
                        .child(card_title("电池"))
                        .when_some(battery, |this, (level, label, raw)| {
                            this.child(SettingRow::new("电量", format!("{level}%")))
                                .child(SettingRow::new("状态", label))
                                .child(SettingRow::new("原始状态码", raw))
                        })
                        .when(!has_battery_status, |this| {
                            this.child(div().text_sm().child("该设备未上报电池状态"))
                        }),
                ))
                // 省电
                .child(widget_card(
                    card()
                        .child(card_title("省电"))
                        .child(stepper_row(
                            "闲置休眠",
                            if sleep == 0 {
                                "从不".to_string()
                            } else {
                                format!("{sleep} 分钟")
                            },
                            cx,
                            |this, cx| this.adjust_sleep_after(-1, cx),
                            |this, cx| this.adjust_sleep_after(1, cx),
                        ))
                        .child(stepper_row(
                            "闲置降低亮度",
                            if dim == 0 {
                                "从不".to_string()
                            } else {
                                format!("{dim} 分钟")
                            },
                            cx,
                            |this, cx| this.adjust_dim_after(-1, cx),
                            |this, cx| this.adjust_dim_after(1, cx),
                        ))
                        .child(toggle_button(
                            "pw-lowpower",
                            "低功耗模式",
                            low_power,
                            cx,
                            |this, cx| this.toggle_low_power_mode(cx),
                        ))
                        .child(div().text_xs().child(
                            "低功耗模式会降低灯光与轮询率以延长续航——雷云日志中 LowPower 出现 278 次。",
                        )),
                ))
                // 电池健康优化
                .child(widget_card(
                    card()
                        .child(card_title("电池健康优化功能"))
                        .child(toggle_button(
                            "battery-health",
                            "启用电池健康优化",
                            health,
                            cx,
                            |this, cx| this.toggle_battery_health_optimizer(cx),
                        ))
                        .child(stepper_row(
                            "停止充电阈值",
                            format!("{threshold}%"),
                            cx,
                            |this, cx| this.adjust_battery_health_threshold(-1, cx),
                            |this, cx| this.adjust_battery_health_threshold(1, cx),
                        ))
                        .child(div().text_xs().child(
                            "雷云原文：当达到设定的百分比时，鼠标垫将停止为设备充电。",
                        ))
                        .child(div().text_xs().child(
                            "要启用此功能，必须配对兼容的 Razer 雷蛇鼠标并将其设置为 HyperSpeed Wireless 无线模式。",
                        )),
                ))
                // 灯光与电池
                .child(widget_card(
                    card()
                        .child(card_title("灯光与电池"))
                        .child(stepper_row(
                            "启用时的亮度",
                            format!("{brightness}%"),
                            cx,
                            |this, cx| this.adjust_brightness_when_active(-1, cx),
                            |this, cx| this.adjust_brightness_when_active(1, cx),
                        ))
                        .child(toggle_button(
                            "lighting-on-battery",
                            "电池供电时开灯",
                            lighting_on_battery,
                            cx,
                            |this, cx| this.toggle_lighting_on_battery(cx),
                        ))
                        .child(div().text_xs().child(
                            "关掉后，使用电池时会关闭灯光以延长续航。",
                        )),
                )),
        )
        .into_any_element()
}