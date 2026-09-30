//! 自定义页：profile、Standard/Hypershift 层和设备输入点动作编辑。

use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::model::{action_label_zh, region_label_zh, BUTTON_ACTIONS, DkmKey};
use crate::shell::AppShell;
use crate::ui::widgets::{
    btn, card, card_title, not_wired_hint, select_row, toggle_button, EmptyState, PageLayout,
    SettingRow,
};

/// 渲染 Razer Customize 页面。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };

    let bindings = &device.dkm_keys;
    let active_profile = device
        .profiles
        .iter()
        .find(|profile| profile.id == device.active_profile);

    PageLayout::new("自定义", device.display_name())
        .subtitle(format!(
            "{} · {} 个可自定义输入点 · 当前配置：{}",
            device.display_name(),
            bindings.len(),
            active_profile
                .map(|profile| profile.name.as_str())
                .unwrap_or("未选择")
        ))
        .widget(profile_bar(device, cx))
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

fn profile_bar(device: &crate::model::Device, cx: &mut Context<AppShell>) -> AnyElement {
    let active_id = device.active_profile.clone();
    let profiles = device.profiles.clone();

    card()
        .child(
            h_flex()
                .w_full()
                .justify_between()
                .items_center()
                .child(
                    v_flex()
                        .gap_1()
                        .child(card_title("配置文件"))
                        .child(div().text_xs().child(
                            "Standard 与 Hypershift 使用当前配置文件的独立映射。",
                        )),
                )
                .when(profiles.is_empty(), |this| {
                    this.child(div().text_xs().child("设备没有上报可切换的 profile"))
                }),
        )
        .children(profiles.into_iter().enumerate().map(|(index, profile)| {
            let profile_id = profile.id.clone();
            let selected = profile_id == active_id;
            let button_id = format!("profile-{}", stable_id(&profile_id));
            let label = if selected {
                format!("✓ {}", profile.name)
            } else {
                profile.name
            };

            btn(button_id, label)
                .when(selected, |this| this.border_1().border_color(cx.theme().primary))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.select_profile(index, cx);
                }))
        }))
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

fn no_action_parameter(action: &str) -> &'static str {
    match action {
        "Default" | "Disabled" | "Left Click" | "Right Click" | "Middle Click" => {
            "该动作不需要额外参数"
        }
        "Keyboard" => "键盘按键或组合键由设备动作数据提供",
        "Macro" => "目标宏由设备宏库提供",
        "Multimedia" => "媒体命令由设备动作数据提供",
        "Sensitivity Clutch" => "灵敏度离合档位由设备 profile 提供",
        "Hypershift" => "按住 Hypershift 后使用第二层映射",
        "Switch Profile" => "目标 profile 由设备动作数据提供",
        "Windows Shortcut" => "Windows 快捷键由设备动作数据提供",
        _ => "设备未提供该动作的参数描述",
    }
}

fn action_group(action: &str) -> &'static str {
    match action {
        "Default" | "Disabled" => "基础",
        "Left Click" | "Right Click" | "Middle Click" => "鼠标",
        "Keyboard" | "Windows Shortcut" => "键盘与快捷键",
        "Macro" | "Multimedia" => "宏与媒体",
        "Sensitivity Clutch" | "Hypershift" | "Switch Profile" => "Razer 功能",
        _ => "设备动作",
    }
}

fn empty_bindings() -> AnyElement {
    card()
        .child(card_title("Standard · 按键指派"))
        .child(div().text_sm().child(
            "该设备未上报 dkmKeys，因此没有可指派的输入点。",
        ))
        .into_any_element()
}

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
            let id = format!("binding-{}", stable_id(&input_id));
            let parameter = no_action_parameter(&action);

            v_flex()
                .w_full()
                .gap_1()
                .child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .gap_3()
                        .child(v_flex().gap_1().child(div().text_sm().child(format!(
                            "{} · {}",
                            region_label_zh(&input_id),
                            input_id
                        ))).child(div().text_xs().child(format!("键码 {key}"))))
                        .child(select_row(
                            id.clone(),
                            action_group(&action),
                            action_label_zh(&action),
                            &BUTTON_ACTIONS,
                            open_select == Some(id.as_str()),
                            cx,
                            move |this, picked, cx| this.set_binding_action(index, picked, cx),
                        )),
                )
                .child(SettingRow::new("动作参数", parameter.to_string()))
                .into_any_element()
        })
        .collect();

    card()
        .child(card_title("Standard · 按键动作编辑器"))
        .child(div().text_xs().child(
            "点击设备图形上的按键，或从输入点列表选择动作。动作类型与参数区域分开显示。",
        ))
        .children(rows)
        .into_any_element()
}

fn boss_key_card(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let boss = app
        .current()
        .and_then(|device| device.features.boss_key.as_ref());
    let enabled = boss.map(|boss| boss.enabled).unwrap_or(false);
    let action = boss
        .map(|boss| boss.action.clone())
        .unwrap_or_else(|| "未配置".to_string());

    card()
        .child(card_title("老板键"))
        .child(div().text_xs().child("配置鼠标老板键按下时执行的操作。"))
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

fn hypershift_card(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let enabled = app
        .current()
        .map(|device| device.features.hypershift_enabled)
        .unwrap_or(false);
    let bindings = app
        .current()
        .map(|device| device.features.hypershift_bindings.clone())
        .unwrap_or_default();

    let rows: Vec<AnyElement> = bindings
        .iter()
        .enumerate()
        .map(|(index, binding)| {
            let input_id = binding.input_id.clone();
            let action = binding.button_key.clone();
            let key = binding.key;
            let id = format!("hypershift-binding-{}", stable_id(&input_id));
            let parameter = no_action_parameter(&action);

            v_flex()
                .w_full()
                .gap_1()
                .child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .gap_3()
                        .child(v_flex().gap_1().child(div().text_sm().child(format!(
                            "{} · {}",
                            region_label_zh(&input_id),
                            input_id
                        ))).child(div().text_xs().child(format!("键码 {key}"))))
                        .child(select_row(
                            id.clone(),
                            action_group(&action),
                            action_label_zh(&action),
                            &BUTTON_ACTIONS,
                            app.open_select.as_deref() == Some(id.as_str()),
                            cx,
                            move |this, picked, cx| {
                                this.set_hypershift_action(index, picked, cx)
                            },
                        )),
                )
                .child(SettingRow::new("动作参数", parameter.to_string()))
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
                        .child(card_title("Hypershift · 第二层映射"))
                        .child(div().text_xs().child(
                            "按住 Hypershift 键时启用独立的输入点动作，不覆盖 Standard 层。",
                        )),
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
            this.child(div().text_sm().child("该设备未上报第二层输入点映射"))
        })
        .children(rows)
        .into_any_element()
}
