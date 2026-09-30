//! 混音器页（`TAB_MIXER`）。
//!
//! 布局与所有设备页共用骨架（顶部产品图区 + 600px 卡片两列换行），见
//! [`docs/screens/`](../../docs/screens/README.md)。
//!
//! 文案依据：
//!
//! | key | 中文 |
//! |---|---|
//! | `AUDIO_MIX` | Audio Mix |
//! | `AUDIO_MIRRORING_TOOLTIP` | 将前置扬声器中的立体声内容镜像到后置扬声器…… |
//! | `MIXER` | 混音器 |
//!
//! 混音器与声音页共用同一份 `Sound` 数据（两者在雷云里都是音频输出侧设置）。

use gpui_kit::component::*;
use gpui_kit::*;

use crate::app::AppShell;

use crate::pages::widgets::{card_title, 
    body_widgets, card, EmptyState, PageHeader, ProductBanner, SettingRow, slider_row,
    toggle_button, widget_card,
};

/// 渲染混音器页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    let Some(sound) = device.features.sound.as_ref() else {
        return v_flex()
            .size_full()
            .gap_4()
            .child(PageHeader::new(
                "混音器",
                format!("{} · 该设备没有混音设置", device.display_name()),
            ))
            .child(EmptyState::new(
"混音器属于音频输出侧设置，仅耳机/音频设备具备。",
            ))
            .into_any_element();
    };

    let chat_mix = sound.chat_mix;
    let volume = sound.volume;
    let mirroring = sound.audio_mirroring;
    let device_name = device.display_name().to_string();

    let mix = widget_card(
        card()
            .child(card_title("混音"))
            // 连续型数值 → 真实滑块 `.slider-container`（0–100，步进 1）。
            .child(slider_row(
                "mixer-volume",
                "主音量",
                volume as f32,
                0.,
                100.,
                1.,
                format!("{volume}%"),
                true,
                cx,
                |this, value, cx| this.set_volume(value, cx),
            ))
            .child(slider_row(
                "mixer-chat",
                "游戏 / 聊天",
                chat_mix as f32,
                0.,
                100.,
                1.,
                format!("{chat_mix}%"),
                true,
                cx,
                |this, value, cx| this.set_chat_mix(value, cx),
            ))
            .child(SettingRow::new("通道", "前置 / 后置".to_string())),
    );

    let mirror = widget_card(
        card()
            .child(card_title("镜像"))
            .child(toggle_button(
                "mixer-mirror",
                "立体声镜像到后置",
                mirroring,
                cx,
                |this, cx| this.toggle_audio_mirroring(cx),
            ))
            .child(div().text_xs().child(
                "雷云原文：将前置扬声器中的立体声内容镜像到后置扬声器，为你带来更响亮的音效；播放多声道音频内容时，请取消选中该选项。",
            )),
    );

    v_flex()
        .size_full()
        .gap_2()
        .child(PageHeader::new(
            "混音器",
            format!("{} · 更改会立即保存", device_name),
        ))
        .child(ProductBanner::new(device_name.clone()))
        .child(body_widgets().child(mix).child(mirror))
        .into_any_element()
}
