//! 滚动页。
//!
//! 对齐 `MapScrolling` 的最小结构：滚轮阶段下拉、当前启用阶段列表和编辑
//! 链接。高分辨率、水平滚动、滚动阻力、加速和独立触觉开关没有被该页面证明，
//! 不在这里渲染。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::nav::Tab;
use crate::shell::AppShell;
use crate::ui::widgets::{card, card_title, EmptyState, PageLayout, select_row};

const SCROLL_MODE_LABELS: [&str; 2] = ["滚动模式", "自由滚动"];

pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    let device_name = device.display_name().to_string();
    let Some(scrolling) = device.features.scrolling.as_ref() else {
        return PageLayout::new("滚动", device_name)
            .widget(card().child(card_title("滚动")).child(EmptyState::new("该设备没有滚动设置。")))
            .into_any_element();
    };

    let mode = scrolling.mode;
    let active_stages = scrolling
        .stages
        .iter()
        .filter(|stage| !stage.disabled)
        .map(|stage| (stage.index, stage.haptics))
        .collect::<Vec<_>>();

    let edit_link = div()
        .id("scroll-edit-link")
        .mt_2()
        .mb_2()
        .text_sm()
        .text_color(cx.theme().foreground)
        .underline()
        .cursor_pointer()
        .child("编辑滚轮阶段")
        .on_click(cx.listener(|this, _, _, cx| {
            this.tab = Tab::Customize;
            cx.notify();
        }));

    PageLayout::new("滚动", device_name)
        .widget(
            card()
                .child(card_title("滚轮阶段"))
                .child(select_row(
                    "scroll-mode",
                    "滚轮模式",
                    mode.zh(),
                    &SCROLL_MODE_LABELS,
                    app.open_select.as_deref() == Some("scroll-mode"),
                    cx,
                    |this, index, cx| {
                        if let Some(mode) = crate::domain::ScrollingMode::ALL.get(index).copied() {
                            this.set_scrolling_mode(mode, cx);
                        }
                    },
                ))
                .child(
                    v_flex()
                        .mt_2()
                        .gap_1()
                        .child(div().text_sm().text_color(cx.theme().foreground).child("启用的滚动阶段"))
                        .children(active_stages.into_iter().enumerate().map(|(position, (stage, haptics))| {
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("Stage {} · 触觉 {}%", position + 1, haptics))
                                .when(stage == 0, |this| this.child(""))
                        })),
                )
                .child(edit_link),
        )
        .into_any_element()
}
