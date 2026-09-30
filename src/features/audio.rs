//! 音频页（`TAB_AUDIO`）。
//!
//! # 布局
//!
//! 与所有设备页共用骨架：顶部 `.widget-prod` 产品图区（250px）
//! ＋ 下方 `.body-widgets` 里固定 600px 宽的 `.widget` 两列换行。
//!
//! # 文案依据
//!
//! | key | 中文 |
//! |---|---|
//! | `TAB_AUDIO` | 音频 |
//! | `AUDIO_DEVICE` / `AUDIO_DEVICES` | 音频设备 |
//! | `AUDIO_FUNCTION` | 音频功能 |
//! | `AUDIO_FUNCTION_DESC` | 在切换音频设备时更改音频输出。 |
//! | `AUDIO_FUNCTION_DESC_AT_LEAST_ONE_AUDIO_DEVICE` | 此功能需要至少一个支持 Razer Synapse 雷云的音频设备。 |
//! | `AUDIO_MODE` | 音频模式 |
//!
//! # 与「声音」页的区别
//!
//! `TAB_AUDIO` 管的是**音频设备与输出路由**（选默认设备、切换行为），
//! `TAB_SOUND` 管的是**这台设备自己的音效**（音量/EQ/增强）。
//!
//! # ⚠️ 设备归属未实测
//!
//! 已下载的三份设备模块（鼠标 182 / 键盘 653 / 耳机 777）**都没有声明 `TAB_AUDIO`**。
//! 因此没有数据的设备显示空状态并说明原因，不编造参数。

use gpui_kit::component::*;
use gpui_kit::*;

use crate::shell::AppShell;
use crate::ui::widgets::{card_title, 
    body_widgets, card, EmptyState, PageHeader, ProductBanner, SettingRow, widget_card,
};

/// 渲染音频页。
pub fn render(app: &AppShell, _cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };
    let device_name = device.display_name().to_string();

    // TAB_AUDIO 读取系统音频服务，不等价于设备自身的 TAB_SOUND。
    // 在 simple_service 接入前保持明确的 unavailable 状态，不填造输出设备。
    let audio_service_available = false;

    v_flex()
        .size_full()
        .gap_2()
        .child(PageHeader::new(
            "音频",
            format!("{device_name} · 更改会立即保存"),
        ))
        .child(ProductBanner::new(device_name.clone()))
        .child(
            body_widgets()
                .child(widget_card(
                    card()
                        .child(card_title("音频设备"))
                        .child(SettingRow::new(
                            "输出设备",
                            if audio_service_available {
                                "等待系统音频设备枚举".to_string()
                            } else {
                                "系统音频服务未连接".to_string()
                            },
                        ))
                        .child(SettingRow::new(
                            "功能状态",
                            if audio_service_available {
                                "可用".to_string()
                            } else {
                                "不可用：需要 simple_service 音频枚举".to_string()
                            },
                        ))
                        .child(div().text_xs().child(
                            "雷云原文：AUDIO_FUNCTION_DESC —— 在切换音频设备时更改音频输出。",
                        ))
                        .child(div().text_xs().child(
                            "AUDIO_FUNCTION_DESC_AT_LEAST_ONE_AUDIO_DEVICE —— 此功能需要至少一个支持 Razer Synapse 雷云的音频设备。",
                        )),
                ))
                .child(widget_card(
                    card()
                        .child(card_title("设备枚举状态"))
                        .child(SettingRow::new(
                            "服务状态",
                            if audio_service_available {
                                "已连接".to_string()
                            } else {
                                "未连接".to_string()
                            },
                        ))
                        .child(div().text_sm().child(
                            "雷云通过 simple_service.dll 的 simpleEnumerateAudioDevices 枚举音频设备。\
                             该导出已在引擎清单里登记（见 --probe simple_service），但本项目尚未接入调用。",
                        ))
                        .child(div().text_xs().child(
                            "在接入之前，本页只呈现结构，不填造设备列表。",
                        )),
                ))
                .child(widget_card(
                    card()
                        .child(card_title("该页在雷云里的内容"))
                        .child(SettingRow::new("标签页", "音频（TAB_AUDIO）".to_string()))
                        .child(SettingRow::new("音频设备", "AUDIO_DEVICE / AUDIO_DEVICES".to_string()))
                        .child(SettingRow::new("音频功能", "AUDIO_FUNCTION".to_string()))
                        .child(SettingRow::new("音频模式", "AUDIO_MODE / AUDIO_MODES".to_string()))
                        .child(SettingRow::new(
                            "设备归属",
                            "未实测（三份已下载模块都没有声明该页）".to_string(),
                        )),
                )),
        )
        .into_any_element()
}
