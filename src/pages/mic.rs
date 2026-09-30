//! 麦克风页（`TAB_MIC`），仅耳机/音频设备。
//!
//! # 布局
//!
//! 与所有设备页共用骨架（见 [`docs/screens/09-mic.md`](../../docs/screens/09-mic.md) §2.0）：
//! 顶部 `.widget-prod` 产品图区（高 250px）＋ 下方 `.body-widgets`
//! 里固定 600px 宽的 `.widget` 两列换行。
//!
//! # 该页面的分区（引自 `docs/screens/09-mic.md` §2.1）
//!
//! | 分区 | 类名 | 真实 CSS |
//! |---|---|---|
//! | ① 麦克风音量 / 增益 | `mic-container` | `position:relative` |
//! | | `mic-container .tip-disabled` | `left:25%; max-width:300px; position:absolute; top:40%` |
//! | ② 监听与降噪 | 整个 `MonitoringDashboard_*`（58 个类名） | section / toggle / metric / sensorList 组成的面板 |
//!
//! 该页**主体是一个监控面板**——`MonitoringDashboard_*` 占该页 64 个类名里的 58 个。
//!
//! 文案依据：`MIC_GAIN`、`MIC_BOOST`、`MIC_MONITORING_SIDETONE`、
//! `MIC_AI_NOISE_CANCELLATION`、`MICROPHONE_V2_TOOLTIP`。

use gpui_kit::component::*;
use gpui_kit::*;

use crate::app::AppShell;
use crate::pages::widgets::{card_title, select_row, 
    body_widgets, card, EmptyState, PageHeader, ProductBanner, SettingRow, stepper_row, toggle_button,
    widget_card,
};

/// 渲染麦克风页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    let Some(mic) = device.features.mic.as_ref() else {
        return v_flex()
            .size_full()
            .gap_4()
            .child(PageHeader::new(
                "麦克风",
                format!("{} · 该设备没有麦克风设置", device.display_name()),
            ))
            .child(EmptyState::new(
"该设备没有麦克风设置。实测：只有耳机（777，Kraken）显示该页面。",
            ))
            .into_any_element();
    };

    let gain = mic.gain;
    let boost = mic.boost;
    let sidetone = mic.sidetone;
    let monitoring = mic.monitoring;
    let ai_nc = mic.ai_noise_cancellation;
    let hpf = mic.high_pass_filter;
    let limiter = mic.analogue_gain_limiter;
    let rate = mic.sampling_rate.label();
    let muted = mic.muted;
    let device_name = device.display_name().to_string();

    // .widget 1：麦克风输入（对应 ① mic-container / micboost）
    let input = widget_card(
        card()
            .child(card_title("麦克风输入"))
            .child(stepper_row(
                "麦克风增益",
                format!("{gain}%"),
                cx,
                |this, cx| this.adjust_mic_gain(-1, cx),
                |this, cx| this.adjust_mic_gain(1, cx),
            ))
            .child(stepper_row(
                "麦克风增强",
                format!("{boost}"),
                cx,
                |_this, _cx| {},
                |this, cx| this.cycle_mic_boost(cx),
            ))
            .child(
                h_flex().gap_4().flex_wrap().child(toggle_button(
                    "mic-mute",
                    "静音",
                    muted,
                    cx,
                    |this, cx| this.toggle_mic_mute(cx),
                )),
            ),
    );

    // .widget 2：监听 / 侧音（对应 ② 的 toggle 部分）
    let monitoring_widget = widget_card(
        card()
            .child(card_title("麦克风监听（侧音）"))
            .child(toggle_button(
                "mic-monitoring",
                "启用监听",
                monitoring,
                cx,
                |this, cx| this.toggle_mic_monitoring(cx),
            ))
            .child(stepper_row(
                "侧音电平",
                format!("{sidetone}%"),
                cx,
                |this, cx| this.adjust_sidetone(-1, cx),
                |this, cx| this.adjust_sidetone(1, cx),
            ))
            .child(div().text_xs().child(
                "雷云原文：使用麦克风监听功能可即时了解自己的语音的输出情况，以便知道是否需要调整说话音量、清晰度和节奏。",
            )),
    );

    // .widget 3：麦克风增强
    let enhance = widget_card(
        card()
            .child(card_title("麦克风增强"))
            .child(
                h_flex()
                    .gap_4()
                    .flex_wrap()
                    .child(toggle_button(
                        "mic-ai-nc",
                        "麦克风 AI 降噪",
                        ai_nc,
                        cx,
                        |this, cx| this.toggle_ai_noise_cancellation(cx),
                    ))
                    .child(toggle_button(
                        "mic-hpf",
                        "高通滤波器",
                        hpf,
                        cx,
                        |this, cx| this.toggle_high_pass_filter(cx),
                    ))
                    .child(toggle_button(
                        "mic-limiter",
                        "模拟增益限制器",
                        limiter,
                        cx,
                        |this, cx| this.toggle_analogue_gain_limiter(cx),
                    )),
            )
            .child(div().text_xs().child("AI 降噪：抑制背景噪音，并提升语音交流。"))
            .child(div().text_xs().child(
                "高通滤波器：过滤低频率的隆隆声和嗡嗡声，例如空调系统的风噪声。",
            ))
            .child(div().text_xs().child(
                "模拟增益限制器：自动防止削波、峰值和语音失真；启用后输入栏轮廓呈橙色。",
            )),
    );

    // .widget 4：采样率 + 监控面板（对应 MonitoringDashboard）
    let dashboard = widget_card(
        card()
            .child(card_title("采样率与监控"))
            .child(select_row(
                "sampling-rate",
                "采样率",
                rate,
                &crate::features::SamplingRate::LABELS,
                app.open_select.as_deref() == Some("sampling-rate"),
                cx,
                |this, index, cx| {
                    if let Some(picked) = crate::features::SamplingRate::ALL.get(index).copied() {
                        this.set_sampling_rate(picked, cx);
                    }
                },
            ))
            .child(div().text_xs().child("雷云原文：修改采样率以控制录音的解析度。"))
            .child(SettingRow::new("输入电平", "（实时数据需接后端）".to_string()))
            .child(div().text_xs().child(
                "雷云该页主体是 MonitoringDashboard 面板，含 section / metric / sensorList 等子块。",
            )),
    );

    v_flex()
        .size_full()
        .gap_2()
        .child(PageHeader::new(
            "麦克风",
            format!("{} · 更改会立即保存", device_name),
        ))
        .child(ProductBanner::new(device_name.clone()))
        .child(
            body_widgets()
                .child(input)
                .child(monitoring_widget)
                .child(enhance)
                .child(dashboard),
        )
        .into_any_element()
}