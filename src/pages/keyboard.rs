//! 键盘页：游戏模式、全键无冲、布局、背光超时。
//!
//! 对应雷云键盘设备的 Customize / Lighting 之外的那部分设置。
//! 「游戏模式」在雷云里就是**禁用 Windows 键**（`docs/FEATURES.md` C4 输入重定向）。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::AppShell;
use crate::features::KEYBOARD_LAYOUTS;
use crate::pages::widgets::{
    PageHeader, PageLayout, SettingRow, btn, card, card_title, select_row, stepper_row,
    toggle_button,
};

/// 渲染键盘页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return crate::pages::widgets::EmptyState::new("未检测到设备").into_any_element();
    };

    let Some(keyboard) = device.features.keyboard.as_ref() else {
        return v_flex()
            .size_full()
            .gap_4()
            .child(PageHeader::new(
                "键盘",
                format!("{} · 当前设备不是键盘", device.display_name()),
            ))
            .child(crate::pages::widgets::EmptyState::new(
                "该设备没有键盘专属设置。接入雷蛇键盘后这里会出现游戏模式、\
                 全键无冲、布局与背光超时等选项。",
            ))
            .into_any_element();
    };

    let gaming_mode = keyboard.gaming_mode;
    let lock_alt_tab = keyboard.lock_alt_tab;
    let lock_alt_f4 = keyboard.lock_alt_f4;
    let rollover = keyboard.n_key_rollover;
    let layout = keyboard.layout.clone();
    let backlight = keyboard.backlight_timeout_sec;

    let layout_label = KEYBOARD_LAYOUTS
        .iter()
        .find(|(code, _)| *code == layout)
        .map(|(_, name)| *name)
        .unwrap_or("未知");

    PageLayout::new("键盘", device.display_name())
        .widget(
            card()
                .child(card_title("游戏模式"))
                .child(div().text_xs().child(
                    "雷云的游戏模式本质是拦截 Windows 键等系统组合键，\
                     由 mapping_engine 的输入重定向实现",
                ))
                .child(h_flex().gap_2().child(toggle_button(
                    "kb-gaming",
                    "游戏模式（禁用 Windows 键）",
                    gaming_mode,
                    cx,
                    |this, cx| this.toggle_gaming_mode(cx),
                )))
                .child(
                    h_flex()
                        .gap_2()
                        .child(toggle_button(
                            "kb-alttab",
                            "同时锁定 Alt + Tab",
                            lock_alt_tab,
                            cx,
                            |this, cx| this.toggle_lock_alt_tab(cx),
                        ))
                        .child(toggle_button(
                            "kb-altf4",
                            "同时锁定 Alt + F4",
                            lock_alt_f4,
                            cx,
                            |this, cx| this.toggle_lock_alt_f4(cx),
                        )),
                ),
        )
        .widget(
            card()
                .child(card_title("键盘设置"))
                .child(h_flex().gap_2().child(toggle_button(
                    "kb-nkey",
                    "全键无冲",
                    rollover,
                    cx,
                    |this, cx| this.toggle_n_key_rollover(cx),
                )))
                .child(stepper_row(
                    "键盘布局",
                    format!("{layout} · {layout_label}"),
                    cx,
                    |this, cx| this.cycle_keyboard_layout(-1, cx),
                    |this, cx| this.cycle_keyboard_layout(1, cx),
                ))
                .child(stepper_row(
                    "背光自动关闭",
                    if backlight == 0 {
                        "常亮".to_string()
                    } else {
                        format!("{backlight} 秒")
                    },
                    cx,
                    |this, cx| this.adjust_backlight_timeout(-1, cx),
                    |this, cx| this.adjust_backlight_timeout(1, cx),
                ))
                .child(SettingRow::new("轮询率", "见「性能」页".to_string())),
        )
        .widget(snap_tap_section(keyboard, app.open_select.as_deref(), cx))
        .widget(dynamic_key_stroke_section(keyboard, cx))
        .widget(actuation_section(keyboard, cx))
        .into_any_element()
}

/// Snap Tap 快速敲击（`SNAP_TAP`）。
///
/// 文案依据：`SNAP_TAP_DESC_V3`「该功能可防止所选按键同时触发，并且后面按下的
/// 按键会覆盖前面按下的按键。最多可自定义**四对**独特的按键。」、
/// `SNAP_TAP_TOOLTIP_RESTRICTED`「不可指定以下按键：Windows 键、fn 键、菜单键。」
fn snap_tap_section(
    keyboard: &crate::features::KeyboardSettings,
    open_select: Option<&str>,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let Some(snap) = keyboard.snap_tap.as_ref() else {
        return card()
            .child(card_title("快速敲击"))
            .child(div().text_sm().child("该设备不支持 Snap Tap 快速敲击"))
            .into_any_element();
    };
    let enabled = snap.enabled;
    let can_add = snap.can_add();
    let pairs: Vec<(String, String, crate::features::SnapTapMode)> = snap
        .pairs
        .iter()
        .map(|pair| (pair.left.clone(), pair.right.clone(), pair.mode))
        .collect();
    let count = pairs.len();
    let mode_of = |mode: crate::features::SnapTapMode| mode;

    card()
        .child(card_title("Snap Tap 快速敲击"))
        .child(div().text_xs().child(
            "雷云原文：该功能可防止所选按键同时触发，并且后面按下的按键会覆盖前面按下的按键。",
        ))
        .child(toggle_button(
            "snap-toggle",
            "Snap Tap 功能切换",
            enabled,
            cx,
            |this, cx| this.toggle_snap_tap(cx),
        ))
        .child(SettingRow::new(
            "已配置组数",
            format!("{count} / {}", crate::features::SNAP_TAP_MAX_PAIRS),
        ))
        .when(pairs.is_empty(), |this| {
            this.child(div().text_sm().child("尚未指定按键组"))
        })
        .children(
            pairs
                .into_iter()
                .enumerate()
                .map(|(index, (left, right, mode))| {
                    v_flex()
                        .gap_1()
                        .child(SettingRow::new(
                            format!("第 {} 组", index + 1),
                            format!("{left} / {right}"),
                        ))
                        .child(select_row(
                            format!("snap-tap-mode-{index}"),
                            format!("第 {} 组录入模式", index + 1),
                            mode_of(mode).zh().to_string(),
                            &crate::features::SnapTapMode::LABELS,
                            open_select == Some(format!("snap-tap-mode-{index}").as_str()),
                            cx,
                            move |this, picked_index, cx| {
                                if let Some(picked) =
                                    crate::features::SnapTapMode::ALL.get(picked_index).copied()
                                {
                                    this.set_snap_tap_mode(index, picked, cx);
                                }
                            },
                        ))
                        // 直接引用雷云对该模式的原文说明，不改写措辞。
                        .child(
                            div()
                                .text_xs()
                                .child(crate::i18n::t_or(mode_of(mode).desc_key(), "")),
                        )
                        .into_any_element()
                }),
        )
        .child(
            h_flex()
                .gap_2()
                .child(
                    btn(
                        "snap-add",
                        if can_add {
                            "添加另一组"
                        } else {
                            // 到达上限时按钮文字说明原因，而不是静默失效。
                            "已达 4 组上限"
                        },
                    )
                    .disabled(!can_add)
                    .on_click(cx.listener(|this, _, _, cx| this.add_snap_tap_pair(cx))),
                )
                .child(
                    btn("snap-remove", "移除最后一组")
                        .disabled(pairs_is_empty(count))
                        .on_click(cx.listener(|this, _, _, cx| this.remove_snap_tap_pair(cx))),
                ),
        )
        .child(
            div()
                .text_xs()
                .child("雷云原文：不可指定以下按键：Windows 键、fn 键、菜单键。"),
        )
        .into_any_element()
}

/// `pairs` 在上面的 `into_iter()` 里已被消费，这里只看它是否为空。
fn pairs_is_empty(count: usize) -> bool {
    count == 0
}

/// 动态按键敲击（`DYNAMIC_KEY_STROKE`）。
fn dynamic_key_stroke_section(
    keyboard: &crate::features::KeyboardSettings,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    use crate::features::KeyStrokePhase;

    let Some(stroke) = keyboard.dynamic_key_stroke.as_ref() else {
        return card()
            .child(card_title("动态按键敲击"))
            .child(div().text_sm().child("该设备不支持动态按键敲击"))
            .into_any_element();
    };
    let enabled = stroke.enabled;
    let phases: Vec<(KeyStrokePhase, String)> = KeyStrokePhase::ALL
        .iter()
        .map(|phase| (*phase, stroke.phase(*phase).to_string()))
        .collect();
    let start_sens = stroke.press_start_sensitivity;
    let end_sens = stroke.press_end_sensitivity;

    card()
        .child(card_title("动态按键敲击"))
        .child(div().text_xs().child(
            "雷云原文：最多可为按键敲击的四个阶段分别指定四个绑定：按下开始、\
             按下结束、释放开始和释放结束。",
        ))
        .child(toggle_button(
            "dks-toggle",
            "动态按键敲击启用中",
            enabled,
            cx,
            |this, cx| this.toggle_dynamic_key_stroke(cx),
        ))
        .children(phases.into_iter().map(|(phase, binding)| {
            let shown = if binding.is_empty() {
                "（未指定）".to_string()
            } else {
                binding
            };
            SettingRow::new(phase.zh(), shown)
        }))
        .child(stepper_row(
            "按下开始灵敏度",
            format!("{start_sens:.1} mm"),
            cx,
            |this, cx| this.adjust_key_stroke_sensitivity(true, -1.0, cx),
            |this, cx| this.adjust_key_stroke_sensitivity(true, 1.0, cx),
        ))
        .child(stepper_row(
            "按下结束灵敏度",
            format!("{end_sens:.1} mm"),
            cx,
            |this, cx| this.adjust_key_stroke_sensitivity(false, -1.0, cx),
            |this, cx| this.adjust_key_stroke_sensitivity(false, 1.0, cx),
        ))
        .child(div().text_xs().child(
            "雷云原文：要触发单次按键激活，请单击加号图标一次。对于连续的按键行为，\
             单击加号图标并将其拖动到其他加号图标上。",
        ))
        .into_any_element()
}

/// 可调触发点（`ACTUATION_POINT`）。
fn actuation_section(
    keyboard: &crate::features::KeyboardSettings,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let Some(act) = keyboard.actuation.as_ref() else {
        return card()
            .child(card_title("可调节触发"))
            .child(div().text_sm().child("该设备不是模拟光轴 / 霍尔磁性轴键盘"))
            .into_any_element();
    };
    let enabled = act.enabled;
    let primary = act.primary_mm;
    let secondary = act.secondary_mm;
    let rapid = act.rapid_trigger;
    let feedback = act.feedback;
    let risky = act.primary_is_risky();
    let secondary_valid = act.secondary_is_valid();

    card()
        .child(card_title("可调节触发"))
        .child(
            div()
                .text_xs()
                .child("雷云原文：你可以根据个人偏好调节每个 Razer 雷蛇模拟光轴的触发点。"),
        )
        .child(toggle_button(
            "act-toggle",
            "启用可调节触发",
            enabled,
            cx,
            |this, cx| this.toggle_actuation(cx),
        ))
        .child(stepper_row(
            "主触发（毫米）",
            format!("{primary:.1} mm"),
            cx,
            |this, cx| this.adjust_primary_actuation(-1.0, cx),
            |this, cx| this.adjust_primary_actuation(1.0, cx),
        ))
        .child(stepper_row(
            "第二触发（毫米）",
            format!("{secondary:.1} mm"),
            cx,
            |this, cx| this.adjust_secondary_actuation(-1.0, cx),
            |this, cx| this.adjust_secondary_actuation(1.0, cx),
        ))
        // 约束违反时**在控件旁边**说明，而不是等提交才报错。
        .when(!secondary_valid, |this| {
            this.child(
                div()
                    .text_xs()
                    .child("第二触发距离必须大于或等于主触发距离（SECONDARY_ACTUATION_DESC）。"),
            )
        })
        // 低于 1.0mm 时给出雷云自己的警告原文。
        .when(risky, |this| {
            this.child(
                div()
                    .text_xs()
                    .child(crate::i18n::t_or("ACTUATION_WARINING", "")),
            )
        })
        .child(toggle_button(
            "act-rapid",
            "快速触发",
            rapid,
            cx,
            |this, cx| this.toggle_rapid_trigger(cx),
        ))
        .child(div().text_xs().child(format!(
            "量程 {:.1}–{:.1} mm（RAPID_TRIGGER_ADJUSTABLE_ACTUATION_CONTENT_1_BODY）",
            crate::features::ACTUATION_MIN_MM,
            crate::features::ACTUATION_MAX_MM
        )))
        .child(toggle_button(
            "act-feedback",
            "触发反馈（按下红色 / 释放绿色）",
            feedback,
            cx,
            |this, cx| this.toggle_actuation_feedback(cx),
        ))
        .into_any_element()
}
