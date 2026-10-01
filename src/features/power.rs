//! 电源页。
//!
//! 页面只渲染原版已经确认的省电设置。电量状态属于产品顶栏，不在这里复制
//! 电池卡片；电池健康度、统一电池卡、亮度和自动关机没有对应的页面证据，
//! 因此不在电源正文中渲染。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::domain::SLEEP_AFTER_STEPS;
use crate::shell::AppShell;
use crate::ui::widgets::{card, card_title, EmptyState, PageLayout, toggle_button};

const POWER_SAVING_VALUES: [u16; 3] = [15, 30, 45];

pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    let device_name = device.display_name().to_string();
    let Some(power) = device.features.power.as_ref() else {
        return PageLayout::new("电源", device_name)
            .widget(card().child(card_title("电源")).child(EmptyState::new("该设备没有电源设置。")))
            .into_any_element();
    };

    let current_sleep = power.sleep_after_min;
    let low_power_mode = power.low_power_mode;
    let active_index = SLEEP_AFTER_STEPS
        .iter()
        .position(|value| *value == current_sleep);

    let power_saving_buttons = h_flex()
        .gap_2()
        .children(POWER_SAVING_VALUES.into_iter().map(|value| {
            let active = current_sleep == value;
            let delta = match (active_index, SLEEP_AFTER_STEPS.iter().position(|item| *item == value)) {
                (Some(current), Some(target)) => target as i32 - current as i32,
                _ => 0,
            };
            div()
                .id(format!("power-saving-{value}"))
                .min_w(px(90.))
                .px(px(30.))
                .py(px(7.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .border_1()
                .border_color(if active { cx.theme().primary } else { cx.theme().border })
                .bg(if active { cx.theme().secondary } else { cx.theme().group_box })
                .text_color(cx.theme().foreground)
                .text_xs()
                .cursor_pointer()
                .hover(|this| this.border_color(cx.theme().primary))
                .child(format!("{value} 分钟"))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if delta != 0 {
                        this.adjust_sleep_after(delta, cx);
                    }
                }))
        }));

    PageLayout::new("电源", device_name)
        .widget(
            card()
                .child(card_title("省电"))
                .child(div().text_sm().child("设备无操作后进入省电状态"))
                .child(power_saving_buttons)
                .child(div().mt_2().text_xs().text_color(cx.theme().muted_foreground).child(
                    "该设置仅在无线且未充电时生效。",
                )),
        )
        .widget(
            card()
                .child(card_title("低功耗模式"))
                .child(toggle_button(
                    "power-low-mode",
                    "启用低功耗模式",
                    low_power_mode,
                    cx,
                    |this, cx| this.toggle_low_power_mode(cx),
                ))
                .child(div().mt_2().text_xs().text_color(cx.theme().muted_foreground).child(
                    "低功耗模式的可用状态由设备和当前无线连接条件决定。",
                )),
        )
        .into_any_element()
}
