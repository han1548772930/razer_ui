//! 灯光页：全局亮度、区域效果、颜色/速度参数、设备预览和 Chroma 联动状态。
//!
//! 效果、区域和参数严格来自设备的 `features.lighting` 能力；没有 Chroma
//! 能力的设备使用 unavailable 状态。硬件写回尚未接通时，页面显示
//! awaiting-device，而不是把本地配置保存冒充成设备应用成功。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::features::{LightingEffect, LightingZone};
use crate::shell::AppShell;
use crate::ui::widgets::{
    btn, card, card_title, ColorSwatch, EmptyState, PageLayout, slider_row, toggle_button,
};

/// 渲染灯光页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    let device_name = device.display_name().to_string();
    if device.features.lighting.is_empty() {
        return PageLayout::new("灯光", device_name.clone())
            .subtitle(format!("{device_name} · 该设备未声明 Chroma 能力"))
            .widget(
                card()
                    .child(card_title("Chroma 灯光能力"))
                    .child(EmptyState::new("该设备没有可配置的灯光区域。"))
                    .child(div().text_xs().child(
                        "区域、效果和参数由设备 manifest 过滤；键盘专属区域不会出现在不支持的设备上。",
                    )),
            )
            .into_any_element();
    }

    let zones = device.features.lighting.clone();
    let zone_cards = zones
        .iter()
        .enumerate()
        .map(|(index, zone)| zone_card(index, zone, cx))
        .collect::<Vec<_>>();

    let global_enabled = app.global_brightness.enabled;
    let global_level = app.global_brightness.level;

    PageLayout::new("灯光", device_name.clone())
        .subtitle(format!("{device_name} · 更改会立即保存"))
        .widget(
            card()
                .child(card_title("全局亮度"))
                .child(toggle_button(
                    "global-lighting",
                    "启用全局亮度",
                    global_enabled,
                    cx,
                    |this, cx| this.toggle_global_brightness(cx),
                ))
                .child(slider_row(
                    "global-brightness",
                    "亮度",
                    global_level as f32,
                    0.,
                    100.,
                    5.,
                    format!("{global_level}%"),
                    global_enabled,
                    cx,
                    |this, value, cx| this.set_global_brightness(value, cx),
                ))
                .child(div().text_xs().child(
                    "全局亮度会一次性影响所有支持 LED 的设备；设备应用前状态为 awaiting-device。",
                )),
        )
        .widgets(zone_cards)
        .widget(preview_card(&zones))
        .widget(
            card()
                .child(card_title("Chroma Connect"))
                .child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .child(div().text_sm().child("应用联动"))
                        .child(div().text_sm().text_color(cx.theme().warning).child("awaiting-device")),
                )
                .child(div().text_xs().child(
                    "第三方应用同步、Chroma Studio 和 Visualizer 需要 Chroma 服务注册与事件确认；当前只显示能力状态，不伪造已连接。",
                )),
        )
        .widget(
            card()
                .child(card_title("灯光写回状态"))
                .child(div().text_sm().text_color(cx.theme().warning).child("awaiting-device"))
                .child(div().text_xs().mt_1().child(
                    "当前修改可写入本地配置，但硬件 discover → configure → ack 通道尚未接通；确认前不会显示“已应用”。",
                )),
        )
        .into_any_element()
}

/// 单个灯光区域：标题、效果网格、颜色、亮度和速度。
fn zone_card(index: usize, zone: &LightingZone, cx: &mut Context<AppShell>) -> AnyElement {
    card()
        .child(card_title(zone.name.clone()))
        .child(effect_grid(index, zone, cx))
        .when(zone.effect.uses_color(), |this| {
            this.child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .gap_3()
                    .child(div().text_sm().child("颜色"))
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(ColorSwatch::new(zone.color))
                            .child(div().text_xs().child(format!(
                                "#{:02X}{:02X}{:02X}",
                                zone.color[0], zone.color[1], zone.color[2]
                            )))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().warning)
                                    .child("颜色写回 awaiting-device"),
                            ),
                    ),
            )
        })
        .child(slider_row(
            "lighting-brightness",
            "亮度",
            zone.brightness as f32,
            0.,
            100.,
            5.,
            format!("{}%", zone.brightness),
            true,
            cx,
            move |this, value, cx| this.set_zone_brightness(index, value, cx),
        ))
        .when(zone.effect.uses_speed(), |this| {
            this.child(slider_row(
                "lighting-speed",
                "速度",
                zone.speed as f32,
                0.,
                100.,
                5.,
                format!("{}%", zone.speed),
                true,
                cx,
                move |this, value, cx| this.set_zone_speed(index, value, cx),
            ))
        })
        .into_any_element()
}

/// 雷云的效果选择使用网格而不是只有一个文字下拉。
fn effect_grid(index: usize, zone: &LightingZone, cx: &mut Context<AppShell>) -> AnyElement {
    h_flex()
        .w_full()
        .flex_wrap()
        .gap_2()
        .children(
            LightingEffect::ALL
                .iter()
                .copied()
                .enumerate()
                .map(|(effect_index, effect)| {
                    let label = if effect == zone.effect {
                        format!("✓ {}", LightingEffect::LABELS[effect_index])
                    } else {
                        LightingEffect::LABELS[effect_index].to_string()
                    };
                    btn(
                        format!("lighting-zone-{index}-effect-{effect_index}"),
                        label,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_zone_effect(index, effect, cx);
                    }))
                    .into_any_element()
                }),
        )
        .into_any_element()
}

/// 当前效果的设备预览；不是静态占位图，而是反映当前区域参数。
fn preview_card(zones: &[LightingZone]) -> AnyElement {
    let Some(zone) = zones.first() else {
        return card()
            .child(card_title("预览"))
            .child(EmptyState::new("没有可预览的灯光区域"))
            .into_any_element();
    };

    card()
        .child(card_title("灯光预览"))
        .child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .size(px(72.))
                        .rounded(px(5.))
                        .bg(rgb(
                            ((zone.color[0] as u32) << 16)
                                | ((zone.color[1] as u32) << 8)
                                | zone.color[2] as u32,
                        ))
                        .child(div().size_full()),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .gap_1()
                        .child(div().text_sm().child(zone.name.clone()))
                        .child(div().text_xs().child(format!(
                            "{} · 亮度 {}% · 速度 {}%",
                            zone.effect.zh(), zone.brightness, zone.speed
                        )))
                        .child(div().text_xs().child(format!(
                            "颜色 #{:02X}{:02X}{:02X}",
                            zone.color[0], zone.color[1], zone.color[2]
                        ))),
                ),
        )
        .child(div().text_xs().mt_2().child(
            "预览来自当前区域配置；硬件确认前只代表本地草稿。",
        ))
        .into_any_element()
}
