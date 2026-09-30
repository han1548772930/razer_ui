//! 777 耳机的声音页：音量、游戏/聊天混音、均衡器、增强和音频功能。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::shell::AppShell;
use crate::ui::widgets::{
    card, card_title, not_wired_hint, select_row, slider_row, toggle_button, EmptyState,
    PageLayout, SettingRow,
};

pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };
    let Some(sound) = device.features.sound.as_ref() else {
        return PageLayout::new("声音", device.display_name())
            .subtitle("当前设备未声明声音能力")
            .widget(EmptyState::new("该设备没有声音设置"))
            .into_any_element();
    };

    let equalizer = &sound.equalizer;
    let volume = sound.volume;
    let chat_mix = sound.chat_mix;
    let enhancement = sound.enhancement;

    let mut layout = PageLayout::new("声音", device.display_name())
        .subtitle("输出音量、均衡器和声音处理")
        .widget(
            card()
                .child(card_title("音量"))
                .child(slider_row(
                    "sound-volume",
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
                    "sound-chat-mix",
                    "游戏 / 聊天混音",
                    chat_mix as f32,
                    0.,
                    100.,
                    1.,
                    format!("{chat_mix}%"),
                    true,
                    cx,
                    |this, value, cx| this.set_chat_mix(value, cx),
                )),
        )
        .widget(
            card()
                .child(card_title("音频均衡器"))
                .child(toggle_button(
                    "sound-eq-enabled",
                    "启用均衡器",
                    equalizer.enabled,
                    cx,
                    |this, cx| this.toggle_eq(cx),
                ))
                .child(toggle_button(
                    "sound-eq-esports",
                    "电竞均衡器",
                    equalizer.esports,
                    cx,
                    |this, cx| this.toggle_eq_esports(cx),
                ))
                .child(SettingRow::new("当前预设", equalizer.preset.clone()))
                .child(div().text_xs().child(
                    "电竞均衡器调整保存在耳机上；标准均衡器保存到当前 profile。",
                )),
        )
        .widget(
            card()
                .child(card_title("音效增强"))
                .child(select_row(
                    "sound-enhancement",
                    "模式",
                    enhancement.zh().to_string(),
                    &crate::domain::AudioEnhancement::LABELS,
                    app.open_select.as_deref() == Some("sound-enhancement"),
                    cx,
                    |this, index, cx| {
                        if let Some(value) = crate::domain::AudioEnhancement::ALL.get(index).copied() {
                            this.set_enhancement(value, cx);
                        }
                    },
                ))
                .child(div().text_xs().child(
                    "使用可用的声音处理模式修改音频播放。",
                )),
        )
        .widget(
            card()
                .child(card_title("音频功能"))
                .child(toggle_button(
                    "sound-audio-meter",
                    "音频计",
                    sound.audio_meter,
                    cx,
                    |this, cx| this.toggle_audio_meter(cx),
                ))
                .child(toggle_button(
                    "sound-audio-mirroring",
                    "立体声镜像",
                    sound.audio_mirroring,
                    cx,
                    |this, cx| this.toggle_audio_mirroring(cx),
                ))
                .child(toggle_button(
                    "sound-power-saving",
                    "无线省电时降低音频",
                    sound.power_saving,
                    cx,
                    |this, cx| this.toggle_audio_power_saving(cx),
                ))
                .child(div().text_xs().child(
                    "立体声镜像适用于前置/后置扬声器；多声道内容播放时应关闭。",
                )),
        )
        ;

    if device.features.key_shifter.is_some() {
        layout = layout.widget(key_shifter_card(device, cx));
    }

    layout.widget(not_wired_hint()).into_any_element()
}

fn key_shifter_card(device: &crate::model::Device, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(shifter) = device.features.key_shifter.as_ref() else {
        return EmptyState::new("未声明变调能力").into_any_element();
    };

    card()
        .child(card_title("变调"))
        .child(div().text_xs().child(
            "启用后调整线路输入端口音频的音高和速度。",
        ))
        .child(toggle_button(
            "sound-key-shifter",
            "启用变调",
            shifter.enabled,
            cx,
            |this, cx| this.toggle_key_shifter(cx),
        ))
        .child(slider_row(
            "sound-key-shifter-pitch",
            "音高",
            shifter.pitch as f32,
            -12.,
            12.,
            1.,
            format!("{:+} 半音", shifter.pitch),
            shifter.enabled,
            cx,
            |this, value, cx| this.set_key_shifter_pitch(value, cx),
        ))
        .child(slider_row(
            "sound-key-shifter-speed",
            "速度",
            shifter.speed as f32,
            50.,
            150.,
            1.,
            format!("{}%", shifter.speed),
            shifter.enabled,
            cx,
            |this, value, cx| this.set_key_shifter_speed(value, cx),
        ))
        .into_any_element()
}
