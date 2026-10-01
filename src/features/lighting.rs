//! 653/777 灯光页。
//!
//! 这两个设备模块都使用 brightness / quick-effect 组件，但 render 条件不同：
//! 653 是键盘的普通/硬件效果分支，777 是耳机的 Chroma 资源分支。这里不把
//! 通用 `LightingZone` 列表当作原版布局，也不把未被 manifest 声明的控件补上。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::features::{LightingEffect, LightingZone};
use crate::shell::AppShell;
use crate::ui::widgets::{
    btn, card, card_title, slider_row, toggle_button, ColorSwatch, EmptyState, PageLayout,
};

const BLACKWIDOW_V4_PRO: u32 = 653;
const KRAKEN_BT_SANRIO: u32 = 777;

#[derive(Clone, Copy)]
struct EffectOption {
    label: &'static str,
    effect: LightingEffect,
}

const BLACKWIDOW_SOFTWARE_EFFECTS: [EffectOption; 12] = [
    EffectOption { label: "Ambient", effect: LightingEffect::Off },
    EffectOption { label: "Audio Meter", effect: LightingEffect::AudioMeter },
    EffectOption { label: "Breathing", effect: LightingEffect::Breathing },
    EffectOption { label: "Fire", effect: LightingEffect::Breathing },
    EffectOption { label: "Reactive", effect: LightingEffect::Reactive },
    EffectOption { label: "Ripple", effect: LightingEffect::Ripple },
    EffectOption { label: "Spectrum", effect: LightingEffect::SpectrumCycling },
    EffectOption { label: "Starlight", effect: LightingEffect::Starlight },
    EffectOption { label: "Static", effect: LightingEffect::Static },
    EffectOption { label: "Tidal", effect: LightingEffect::Wave },
    EffectOption { label: "Wave", effect: LightingEffect::Wave },
    EffectOption { label: "Wheel", effect: LightingEffect::Wave },
];

const BLACKWIDOW_HARDWARE_EFFECTS: [EffectOption; 7] = [
    EffectOption { label: "Breathing", effect: LightingEffect::Breathing },
    EffectOption { label: "Reactive", effect: LightingEffect::Reactive },
    EffectOption { label: "Spectrum", effect: LightingEffect::SpectrumCycling },
    EffectOption { label: "Starlight", effect: LightingEffect::Starlight },
    EffectOption { label: "Static", effect: LightingEffect::Static },
    EffectOption { label: "Tidal", effect: LightingEffect::Wave },
    EffectOption { label: "Wave", effect: LightingEffect::Wave },
];

const KRAKEN_EFFECTS: [EffectOption; 10] = [
    EffectOption { label: "关闭", effect: LightingEffect::Off },
    EffectOption { label: "静态", effect: LightingEffect::Static },
    EffectOption { label: "光谱循环", effect: LightingEffect::SpectrumCycling },
    EffectOption { label: "波浪", effect: LightingEffect::Wave },
    EffectOption { label: "呼吸", effect: LightingEffect::Breathing },
    EffectOption { label: "响应", effect: LightingEffect::Reactive },
    EffectOption { label: "星光", effect: LightingEffect::Starlight },
    EffectOption { label: "音频计", effect: LightingEffect::AudioMeter },
    EffectOption { label: "涟漪", effect: LightingEffect::Ripple },
    EffectOption { label: "萤火", effect: LightingEffect::Firefly },
];

pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    if !matches!(device.product_id, BLACKWIDOW_V4_PRO | KRAKEN_BT_SANRIO) {
        return EmptyState::new("当前设备没有 653/777 灯光模块").into_any_element();
    }

    let device_name = device.display_name();
    if device.features.lighting.is_empty() {
        return PageLayout::new("灯光", device_name)
            .without_product_banner()
            .widget(EmptyState::new("该设备的 manifest 没有声明灯光能力"))
            .into_any_element();
    }

    let resource_ready = device.is_chroma_device;
    let hardware_effect = device.product_id == BLACKWIDOW_V4_PRO && device.use_ble;
    let zones = device.features.lighting.clone();

    let mut page = PageLayout::new("灯光", device_name)
        .without_product_banner()
        .subtitle("亮度和效果来自当前设备模块；效果参数随所选效果变化")
        .widget(brightness_widget(app, cx));

    if device.product_id == KRAKEN_BT_SANRIO && !resource_ready {
        page = page.widget(resource_status_widget());
    } else {
        page = page.widget(modes_widget(
            device.product_id,
            hardware_effect,
            resource_ready,
            &zones,
            app.open_select.as_deref(),
            cx,
        ));
    }

    page.into_any_element()
}

fn brightness_widget(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let enabled = app.global_brightness.enabled;
    let level = app.global_brightness.level;

    card()
        .child(card_title("亮度"))
        .child(toggle_button(
            "lighting-brightness-switch",
            "启用亮度",
            enabled,
            cx,
            |this, cx| this.toggle_global_brightness(cx),
        ))
        .child(slider_row(
            "lighting-brightness",
            "亮度",
            level as f32,
            0.,
            100.,
            1.,
            format!("{level}%"),
            enabled,
            cx,
            |this, value, cx| this.set_global_brightness(value, cx),
        ))
        .into_any_element()
}

fn resource_status_widget() -> AnyElement {
    card()
        .child(card_title("Chroma 资源"))
        .child(div().text_sm().child("正在检查 Chroma 资源安装状态"))
        .child(div().text_xs().mt_1().child(
            "资源未确认完整前不渲染效果设置，避免显示不可用的 profile 或 Chroma Studio 控件。",
        ))
        .child(btn("lighting-install-chroma", "安装 Chroma 资源").disabled(true))
        .into_any_element()
}

fn modes_widget(
    product_id: u32,
    hardware_effect: bool,
    resource_ready: bool,
    zones: &[LightingZone],
    open_select: Option<&str>,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let Some(zone) = zones.first() else {
        return EmptyState::new("设备没有可配置的灯光效果").into_any_element();
    };

    let options = if product_id == BLACKWIDOW_V4_PRO {
        if hardware_effect {
            BLACKWIDOW_HARDWARE_EFFECTS.to_vec()
        } else {
            BLACKWIDOW_SOFTWARE_EFFECTS.to_vec()
        }
    } else {
        KRAKEN_EFFECTS.to_vec()
    };

    let mode_id = "lighting-effect";
    let selected = options
        .iter()
        .position(|option| option.effect == zone.effect)
        .unwrap_or(0);

    card()
        .child(card_title("快速效果"))
        .child(div().text_xs().child(if hardware_effect {
            "硬件效果：只显示设备 OBM 支持的效果。"
        } else if product_id == BLACKWIDOW_V4_PRO {
            "软件效果：效果参数由当前 quick effect 决定。"
        } else {
            "Chroma 效果：资源安装完成后显示效果设置。"
        }))
        .when(!resource_ready, |this| {
            this.child(div().text_xs().text_color(cx.theme().warning).child(
                "Chroma 资源状态未确认，效果控件可能不可用。",
            ))
        })
        .child(
            h_flex()
                .w_full()
                .items_center()
                .gap_3()
                .child(div().text_sm().child("效果"))
                .child(
                    h_flex()
                        .flex_1()
                        .flex_wrap()
                        .gap_2()
                        .children(options.iter().enumerate().map(|(index, option)| {
                            let label = if index == selected {
                                format!("✓ {}", option.label)
                            } else {
                                option.label.to_string()
                            };
                            let effect = option.effect;
                            btn(format!("lighting-effect-{index}"), label)
                                .disabled(!resource_ready)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.set_zone_effect(0, effect, cx)
                                }))
                                .into_any_element()
                        })),
                ),
        )
        .child(effect_parameters(zone, mode_id, open_select, cx))
        .into_any_element()
}

fn effect_parameters(
    zone: &LightingZone,
    _mode_id: &str,
    _open_select: Option<&str>,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    v_flex()
        .w_full()
        .gap_3()
        .when(zone.effect.uses_color(), |this| {
            this.child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(div().text_sm().child("颜色"))
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(ColorSwatch::new(zone.color))
                            .child(div().text_xs().child(format!(
                                "#{:02X}{:02X}{:02X}",
                                zone.color[0], zone.color[1], zone.color[2]
                            ))),
                    ),
            )
        })
        .when(zone.effect.uses_speed(), |this| {
            this.child(slider_row(
                "lighting-effect-speed",
                "速度",
                zone.speed as f32,
                0.,
                100.,
                1.,
                format!("{}%", zone.speed),
                true,
                cx,
                |this, value, cx| this.set_zone_speed(0, value, cx),
            ))
        })
        .into_any_element()
}
