//! 触觉页（`TAB_HAPTICS`）。
//!
//! # 布局
//!
//! 与所有设备页共用骨架：顶部 `.widget-prod` 产品图区（250px）
//! ＋ 下方 `.body-widgets` 里固定 600px 宽的 `.widget` 两列换行。
//! 见 [`docs/RAZER-SYNAPSE-UI-SPEC.md`](../../docs/RAZER-SYNAPSE-UI-SPEC.md) §12.11。
//!
//! # 文案依据
//!
//! | key | 中文 |
//! |---|---|
//! | `TAB_HAPTICS` | 触觉 |
//! | `AUDIO_TO_HAPTICS_TITLE` | （音频转触觉） |
//! | `AUDIO_TO_HAPTICS_GAIN_LEVEL` | 增益等级 |
//! | `AUDIO_DRIVEN_HAPTICS` | 由音频转化而成的触觉反馈 |
//! | `MULTI_DEVICE_HARMONIZED_HAPTICS` | 设备间的触觉效果协调一致 |
//!
//! # ⚠️ 设备归属未实测
//!
//! `TAB_HAPTICS` 确实存在于语言包与设备模块的标签页词汇表里，
//! 但已下载的三份设备模块（鼠标 182 / 键盘 653 / 耳机 777）**都没有声明该页**。
//! 它应属于支持 Razer Sensa HD 的设备。
//!
//! 因此本页**不填任何默认值**；没有数据的设备显示明确的空状态并说明原因，
//! 而不是编一套参数出来。

use gpui_kit::component::*;
use gpui_kit::*;

use crate::shell::AppShell;
use crate::ui::widgets::{card_title, 
    body_widgets, card, EmptyState, PageHeader, ProductBanner, SettingRow, stepper_row, toggle_button,
    widget_card,
};

/// 渲染触觉页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };
    let device_name = device.display_name().to_string();

    let Some(haptics) = device.features.haptics.as_ref() else {
        return v_flex()
            .size_full()
            .gap_2()
            .child(PageHeader::new(
                "触觉",
                format!("{device_name} · 该设备没有触觉设置"),
            ))
            .child(ProductBanner::new(device_name.clone()))
            .child(
                body_widgets()
                    .child(widget_card(
                        card()
                            .child(card_title("为什么这里没有设置"))
                            .child(div().text_sm().child(
                                "TAB_HAPTICS 在语言包与标签页词汇表里都存在，但已下载的三份设备模块（鼠标 182 / 键盘 653 / 耳机 777）都没有声明该页。",
                            ))
                            .child(div().text_sm().child(
                                "它应属于支持 Razer Sensa HD 的设备。在没有对应设备模块之前，本页不编造参数。",
                            )),
                    ))
                    .child(widget_card(
                        card()
                            .child(card_title("该页在雷云里的内容"))
                            .child(SettingRow::new("标题", "触觉（TAB_HAPTICS）".to_string()))
                            .child(SettingRow::new("音频转触觉", "AUDIO_TO_HAPTICS_TITLE".to_string()))
                            .child(SettingRow::new("增益等级", "AUDIO_TO_HAPTICS_GAIN_LEVEL".to_string()))
                            .child(SettingRow::new(
                                "说明",
                                "由音频转化而成的触觉反馈（AUDIO_DRIVEN_HAPTICS）".to_string(),
                            )),
                    )),
            )
            .into_any_element();
    };

    let a2h = haptics.audio_to_haptics;
    let gain = haptics.gain;
    let intensity = haptics.intensity;
    let min_frequency = haptics.min_frequency;
    let max_frequency = haptics.max_frequency;

    v_flex()
        .size_full()
        .gap_2()
        .child(PageHeader::new(
            "触觉",
            format!("{device_name} · 更改会立即保存"),
        ))
        .child(ProductBanner::new(device_name.clone()))
        .child(
            body_widgets()
                .child(widget_card(
                    card()
                        .child(card_title("音频转触觉"))
                        .child(toggle_button(
                            "haptics-a2h",
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
                        )),
                ))
                .child(widget_card(
                    card()
                        .child(card_title("频率范围"))
                        .child(SettingRow::new("最低频率", format!("{min_frequency} Hz")))
                        .child(SettingRow::new("最高频率", format!("{max_frequency} Hz")))
                        .child(div().text_xs().child(
                            "雷云原文：Razer Sensa HD 可实时自动完成音频到触觉的转换，适用于所有游戏、电影和音乐。",
                        )),
                )),
        )
        .into_any_element()
}
