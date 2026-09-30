//! 宏页：宏列表与步骤编辑。
//!
//! ⚠️ **录制**尚未接入：雷云用 `mapping_engine.dll` 的
//! `startMacroRecording` / `stopMacroRecording` + `macroitem` 事件流实现录制
//! （`docs/FEATURES.md` D1/D2）。这里先做手动编辑与持久化。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::AppShell;
use crate::pages::widgets::{
    EmptyState, PageLayout, SettingRow, btn, card, card_title, not_wired_hint, select_row,
    slider_row,
};

/// 渲染宏页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    let macros: Vec<(String, bool, u32, Vec<(String, u32)>)> = device
        .features
        .macros
        .iter()
        .map(|mac| {
            (
                mac.name.clone(),
                mac.loop_until_release,
                mac.total_ms(),
                mac.steps
                    .iter()
                    .map(|step| (step.action.clone(), step.delay_ms))
                    .collect(),
            )
        })
        .collect();

    let count = macros.len();
    let cards: Vec<AnyElement> = macros
        .into_iter()
        .enumerate()
        .map(|(index, mac)| macro_card(index, mac, app.open_select.as_deref(), cx))
        .collect();

    PageLayout::new("宏", device.display_name())
        .widget(
            card()
                .child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .child(card_title("宏列表"))
                        .child(
                            btn("macro-add", "新建宏")
                                .on_click(cx.listener(|this, _, _, cx| this.add_macro(cx))),
                        ),
                )
                .when(count == 0, |this| {
                    this.child(
                        div()
                            .text_sm()
                            .child("还没有宏。点「新建宏」开始，之后可在「自定义」页把它绑到按键上。"),
                    )
                }),
        )
        .widgets(cards)
        .widget(not_wired_hint())
        .into_any_element()
}

/// 一个宏的卡片。
///
/// `mac` 是 `render` 里预先摊平的四元组
/// `(名称, 按住循环, 总时长 ms, [(事件, 延迟 ms)])`——
/// 摊平是为了不把 `features::Macro` 的借用带进闭包，`cx` 还需要可变借用。
///
/// # 控件选型
///
/// - **事件**：`MACRO_ACTIONS` 是固定集合 → `.s3-dropdown` 直选。
///   之前这里是「◀ 值 ▶」步进器，既不是真实控件，也没法跳到想要的事件。
/// - **延迟**：连续值 → 真实滑块 `.slider-container`。
fn macro_card(
    index: usize,
    mac: (String, bool, u32, Vec<(String, u32)>),
    open_select: Option<&str>,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let (name, loop_until_release, total_ms, steps) = mac;
    let step_count = steps.len();

    card()
        .child(
            h_flex()
                .w_full()
                .justify_between()
                .items_center()
                .gap_3()
                .child(card_title(name))
                .child(
                    btn(("macro-del", index), "删除")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.remove_macro(index, cx);
                        })),
                ),
        )
        .child(SettingRow::new("总时长", format!("{total_ms} ms")))
        .child(SettingRow::new(
            "按住循环",
            if loop_until_release { "是" } else { "否" }.to_string(),
        ))
        .when(step_count == 0, |this| {
            this.child(div().text_sm().child("这个宏还没有步骤"))
        })
        .children(steps.into_iter().enumerate().map(
            |(step_index, (action, delay))| {
                let id = format!("macro-{index}-step-{step_index}");
                v_flex()
                    .w_full()
                    .gap_2()
                    .mt_2()
                    .child(div().text_xs().child(format!("第 {} 步", step_index + 1)))
                    .child(select_row(
                        id.clone(),
                        "事件",
                        action,
                        &crate::features::MACRO_ACTIONS,
                        open_select == Some(id.as_str()),
                        cx,
                        move |this, picked, cx| {
                            this.set_macro_action(index, step_index, picked, cx);
                        },
                    ))
                    .child(slider_row(
                        "macro-delay",
                        "前置延迟",
                        delay as f32,
                        0.,
                        2000.,
                        10.,
                        format!("{delay} ms"),
                        true,
                        cx,
                        move |this, value, cx| {
                            this.set_macro_delay(index, step_index, value, cx);
                        },
                    ))
                    .into_any_element()
            },
        ))
        .child(
            h_flex().mt_3().child(
                btn(("macro-step-add", index), "添加步骤").on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.add_macro_step(index, cx);
                    },
                )),
            ),
        )
        .into_any_element()
}




