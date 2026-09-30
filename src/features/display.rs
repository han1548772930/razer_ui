//! 显示页（`TAB_DISPLAY`），笔记本专属。
//!
//! 布局与文案依据 `docs/screens/` 与主规范 §12.11：
//!
//! | key | 中文 |
//! |---|---|
//! | `PERFORMANCE_MODE_SCREEN_REFRESH_RATE_HEADER` | 屏幕刷新率 |
//! | `PERFORMANCE_MODE_SCREEN_COLOR_PROFILE_HEADER` | 屏幕色彩配置文件 |
//! | `PERFORMANCE_EXTERNAL_DISPLAY_GUIDE` | 外接显示器指引 |
//! | `PERFORMANCE_LAPTOP_SCREEN` | 笔记本屏幕 |
//!
//! ⚠️ 该页的**设备归属尚未实测**：我下载的三份设备模块（鼠标 182 / 键盘 653 /
//! 耳机 777）都没有 `TAB_DISPLAY`，它应属于笔记本（Razer Blade）。
//! 因此这里不编造默认值，没有数据的设备会显示「该设备没有显示设置」。

use gpui_kit::component::*;
use gpui_kit::*;

use crate::shell::AppShell;
use crate::ui::widgets::{card_title, PageLayout, card, EmptyState, PageHeader, SettingRow, stepper_row, toggle_button};

/// 渲染显示页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    let Some(display) = device.features.display.as_ref() else {
        return v_flex()
            .size_full()
            .gap_4()
            .child(PageHeader::new(
                "显示",
                format!("{} · 该设备没有显示设置", device.display_name()),
            ))
            .child(EmptyState::new(
"显示设置属于笔记本（Razer Blade）。已下载的三份设备模块（鼠标/键盘/耳机）都没有 TAB_DISPLAY。",
            ))
            .into_any_element();
    };

    let rate = display.refresh_rate;
    let profile = display.color_profile.clone();
    let performance_mode = display.performance_mode;
    let rate_count = display.refresh_rates.len();

    PageLayout::new("显示", device.display_name())
        .widget(
            card()
                .child(card_title("笔记本屏幕"))
                .child(stepper_row(
                    "屏幕刷新率",
                    format!("{rate} Hz"),
                    cx,
                    |this, cx| this.cycle_refresh_rate(-1, cx),
                    |this, cx| this.cycle_refresh_rate(1, cx),
                ))
                .child(SettingRow::new("可选刷新率数量", format!("{rate_count}")))
                .child(SettingRow::new("色彩配置文件", profile))
                .child(
                    h_flex().gap_4().child(toggle_button(
                        "display-perf",
                        "性能模式联动",
                        performance_mode,
                        cx,
                        |this, cx| this.toggle_display_performance_mode(cx),
                    )),
                ),
        )
        .widget(
            card()
                .child(card_title("外接显示器"))
                .child(div().text_xs().child(
                    "雷云原文（PERFORMANCE_EXTERNAL_DISPLAY_GUIDE）：外接显示器不受本页设置影响，请在系统显示设置中调整。",
                )),
        )
        .widget(
            card()
                .child(card_title("证据说明"))
                .child(SettingRow::new("标签页 key", "TAB_DISPLAY".to_string()))
                .child(SettingRow::new("设备归属", "未实测（应为 Razer Blade 笔记本）".to_string()))
                .child(div().text_xs().child(
                    "已下载设备模块：鼠标 182、键盘 653、耳机 777 —— 三者都没有 TAB_DISPLAY。",
                )),
        )
        .into_any_element()
}
