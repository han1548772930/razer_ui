//! 653 Customize 内嵌的 MapMacro 组件。
//!
//! 原版不是独立的宏列表、录制卡片或步骤编辑页。MapMacro 只在 keyboard-svg
//! 选中支持宏的按键映射时出现，包含宏选择、执行选项以及条件显示的重复次数。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::shell::AppShell;
use crate::ui::widgets::{card, card_title, EmptyState, PageLayout, SettingRow};

pub fn render(app: &AppShell, _cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    if device.product_id != 653 {
        return EmptyState::new("宏映射模块仅适用于 productId 653").into_any_element();
    }

    PageLayout::new("宏", device.display_name())
        .without_product_banner()
        .subtitle("MapMacro 仅作为键盘按键映射的一部分显示")
        .widget(map_macro_card(&device.features.macros))
        .into_any_element()
}

fn map_macro_card(macros: &[crate::domain::Macro]) -> AnyElement {
    let macro_available = !macros.is_empty();
    let selected_macro = macros
        .first()
        .map(|item| item.name.as_str())
        .unwrap_or("无可用宏");

    card()
        .id("map-macro")
        .child(card_title("MapMacro"))
        .child(div().text_xs().child(
            "此组件由 Customize 的 activeButton 驱动；未选中支持宏的按键时，原版不会显示独立宏页面。",
        ))
        .child(static_dropdown("宏", selected_macro, macro_available))
        .child(static_dropdown(
            "执行选项",
            "由当前按键映射决定",
            macro_available,
        ))
        .when(macro_available, |this| {
            this.child(SettingRow::new(
                "重复次数",
                "仅选择“多次执行”时显示，范围 1–99，步长 1",
            ))
        })
        .when(!macro_available, |this| {
            this.child(
                div()
                    .text_sm()
                    .text_color(rgb(0x707070))
                    .child("宏不可用；原版会降低该区域透明度并禁用执行选项。"),
            )
        })
        .child(div().text_xs().child(
            "宏录制、宏删除、宏步骤列表和独立保存成功提示不属于 653 MapMacro 组件。",
        ))
        .into_any_element()
}

fn static_dropdown(label: &'static str, value: &str, enabled: bool) -> AnyElement {
    h_flex()
        .w_full()
        .justify_between()
        .items_center()
        .child(div().text_sm().child(label))
        .child(
            div()
                .w(px(220.))
                .h(px(32.))
                .px(px(10.))
                .border_1()
                .border_color(if enabled { rgb(0x5D5D5D) } else { rgb(0x333333) })
                .text_color(if enabled { rgb(0xCCCCCC) } else { rgb(0x707070) })
                .text_sm()
                .child(value.to_string()),
        )
        .into_any_element()
}
