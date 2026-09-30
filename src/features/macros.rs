//! 宏页：录制入口、宏步骤编辑和失败草稿保护。
//!
//! 设备录制事件流尚未在当前 feature API 暴露，因此页面不伪造“录制成功”或“已下发”。
//! 已有步骤仍然可以编辑为本地草稿，并通过统一保存流程提交。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::domain::MACRO_ACTIONS;
use crate::shell::AppShell;
use crate::ui::widgets::{
    btn, card, card_title, not_wired_hint, select_row, EmptyState, PageLayout, SettingRow,
    slider_row,
};

const MACRO_DELAY_ID: &str = "macro-delay-slider";

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

    let cards: Vec<AnyElement> = macros
        .into_iter()
        .enumerate()
        .map(|(index, mac)| macro_card(index, mac, app.open_select.as_deref(), cx))
        .collect();

    PageLayout::new("宏", device.display_name())
        .subtitle("编辑当前 profile 的宏步骤；本地修改会保留为草稿")
        .widget(recording_card())
        .widget(draft_status_card(app))
        .widget(
            card()
                .child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .child(card_title("宏列表"))
                        .child(btn("macro-add", "新建宏").on_click(cx.listener(|this, _, _, cx| {
                            this.add_macro(cx)
                        }))),
                )
                .when(cards.is_empty(), |this| {
                    this.child(div().text_sm().child("当前 profile 还没有宏步骤"))
                }),
        )
        .widgets(cards)
        .widget(not_wired_hint())
        .into_any_element()
}

fn recording_card() -> AnyElement {
    card()
        .child(card_title("宏录制"))
        .child(div().text_sm().child(
            "录制需要设备宏服务提供 startMacroRecording / stopMacroRecording 事件流。",
        ))
        .child(
            h_flex()
                .gap_2()
                .child(btn("macro-record-start", "开始录制").disabled(true))
                .child(btn("macro-record-stop", "停止录制").disabled(true))
                .child(div().text_xs().child("等待设备录制服务")),
        )
        .into_any_element()
}

fn draft_status_card(app: &AppShell) -> AnyElement {
    let (status, detail) = if app.dirty {
        (
            "草稿未保存",
            "当前修改仍保留在本地草稿中；保存失败时不会清空步骤。",
        )
    } else {
        (
            "无未保存草稿",
            "设备写入确认由统一保存流程返回，不在此处伪造成功状态。",
        )
    };

    card()
        .child(card_title("失败草稿保护"))
        .child(SettingRow::new("当前状态", status.to_string()))
        .child(div().text_xs().child(detail))
        .into_any_element()
}

fn action_group(action: &str) -> &'static str {
    if action.starts_with("key_") {
        "键盘事件"
    } else if action.starts_with("mouse_") {
        "鼠标事件"
    } else if action.starts_with("media_") {
        "媒体事件"
    } else {
        "设备事件"
    }
}

fn stable_id(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn macro_card(
    index: usize,
    mac: (String, bool, u32, Vec<(String, u32)>),
    open_select: Option<&str>,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let (name, loop_until_release, total_ms, steps) = mac;
    let macro_id = stable_id(&name);
    let step_count = steps.len();

    card()
        .child(
            h_flex()
                .w_full()
                .justify_between()
                .items_center()
                .gap_3()
                .child(v_flex().gap_1().child(card_title(name.clone())).child(
                    div().text_xs().child(format!("{} 个步骤 · {} ms", step_count, total_ms)),
                ))
                .child(btn(
                    format!("macro-delete-{macro_id}"),
                    "删除",
                ).on_click(cx.listener(move |this, _, _, cx| {
                    this.remove_macro(index, cx);
                }))),
        )
        .child(SettingRow::new(
            "按住循环",
            if loop_until_release {
                "已启用（来自设备 profile）"
            } else {
                "未启用"
            },
        ))
        .when(step_count == 0, |this| {
            this.child(div().text_sm().child("这个宏还没有步骤；可以添加第一步开始编辑。"))
        })
        .children(steps.into_iter().enumerate().map(|(step_index, (action, delay))| {
            let action_id = format!("macro-{macro_id}-step-{step_index}-action");
            let delay_id = format!("macro-{macro_id}-step-{step_index}-delay");
            let action_group = action_group(&action);

            v_flex()
                .w_full()
                .gap_2()
                .mt_2()
                .child(div().text_xs().child(format!("步骤 {} · {action_group}", step_index + 1)))
                .child(select_row(
                    action_id.clone(),
                    "事件",
                    action,
                    &MACRO_ACTIONS,
                    open_select == Some(action_id.as_str()),
                    cx,
                    move |this, picked, cx| this.set_macro_action(index, step_index, picked, cx),
                ))
                .child(slider_row(
                    MACRO_DELAY_ID,
                    "前置延迟",
                    delay as f32,
                    0.,
                    2000.,
                    10.,
                    format!("{delay} ms"),
                    true,
                    cx,
                    move |this, value, cx| this.set_macro_delay(index, step_index, value, cx),
                ))
                .child(div().text_xs().child(format!("步骤 ID：{delay_id}")))
                .into_any_element()
        }))
        .child(
            h_flex().mt_3().child(btn(
                format!("macro-{macro_id}-step-add"),
                "添加步骤",
            ).on_click(cx.listener(move |this, _, _, cx| {
                this.add_macro_step(index, cx);
            }))),
        )
        .into_any_element()
}
