//! 增强页（`TAB_ENHANCEMENT`）。
//!
//! 布局与所有设备页共用骨架（产品图区 + 600px 卡片两列换行）。
//!
//! 文案依据：
//!
//! | key | 中文 |
//! |---|---|
//! | `AUDIO_ENHANCEMENT_HEADER` | 音效增强 |
//! | `AUDIO_ENHANCEMENT_TIP` | 使用这些声音处理选项之一修改音频播放。 |
//! | `AUDIO_ENHANCEMENT_THX_TOOLTIP` | 享受 THX 认证的沉浸式影音体验。 |
//! | `AUDIO_ENHANCEMENT_DOLBY_TOOLTIP` | 通过杜比虚拟音箱启用虚拟声音。 |
//! | `AUDIO_DRIVEN_HAPTICS` | 由音频转化而成的触觉反馈 |
//!
//! 该页在雷云里同时覆盖**音频增强**与**音频转触觉**两块。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::AppShell;
use crate::pages::widgets::{card_title, select_row, 
    body_widgets, card, EmptyState, PageHeader, ProductBanner, SettingRow, stepper_row, toggle_button,
    widget_card,
};

/// 渲染增强页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    let sound = device.features.sound.as_ref();
    let haptics = device.features.haptics.as_ref();
    if sound.is_none() && haptics.is_none() {
        return v_flex()
            .size_full()
            .gap_4()
            .child(PageHeader::new(
                "增强",
                format!("{} · 该设备没有增强设置", device.display_name()),
            ))
            .child(EmptyState::new(
"音效增强属于音频设备设置；触觉需支持 Razer Sensa HD。",
            ))
            .into_any_element();
    }

    let enhancement = sound.map(|s| s.enhancement);
    let has_haptics = haptics.is_some();
    let a2h = haptics.map(|h| h.audio_to_haptics).unwrap_or(false);
    let gain = haptics.map(|h| h.gain).unwrap_or(0);
    let intensity = haptics.map(|h| h.intensity).unwrap_or(0);
    let device_name = device.display_name().to_string();

    let audio = widget_card(
        card()
            .child(card_title("音效增强"))
            .when_some(enhancement, |this, current| {
                this.child(select_row(
                    "audio-enhancement",
                    "模式",
                    current.zh().to_string(),
                    &crate::features::AudioEnhancement::LABELS,
                    app.open_select.as_deref() == Some("audio-enhancement"),
                    cx,
                    |this, index, cx| {
                        if let Some(picked) =
                            crate::features::AudioEnhancement::ALL.get(index).copied()
                        {
                            this.set_enhancement(picked, cx);
                        }
                    },
                ))
            })
            .when(enhancement.is_none(), |this| {
                this.child(div().text_sm().child("该设备没有音效增强选项"))
            })
            .child(SettingRow::new("THX", "沉浸式影音体验".to_string()))
            .child(SettingRow::new("杜比", "虚拟音箱".to_string()))
            .child(div().text_xs().child(
                "雷云原文：使用这些声音处理选项之一修改音频播放。",
            )),
    );

    let haptic_widget = widget_card(
        card()
            .child(card_title("音频转触觉"))
            .when(!has_haptics, |this| {
                this.child(div().text_sm().child(
                    "该设备没有触觉设置。Razer Sensa HD 可实时把音频转成触觉。",
                ))
            })
            .when(has_haptics, |this| {
                this.child(toggle_button(
                    "a2h-on",
                    "启用音频转触觉",
                    a2h,
                    cx,
                    |this, cx| this.toggle_audio_to_haptics(cx),
                ))
                .child(stepper_row(
                    "增益等级",
                    format!("{gain}%"),
                    cx,
                    |this, cx| this.adjust_haptics_gain(-1, cx),
                    |this, cx| this.adjust_haptics_gain(1, cx),
                ))
                .child(stepper_row(
                    "强度",
                    format!("{intensity}%"),
                    cx,
                    |this, cx| this.adjust_haptics_intensity(-1, cx),
                    |this, cx| this.adjust_haptics_intensity(1, cx),
                ))
            })
            .child(div().text_xs().child(
                "雷云原文：Razer Sensa HD 可实时自动完成音频到触觉的转换，适用于所有游戏、电影和音乐。",
            )),
    );

    v_flex()
        .size_full()
        .gap_2()
        .child(PageHeader::new(
            "增强",
            format!("{} · 更改会立即保存", device_name),
        ))
        .child(ProductBanner::new(device_name.clone()))
        .child(body_widgets().child(audio).child(haptic_widget))
        .into_any_element()
}