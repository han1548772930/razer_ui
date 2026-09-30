//! 灯光页：Chroma 区域与效果。
//!
//! ⚠️ 效果名清单是 **[推断]**：真正的效果名定义在远程前端里，
//! 本机访问不到 `apps.razer.com`（见 `docs/FEATURES.md` §5 未解项 1）。
//! 日志实测出现过 `static`（213 次）与 `Reactive`（60 次）。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::AppShell;
use crate::features::{LightingEffect, LightingZone};
use crate::pages::widgets::{
    ColorSwatch, EmptyState, PageHeader, PageLayout, card, card_title, not_wired_hint,
    select_row, slider_row,
};

/// 渲染灯光页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    if device.features.lighting.is_empty() {
        return v_flex()
            .size_full()
            .gap_4()
            .child(PageHeader::new(
                "灯光",
                format!("{} · 该设备未上报为 Chroma 设备", device.display_name()),
            ))
            .child(EmptyState::new(
"雷云实测本机两台设备的 isChromaDevice 均为 false，因此没有可配置的灯光区域。\
                 接入 Chroma 设备后，这里会出现区域划分与效果设置。",
            ))
            .into_any_element();
    }

    let rows: Vec<AnyElement> = device
        .features
        .lighting
        .iter()
        .enumerate()
        .map(|(index, zone)| zone_card(index, zone, app.open_select.as_deref(), cx))
        .collect();

    PageLayout::new("灯光", device.display_name())
        .widgets(rows)
        .widget(dim_on_battery_card(device, cx))
        .widget(not_wired_hint())
        .into_any_element()
}

/// 单个灯光区域。
///
/// 一个区域包含四件事：效果、主色、亮度、速度。**只有部分效果用到主色**
/// （`LightingEffect::uses_color`），所以主色那行是条件渲染的——
/// 这也是雷云的行为：选了「光谱循环」就不该再让你挑一个用不上的颜色。
fn zone_card(
    index: usize,
    zone: &LightingZone,
    open_select: Option<&str>,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let effect = zone.effect;
    let color = zone.color;
    let brightness = zone.brightness;
    let speed = zone.speed;
    let name = zone.name.clone();

    card()
        .child(card_title(name))
        // 十选一的枚举 → 真实下拉 `.s3-dropdown`。
        .child(select_row(
            format!("lighting-effect-{index}"),
            "效果",
            effect.zh().to_string(),
            &LightingEffect::LABELS,
            open_select == Some(format!("lighting-effect-{index}").as_str()),
            cx,
            move |this, picked, cx| {
                if let Some(effect) = LightingEffect::ALL.get(picked).copied() {
                    this.set_zone_effect(index, effect, cx);
                }
            },
        ))
        .when(effect.uses_color(), |this| {
            this.child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .gap_3()
                    .child(div().text_sm().child("主色"))
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(ColorSwatch::new(color))
                            .child(div().text_xs().child(format!(
                                "#{:02X}{:02X}{:02X}",
                                color[0], color[1], color[2]
                            ))),
                    ),
            )
        })
        // 亮度与速度是连续值 → 真实滑块 `.slider-container`。
        .child(slider_row(
            "lighting-brightness",
            "亮度",
            brightness as f32,
            0.,
            100.,
            1.,
            format!("{brightness}%"),
            true,
            cx,
            move |this, value, cx| this.set_zone_brightness(index, value, cx),
        ))
        .when(effect.uses_speed(), |this| {
            this.child(slider_row(
                "lighting-speed",
                "速度",
                speed as f32,
                0.,
                100.,
                1.,
                format!("{speed}%"),
                true,
                cx,
                move |this, value, cx| this.set_zone_speed(index, value, cx),
            ))
        })
        .into_any_element()
}

/// 「无活动后调暗」。
///
/// 这是**键盘背光**的行为，所以读的是 `KeyboardSettings::dim_on_battery_after_min`，
/// 而不是电源页那个「闲置降低亮度」（后者是整机亮度）。
/// `0` 表示已关闭。
fn dim_on_battery_card(
    device: &crate::model::Device,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let Some(keyboard) = device.features.keyboard.as_ref() else {
        return card()
            .child(card_title("无活动后调暗"))
            .child(div().text_sm().child("该设备没有键盘背光"))
            .into_any_element();
    };
    let minutes = keyboard.dim_on_battery_after_min;
    let text = if minutes == 0 {
        "已关闭".to_string()
    } else {
        format!("{minutes} 分钟")
    };

    card()
        .child(card_title("无活动后调暗"))
        .child(div().text_xs().child(
            "雷云原文：在指定时间没有活动后，自动降低键盘背光亮度以省电。",
        ))
        .child(
            h_flex()
                .w_full()
                .justify_between()
                .items_center()
                .gap_3()
                .mt_2()
                .child(div().text_sm().child("无活动后"))
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            crate::pages::widgets::btn("dim-battery-dec", "−")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.adjust_dim_on_battery(-1, cx);
                                })),
                        )
                        .child(div().text_sm().w(px(80.)).child(text))
                        .child(
                            crate::pages::widgets::btn("dim-battery-inc", "+")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.adjust_dim_on_battery(1, cx);
                                })),
                        ),
                ),
        )
        .into_any_element()
}


