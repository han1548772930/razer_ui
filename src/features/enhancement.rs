//! 777 增强页：音效增强与音频转触觉。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::shell::AppShell;
use crate::ui::widgets::{
    card, card_title, not_wired_hint, select_row, slider_row, toggle_button, EmptyState,
    PageLayout,
};

pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };
    let sound = device.features.sound.as_ref();
    let haptics = device.features.haptics.as_ref();
    if sound.is_none() && haptics.is_none() {
        return PageLayout::new("增强", device.display_name())
            .subtitle("当前设备未声明声音增强或触觉能力")
            .widget(EmptyState::new("该设备没有增强设置"))
            .into_any_element();
    }

    let mut layout = PageLayout::new("增强", device.display_name())
        .subtitle("声音处理与音频转触觉")
        ;

    if let Some(sound) = sound {
        layout = layout.widget(
                card()
                    .child(card_title("音效增强"))
                    .child(select_row(
                        "enhancement-mode",
                        "模式",
                        sound.enhancement.zh().to_string(),
                        &crate::domain::AudioEnhancement::LABELS,
                        app.open_select.as_deref() == Some("enhancement-mode"),
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
            );
    }

    if let Some(haptics) = haptics {
        layout = layout.widget(
                card()
                    .child(card_title("音频转触觉"))
                    .child(toggle_button(
                        "enhancement-audio-to-haptics",
                        "启用音频转触觉",
                        haptics.audio_to_haptics,
                        cx,
                        |this, cx| this.toggle_audio_to_haptics(cx),
                    ))
                    .child(slider_row(
                        "enhancement-haptics-gain",
                        "增益等级",
                        haptics.gain as f32,
                        0.,
                        100.,
                        1.,
                        format!("{}%", haptics.gain),
                        haptics.audio_to_haptics,
                        cx,
                        |this, value, cx| this.set_haptics_gain(value, cx),
                    ))
                    .child(slider_row(
                        "enhancement-haptics-intensity",
                        "强度",
                        haptics.intensity as f32,
                        0.,
                        100.,
                        1.,
                        format!("{}%", haptics.intensity),
                        haptics.audio_to_haptics,
                        cx,
                        |this, value, cx| this.set_haptics_intensity(value, cx),
                    ))
                    .child(div().text_xs().child(
                        "Razer Sensa HD 可将游戏、电影和音乐实时转换为触觉反馈。",
                    )),
            );
    }

    layout.widget(not_wired_hint()).into_any_element()
}
