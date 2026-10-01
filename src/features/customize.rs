//! 182 鼠标的 Customize 页面。
//!
//! 该页面不使用通用设置卡片作为主布局。182 的原始页面以
//! `.config-wrapper`/`.config-block` 为核心，设备图居中，按键分列在两侧，
//! 下面再接 Standard/Hypershift 层切换和按键抽屉。

use gpui_kit::assets::IconName;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::model::{action_label_zh, region_label_zh, BUTTON_ACTIONS, DkmKey};
use crate::shell::AppShell;
use crate::ui::widgets::{body_widgets, btn, select_row, toggle_button, EmptyState};

const CONFIG_HEIGHT: f32 = 340.0;
const CONFIG_WIDTH: f32 = 770.0;
const BUTTON_COLUMN_WIDTH: f32 = 235.0;
const MOUSE_IMAGE_SIZE: f32 = 300.0;
const DRAWING_GREEN: u32 = 0x44D62C;
const HYPERSHIFT_ORANGE: u32 = 0xFD8611;

/// 渲染 182 的真实自定义页结构。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };
    if device.product_id != 182 {
        return EmptyState::new("自定义页当前只为 productId 182 鼠标实现").into_any_element();
    }

    let bindings = device.dkm_keys.clone();
    let profiles = device.profiles.clone();
    let active_profile = device.active_profile.clone();
    let hypershift_enabled = device.features.hypershift_enabled;
    let hypershift_bindings = device.features.hypershift_bindings.clone();
    let device_name = device.display_name();

    body_widgets()
        .flex_col()
        .items_center()
        .child(profile_strip(&device_name, &profiles, &active_profile, cx))
        .child(config_wrapper(&bindings, hypershift_enabled))
        .child(config_row(hypershift_enabled, cx))
        .child(button_panel(
            &bindings,
            &hypershift_bindings,
            hypershift_enabled,
            app.open_select.as_deref(),
            cx,
        ))
        .into_any_element()
}

fn profile_strip(
    device_name: &str,
    profiles: &[crate::model::Profile],
    active_profile: &str,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    h_flex()
        .w(px(CONFIG_WIDTH))
        .max_w_full()
        .h(px(42.))
        .items_center()
        .justify_between()
        .border_b_1()
        .border_color(rgb(0x5D5D5D))
        .child(
            div()
                .text_size(px(14.))
                .text_color(rgb(0xCCCCCC))
                .child(format!("{} · 配置文件", device_name)),
        )
        .child(
            h_flex()
                .gap_1()
                .children(profiles.iter().enumerate().map(|(index, profile)| {
                    let selected = profile.id == active_profile;
                    let profile_id = profile.id.clone();
                    btn(
                        format!("profile-{}", stable_id(&profile_id)),
                        profile.name.clone(),
                    )
                    .when(selected, |this| {
                        this.border_1().border_color(rgb(DRAWING_GREEN))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_profile(index, cx);
                    }))
                })),
        )
        .into_any_element()
}

fn config_wrapper(bindings: &[DkmKey], hypershift_enabled: bool) -> AnyElement {
    let left = [0usize, 2, 6, 5, 7];
    let right = [1usize, 3, 4];

    div()
        .id("config-wrapper")
        .w_full()
        .min_w(px(CONFIG_WIDTH))
        .max_w(px(1220.))
        .h(px(CONFIG_HEIGHT))
        .relative()
        .child(
            div()
                .id("config-block")
                .w(px(CONFIG_WIDTH))
                .h(px(CONFIG_HEIGHT))
                .mx_auto()
                .relative()
                .child(connection_canvas(hypershift_enabled))
                .child(button_column("config-buttons-left", &left, bindings, false))
                .child(mouse_visual(hypershift_enabled))
                .child(button_column("config-buttons-right", &right, bindings, true)),
        )
        .into_any_element()
}

fn connection_canvas(hypershift_enabled: bool) -> AnyElement {
    let line_color = if hypershift_enabled {
        rgb(HYPERSHIFT_ORANGE)
    } else {
        rgb(DRAWING_GREEN)
    };

    div()
        .id("config-ctx")
        .absolute()
        .left(px(0.))
        .top(px(0.))
        .w(px(CONFIG_WIDTH))
        .h(px(CONFIG_HEIGHT))
        .child(
            div()
                .absolute()
                .left(px(235.))
                .top(px(78.))
                .w(px(300.))
                .h(px(1.))
                .bg(line_color),
        )
        .child(
            div()
                .absolute()
                .left(px(235.))
                .top(px(258.))
                .w(px(300.))
                .h(px(1.))
                .bg(line_color),
        )
        .child(
            div()
                .absolute()
                .left(px(384.))
                .top(px(30.))
                .w(px(1.))
                .h(px(280.))
                .bg(line_color),
        )
        .into_any_element()
}

fn mouse_visual(hypershift_enabled: bool) -> AnyElement {
    div()
        .id("mouse-svg")
        .absolute()
        .left(px((CONFIG_WIDTH - MOUSE_IMAGE_SIZE) / 2.))
        .top(px(0.))
        .w(px(MOUSE_IMAGE_SIZE))
        .h(px(CONFIG_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .child(
            v_flex()
                .items_center()
                .justify_center()
                .gap_2()
                .w(px(170.))
                .h(px(300.))
                .rounded(px(82.))
                .border_1()
                .border_color(if hypershift_enabled {
                    rgb(HYPERSHIFT_ORANGE)
                } else {
                    rgb(0x5D5D5D)
                })
                .bg(rgb(0x111111))
                .child(
                    Icon::new(IconName::Mouse)
                        .w(px(92.))
                        .h(px(92.))
                        .text_color(rgb(if hypershift_enabled {
                            HYPERSHIFT_ORANGE
                        } else {
                            0x707070
                        })),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(0x707070))
                        .child("Razer DeathAdder V3 Pro"),
                ),
        )
        .into_any_element()
}

fn button_column(
    id: &'static str,
    indices: &[usize],
    bindings: &[DkmKey],
    right: bool,
) -> AnyElement {
    v_flex()
        .id(id)
        .absolute()
        .top(px(0.))
        .when(right, |this| this.right_0())
        .when(!right, |this| this.left_0())
        .w(px(BUTTON_COLUMN_WIDTH))
        .h(px(CONFIG_HEIGHT))
        .justify_between()
        .children(indices.iter().map(|index| {
            mapping_button(bindings.get(*index), *index, right)
        }))
        .into_any_element()
}

fn mapping_button(binding: Option<&DkmKey>, index: usize, right: bool) -> AnyElement {
    let (label, action, enabled) = match binding {
        Some(binding) => (
            region_label_zh(&binding.input_id),
            action_label_zh(&binding.button_key),
            true,
        ),
        None => (format!("按钮 {}", index + 1), "未上报".to_string(), false),
    };

    h_flex()
        .id(format!("config-button-{index}"))
        .w(px(BUTTON_COLUMN_WIDTH))
        .h(px(48.))
        .px(px(10.))
        .gap_2()
        .items_center()
        .justify_between()
        .rounded(px(3.))
        .border_1()
        .border_color(if enabled {
            rgb(0x5D5D5D)
        } else {
            rgb(0x333333)
        })
        .bg(rgb(0x111111))
        .when(!enabled, |this| this.opacity(0.45))
        .child(if right {
            div().text_xs().child(action.clone())
        } else {
            div().text_xs().child(label.clone())
        })
        .child(
            div()
                .text_size(px(12.))
                .text_color(rgb(0x707070))
                .child(if right { label } else { action }),
        )
        .into_any_element()
}

fn config_row(hypershift_enabled: bool, cx: &mut Context<AppShell>) -> AnyElement {
    h_flex()
        .id("config-row")
        .w(px(CONFIG_WIDTH))
        .max_w_full()
        .mt(px(20.))
        .mb(px(10.))
        .items_center()
        .justify_center()
        .gap_2()
        .child(btn("standard-layer", "Standard"))
        .child(toggle_button(
            "hypershift-layer",
            "Hypershift",
            hypershift_enabled,
            cx,
            |this, cx| this.toggle_hypershift(cx),
        ))
        .into_any_element()
}

fn button_panel(
    standard: &[DkmKey],
    hypershift: &[DkmKey],
    hypershift_enabled: bool,
    open_select: Option<&str>,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let bindings = if hypershift_enabled { hypershift } else { standard };
    let rows: Vec<AnyElement> = bindings
        .iter()
        .enumerate()
        .map(|(index, binding)| {
            let input_id = binding.input_id.clone();
            let action = binding.button_key.clone();
            let row_id = format!("drawer-binding-{}", stable_id(&input_id));
            v_flex()
                .w_full()
                .gap_1()
                .pb(px(10.))
                .border_b_1()
                .border_color(rgb(0x333333))
                .child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .child(
                            v_flex()
                                .gap_1()
                                .child(div().text_sm().child(region_label_zh(&input_id)))
                                .child(
                                    div()
                                        .text_size(px(10.))
                                        .text_color(rgb(0x707070))
                                        .child(format!("按钮 {} · 键码 {}", index + 1, binding.key)),
                                ),
                        )
                        .child(select_row(
                            row_id.clone(),
                            action_group(&action),
                            action_label_zh(&action),
                            &BUTTON_ACTIONS,
                            open_select == Some(row_id.as_str()),
                            cx,
                            move |this, picked, cx| this.set_binding_action(index, picked, cx),
                        )),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(0x999999))
                        .child(no_action_parameter(&action)),
                )
                .into_any_element()
        })
        .collect();

    div()
        .id("config-drawer")
        .w(px(600.))
        .max_w_full()
        .p(px(20.))
        .bg(rgb(0x111111))
        .border_1()
        .border_color(rgb(0x5D5D5D))
        .rounded(px(5.))
        .child(
            h_flex()
                .w_full()
                .justify_between()
                .items_center()
                .pb(px(14.))
                .child(div().text_size(px(16.)).child("按键面板"))
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(0x707070))
                        .child(if hypershift_enabled {
                            "Hypershift 层"
                        } else {
                            "Standard 层"
                        }),
                ),
        )
        .when(rows.is_empty(), |this| {
            this.child(
                div()
                    .text_size(px(14.))
                    .text_color(rgb(0x999999))
                    .child("当前层没有设备上报的按键映射。"),
            )
        })
        .children(rows)
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
