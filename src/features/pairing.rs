//! productId 182 鼠标的无线配对页。
//!
//! 页面只表达 182 PairingContent 能证明的接收器、扫描、设备卡和解除配对状态。

use gpui_kit::assets::IconName;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::shell::AppShell;
use crate::ui::widgets::{btn, widget_slot, EmptyState, PageLayout};

const DEVICE_CARD_W: f32 = 290.0;
const DEVICE_CARD_H: f32 = 220.0;

pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };
    if device.product_id != 182 {
        return EmptyState::new("配对页当前只为 productId 182 鼠标实现").into_any_element();
    }
    let Some(_pairing) = device.features.pairing.as_ref() else {
        return EmptyState::new("该设备没有无线配对设置").into_any_element();
    };

    PageLayout::new("配对", device.display_name())
        .without_product_banner()
        .widget(widget_slot(pairing_content(app, cx)))
        .into_any_element()
}

fn pairing_content(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("配对数据不可用").into_any_element();
    };
    let Some(pairing) = device.features.pairing.as_ref() else {
        return EmptyState::new("配对数据不可用").into_any_element();
    };

    let dongle_connected = app.devices.iter().any(|device| device.product_id == 179);
    let pairing_now = pairing.pairing;
    let paired_devices = pairing.paired_devices.clone();

    let mut devices = h_flex()
        .w_full()
        .flex_wrap()
        .gap(px(20.))
        .child(dongle_card(pairing.dongle.zh(), pairing.dongle_is_latest, dongle_connected, cx));

    for (index, name) in paired_devices.iter().cloned().enumerate() {
        devices = devices.child(device_card(index, name, cx));
    }

    if pairing_now {
        devices = devices.child(skeleton_card(cx));
    } else if paired_devices.is_empty() {
        devices = devices.child(empty_pairing_card("扫描后会在这里显示兼容设备", cx));
    }

    v_flex()
        .w_full()
        .gap_3()
        .p(px(30.))
        .rounded(px(5.))
        .bg(cx.theme().group_box)
        .child(div().text_size(px(16.)).font_bold().child("无线配对"))
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().foreground)
                .child("将设备放在 HyperPolling 无线接收器附近，然后开始扫描。"),
        )
        .child(devices)
        .child(
            h_flex()
                .gap_2()
                .child(
                    btn("pair-scan", if pairing_now { "停止扫描" } else { "开始扫描" })
                        .disabled(!dongle_connected)
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_pairing(cx))),
                )
                .child(
                    btn("pair-unpair", "取消配对")
                        .disabled(paired_devices.is_empty() || !dongle_connected)
                        .on_click(cx.listener(|this, _, _, cx| this.unpair_all(cx))),
                ),
        )
        .when(!dongle_connected, |this| {
            this.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("未检测到 HyperPolling 无线接收器；扫描和解除配对不可用。"),
            )
        })
        .when(pairing_now, |this| {
            this.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("正在等待设备服务返回扫描结果；未收到结果前不会显示配对成功。"),
            )
        })
        .into_any_element()
}

fn dongle_card(
    name: String,
    latest: bool,
    connected: bool,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    device_card_base(cx)
        .child(Icon::new(IconName::Cable).w(px(64.)).h(px(64.)))
        .child(div().text_sm().child(name))
        .child(
            div()
                .text_xs()
                .text_color(if connected {
                    cx.theme().success
                } else {
                    cx.theme().muted_foreground
                })
                .child(if connected { "已连接" } else { "未检测到" }),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(if latest { "固件已是最新" } else { "固件有更新" }),
        )
        .into_any_element()
}

fn device_card(index: usize, name: String, cx: &mut Context<AppShell>) -> AnyElement {
    device_card_base(cx)
        .id(format!("paired-device-{index}"))
        .child(Icon::new(IconName::Mouse).w(px(64.)).h(px(64.)))
        .child(div().text_sm().child(name))
        .child(div().text_xs().text_color(cx.theme().success).child("已配对"))
        .into_any_element()
}

fn device_card_base(cx: &mut Context<AppShell>) -> Div {
    v_flex()
        .flex_shrink_0()
        .w(px(DEVICE_CARD_W))
        .h(px(DEVICE_CARD_H))
        .items_center()
        .justify_center()
        .gap_2()
        .p(px(20.))
        .rounded(px(5.))
        .bg(cx.theme().background)
        .text_color(cx.theme().foreground)
}

fn skeleton_card(cx: &mut Context<AppShell>) -> AnyElement {
    v_flex()
        .flex_shrink_0()
        .w(px(DEVICE_CARD_W))
        .h(px(DEVICE_CARD_H))
        .gap_3()
        .p(px(10.))
        .rounded(px(5.))
        .bg(rgba(0x0000004d))
        .child(div().w(px(76.)).h(px(20.)).rounded_full().border_1().border_color(cx.theme().muted_foreground))
        .child(div().w(px(248.)).h(px(99.)).rounded(px(3.)).bg(rgba(0xffffff08)))
        .child(div().w(px(248.)).h(px(16.)).rounded(px(3.)).bg(rgba(0xffffff08)))
        .into_any_element()
}

fn empty_pairing_card(text: &'static str, cx: &mut Context<AppShell>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .w(px(DEVICE_CARD_W))
        .h(px(DEVICE_CARD_H))
        .p(px(36.))
        .rounded(px(5.))
        .border_1()
        .border_color(cx.theme().muted_foreground)
        .text_sm()
        .text_center()
        .child(text)
        .into_any_element()
}
