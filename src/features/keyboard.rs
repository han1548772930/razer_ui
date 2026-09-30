//! 键盘页：游戏模式、标准输入设置、Snap Tap、Dynamic Keystroke 和 Actuation。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::features::{KeyStrokePhase, KEYBOARD_LAYOUTS};
use crate::shell::AppShell;
use crate::ui::widgets::{
    btn, card, card_title, select_row, stepper_row, toggle_button, EmptyState, PageLayout, SettingRow,
};

pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    let Some(keyboard) = device.features.keyboard.as_ref() else {
        return PageLayout::new("键盘", device.display_name())
            .subtitle("当前设备未声明键盘能力")
            .widget(EmptyState::new("该设备没有键盘专属设置"))
            .into_any_element();
    };

    let layout = keyboard.layout.clone();
    let layout_index = KEYBOARD_LAYOUTS
        .iter()
        .position(|(code, _)| *code == layout)
        .unwrap_or(0);
    let layout_label = KEYBOARD_LAYOUTS
        .get(layout_index)
        .map(|(_, name)| *name)
        .unwrap_or("未知");

    PageLayout::new("键盘", device.display_name())
        .subtitle("键盘输入与快速触发功能")
        .widget(gaming_mode_card(keyboard, cx))
        .widget(input_settings_card(
            keyboard,
            layout_index,
            layout_label,
            app.open_select.as_deref(),
            cx,
        ))
        .widget(snap_tap_section(keyboard, app.open_select.as_deref(), cx))
        .widget(dynamic_key_stroke_section(keyboard, cx))
        .widget(actuation_section(keyboard, cx))
        .into_any_element()
}

fn gaming_mode_card(
    keyboard: &crate::features::KeyboardSettings,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    card()
        .child(card_title("游戏模式"))
        .child(div().text_xs().child(
            "游戏模式用于拦截 Windows 键及所选系统组合键，避免游戏中误触。",
        ))
        .child(toggle_button(
            "keyboard-gaming-mode",
            "游戏模式（禁用 Windows 键）",
            keyboard.gaming_mode,
            cx,
            |this, cx| this.toggle_gaming_mode(cx),
        ))
        .child(
            h_flex()
                .gap_3()
                .child(toggle_button(
                    "keyboard-lock-alt-tab",
                    "锁定 Alt + Tab",
                    keyboard.lock_alt_tab,
                    cx,
                    |this, cx| this.toggle_lock_alt_tab(cx),
                ))
                .child(toggle_button(
                    "keyboard-lock-alt-f4",
                    "锁定 Alt + F4",
                    keyboard.lock_alt_f4,
                    cx,
                    |this, cx| this.toggle_lock_alt_f4(cx),
                )),
        )
        .into_any_element()
}

fn input_settings_card(
    keyboard: &crate::features::KeyboardSettings,
    layout_index: usize,
    layout_label: &str,
    open_select: Option<&str>,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let layout_value = format!("{} · {}", keyboard.layout, layout_label);
    let layout_options: Vec<&'static str> = KEYBOARD_LAYOUTS.iter().map(|(_, label)| *label).collect();
    let layout_options: &'static [&'static str] = Box::leak(layout_options.into_boxed_slice());

    card()
        .child(card_title("键盘输入"))
        .child(toggle_button(
            "keyboard-n-key-rollover",
            "全键无冲（N-key rollover）",
            keyboard.n_key_rollover,
            cx,
            |this, cx| this.toggle_n_key_rollover(cx),
        ))
        .child(select_row(
            "keyboard-layout",
            "键盘布局",
            layout_value,
            layout_options,
            open_select == Some("keyboard-layout"),
            cx,
            move |this, picked, cx| {
                let delta = picked as i32 - layout_index as i32;
                if delta != 0 {
                    this.cycle_keyboard_layout(delta, cx);
                }
            },
        ))
        .child(stepper_row(
            "背光自动关闭",
            if keyboard.backlight_timeout_sec == 0 {
                "常亮".to_string()
            } else {
                format!("{} 秒", keyboard.backlight_timeout_sec)
            },
            cx,
            |this, cx| this.adjust_backlight_timeout(-1, cx),
            |this, cx| this.adjust_backlight_timeout(1, cx),
        ))
        .child(SettingRow::new("轮询率", "见「性能」页".to_string()))
        .into_any_element()
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

fn snap_tap_section(
    keyboard: &crate::features::KeyboardSettings,
    open_select: Option<&str>,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let Some(snap) = keyboard.snap_tap.as_ref() else {
        return card()
            .child(card_title("Snap Tap 快速敲击"))
            .child(div().text_sm().child("该设备未声明 Snap Tap 能力"))
            .into_any_element();
    };

    let enabled = snap.enabled;
    let count = snap.pairs.len();
    let can_add = snap.can_add();
    let rows: Vec<AnyElement> = snap
        .pairs
        .iter()
        .enumerate()
        .map(|(index, pair)| {
            let pair_id = format!(
                "snap-tap-{}-{}",
                stable_id(&pair.left),
                stable_id(&pair.right)
            );
            let mode_id = format!("{pair_id}-mode");
            let left = pair.left.clone();
            let right = pair.right.clone();
            let mode = pair.mode;
            v_flex()
                .gap_1()
                .child(SettingRow::new(
                    "按键组合",
                    format!("{left} / {right}"),
                ))
                .child(select_row(
                    mode_id.clone(),
                    "录入模式",
                    mode.zh().to_string(),
                    &crate::features::SnapTapMode::LABELS,
                    open_select == Some(mode_id.as_str()),
                    cx,
                    move |this, picked, cx| {
                        if let Some(mode) = crate::features::SnapTapMode::ALL.get(picked).copied() {
                            this.set_snap_tap_mode(index, mode, cx);
                        }
                    },
                ))
                .child(div().text_xs().child(crate::i18n::t_or(mode.desc_key(), "")))
                .into_any_element()
        })
        .collect();

    card()
        .child(card_title("Snap Tap 快速敲击"))
        .child(div().text_xs().child(
            "防止所选按键同时触发；后按下的按键覆盖先按下的按键，最多四组。",
        ))
        .child(toggle_button(
            "keyboard-snap-tap",
            "启用 Snap Tap",
            enabled,
            cx,
            |this, cx| this.toggle_snap_tap(cx),
        ))
        .child(SettingRow::new(
            "已配置组数",
            format!("{count} / {}", crate::features::SNAP_TAP_MAX_PAIRS),
        ))
        .when(rows.is_empty(), |this| {
            this.child(div().text_sm().child("尚未指定按键组合"))
        })
        .children(rows)
        .child(
            h_flex()
                .gap_2()
                .child(
                    btn(
                        "keyboard-snap-tap-add",
                        if can_add { "添加按键组合" } else { "已达四组上限" },
                    )
                    .disabled(!can_add)
                    .on_click(cx.listener(|this, _, _, cx| this.add_snap_tap_pair(cx))),
                )
                .child(
                    btn("keyboard-snap-tap-remove", "移除最后一组")
                        .disabled(count == 0)
                        .on_click(cx.listener(|this, _, _, cx| this.remove_snap_tap_pair(cx))),
                ),
        )
        .child(div().text_xs().child("不可指定 Windows 键、fn 键和菜单键。"))
        .into_any_element()
}

fn dynamic_key_stroke_section(
    keyboard: &crate::features::KeyboardSettings,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let Some(stroke) = keyboard.dynamic_key_stroke.as_ref() else {
        return card()
            .child(card_title("Dynamic Keystroke 动态按键敲击"))
            .child(div().text_sm().child("该设备未声明 Dynamic Keystroke 能力"))
            .into_any_element();
    };

    card()
        .child(card_title("Dynamic Keystroke 动态按键敲击"))
        .child(div().text_xs().child(
            "按下开始、按下结束、释放开始和释放结束分别对应四个绑定阶段。",
        ))
        .child(toggle_button(
            "keyboard-dynamic-keystroke",
            "启用动态按键敲击",
            stroke.enabled,
            cx,
            |this, cx| this.toggle_dynamic_key_stroke(cx),
        ))
        .children(KeyStrokePhase::ALL.iter().map(|phase| {
            let binding = stroke.phase(*phase);
            SettingRow::new(
                phase.zh(),
                if binding.is_empty() {
                    "未指定".to_string()
                } else {
                    binding.to_string()
                },
            )
        }))
        .child(stepper_row(
            "按下开始灵敏度",
            format!("{:.1} mm", stroke.press_start_sensitivity),
            cx,
            |this, cx| this.adjust_key_stroke_sensitivity(true, -0.1, cx),
            |this, cx| this.adjust_key_stroke_sensitivity(true, 0.1, cx),
        ))
        .child(stepper_row(
            "按下结束灵敏度",
            format!("{:.1} mm", stroke.press_end_sensitivity),
            cx,
            |this, cx| this.adjust_key_stroke_sensitivity(false, -0.1, cx),
            |this, cx| this.adjust_key_stroke_sensitivity(false, 0.1, cx),
        ))
        .into_any_element()
}

fn actuation_section(
    keyboard: &crate::features::KeyboardSettings,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let Some(act) = keyboard.actuation.as_ref() else {
        return card()
            .child(card_title("Actuation 可调触发"))
            .child(div().text_sm().child("该设备不是模拟光轴 / 霍尔磁性轴键盘"))
            .into_any_element();
    };

    card()
        .child(card_title("Actuation 可调触发"))
        .child(div().text_xs().child(
            "调节每个模拟光轴的触发点。主触发距离低于 1.0 mm 时输入错误概率可能增加。",
        ))
        .child(toggle_button(
            "keyboard-actuation",
            "启用可调触发",
            act.enabled,
            cx,
            |this, cx| this.toggle_actuation(cx),
        ))
        .child(stepper_row(
            "主触发",
            format!("{:.1} mm", act.primary_mm),
            cx,
            |this, cx| this.adjust_primary_actuation(-0.1, cx),
            |this, cx| this.adjust_primary_actuation(0.1, cx),
        ))
        .child(stepper_row(
            "第二触发",
            format!("{:.1} mm", act.secondary_mm),
            cx,
            |this, cx| this.adjust_secondary_actuation(-0.1, cx),
            |this, cx| this.adjust_secondary_actuation(0.1, cx),
        ))
        .when(!act.secondary_is_valid(), |this| {
            this.child(div().text_xs().child("第二触发距离必须大于或等于主触发距离。"))
        })
        .child(toggle_button(
            "keyboard-rapid-trigger",
            "快速触发",
            act.rapid_trigger,
            cx,
            |this, cx| this.toggle_rapid_trigger(cx),
        ))
        .child(div().text_xs().child(format!(
            "触发行程范围：{:.1}–{:.1} mm",
            crate::features::ACTUATION_MIN_MM,
            crate::features::ACTUATION_MAX_MM
        )))
        .child(toggle_button(
            "keyboard-actuation-feedback",
            "触发反馈（按下红色 / 释放绿色）",
            act.feedback,
            cx,
            |this, cx| this.toggle_actuation_feedback(cx),
        ))
        .into_any_element()
}
