//! 首页（对应雷云 4 的 Dashboard）。
//!
//! # 结构
//!
//! 不是「一列卡片」，而是 **`.dashboard` 容器 + 若干可折叠分段 `.box-group`**。
//! 逐条引自 `.ref/frontend/static/css/55.a5b041a2.chunk.css`：
//!
//! ```css
//! .dashboard { flex-direction:column; margin:0 auto; max-width:1220px; min-width:620px }
//! .dashboard.reflow { max-width:2460px }
//! .dashboard .box-group { margin:10px 0; min-height:18px; width:100% }
//! .dashboard .box-group .title { align-items:stretch; display:flex; font-size:14px; color:#ccc }
//! .dashboard .box-group .title .collapse { align-items:center; display:flex; flex:1 1; z-index:2 }
//! .dashboard .box-group .title .collapse .icon { height:10px; width:10px; margin-right:10px;
//!                                                transform:rotate(-90deg); transition:transform .3s linear }
//! .dashboard .box-group.expand .title .icon { transform:rotate(0deg) }
//! .dashboard .box-group .title .drag-div { align-items:center; cursor:grab; display:flex;
//!                                          flex:1 1 auto; height:17px; justify-content:center }
//! .dashboard .box-group .title .drag-icon:before { height:19px; width:22px; transform:rotate(90deg);
//!                                                  cursor:grab }
//! .dashboard .box-group .content { margin-top:10px; max-height:0 }
//! .dashboard .box-group .content .content-inner { display:flex; flex-wrap:wrap; gap:20px }
//! .dashboard .box-group .backdrop-box { position:absolute; left:0; right:0; top:0; bottom:10px;
//!                                       max-height:40px; background-color:#333; border-radius:5px;
//!                                       opacity:0; visibility:hidden; z-index:0 }
//! .dashboard .box-group.expand .backdrop-box { bottom:0; max-height:2000px }
//! .dashboard .box-group.dragging .backdrop-box { background-color:#3333334d;
//!                                                border:2px solid #44d62c; border-radius:5px; opacity:1 }
//! .dashboard .box-group #devices { position:relative; z-index:1 }
//! ```
//!
//! 卡片上展示的每个字段都来自实测 Device 对象。
//!
//! # 与真实布局的差异（已登记在 `docs/screens/00-app-shell.md` §5.2）
//!
//! | 项 | 原版 | 本实现 |
//! |---|---|---|
//! | 折叠动画 | `max-height` + `transition .3s` | 直接条件渲染（GPUI 侧未接动画） |
//! | 重排 | 拖动把手 `.drag-div` / `.drag-icon` | **上/下移按钮**——拖拽本身没有键盘路径，按钮有 |
//! | 折叠箭头 | 同一个图标 `rotate(-90deg)` ↔ `0` | 按状态换 `ChevronRight` / `ChevronDown`，视觉等价 |

use gpui_kit::component::*;
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{AppShell, HomeSection};
use crate::model::{Device, SetupStatus};
use crate::nav::Tab;
use crate::pages::widgets::{EmptyState, PageHeader, SettingRow, btn, card_title};

/// 渲染首页。
///
/// `docs/screens/00-app-shell.md` §2 记录了它的真实结构。
/// 这是**应用级首页**（`Tab::Home`），不是设备标签页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let ready = app
        .devices
        .iter()
        .filter(|device| device.setup_status == SetupStatus::Ready)
        .count();

    // 分段列表按当前顺序渲染；每个分段自己决定展开态与内容。
    let sections: Vec<AnyElement> = app
        .home_sections
        .iter()
        .map(|(section, expanded)| box_group(*section, *expanded, app, cx))
        .collect();

    v_flex()
        .size_full()
        .gap_2()
        .child(PageHeader::new(
            "首页",
            format!(
                "共 {} 台设备，其中 {} 台就绪 · 数据取自本机雷云实测快照",
                app.devices.len(),
                ready
            ),
        ))
        .child(
            // `.dashboard { flex-direction:column; margin:0 auto;
            //              max-width:1220px; min-width:620px; position:relative }`
            v_flex()
                .w_full()
                .max_w(px(1220.))
                .min_w(px(620.))
                .mx_auto()
                .relative()
                .children(sections),
        )
        .when(app.demo, |this| this.child(demo_banner(cx)))
        .into_any_element()
}

/// `.dashboard .box-group` —— 一个可折叠的首页分段。
///
/// ```css
/// .dashboard .box-group { margin:10px 0; min-height:18px; width:100% }
/// .dashboard .box-group .title { align-items:stretch; display:flex; font-size:14px; color:#ccc }
/// .dashboard .box-group .title .collapse { align-items:center; display:flex; flex:1 1; z-index:2 }
/// .dashboard .box-group .title .collapse .icon { height:10px; width:10px; margin-right:10px }
/// .dashboard .box-group .title .drag-div { align-items:center; cursor:grab; display:flex;
///                                          flex:1 1 auto; height:17px; justify-content:center }
/// .dashboard .box-group .title .drag-icon:before { height:19px; width:22px }
/// .dashboard .box-group .content { margin-top:10px }
/// .dashboard .box-group .content .content-inner { display:flex; flex-wrap:wrap; gap:20px }
/// .dashboard .box-group .backdrop-box { position:absolute; left:0; right:0; top:0; bottom:10px;
///                                       max-height:40px; background-color:#333; border-radius:5px; z-index:0 }
/// .dashboard .box-group.expand .backdrop-box { bottom:0; max-height:2000px }
/// ```
///
/// # 与原版的两处刻意差异（已登记在 `00-app-shell.md` §5.2）
///
/// - **折叠动画**：原版用 `max-height` + `transition .3s`；GPUI 侧未接动画，
///   所以这里直接条件渲染，收起时只保留标题条。
/// - **重排**：原版是拖动把手 `.drag-div`（`cursor:grab`）。拖拽没有键盘等价操作，
///   而《Design Guides · Accessibility》要求每个动作都有键盘可达的路径，
///   因此改用「上移 / 下移」按钮——把手图标仍按规格渲染出来。
fn box_group(
    section: HomeSection,
    expanded: bool,
    app: &AppShell,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let content: AnyElement = match section {
        HomeSection::Devices => {
            let cards: Vec<AnyElement> = app
                .devices
                .iter()
                .enumerate()
                .map(|(index, device)| {
                    device_card(index, device, index == app.selected, cx)
                })
                .collect();
            if cards.is_empty() {
                EmptyState::new("没有设备").into_any_element()
            } else {
                // `.content-inner { display:flex; flex-wrap:wrap; gap:20px }`
                div()
                    .w_full()
                    .flex()
                    .flex_wrap()
                    .gap(px(20.))
                    .children(cards)
                    .into_any_element()
            }
        }
        HomeSection::LinkedGames => {
            if app.linked_games.is_empty() {
                EmptyState::new("尚未关联任何游戏或程序").into_any_element()
            } else {
                v_flex()
                    .w_full()
                    .gap_2()
                    .children(app.linked_games.iter().map(|game| {
                        SettingRow::new(game.game.clone(), game.profile_guid.clone())
                    }))
                    .into_any_element()
            }
        }
        HomeSection::Engines => crate::pages::engines::render(cx),
    };

    div()
        .w_full()
        .my(px(10.))
        .min_h(px(18.))
        .relative()
        .child(
            // `.backdrop-box { top:0; bottom:10px; max-height:40px; background-color:#333;
            //                 border-radius:5px; z-index:0 }`
            // 展开时 `bottom:0; max-height:2000px`。
            div()
                .absolute()
                .left_0()
                .right_0()
                .top_0()
                .when(expanded, |this| this.bottom_0())
                .when(!expanded, |this| this.bottom(px(10.)))
                .rounded(px(5.))
                .bg(cx.theme().secondary),
        )
        .child(
            v_flex()
                .relative()
                .w_full()
                .child(
                    // `.title { align-items:stretch; display:flex; font-size:14px; color:#ccc }`
                    h_flex()
                        .w_full()
                        .items_center()
                        .text_sm()
                        .child(
                            // `.collapse { align-items:center; display:flex; flex:1 1; z-index:2 }`
                            h_flex()
                                .flex_grow(1.)
                                .flex_shrink(1.)
                                .items_center()
                                .id(("box-group", section as usize))
                                .cursor_pointer()
                                .child(
                                    // `.icon { height:10px; width:10px; margin-right:10px }`
                                    // 原版是同一个图标 `rotate(-90deg)` ↔ `0`，
                                    // 这里按状态换图标，视觉等价。
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
                            // `.drag-div { height:17px; cursor:grab }`
                            // 把手按规格渲染，但重排走下面的两个按钮。
                            h_flex()
                                .flex_grow(1.)
                                .flex_shrink(1.)
                                .h(px(17.))
                                .justify_center()
                                .items_center()
                                .cursor_grab()
                                .child(
                                    Icon::new(IconName::GripVertical).w(px(22.)).h(px(19.)),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_1()
                                .child(
                                    btn(("section-up", section as usize), "上移")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.move_home_section(section, -1, cx);
                                        })),
                                )
                                .child(
                                    btn(("section-down", section as usize), "下移")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.move_home_section(section, 1, cx);
                                        })),
                                ),
                        ),
                )
                // `.content { margin-top:10px; max-height:0 }` /
                // `.expand .content { max-height:none }`
                .when(expanded, |this| this.child(div().mt(px(10.)).child(content))),
        )
        .into_any_element()
}

/// 一张设备卡片（`DeviceCard`）。
///
/// ```css
/// DeviceCard_deviceCard { background:#0000004d; border-radius:5px; flex-direction:column;
///                         min-height:220px; padding:10px; width:290px }
/// DeviceCard_deviceImageContainer { min/max-height:140px }
/// DeviceCard_deviceInfo { font-size:14px; gap:8px; min-height:50px; text-align:center;
///                         text-transform:uppercase }
/// ```
fn device_card(
    index: usize,
    device: &Device,
    selected: bool,
    cx: &mut Context<AppShell>,
) -> AnyElement {
    let name = device.display_name();
    let battery = device.power_status.as_ref().map(|status| {
        format!("{} % · {}", status.level, status.label_zh())
    });

    div()
        .id(("device-card", index))
        .flex()
        .flex_col()
        .flex_shrink_0()
        .w(px(290.))
        .min_h(px(220.))
        .p(px(10.))
        .rounded(px(5.))
        // `background:#0000004d` —— 30% 的黑，主题里没有这个档位，
        // 所以用带出处的常量而不是借用 group_box（#111）。
        .bg(rgba(DEVICE_CARD_BG))
        .border_1()
        .border_color(if selected {
            cx.theme().primary
        } else {
            cx.theme().group_box
        })
        .cursor_pointer()
        .child(
            // `DeviceCard_deviceImageContainer { min/max-height:140px }`
            div()
                .flex()
                .items_center()
                .justify_center()
                .h(px(140.))
                .child(
                    Icon::new(match crate::nav::DeviceKind::from_enum(device.category) {
                        crate::nav::DeviceKind::Mouse => IconName::Mouse,
                        crate::nav::DeviceKind::Keyboard => IconName::Keyboard,
                        crate::nav::DeviceKind::Headset => IconName::Headphones,
                        crate::nav::DeviceKind::Accessory => IconName::Cable,
                        crate::nav::DeviceKind::Laptop => IconName::Laptop,
                        crate::nav::DeviceKind::Other => IconName::Gamepad2,
                    })
                    .w(px(64.))
                    .h(px(64.)),
                ),
        )
        .child(
            // `DeviceCard_deviceInfo { font-size:14px; gap:8px; min-height:50px;
            //                          text-align:center; text-transform:uppercase }`
            v_flex()
                .gap(px(8.))
                .min_h(px(50.))
                .items_center()
                .justify_center()
                .text_sm()
                .text_center()
                // `text-transform:uppercase`
                .child(div().child(name.to_uppercase())),
        )
        // 下面是本实现附加的诊断信息：实测快照里每个字段都有出处，
        // 展示出来便于与 `--probe` 的输出对照。
        .child(
            v_flex()
                .w_full()
                .gap_1()
                .mt_2()
                .child(SettingRow::new("productId", format!("{}", device.product_id)))
                .child(SettingRow::new("类别", device.category.label_zh().to_string()))
                .child(SettingRow::new("固件", device.firmware_info.current_fw_version.clone()))
                .when_some(battery, |this, text| {
                    this.child(SettingRow::new("电量", text))
                }),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.select_device(index, cx);
        }))
        .into_any_element()
}

/// `DeviceCard_deviceCard { background:#0000004d }` —— 30% 黑。
const DEVICE_CARD_BG: u32 = 0x4D00_0000;

/// 演示设备横幅。
///
/// `--demo-keyboard` 注入的是**合成**设备，界面上必须显式说明，
/// 否则看起来像真的插了一台黑寡妇。
fn demo_banner(cx: &mut Context<AppShell>) -> AnyElement {
    div()
        .w_full()
        .max_w(px(1220.))
        .mx_auto()
        .my(px(10.))
        .p(px(12.))
        .rounded(px(5.))
        .bg(cx.theme().secondary)
        .child(card_title("演示模式"))
        .child(div().text_xs().child(
            "本机实测只有鼠标与无线接收器，没有键盘。\
             为了演示键盘功能，`--demo-keyboard` 注入了一台**合成**键盘\
             （productId 9001，名称带「演示」字样）——它不是真实设备。",
        ))
        .into_any_element()
}


