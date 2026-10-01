//! 777 声音页：音频 EQ 编辑器。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::shell::AppShell;
use crate::ui::widgets::{card_title, slider_row, EmptyState, PageLayout};

const EQ_WIDTH: f32 = 940.0;
const EQ_MIN_HEIGHT: f32 = 473.0;
const EQ_MIN: f32 = -5.0;
const EQ_MAX: f32 = 5.0;
const EQ_STEP: f32 = 1.0;

const AUDIO_PRESETS: [&str; 6] = [
    "默认",
    "增强",
    "人声",
    "低音增强",
    "清晰度增强",
    "自定义",
];

pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    if device.product_id != 777 {
        return PageLayout::new("声音", device.display_name())
            .widget(EmptyState::new("声音 EQ 页面仅适用于 productId 777。"))
            .into_any_element();
    }

    let Some(sound) = device.features.sound.as_ref() else {
        return PageLayout::new("声音", device.display_name())
            .widget(EmptyState::new("该设备没有声音 EQ 设置。"))
            .into_any_element();
    };

    let bands = sound.equalizer.bands.clone();
    let active_preset = sound.equalizer.preset.clone();

    PageLayout::new("声音", device.display_name())
        .subtitle("Audio EQ · narrow · -5dB 至 +5dB · 步长 1")
        .widget(audio_eq_panel(&bands, &active_preset, cx))
        .into_any_element()
}

fn audio_eq_panel(
    bands: &[i8],
    active_preset: &str,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let theme = cx.theme().clone();

    div()
        .id("sound-eqBox")
        .w(px(EQ_WIDTH))
        .min_w(px(EQ_WIDTH))
        .min_h(px(EQ_MIN_HEIGHT))
        .my(px(10.0))
        .p(px(30.0))
        .rounded(px(5.0))
        .bg(theme.group_box)
        .text_size(px(14.0))
        .child(card_title("音频 EQ"))
        .child(
            h_flex()
                .mt(px(18.0))
                .gap(px(6.0))
                .children(AUDIO_PRESETS.iter().enumerate().map(|(index, preset)| {
                    let active = active_preset == *preset
                        || (active_preset.is_empty() && index == 0);
                    div()
                        .id(SharedString::from(format!("sound-eq-preset-{index}")))
                        .px(px(10.0))
                        .py(px(6.0))
                        .rounded(px(3.0))
                        .text_size(px(12.0))
                        .text_color(if active {
                            theme.foreground
                        } else {
                            theme.muted_foreground
                        })
                        .when(active, |this| {
                            this.bg(theme.primary).text_color(rgb(0x000000))
                        })
                        .child(*preset)
                })),
        )
        .child(
            v_flex()
                .mt(px(28.0))
                .gap(px(18.0))
                .children(bands.iter().enumerate().map(|(index, gain)| {
                    let value = (*gain as f32).clamp(EQ_MIN, EQ_MAX);
                    slider_row(
                        Box::leak(format!("sound-eq-band-{index}").into_boxed_str()),
                        format!("频段 {}", index + 1).as_str(),
                        value,
                        EQ_MIN,
                        EQ_MAX,
                        EQ_STEP,
                        format!("{:+} dB", value as i8),
                        true,
                        cx,
                        move |this, value, cx| this.set_eq_band(index, value, cx),
                    )
                })),
        )
        .into_any_element()
}
