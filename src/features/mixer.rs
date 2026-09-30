//! 777 混音器页：主音量、游戏/聊天混音与立体声镜像。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::shell::AppShell;
use crate::ui::widgets::{
    card, card_title, not_wired_hint, slider_row, toggle_button, EmptyState, PageLayout,
};

pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };
    let Some(sound) = device.features.sound.as_ref() else {
        return PageLayout::new("混音器", device.display_name())
            .subtitle("当前设备未声明音频混音能力")
            .widget(EmptyState::new("该设备没有混音器设置"))
            .into_any_element();
    };

    PageLayout::new("混音器", device.display_name())
        .subtitle("主音量和游戏 / 聊天音频通道")
        .widget(
            card()
                .child(card_title("音频混音"))
                .child(slider_row(
                    "mixer-volume",
                    "主音量",
                    sound.volume as f32,
                    0.,
                    100.,
                    1.,
                    format!("{}%", sound.volume),
                    true,
                    cx,
                    |this, value, cx| this.set_volume(value, cx),
                ))
                .child(slider_row(
                    "mixer-chat-mix",
                    "游戏 / 聊天混音",
                    sound.chat_mix as f32,
                    0.,
                    100.,
                    1.,
                    format!("{}%", sound.chat_mix),
                    true,
                    cx,
                    |this, value, cx| this.set_chat_mix(value, cx),
                )),
        )
        .widget(
            card()
                .child(card_title("立体声镜像"))
                .child(toggle_button(
                    "mixer-audio-mirroring",
                    "镜像到后置扬声器",
                    sound.audio_mirroring,
                    cx,
                    |this, cx| this.toggle_audio_mirroring(cx),
                ))
                .child(div().text_xs().child(
                    "将前置扬声器中的立体声内容镜像到后置扬声器；播放多声道内容时关闭。",
                )),
        )
        .widget(not_wired_hint())
        .into_any_element()
}
