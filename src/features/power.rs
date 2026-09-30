//! 电源页：电量/充电摘要、睡眠与闲置超时、省电、亮度和充电保护。
//!
//! 页面严格使用设备页共用骨架：产品图区 250px，随后是固定 600px widget
//! 两列换行。所有电源区块都由 `features.power` 与 `power_status` 能力过滤；
//! 没有电池状态时不显示伪造的 0%，没有硬件写回通道时明确显示 awaiting-device。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::shell::AppShell;
use crate::ui::widgets::{
    card, card_title, EmptyState, PageLayout, SettingRow, slider_row, stepper_row, toggle_button,
    widget_card,
};

/// 渲染电源页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    let device_name = device.display_name().to_string();
    let Some(power) = device.features.power.as_ref() else {
        return PageLayout::new("电源", device_name.clone())
            .subtitle(format!("{device_name} · 该设备未声明电源管理能力"))
            .widget(
                card()
                    .child(card_title("电源能力"))
                    .child(EmptyState::new("该设备没有电源管理设置。"))
                    .child(div().text_xs().child(
                        "设备能力清单未声明电池、睡眠或省电控制，因此本页不会显示不可写的空控件。",
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
    let battery = device.power_status.as_ref();

    let battery_card = card()
        .child(card_title("电量与充电"))
        .when_some(battery, |this, status| {
            let level_color = if status.is_low() {
                cx.theme().danger
            } else {
                cx.theme().foreground
            };
            let charging = if status.is_charging() {
                "充电中"
            } else if status.level >= 100 {
                "已充满"
            } else {
                "使用电池"
            };
            this.child(
                h_flex()
                    .w_full()
                    .h(px(46.))
                    .items_center()
                    .justify_between()
                    .child(div().text_sm().child("电量"))
                    .child(
                        h_flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(level_color)
                                    .child(format!("{}%", status.level)),
                            )
                            .child(div().text_sm().child(charging)),
                    ),
            )
            .child(SettingRow::new("连接状态", status.charging_status.clone()))
            .when(status.is_low(), |this| {
                this.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child("电量较低，请连接充电线或充电底座。"),
                )
            })
        })
        .when(battery.is_none(), |this| {
            this.child(EmptyState::new("未收到电池状态"))
                .child(div().text_xs().child(
                    "设备支持电源设置，但当前服务没有上报电量或充电状态；不会显示假的百分比。",
                ))
        });

    PageLayout::new("电源", device_name.clone())
        .subtitle(format!("{device_name} · 更改会立即保存"))
        .widget(widget_card(battery_card))
        .widget(
            card()
                .child(card_title("睡眠与自动省电"))
                .child(stepper_row(
                    "闲置睡眠",
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
                    "power-low-mode",
                    "低功耗模式",
                    low_power,
                    cx,
                    |this, cx| this.toggle_low_power_mode(cx),
                ))
                .child(div().text_xs().child(
                    "低功耗模式会降低灯光与无线活动以延长续航；修改后状态会先保存到本地配置。",
                )),
        )
        .widget(
            card()
                .child(card_title("充电保护"))
                .child(toggle_button(
                    "battery-health",
                    "启用电池健康优化",
                    health,
                    cx,
                    |this, cx| this.toggle_battery_health_optimizer(cx),
                ))
                .child(slider_row(
                    "battery-health-threshold",
                    "停止充电阈值",
                    threshold as f32,
                    50.,
                    100.,
                    5.,
                    format!("{threshold}%"),
                    health,
                    cx,
                    |this, value, cx| this.set_battery_health_threshold(value, cx),
                ))
                .child(div().text_xs().child(
                    "达到设定百分比后停止继续充电；未启用或设备未确认时不会显示为硬件已应用。",
                )),
        )
        .widget(
            card()
                .child(card_title("电池供电时的灯光"))
                .child(slider_row(
                    "power-brightness",
                    "启用时的亮度",
                    brightness as f32,
                    0.,
                    100.,
                    5.,
                    format!("{brightness}%"),
                    true,
                    cx,
                    |this, value, cx| this.set_brightness_when_active(value, cx),
                ))
                .child(toggle_button(
                    "lighting-on-battery",
                    "电池供电时开灯",
                    lighting_on_battery,
                    cx,
                    |this, cx| this.toggle_lighting_on_battery(cx),
                ))
                .child(div().text_xs().child(
                    "关闭后，设备使用电池时会关闭灯光以延长续航。亮度与灯光写入等待设备服务确认。",
                )),
        )
        .widget(
            card()
                .child(card_title("自动关机与写入状态"))
                .child(SettingRow::new("自动关机", "unsupported".to_string()))
                .child(div().text_xs().child(
                    "当前设备能力模型没有声明独立的自动关机控制；不会把睡眠设置冒充为自动关机。",
                ))
                .child(div().text_xs().mt_1().child(
                    "本地配置修改后等待硬件确认；确认前不得显示“设备已应用”。",
                )),
        )
        .into_any_element()
}
