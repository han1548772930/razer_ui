//! 653 键盘模块。
//!
//! 653 的真实键盘能力位于 Customize 的 keyboard-svg/MapKeyboard 组件中，
//! Snap Tap 和 OBM 是同一产品模块的条件能力；不渲染通用游戏模式、
//! Dynamic Keystroke 或 Actuation 页面。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::domain::{SnapTapMode, SNAP_TAP_MAX_PAIRS};
use crate::model::{action_label_zh, region_label_zh, BUTTON_ACTIONS};
use crate::shell::AppShell;
use crate::ui::widgets::{
    btn, card, card_title, select_row, toggle_button, EmptyState, PageLayout, SettingRow,
};

pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    if device.product_id != 653 {
        return EmptyState::new("键盘专属模块仅适用于 productId 653").into_any_element();
    }

    let Some(keyboard) = device.features.keyboard.as_ref() else {
        return EmptyState::new("该设备没有键盘能力声明").into_any_element();
    };

    PageLayout::new("键盘", device.display_name())
        .without_product_banner()
        .subtitle("653 keyboard-svg、Snap Tap 与 OBM 能力")
        .widget(keyboard_svg_card(
            &device.dkm_keys,
            app.open_select.as_deref(),
            cx,
        ))
        .widget(snap_tap_card(keyboard, app.open_select.as_deref(), cx))
        .widget(obm_card())
        .into_any_element()
}

fn keyboard_svg_card(
    bindings: &[crate::model::DkmKey],
    open_select: Option<&str>,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    card()
        .id("keyboard-svg")
        .child(card_title("Keyboard SVG / 按键映射"))
        .child(div().text_xs().child(
            "653 的键盘区域由可点击 keyboard-svg 与 keymap-action 映射组成；当前只显示设备实际提供的输入点。",
        ))
        .when(bindings.is_empty(), |this| {
            this.child(EmptyState::new("设备没有上报键盘输入点"))
        })
        .children(bindings.iter().enumerate().map(|(index, binding)| {
            let row_id = format!("keymap-action-{index}");
            let action = binding.button_key.clone();
            h_flex()
                .w_full()
                .justify_between()
                .items_center()
                .child(SettingRow::new(
                    region_label_zh(&binding.input_id),
                    action_label_zh(&binding.button_key),
                ))
                .child(select_row(
                    row_id.clone(),
                    "动作",
                    action_label_zh(&action),
                    &BUTTON_ACTIONS,
                    open_select == Some(row_id.as_str()),
                    cx,
                    move |this, picked, cx| this.set_binding_action(index, picked, cx),
                ))
                .into_any_element()
        }))
        .into_any_element()
}

fn snap_tap_card(
    keyboard: &crate::domain::KeyboardSettings,
    open_select: Option<&str>,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let Some(snap_tap) = keyboard.snap_tap.as_ref() else {
        return card()
            .child(card_title("Snap Tap"))
            .child(SettingRow::new("状态", "设备未声明 Snap Tap"))
            .into_any_element();
    };

    card()
        .id("snap-tap-widget")
        .child(card_title("Snap Tap"))
        .child(div().text_xs().child(
            "最多四组按键组合；录入模式和启用状态由设备映射数据控制。",
        ))
        .child(toggle_button(
            "keyboard-snap-tap",
            "启用 Snap Tap",
            snap_tap.enabled,
            cx,
            |this, cx| this.toggle_snap_tap(cx),
        ))
        .children(snap_tap.pairs.iter().enumerate().map(|(index, pair)| {
            let select_id = format!("snap-tap-mode-{index}");
            let mode_index = SnapTapMode::ALL
                .iter()
                .position(|mode| *mode == pair.mode)
                .unwrap_or(0);
            h_flex()
                .w_full()
                .justify_between()
                .items_center()
                .child(SettingRow::new(
                    format!("组合 {}", index + 1),
                    format!("{} + {}", pair.left, pair.right),
                ))
                .child(select_row(
                    select_id.clone(),
                    "模式",
                    SnapTapMode::LABELS[mode_index].to_string(),
                    &SnapTapMode::LABELS,
                    open_select == Some(select_id.as_str()),
                    cx,
                    move |this, picked, cx| {
                        if let Some(mode) = SnapTapMode::ALL.get(picked).copied() {
                            this.set_snap_tap_mode(index, mode, cx);
                        }
                    },
                ))
                .into_any_element()
        }))
        .child(
            h_flex()
                .gap_2()
                .child(
                    btn("keyboard-snap-tap-add", "添加组合")
                        .disabled(!snap_tap.can_add())
                        .on_click(cx.listener(|this, _, _, cx| this.add_snap_tap_pair(cx))),
                )
                .child(
                    btn("keyboard-snap-tap-remove", "移除最后一组")
                        .disabled(snap_tap.pairs.is_empty())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.remove_snap_tap_pair(cx)
                        })),
                )
                .child(div().text_xs().child(format!(
                    "{}/{} 组",
                    snap_tap.pairs.len(),
                    SNAP_TAP_MAX_PAIRS
                ))),
        )
        .into_any_element()
}

fn obm_card() -> AnyElement {
    card()
        .id("obm-widget")
        .child(card_title("板载内存（OBM）"))
        .child(SettingRow::new("设备状态", "653 声明 isOBMDevice"))
        .child(SettingRow::new("板载配置槽", "4"))
        .child(SettingRow::new(
            "支持映射组",
            "keyboardGroup · macroGroup · multimediaGroup",
        ))
        .child(div().text_xs().child(
            "这里只展示原始 OBM 能力边界；未凭空添加未在设备模块中证明的写入控件。",
        ))
        .into_any_element()
}
