//! Dashboard 首页。
//!
//! 对应主前端的 `.dashboard`、`.box-group` 与 `DeviceCard`，不复用设备产品页
//! 的 600px widget 两列布局。页面只展示设备服务已经提供的状态；扫描服务未接入时
//! 明确显示不可用原因，不伪造扫描成功或设备在线状态。

use gpui_kit::assets::IconName;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::model::{Device, SetupStatus};
use crate::shell::{AppShell, HomeSection};
use crate::ui::widgets::{btn, card_title, EmptyState, SettingRow};

const DASHBOARD_MAX_WIDTH: f32 = 1220.0;
const DASHBOARD_MIN_WIDTH: f32 = 620.0;
const GROUP_RADIUS: f32 = 5.0;
const DEVICE_CARD_WIDTH: f32 = 290.0;
const DEVICE_CARD_HEIGHT: f32 = 220.0;
const DEVICE_IMAGE_HEIGHT: f32 = 140.0;
const DEVICE_INFO_HEIGHT: f32 = 50.0;

/// 渲染应用级首页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let ready = app
        .devices
        .iter()
        .filter(|device| device.setup_status == SetupStatus::Ready)
        .count();

    let sections = app
        .home_sections
        .iter()
        .map(|(section, expanded)| box_group(*section, *expanded, app, cx))
        .collect::<Vec<_>>();

    v_flex()
        .size_full()
        .min_w(px(DASHBOARD_MIN_WIDTH))
        .child(
            v_flex()
                .w_full()
                .max_w(px(DASHBOARD_MAX_WIDTH))
                .min_w(px(DASHBOARD_MIN_WIDTH))
                .mx_auto()
                .relative()
                .children(sections)
                .when(!app.devices.is_empty(), |this| {
                    this.child(
                        div()
                            .mt(px(2.))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{} 台设备 · {} 台就绪", app.devices.len(), ready)),
                    )
                }),
        )
        .when(app.demo, |this| this.child(demo_banner(cx)))
        .into_any_element()
}

/// Dashboard 分组：标题栏、折叠状态、拖拽视觉和分组内容保持独立。
fn box_group(
    section: HomeSection,
    expanded: bool,
    app: &AppShell,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let content = match section {
        HomeSection::Devices => device_group(app, cx),
        HomeSection::LinkedGames => linked_games_group(app),
        HomeSection::Engines => crate::features::engines::render(cx),
    };

    div()
        .id(ElementId::Name(
            format!("dashboard-group-{}", section_key(section)).into(),
        ))
        .w_full()
        .my(px(10.))
        .min_h(px(18.))
        .relative()
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .top_0()
                .when(expanded, |this| this.bottom_0())
                .when(!expanded, |this| this.bottom(px(10.)))
                .rounded(px(GROUP_RADIUS))
                .bg(cx.theme().secondary),
        )
        .child(
            v_flex()
                .relative()
                .w_full()
                .child(group_title(section, expanded, app, cx))
                .when(expanded, |this| this.child(div().mt(px(10.)).child(content))),
        )
        .into_any_element()
}

fn group_title(
    section: HomeSection,
    expanded: bool,
    app: &AppShell,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let first = app
        .home_sections
        .first()
        .map(|(current, _)| *current == section)
        .unwrap_or(true);
    let last = app
        .home_sections
        .last()
        .map(|(current, _)| *current == section)
        .unwrap_or(true);

    h_flex()
        .id(ElementId::Name(
            format!("dashboard-group-title-{}", section_key(section)).into(),
        ))
        .w_full()
        .items_center()
        .text_sm()
        .child(
            h_flex()
                .id(ElementId::Name(
                    format!("dashboard-group-toggle-{}", section_key(section)).into(),
                ))
                .flex_grow(1.)
                .flex_shrink(1.)
                .items_center()
                .cursor_pointer()
                .child(
                    Icon::new(if expanded {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .w(px(10.))
                    .h(px(10.))
                    .mr(px(10.)),
                )
                .child(section.title())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_home_section(section, cx);
                })),
        )
        .child(
            h_flex()
                .flex_grow(1.)
                .flex_shrink(1.)
                .h(px(17.))
                .justify_center()
                .items_center()
                .cursor_grab()
                .child(Icon::new(IconName::GripVertical).w(px(22.)).h(px(19.))),
        )
        .child(
            h_flex()
                .gap_1()
                .child(
                    btn(
                        ElementId::Name(
                            format!("dashboard-group-up-{}", section_key(section)).into(),
                        ),
                        "↑",
                    )
                        .disabled(first)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.move_home_section(section, -1, cx);
                        })),
                )
                .child(
                    btn(
                        ElementId::Name(
                            format!("dashboard-group-down-{}", section_key(section)).into(),
                        ),
                        "↓",
                    )
                        .disabled(last)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.move_home_section(section, 1, cx);
                        })),
                ),
        )
        .into_any_element()
}

fn device_group(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    if app.devices.is_empty() {
        return dashboard_empty_state(cx);
    }

    div()
        .w_full()
        .flex()
        .flex_wrap()
        .gap(px(20.))
        .children(app.devices.iter().enumerate().map(|(index, device)| {
            device_card(device.serial_number.clone(), device, index == app.selected, cx)
        }))
        .into_any_element()
}

fn linked_games_group(app: &AppShell) -> AnyElement {
    if app.linked_games.is_empty() {
        return EmptyState::new("尚未关联任何游戏或程序。可在设置页的应用程序配置文件中添加关联。")
            .into_any_element();
    }

    v_flex()
        .w_full()
        .gap_2()
        .children(app.linked_games.iter().map(|game| {
            SettingRow::new(game.game.clone(), game.profile_guid.clone())
        }))
        .into_any_element()
}

fn device_card(
    serial_number: String,
    device: &Device,
    selected: bool,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let name = device.display_name();
    let status = device_status(device.setup_status);
    let battery = device
        .power_status
        .as_ref()
        .map(|power| format!("{}% · {}", power.level, power.label_zh()));
    let secondary = battery.unwrap_or_else(|| {
        format!("{} · 固件 {}", device.category.label_zh(), device.firmware_info.current_fw_version)
    });
    let primary = cx.theme().primary;
    let card_surface = with_alpha(cx.theme().title_bar, 0.3);
    let icon = match crate::nav::DeviceKind::from_enum(device.category) {
        crate::nav::DeviceKind::Mouse => IconName::Mouse,
        crate::nav::DeviceKind::Keyboard => IconName::Keyboard,
        crate::nav::DeviceKind::Headset => IconName::Headphones,
        crate::nav::DeviceKind::Accessory => IconName::Cable,
        crate::nav::DeviceKind::Laptop => IconName::Laptop,
        crate::nav::DeviceKind::Other => IconName::Gamepad2,
    };

    div()
        .id(ElementId::Name(format!("device-card-{serial_number}").into()))
        .flex()
        .flex_col()
        .flex_shrink_0()
        .w(px(DEVICE_CARD_WIDTH))
        .h(px(DEVICE_CARD_HEIGHT))
        .p(px(10.))
        .rounded(px(GROUP_RADIUS))
        .bg(card_surface)
        .border_1()
        .border_color(if selected {
            primary
        } else {
            cx.theme().group_box
        })
        .cursor_pointer()
        .hover(move |this| this.border_color(primary))
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .h(px(DEVICE_IMAGE_HEIGHT))
                .child(Icon::new(icon).w(px(64.)).h(px(64.))),
        )
        .child(
            v_flex()
                .h(px(DEVICE_INFO_HEIGHT))
                .min_h(px(DEVICE_INFO_HEIGHT))
                .w_full()
                .gap(px(3.))
                .items_center()
                .justify_start()
                .overflow_hidden()
                .text_sm()
                .text_center()
                .child(
                    div()
                        .w_full()
                        .line_clamp(2)
                        .overflow_hidden()
                        .child(name),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(status_color(device.setup_status, cx))
                        .child(status),
                )
                .child(
                    div()
                        .w_full()
                        .text_xs()
                        .line_clamp(1)
                        .overflow_hidden()
                        .text_color(cx.theme().muted_foreground)
                        .child(secondary),
                ),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.select_device_by_serial(&serial_number, cx);
        }))
        .into_any_element()
}

fn with_alpha(color: gpui::Hsla, alpha: f32) -> gpui::Hsla {
    gpui::Hsla {
        a: color.a * alpha,
        ..color
    }
}

fn device_status(status: SetupStatus) -> &'static str {
    match status {
        SetupStatus::Ready => "在线 · 就绪",
        SetupStatus::Initializing => "扫描/连接中",
        SetupStatus::Updating => "固件更新中",
        SetupStatus::Unsupported => "已发现 · 不支持",
    }
}

fn status_color(status: SetupStatus, cx: &mut Context<AppShell>) -> gpui::Hsla {
    match status {
        SetupStatus::Ready => cx.theme().success,
        SetupStatus::Initializing => cx.theme().warning,
        SetupStatus::Updating => cx.theme().warning,
        SetupStatus::Unsupported => cx.theme().muted_foreground,
    }
}

fn dashboard_empty_state(cx: &mut Context<AppShell>) -> AnyElement {
    v_flex()
        .w_full()
        .items_center()
        .justify_center()
        .gap_2()
        .p(px(32.))
        .rounded(px(GROUP_RADIUS))
        .bg(cx.theme().secondary)
        .child(Icon::new(IconName::Cable).w(px(36.)).h(px(36.)))
        .child(div().font_bold().child("未发现设备"))
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("设备服务尚未返回可用设备；当前不会显示虚假的 0% 电量或在线状态。"),
        )
        .child(btn("dashboard-scan", "扫描设备").disabled(true))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("扫描命令需要设备服务支持，服务接入后此按钮才会启用。"),
        )
        .into_any_element()
}

fn section_key(section: HomeSection) -> &'static str {
    match section {
        HomeSection::Devices => "devices",
        HomeSection::LinkedGames => "linked-games",
        HomeSection::Engines => "engines",
    }
}

fn demo_banner(cx: &mut Context<AppShell>) -> AnyElement {
    div()
        .w_full()
        .max_w(px(DASHBOARD_MAX_WIDTH))
        .mx_auto()
        .my(px(10.))
        .p(px(12.))
        .rounded(px(GROUP_RADIUS))
        .bg(cx.theme().secondary)
        .child(card_title("演示模式"))
        .child(div().text_xs().child(
            "当前设备列表包含合成演示设备；它不代表真实硬件，也不会改变设备服务状态。",
        ))
        .into_any_element()
}
