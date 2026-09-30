//! OLED 页（`TAB_OLED`），带 OLED 显示屏的键盘。
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
//! | `TAB_OLED` | OLED |
//! | `CUSTOMIZE_ANIMATION_TITLE` | 自定义动画 |
//! | `CUSTOMIZE_SYSTEM_INFO_DATE_FORMAT_LABEL` | 日期格式 |
//!
//! `OLED` 是语言包里条目数第三多的命名空间（84 条），说明这是一整块功能域。
//!
//! # ⚠️ 设备归属未实测
//!
//! `TAB_OLED` 在语言包与标签页词汇表里都存在，但已下载的三份设备模块
//! **都没有声明该页**——它应属于带 OLED 屏的键盘（如 BlackWidow V4 Pro 的
//! 更高版本）。因此本页不填默认值。

use gpui_kit::component::*;
use gpui_kit::*;

use crate::shell::AppShell;
use crate::ui::widgets::{card_title, 
    body_widgets, card, EmptyState, PageHeader, ProductBanner, SettingRow, stepper_row, toggle_button,
    widget_card,
};

/// 渲染 OLED 页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };
    let device_name = device.display_name().to_string();

    let Some(oled) = device.features.oled.as_ref() else {
        return v_flex()
            .size_full()
            .gap_2()
            .child(PageHeader::new(
                "OLED",
                format!("{device_name} · 该设备没有 OLED 屏"),
            ))
            .child(ProductBanner::new(device_name.clone()))
            .child(
                body_widgets()
                    .child(widget_card(
                        card()
                            .child(card_title("为什么这里没有设置"))
                            .child(div().text_sm().child(
                                "TAB_OLED 在语言包与标签页词汇表里都存在，但已下载的三份设备模块（鼠标 182 / 键盘 653 / 耳机 777）都没有声明该页。",
                            ))
                            .child(div().text_sm().child(
                                "它应属于带 OLED 显示屏的键盘。在没有对应设备模块之前，本页不编造参数。",
                            )),
                    ))
                    .child(widget_card(
                        card()
                            .child(card_title("该页在雷云里的内容"))
                            .child(SettingRow::new("标签页", "OLED（TAB_OLED）".to_string()))
                            .child(SettingRow::new("自定义动画", "CUSTOMIZE_ANIMATION_TITLE".to_string()))
                            .child(SettingRow::new(
                                "系统信息",
                                "CUSTOMIZE_SYSTEM_INFO_DATE_FORMAT_LABEL（日期格式）".to_string(),
                            ))
                            .child(SettingRow::new(
                                "规模",
                                "OLED 命名空间在语言包里有 84 条文案".to_string(),
                            )),
                    )),
            )
            .into_any_element();
    };

    let brightness = oled.brightness;
    let animation = oled.custom_animation;
    let date_format = oled.date_format.clone();
    let timeout = oled.timeout_sec;

    v_flex()
        .size_full()
        .gap_2()
        .child(PageHeader::new(
            "OLED",
            format!("{device_name} · 更改会立即保存"),
        ))
        .child(ProductBanner::new(device_name.clone()))
        .child(
            body_widgets()
                .child(widget_card(
                    card()
                        .child(card_title("屏幕"))
                        .child(stepper_row(
                            "屏幕亮度",
                            format!("{brightness}%"),
                            cx,
                            |this, cx| this.adjust_oled_brightness(-1, cx),
                            |this, cx| this.adjust_oled_brightness(1, cx),
                        ))
                        .child(stepper_row(
                            "闲置关闭",
                            if timeout == 0 {
                                "从不".to_string()
                            } else {
                                format!("{timeout} 秒")
                            },
                            cx,
                            |this, cx| this.adjust_oled_timeout(-1, cx),
                            |this, cx| this.adjust_oled_timeout(1, cx),
                        )),
                ))
                .child(widget_card(
                    card()
                        .child(card_title("显示内容"))
                        .child(toggle_button(
                            "oled-animation",
                            "自定义动画",
                            animation,
                            cx,
                            |this, cx| this.toggle_oled_animation(cx),
                        ))
                        .child(SettingRow::new("日期格式", date_format)),
                )),
        )
        .into_any_element()
}
