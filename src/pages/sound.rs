//! 声音页（`TAB_SOUND`），仅耳机/音频设备。
//!
//! # 布局
//!
//! 页面骨架（所有设备页共用，数值引自设备模块 CSS，见
//! [`docs/screens/08-sound.md`](../../docs/screens/08-sound.md) §2.0）：
//!
//! ```text
//! .body-wrapper（内边距 10/20/20）
//! ├─ .widget-prod      顶部产品图区，高 250px
//! └─ .body-widgets     横向 + 换行 + 居中，最大宽 1240px
//!    ├─ .widget        固定 600px 宽，内边距 30/40   ← 一行正好两个
//!    └─ .widget
//! ```
//!
//! # 该页面的分区（引自 `docs/screens/08-sound.md` §2.1）
//!
//! | 分区 | 类名 | 真实 CSS |
//! |---|---|---|
//! | ① 音量与输出 | `volume` | `align-items:center; display:flex` |
//! | | `description-volume-map` | `display:flex; flex-direction:column; height:160px; justify-content:space-between` |
//! | | `volume-title` | `align-items:center; display:flex; justify-content:flex-start` |
//! | ② 均衡器 | `switch-eq-item` · `switch-eq-title` · `description-eq-map` | |
//! | ③ 音效增强 | `thx-wrapper` · `thx-head` · `thx-main-title` · `thx-spatial` · `thx-reset` | |
//! | 其它 | `audio` · `audio-tutorial__video` · `launch-sound-app` | |
//!
//! > **更正**：`audio-left` / `audio-right` 曾被误读为「左右两栏」。
//! > 实际 CSS 是 `.widget-prod img.audio-left, .widget-prod img.audio-right
//! > { left:auto; position:static; top:auto }` —— 它们是**产品图片**的类名，
//! > 不是页面分栏。真正的分栏是 `.widget-col`（宽 600px）。
//!
//! 实测只有耳机（777）显示该页；鼠标与键盘没有。

use gpui_kit::component::*;
use gpui_kit::*;

use crate::app::AppShell;
use crate::pages::widgets::{card_title, select_row, 
    body_widgets, card, EmptyState, PageHeader, ProductBanner, SettingRow, stepper_row, toggle_button,
    widget_card,
};

/// 渲染声音页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    let Some(sound) = device.features.sound.as_ref() else {
        return v_flex()
            .size_full()
            .gap_4()
            .child(PageHeader::new(
                "声音",
                format!("{} · 该设备没有音频设置", device.display_name()),
            ))
            .child(EmptyState::new(
"该设备没有声音设置。实测：只有耳机（777，Kraken）显示该页面。",
            ))
            .into_any_element();
    };

    let volume = sound.volume;
    let chat_mix = sound.chat_mix;
    let eq_enabled = sound.equalizer.enabled;
    let eq_esports = sound.equalizer.esports;
    let eq_preset = sound.equalizer.preset.clone();
    let enhancement = sound.enhancement;
    let audio_meter = sound.audio_meter;
    let mirroring = sound.audio_mirroring;
    let power_saving = sound.power_saving;
    let device_name = device.display_name().to_string();

    // .widget 1：音量（对应 ① 音量与输出）
    let volumes = widget_card(
        card()
            .child(card_title("音量"))
            .child(stepper_row(
                "主音量",
                format!("{volume}%"),
                cx,
                |this, cx| this.adjust_volume(-1, cx),
                |this, cx| this.adjust_volume(1, cx),
            ))
            .child(stepper_row(
                "游戏 / 聊天混音",
                format!("{chat_mix}%"),
                cx,
                |this, cx| this.adjust_chat_mix(-1, cx),
                |this, cx| this.adjust_chat_mix(1, cx),
            )),
    );

    // .widget 2：均衡器（对应 ② 均衡器）
    let eq = widget_card(
        card()
            .child(card_title("音频均衡器"))
            .child(
                h_flex()
                    .gap_4()
                    .flex_wrap()
                    .child(toggle_button("eq-on", "启用", eq_enabled, cx, |this, cx| {
                        this.toggle_eq(cx)
                    }))
                    .child(toggle_button(
                        "eq-esports",
                        "电竞均衡器",
                        eq_esports,
                        cx,
                        |this, cx| this.toggle_eq_esports(cx),
                    )),
            )
            .child(SettingRow::new("预设", eq_preset))
            .child(div().text_xs().child(
                "雷云原文：所有电竞均衡器调整都会保存在耳机上；标准均衡器只保存自定义配置文件。",
            )),
    );

    // .widget 3：音效增强（对应 ③ 增强）
    let enhance = widget_card(
        card()
            .child(card_title("音效增强"))
            .child(select_row(
                "sound-enhancement",
                "模式",
                enhancement.zh().to_string(),
                &crate::features::AudioEnhancement::LABELS,
                app.open_select.as_deref() == Some("sound-enhancement"),
                cx,
                |this, index, cx| {
                    if let Some(picked) =
                        crate::features::AudioEnhancement::ALL.get(index).copied()
                    {
                        this.set_enhancement(picked, cx);
                    }
                },
            ))
            .child(SettingRow::new("THX", "THX 认证的沉浸式影音体验".to_string()))
            .child(SettingRow::new("杜比", "通过杜比虚拟音箱启用虚拟声音".to_string()))
            .child(div().text_xs().child(
                "雷云原文：使用这些声音处理选项之一修改音频播放。",
            )),
    );

    // .widget 4：其它
    let others = widget_card(
        card()
            .child(card_title("其它"))
            .child(
                h_flex()
                    .gap_4()
                    .flex_wrap()
                    .child(toggle_button(
                        "audio-meter",
                        "音频计",
                        audio_meter,
                        cx,
                        |this, cx| this.toggle_audio_meter(cx),
                    ))
                    .child(toggle_button(
                        "audio-mirror",
                        "立体声镜像",
                        mirroring,
                        cx,
                        |this, cx| this.toggle_audio_mirroring(cx),
                    ))
                    .child(toggle_button(
                        "audio-ps",
                        "省电时降低音频",
                        power_saving,
                        cx,
                        |this, cx| this.toggle_audio_power_saving(cx),
                    )),
            )
            .child(div().text_xs().child(
                "雷云该页还有教程视频（audio-tutorial__video）与「打开系统声音设置」（launch-sound-app）。",
            )),
    );

    v_flex()
        .size_full()
        .gap_2()
        .child(PageHeader::new(
            "声音",
            format!("{} · 更改会立即保存", device_name),
        ))
        // 顶部产品图区（.widget-prod，高 250px）
        .child(ProductBanner::new(device_name.clone()))
        // 卡片区（.body-widgets）：600px 固定宽，一行两个，超出换行
        .child(
            body_widgets()
                .child(volumes)
                .child(eq)
                .child(enhance)
                .child(others)
                .child(key_shifter_card(app, cx)),
        )
        .into_any_element()
}

/// 变调（`KEY_SHIFTER`）。
///
/// **为什么在声音页**：`KEY_SHIFTER_TOOLTIP` 原文是「启用即可使用滑块调整
/// **线路输入端口**上任意音频输入的音高和速度。」它是音频设备的功能，
/// 不是键盘功能——放在键盘页会声称一个文案不支持的设备归属。
fn key_shifter_card(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let shifter = app
        .current()
        .and_then(|device| device.features.key_shifter.as_ref());
    let enabled = shifter.map(|shifter| shifter.enabled).unwrap_or(false);
    let pitch = shifter.map(|shifter| shifter.pitch).unwrap_or(0);
    let speed = shifter.map(|shifter| shifter.speed).unwrap_or(100);

    card()
        .child(card_title("变调"))
        .child(div().text_xs().child(
            "雷云原文：启用即可使用滑块调整线路输入端口上任意音频输入的音高和速度。",
        ))
        .child(toggle_button(
            "shifter-toggle",
            "启用变调",
            enabled,
            cx,
            |this, cx| this.toggle_key_shifter(cx),
        ))
        .child(stepper_row(
            "音高",
            format!("{pitch:+} 半音"),
            cx,
            |this, cx| this.adjust_key_shifter_pitch(-1, cx),
            |this, cx| this.adjust_key_shifter_pitch(1, cx),
        ))
        .child(stepper_row(
            "速度",
            format!("{speed}%"),
            cx,
            |this, cx| this.adjust_key_shifter_speed(-1, cx),
            |this, cx| this.adjust_key_shifter_speed(1, cx),
        ))
        .into_any_element()
}