//! 均衡器页（`TAB_EQ`）。
//!
//! 布局与所有设备页共用骨架（产品图区 + 600px 卡片两列换行）。
//!
//! 文案依据：
//!
//! | key | 中文 |
//! |---|---|
//! | `AUDIO_EQUALIZER` | 音频均衡器 |
//! | `MIC_EQUALIZER` | 麦克风均衡器 |
//! | `AUDIO_EQ_TOOLTIP` | 所有电竞均衡器调整都会保存在耳机上…… |
//! | `MIC_EQUALIZER_TOOLTIP` | 借助任何可用的预设或根据需要单独调整每个设置…… |
//!
//! 雷云把均衡器分两套：**音频均衡器**（输出）与**麦克风均衡器**（输入）。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::AppShell;
use crate::pages::widgets::{card_title, body_widgets, card, EmptyState, PageHeader, ProductBanner, SettingRow, toggle_button, widget_card, btn};

/// 渲染均衡器页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    let sound = device.features.sound.as_ref();
    let mic = device.features.mic.as_ref();
    if sound.is_none() && mic.is_none() {
        return v_flex()
            .size_full()
            .gap_4()
            .child(PageHeader::new(
                "均衡器",
                format!("{} · 该设备没有均衡器", device.display_name()),
            ))
            .child(EmptyState::new("均衡器属于音频设备设置，仅耳机/音频设备具备。"))
            .into_any_element();
    }

    let eq_enabled = sound.map(|s| s.equalizer.enabled).unwrap_or(false);
    let eq_esports = sound.map(|s| s.equalizer.esports).unwrap_or(false);
    let eq_preset = sound
        .map(|s| s.equalizer.preset.clone())
        .unwrap_or_else(|| "默认".to_string());
    let bands = sound.map(|s| s.equalizer.bands.clone()).unwrap_or_default();
    let has_sound = sound.is_some();
    let device_name = device.display_name().to_string();

    let output = widget_card(
        card()
            .child(card_title("音频均衡器"))
            .when(!has_sound, |this| {
                this.child(div().text_sm().child("该设备没有音频输出均衡器"))
            })
            .when(has_sound, |this| {
                this.child(
                    h_flex()
                        .gap_4()
                        .flex_wrap()
                        .child(toggle_button(
                            "eq-page-on",
                            "启用",
                            eq_enabled,
                            cx,
                            |this, cx| this.toggle_eq(cx),
                        ))
                        .child(toggle_button(
                            "eq-page-esports",
                            "电竞均衡器",
                            eq_esports,
                            cx,
                            |this, cx| this.toggle_eq_esports(cx),
                        )),
                )
                .child(SettingRow::new("预设", eq_preset))
            })
            .child(div().text_xs().child(
                "雷云原文：所有电竞均衡器调整都会保存在耳机上；标准均衡器只保存自定义配置文件。",
            )),
    );

    // 频段单独一个 widget（10 段，数值表）
    let band_widget = widget_card(
        card()
            .child(card_title(format!("频段（{} 段，单位 dB）", bands.len())))
            .children(bands.iter().enumerate().map(|(index, gain)| {
                SettingRow::new(&format!("频段 {}", index + 1), format!("{gain:+}"))
            })),
    );

    let mic_eq = widget_card(
        card()
            .child(card_title("麦克风均衡器"))
            .when(mic.is_none(), |this| {
                this.child(div().text_sm().child("该设备没有麦克风均衡器"))
            })
            .when(mic.is_some(), |this| {
                this.child(div().text_xs().child(
                    "雷云原文：借助任何可用的预设或根据需要单独调整每个设置，从而对麦克风的音效进行自定义。",
                ))
                .child(
                    h_flex().gap_2().child(
                        btn("eq-reset", "重置为平直")
                            .on_click(|_, _, _| {}),
                    ),
                )
            }),
    );

    v_flex()
        .size_full()
        .gap_2()
        .child(PageHeader::new(
            "均衡器",
            format!("{} · 更改会立即保存", device_name),
        ))
        .child(ProductBanner::new(device_name.clone()))
        .child(body_widgets().child(output).child(band_widget).child(mic_eq))
        .into_any_element()
}