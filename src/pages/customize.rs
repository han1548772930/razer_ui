//! 自定义页：按键映射。
//!
//! 数据来自设备实测的 `dkmKeys`：
//! `{"inputID":"DKM_M_01","buttonKey":"SensitivityStageUp","key":32}`
//!
//! 可用动作是**子集**：完整动作列表由设备模块在运行时下发，本地拿不到。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::AppShell;
use crate::model::{BUTTON_ACTIONS, DkmKey, action_label_zh, region_label_zh};
use crate::pages::widgets::{
    EmptyState, PageLayout, SettingRow, btn, card, card_title, not_wired_hint, select_row,
    toggle_button,
};

/// 渲染按键映射页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return no_device().into_any_element();
    };

    let bindings = &device.dkm_keys;

    // 页头用共享的 `PageHeader`（经 `PageLayout`），不再本页手搓
    // `div().text_xl().font_bold()`——依据 gpui-kit 《Coding Guides ·
    // Rendering and composition》：
    // > Compose from the standard semantic component before building a custom
    // > surface.
    PageLayout::new("自定义", device.display_name())
        .subtitle(format!(
            "{} · {} 个可自定义输入点 · 更改会立即保存",
            device.display_name(),
            bindings.len()
        ))
        .widget(if bindings.is_empty() {
            empty_bindings()
        } else {
            bindings_card(bindings, app.open_select.as_deref(), cx)
        })
        .widget(hypershift_card(app, cx))
        .widget(boss_key_card(app, cx))
        .widget(not_wired_hint())
        .into_any_element()
}

/// 未检测到设备时的空态。
///
/// 交给共享的 [`EmptyState`]：依据 gpui-kit 《Coding Guides ·
/// Rendering and composition》——先组合标准语义组件，再考虑自建表面。
fn no_device() -> EmptyState {
    EmptyState::new("未检测到设备")
}

/// 该设备没有上报任何可自定义输入点。
fn empty_bindings() -> AnyElement {
    card()
        .child(card_title("按键指派"))
        .child(div().text_sm().child(
            "该设备未上报 dkmKeys，因此没有可指派的输入点。",
        ))
        .into_any_element()
}

/// 按键指派卡片：每个物理输入点一行，右侧是动作下拉。
///
/// # 为什么动作是下拉而不是「◀ 值 ▶」
///
/// `BUTTON_ACTIONS` 是一个**固定的字符串集合**——雷云里这类设置项一律是
/// `.s3-dropdown` 直选。之前这里用 `stepper_row` 在列表里循环，
/// 既不是真实控件，也没法直接跳到想要的动作。
///
/// ```css
/// .s3-dropdown { background-color:#0000; border:1px solid #515151; }
/// ```
fn bindings_card(
    bindings: &[DkmKey],
    open_select: Option<&str>,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let rows: Vec<AnyElement> = bindings
        .iter()
        .enumerate()
        .map(|(index, binding)| {
            let input_id = binding.input_id.clone();
            let action = binding.button_key.clone();
            let key = binding.key;
            // 每个输入点一个独立的下拉 id，否则同一页的多个下拉会一起展开。
            let id = format!("binding-{index}");

            h_flex()
                .w_full()
                .justify_between()
                .items_center()
                .gap_3()
                .child(
                    v_flex()
                        .gap_1()
                        .child(div().text_sm().child(format!(
                            "{} · {}",
                            region_label_zh(&input_id),
                            input_id
                        )))
                        .child(div().text_xs().child(format!("键码 {key}"))),
                )
                .child(select_row(
                    id.clone(),
                    "",
                    action_label_zh(&action),
                    &BUTTON_ACTIONS,
                    open_select == Some(id.as_str()),
                    cx,
                    move |this, picked, cx| this.set_binding_action(index, picked, cx),
                ))
                .into_any_element()
        })
        .collect();

    card()
        .child(card_title("按键指派"))
        .child(div().text_xs().child(
            "雷云原文：点击设备图形上的按键，或直接在下表中选择要指派的动作。",
        ))
        .children(rows)
        .into_any_element()
}



/// 老板键（`BOSS_KEY`）。
///
/// **为什么在自定义页**：`BOSS_KEY_CONFIGURATION_TIP` 原文是
/// 「配置**鼠标**的老板键功能。」它本质是一项按键分配，因此属于自定义（按键映射）页，
/// 而不是键盘页。
fn boss_key_card(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let boss = app
        .current()
        .and_then(|device| device.features.boss_key.as_ref());
    let enabled = boss.map(|boss| boss.enabled).unwrap_or(false);
    let action = boss
        .map(|boss| boss.action.clone())
        .unwrap_or_else(|| "静音".to_string());

    card()
        .child(card_title("老板键"))
        .child(div().text_xs().child("雷云原文：设置按下老板键时执行的操作。"))
        .child(toggle_button(
            "boss-toggle",
            "老板键配置",
            enabled,
            cx,
            |this, cx| this.toggle_boss_key(cx),
        ))
        .child(SettingRow::new("按下时执行", action))
        .into_any_element()
}

/// Hypershift 第二套映射层。
///
/// 雷云实测：`Razer Hypershift` 在日志中出现 24 次，作用是按住修饰键后
/// 让每个按键拥有第二套动作（`docs/FEATURES.md` C2）。
fn hypershift_card(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let enabled = app
        .current()
        .map(|device| device.features.hypershift_enabled)
        .unwrap_or(false);

    let bindings: Vec<(String, String, u32)> = app
        .current()
        .map(|device| {
            device
                .features
                .hypershift_bindings
                .iter()
                .map(|binding| {
                    (
                        binding.input_id.clone(),
                        binding.button_key.clone(),
                        binding.key,
                    )
                })
                .collect()
        })
        .unwrap_or_default();

    let rows: Vec<AnyElement> = bindings
        .into_iter()
        .enumerate()
        .map(|(index, (input_id, action, key))| {
            h_flex()
                .w_full()
                .justify_between()
                .items_center()
                .gap_3()
                .child(
                    v_flex()
                        .gap_1()
                        .child(div().text_sm().child(format!(
                            "{} · {}",
                            region_label_zh(&input_id),
                            input_id
                        )))
                        .child(div().text_xs().child(format!("键码 {key}"))),
                )
                .child(select_row(
                    format!("hs-binding-{index}"),
                    "",
                    action_label_zh(&action),
                    &BUTTON_ACTIONS,
                    app.open_select.as_deref() == Some(format!("hs-binding-{index}").as_str()),
                    cx,
                    move |this, picked, cx| this.set_hypershift_action(index, picked, cx),
                ))
                .into_any_element()
        })
        .collect();

    card()
        .child(
            h_flex()
                .w_full()
                .justify_between()
                .items_center()
                .gap_3()
                .child(
                    v_flex()
                        .gap_1()
                        .child(card_title("Hypershift 第二层"))
                        .child(
                            div()
                                .text_xs()
                                .child("按住 Hypershift 键后，每个按键改用下面这套动作"),
                        ),
                )
                .child(toggle_button(
                    "hs-toggle",
                    "Hypershift",
                    enabled,
                    cx,
                    |this, cx| this.toggle_hypershift(cx),
                )),
        )
        .when(rows.is_empty(), |this| {
            this.child(
                div()
                    .text_sm()
                    .child("该设备未上报可自定义输入点，因此没有第二层可配"),
            )
        })
        .children(rows)
        .into_any_element()
}


