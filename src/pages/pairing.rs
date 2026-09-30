//! 配对页。
//!
//! 布局依据 `docs/screens/03-pairing.md`：
//!
//! | 分区 | 雷云真实文案 |
//! |---|---|
//! | ① 接收器与固件 | `HYPERPOLLING_WIRELESS` / `DONGLE_IS_LATEST` |
//! | ② 配对指引 | `HYPERPOLLING_WIRELESS_DONGLE_HEADER`、`..._NOTE_FOUR` = 将设备放在接收器附近 |
//! | ③ 已配对设备 | `PAIRED` = 已配对 |
//! | ④ 取消配对 | `HYPERPOLLING_WIRELESS_DONGLE_UNPAIR_CONFIRM_TEXT` |
//!
//! 该页**只在鼠标上**出现（实测：182 有、653 无、777 无）。
//! 本机确实插着接收器：productId 179 `HyperPolling Wireless Dongle`。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::AppShell;
use crate::pages::widgets::{card_title, PageLayout, card, EmptyState, PageHeader, SettingRow, btn};

/// 渲染配对页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    let Some(pairing) = device.features.pairing.as_ref() else {
        return v_flex()
            .size_full()
            .gap_4()
            .child(PageHeader::new(
                "正在配对",
                format!("{} · 该设备无需配对", device.display_name()),
            ))
            .child(EmptyState::new(
"该设备没有无线配对设置。实测：只有鼠标（182）显示该页面。",
            ))
            .into_any_element();
    };

    let dongle = pairing.dongle;
    let is_latest = pairing.dongle_is_latest;
    let paired: Vec<String> = pairing.paired_devices.clone();
    let pairing_now = pairing.pairing;

    // 本机实测的接收器：productId 179。
    let dongle_device = app
        .devices
        .iter()
        .find(|d| d.product_id == 179)
        .map(|d| d.display_name().to_string());

    PageLayout::new("正在配对", device.display_name())
        // ① 接收器
        .widget(
            card()
                .child(card_title("接收器"))
                .child(SettingRow::new("类型", dongle.zh()))
                .child(SettingRow::new(
                    "固件",
                    if is_latest {
                        "已是最新".to_string()
                    } else {
                        "有更新".to_string()
                    },
                ))
                .when_some(dongle_device, |this, name| {
                    this.child(SettingRow::new("本机检测到", name))
                        .child(SettingRow::new("productId", "179".to_string()))
                })
                .child(
                    h_flex().gap_2().child(
                        btn("dongle-cycle", "切换接收器类型")
                            .on_click(cx.listener(|this, _, _, cx| this.cycle_dongle_kind(cx))),
                    ),
                ),
        )
        // ② 配对指引
        .widget(
            card()
                .child(card_title("配对指引"))
                .child(div().text_sm().child(
                    "雷云原文：使用 Razer HyperPolling 无线接收器，你可以配对兼容的设备以获得更优秀的性能。",
                ))
                .child(SettingRow::new(
                    "注意",
                    "将设备放在 HyperPolling 无线接收器附近".to_string(),
                )),
        )
        // ③ 已配对设备
        .widget(
            card()
                .child(card_title("已配对设备"))
                .when(paired.is_empty(), |this| {
                    this.child(div().text_sm().child("尚无已配对设备"))
                })
                .children(paired.iter().map(|name| {
                    h_flex()
                        .w_full()
                        .justify_between()
                        .text_sm()
                        .child(div().child(name.clone()))
                        .child(div().child("已配对"))
                        .into_any_element()
                }))
                .child(
                    h_flex()
                        .gap_2()
                        .flex_wrap()
                        .child(
                            btn("pair-start", if pairing_now { "停止配对" } else { "开始配对" })
                                .on_click(cx.listener(|this, _, _, cx| this.toggle_pairing(cx))),
                        )
                        .child(
                            btn("pair-done", "模拟配对成功")
                                .on_click(cx.listener(|this, _, _, cx| this.complete_pairing(cx))),
                        )
                        .child(
                            btn("pair-unpair", "取消全部配对")
                                .on_click(cx.listener(|this, _, _, cx| this.unpair_all(cx))),
                        ),
                )
                .when(pairing_now, |this| {
                    this.child(div().text_sm().child("正在配对..."))
                })
                .child(div().text_xs().child(
                    "取消配对前雷云会弹确认框：你即将取消 Razer 雷蛇设备与接收器的配对。确定要继续吗？",
                )),
        )
        .widget(
            card()
                .child(card_title("为什么这台设备有这个页面"))
                .child(SettingRow::new("标签页 key", "TAB_PAIRING".to_string()))
                .child(SettingRow::new(
                    "实测分布",
                    "鼠标 182 有 · 键盘 653 无 · 耳机 777 无".to_string(),
                ))
                .child(div().text_xs().child(
                    "该页需要设备模块里存在 TAB_PAIRING，以及本机存在无线接收器。",
                )),
        )
        .into_any_element()
}