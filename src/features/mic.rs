//! 777 麦克风页：输入、监听、麦克风增强和采样率。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::shell::AppShell;
use crate::ui::widgets::{
    card, card_title, not_wired_hint, select_row, slider_row, toggle_button, EmptyState,
    PageLayout, SettingRow,
};

const MIC_BOOST_LABELS: [&str; 3] = ["关闭", "低", "高"];

pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };
    let Some(mic) = device.features.mic.as_ref() else {
        return PageLayout::new("麦克风", device.display_name())
            .subtitle("当前设备未声明麦克风能力")
            .widget(EmptyState::new("该设备没有麦克风设置"))
            .into_any_element();
    };

    let boost_index = usize::from(mic.boost.min(2));
    let sampling_index = crate::domain::SamplingRate::ALL
        .iter()
        .position(|rate| *rate == mic.sampling_rate)
        .unwrap_or(0);
    let current_boost = mic.boost.min(2);

    PageLayout::new("麦克风", device.display_name())
        .subtitle("麦克风输入、监听和录音质量")
        .widget(
            card()
                .child(card_title("麦克风输入"))
                .child(slider_row(
                    "mic-gain",
                    "麦克风增益",
                    mic.gain as f32,
                    0.,
                    100.,
                    1.,
                    format!("{}%", mic.gain),
                    true,
                    cx,
                    |this, value, cx| this.set_mic_gain(value, cx),
                ))
                .child(select_row(
                    "mic-boost",
                    "麦克风增强",
                    MIC_BOOST_LABELS[boost_index].to_string(),
                    &MIC_BOOST_LABELS,
                    app.open_select.as_deref() == Some("mic-boost"),
                    cx,
                    move |this, picked, cx| {
                        let target = picked.min(2) as i32;
                        let delta = (target - current_boost as i32).rem_euclid(3);
                        for _ in 0..delta {
                            this.cycle_mic_boost(cx);
                        }
                    },
                ))
                .child(toggle_button(
                    "mic-mute",
                    "麦克风静音",
                    mic.muted,
                    cx,
                    |this, cx| this.toggle_mic_mute(cx),
                )),
        )
        .widget(
            card()
                .child(card_title("监听 / 侧音"))
                .child(toggle_button(
                    "mic-monitoring",
                    "启用麦克风监听",
                    mic.monitoring,
                    cx,
                    |this, cx| this.toggle_mic_monitoring(cx),
                ))
                .child(slider_row(
                    "mic-sidetone",
                    "侧音电平",
                    mic.sidetone as f32,
                    0.,
                    100.,
                    1.,
                    format!("{}%", mic.sidetone),
                    mic.monitoring,
                    cx,
                    |this, value, cx| this.set_sidetone(value, cx),
                ))
                .child(div().text_xs().child(
                    "监听麦克风未经优化的声音；启用后设备会保持麦克风活动。",
                )),
        )
        .widget(
            card()
                .child(card_title("麦克风增强"))
                .child(toggle_button(
                    "mic-ai-noise-cancellation",
                    "AI 降噪",
                    mic.ai_noise_cancellation,
                    cx,
                    |this, cx| this.toggle_ai_noise_cancellation(cx),
                ))
                .child(toggle_button(
                    "mic-high-pass-filter",
                    "高通滤波器",
                    mic.high_pass_filter,
                    cx,
                    |this, cx| this.toggle_high_pass_filter(cx),
                ))
                .child(toggle_button(
                    "mic-analogue-gain-limiter",
                    "模拟增益限制器",
                    mic.analogue_gain_limiter,
                    cx,
                    |this, cx| this.toggle_analogue_gain_limiter(cx),
                ))
                .child(div().text_xs().child(
                    "高通滤波器过滤低频隆隆声和嗡嗡声；模拟增益限制器防止削波和语音失真。",
                )),
        )
        .widget(
            card()
                .child(card_title("录音质量"))
                .child(select_row(
                    "mic-sampling-rate",
                    "采样率",
                    mic.sampling_rate.label(),
                    &crate::domain::SamplingRate::LABELS,
                    app.open_select.as_deref() == Some("mic-sampling-rate"),
                    cx,
                    |this, index, cx| {
                        if let Some(rate) = crate::domain::SamplingRate::ALL.get(index).copied() {
                            this.set_sampling_rate(rate, cx);
                        }
                    },
                ))
                .child(SettingRow::new("输入电平", "实时电平需接入音频后端".to_string()))
                .child(div().text_xs().child(
                    "采样率控制录音解析度；当前输入电平未接入真实音频流。",
                )),
        )
        .widget(not_wired_hint())
        .into_any_element()
}
