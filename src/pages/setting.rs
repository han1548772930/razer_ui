//! 设置页（`TAB_SETTING`）。
//!
//! # 布局
//!
//! 与所有设备页共用骨架：顶部 `.widget-prod` 产品图区（250px）
//! ＋ 下方 `.body-widgets` 里固定 600px 宽的 `.widget` 两列换行。
//!
//! # 文案依据
//!
//! | key | 中文 |
//! |---|---|
//! | `TAB_SETTING` | 设置 |
//! | `PROFILE_MIGRATION` | 配置文件迁移 |
//! | `PROFILE_SWITCHING` | 配置文件切换 |
//! | `SETTING_IT_UP` | （开始设置） |
//! | `GET_SYSTEM_KEYBOARD_LAYOUT` | 读取系统键盘布局 |
//!
//! 该页是**设备无关**的：所有设备都有，内容是按设备的配置文件与键盘布局。

use gpui_kit::component::{button::*, dialog::AlertDialog, *};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::AppShell;
use crate::features::KEYBOARD_LAYOUTS;
use crate::pages::widgets::{card_title, select_row, toggle_button, widget_slot, body_widgets, card, EmptyState, PageHeader, ProductBanner, SettingRow, stepper_row, widget_card, btn};

/// 渲染设置页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let Some(device) = app.current() else {
        return EmptyState::new("未检测到设备").into_any_element();
    };
    let device_name = device.display_name().to_string();

    let active = device.active_profile_obj().map(|p| p.name.clone());
    let active_guid = device.active_profile.clone();
    let single = device.is_single_profile;
    let profiles: Vec<(String, String, bool)> = device
        .profiles
        .iter()
        .map(|profile| {
            (
                profile.name.clone(),
                profile.id.clone(),
                profile.guid == device.active_profile,
            )
        })
        .collect();
    let profile_count = profiles.len();
    let keyboard_layout = device.features.keyboard.as_ref().map(|k| k.layout.clone());

    v_flex()
        .size_full()
        .gap_2()
        .child(PageHeader::new(
            "设置",
            format!("{device_name} · 更改会立即保存"),
        ))
        .child(ProductBanner::new(device_name.clone()))
        .child(
            body_widgets()
                // 配置文件切换
                .child(widget_card(
                    card()
                        .child(card_title("配置文件切换"))
                        .child(SettingRow::new(
                            "当前配置",
                            active.clone().unwrap_or_else(|| "（无）".to_string()),
                        ))
                        .child(SettingRow::new("配置文件数", format!("{profile_count}")))
                        .child(SettingRow::new(
                            "单配置设备",
                            if single { "是" } else { "否" }.to_string(),
                        ))
                        .when(profile_count > 1, |this| {
                            this.child(stepper_row(
                                "循环切换",
                                active.clone().unwrap_or_default(),
                                cx,
                                |this, cx| this.cycle_profile(-1, cx),
                                |this, cx| this.cycle_profile(1, cx),
                            ))
                        })
                        .when(single, |this| {
                            this.child(div().text_xs().child(
                                "该设备为单配置设备（isSingleProfile），因此没有切换控件。",
                            ))
                        }),
                ))
                // 配置文件列表
                .child(widget_card(
                    card()
                        .child(card_title("配置文件"))
                        .children(profiles.into_iter().enumerate().map(
                            |(index, (name, id, is_active))| {
                                h_flex()
                                    .w_full()
                                    .justify_between()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div().text_sm().child(format!(
                                            "{name}{}",
                                            if id.is_empty() {
                                                String::new()
                                            } else {
                                                format!("（{id}）")
                                            }
                                        )),
                                    )
                                    .child(
                                        // 选中状态常驻可见，而不是只靠 hover。
                                        btn(format!("profile-{index}"), if is_active { "使用中" } else { "切换" })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.select_profile(index, cx)
                                            })),
                                    )
                                    .into_any_element()
                            },
                        ))
                        .child(SettingRow::new(
                            "生效方式",
                            "按 profile 的 guid 切换，而不是索引".to_string(),
                        )),
                ))
                // 按应用切换配置（APP_PROFILE / LINKED_GAMES）
                .child(widget_slot(app_profile_card(app, cx)))
                // 全局亮度（BRIGHTNESS_GLOBAL）
                .child(widget_slot(global_brightness_card(app, cx)))
                // 键盘布局
                .child(widget_card(
                    card()
                        .child(card_title("系统键盘布局"))
                        .when_some(keyboard_layout, |this, layout| {
                            this.child(stepper_row(
                                "键盘布局",
                                layout,
                                cx,
                                |this, cx| this.cycle_keyboard_layout(-1, cx),
                                |this, cx| this.cycle_keyboard_layout(1, cx),
                            ))
                        })
                        .when(
                            device.features.keyboard.is_none(),
                            |this| this.child(div().text_sm().child("该设备不是键盘，没有键盘布局设置")),
                        )
                        .child(div().font_bold().text_sm().child(format!(
                            "可选布局（{}）",
                            KEYBOARD_LAYOUTS.len()
                        )))
                        .children(KEYBOARD_LAYOUTS.iter().map(|(code, label)| {
                            SettingRow::new(*code, *label)
                        }))
                        .child(div().text_xs().child(
                            "雷云原文：GET_SYSTEM_KEYBOARD_LAYOUT —— 读取系统键盘布局。",
                        )),
                ))
                // 迁移
                .child(widget_card(
                    card()
                        .child(card_title("配置文件迁移"))
                        .child(div().text_sm().child(
                            "雷云原文：PROFILE_MIGRATION —— 把旧版雷云的配置迁移到当前版本。",
                        ))
                        .child(SettingRow::new("当前 guid", active_guid))
                        .child(div().text_xs().child(
                            "本实现尚未接入迁移；此页只呈现该功能在雷云里的位置与文案。",
                        )),
                )),
        )
        .into_any_element()
}

/// 按应用切换配置（`APP_PROFILE` / `LINKED_GAMES`）。
///
/// 文案依据：
/// - `PROFILE_SWITCHING_AUTO_DES` / `PROFILE_SWITCHING_MANUAL_DES`：自动 / 手动两种方式
/// - `LINKED_GAMES_TOUR_HEADER`「为每个游戏或应用程序使用特定配置文件和灯光效果」
/// - `ADD_GAME_TITLE`「添加游戏/程序」
/// - `REMOVE_GAME_MESS`（破坏性，提示会一并删除关联的配置文件与幻彩效果）
fn app_profile_card(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let mode = app.profile_switch_mode;
    // 本设备可选的目标配置文件（名称 + guid），供每条关联选择。
    let targets: Vec<(String, String)> = app
        .current()
        .map(|device| {
            device
                .profiles
                .iter()
                .map(|profile| (profile.name.clone(), profile.guid.clone()))
                .collect()
        })
        .unwrap_or_default();
    let (names, guids): (Vec<String>, Vec<String>) = targets.into_iter().unzip();
    let names = std::rc::Rc::new(names);
    let guids = std::rc::Rc::new(guids);

    let games: Vec<(String, bool, String)> = app
        .linked_games
        .iter()
        .map(|entry| {
            // 空 guid 表示「默认」（`GAME_PROFILE_DEFAULT`）；
            // 否则显示配置文件名，找不到就退回显示 guid。
            let target = if entry.profile_guid.is_empty() {
                "默认".to_string()
            } else {
                names
                    .iter()
                    .zip(guids.iter())
                    .find(|(_, guid)| **guid == entry.profile_guid)
                    .map(|(name, _)| name.clone())
                    .unwrap_or_else(|| entry.profile_guid.clone())
            };
            (entry.game.clone(), entry.chroma, target)
        })
        .collect();
    let count = games.len();

    card()
        .child(card_title("应用程序配置文件"))
        // 二选一的枚举 → 真实下拉 `.s3-dropdown`。原先两个回调传的是同一个
        // 循环函数（步进器两边的箭头做同一件事），本身就是错的。
        .child(select_row(
            "profile-switch-mode",
            "配置文件切换",
            mode.zh().to_string(),
            &crate::features::ProfileSwitchMode::LABELS,
            app.open_select.as_deref() == Some("profile-switch-mode"),
            cx,
            |this, index, cx| {
                if let Some(picked) =
                    crate::features::ProfileSwitchMode::ALL.get(index).copied()
                {
                    this.set_profile_switch_mode(picked, cx);
                }
            },
        ))
        .child(div().text_xs().child(crate::i18n::t_or(mode.desc_key(), "")))
        .child(SettingRow::new("已关联的游戏", format!("{count}")))
        .child(SettingRow::new(
            "可选的配置文件",
            format!("{} 个", guids.len()),
        ))
        .when(games.is_empty(), |this| {
            this.child(div().text_sm().child("尚未关联任何游戏或程序"))
        })
        .children(games.into_iter().enumerate().map(|(index, (label, chroma, target))| {
            // 标题、按钮提示、对话框都要用这个名字，各自持有一份。
            let dialog_label = label.clone();
            let status_label = label.clone();
            let guids = guids.clone();
            let names = names.clone();
            v_flex()
                .gap_1()
                .child(SettingRow::new(
                    label,
                    format!(
                        "{} · {}",
                        target,
                        if chroma { "含幻彩效果" } else { "仅配置" }
                    ),
                ))
                .child(
                    h_flex()
                        .gap_2()
                        // 关联到哪个配置文件：在设备的配置文件之间循环。
                        // 没有可选配置文件时置灰，并说明原因。
                        .child(
                            btn(("lg-profile", index), if guids.is_empty() {
                                    "该设备无配置文件".to_string()
                                } else {
                                    format!("关联配置：{target}")
                                })
                                .disabled(guids.is_empty())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    // 空 guid 表示「默认」，然后依次走各配置文件。
                                    let current = this
                                        .linked_games
                                        .get(index)
                                        .map(|entry| entry.profile_guid.clone())
                                        .unwrap_or_default();
                                    let next = match guids.iter().position(|guid| *guid == current) {
                                        // 当前是「默认」→ 选第一个配置文件
                                        None => guids.first().cloned().unwrap_or_default(),
                                        // 已经是最后一个 → 回到「默认」
                                        Some(last) if last + 1 >= guids.len() => String::new(),
                                        Some(now) => guids[now + 1].clone(),
                                    };
                                    let label = names
                                        .iter()
                                        .zip(guids.iter())
                                        .find(|(_, guid)| **guid == next)
                                        .map(|(name, _)| name.clone())
                                        .unwrap_or_else(|| "默认".to_string());
                                    this.set_linked_game_profile(index, next, cx);
                                    this.last_saved =
                                        Some(format!("「{status_label}」已关联到 {label}"));
                                    cx.notify();
                                })),
                        )
                        .child(
                            btn(("lg-chroma", index), if chroma { "幻彩：开" } else { "幻彩：关" })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.toggle_linked_game_chroma(index, cx)
                                })),
                        )
                        // `REMOVE_GAME_MESS` 说明这会删除关联的配置文件与幻彩效果，
                        // 属于破坏性操作，按 《Design Guides · Feedback and overlays》
                        // 用 AlertDialog 并把对象写进标题，而不是「确定吗？」。
                        .child({
                            // `AlertDialog::on_ok` 要求回调返回 `bool`（是否关闭对话框），
                            // 而 `Context::listener` 的回调返回 `()`，因此这里用
                            // `Entity::update` 转发——与 `toggle_button` 同一套路。
                            let entity = cx.entity();
                            AlertDialog::new(cx)
                                .title(format!("从游戏库删除「{dialog_label}」？"))
                                .description(crate::i18n::t_or(
                                    "REMOVE_GAME_MESS",
                                    "你将要从游戏库中删除此游戏。所有关联的设备配置文件和/或 Chroma 幻彩效果也都会被删除。",
                                ))
                                .ok_text("删除")
                                .ok_variant(ButtonVariant::Danger)
                                .cancel_text("取消")
                                .on_ok(move |_, _, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.remove_linked_game(index, cx)
                                    });
                                    true
                                })
                                .trigger(
                                    btn(("lg-del", index), "删除"),
                                )
                        }),
                )
                .into_any_element()
        }))
        .child(
            btn("lg-add", "添加游戏或程序")
                .on_click(cx.listener(|this, _, _, cx| this.add_linked_game(cx))),
        )
        .into_any_element()
}

/// 全局亮度（`BRIGHTNESS_GLOBAL`）。
///
/// 原文：`BRIGHTNESS_GLOBAL_DESC`「一次性调整 Razer Synapse 雷云中所有设备的亮度。」
/// `BRIGHTNESS_GLOBAL_DESC_AT_LEAST_ONE_LED_DEVICE`「此功能需要至少一个支持
/// Razer Synapse 雷云且配有 LED 的设备。」
fn global_brightness_card(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let enabled = app.global_brightness.enabled;
    let level = app.global_brightness.level;
    let led_devices = app
        .devices
        .iter()
        .filter(|device| !device.features.lighting.is_empty())
        .count();

    card()
        .child(card_title("亮度"))
        .child(div().text_xs().child(
            "雷云原文：一次性调整 Razer Synapse 雷云中所有设备的亮度。",
        ))
        .child(toggle_button(
            "global-brightness",
            "一次性调整所有设备的亮度",
            enabled,
            cx,
            |this, cx| this.toggle_global_brightness(cx),
        ))
        .child(stepper_row(
            "全局亮度",
            format!("{level}%"),
            cx,
            |this, cx| this.adjust_global_brightness(-1, cx),
            |this, cx| this.adjust_global_brightness(1, cx),
        ))
        .child(SettingRow::new("配有 LED 的设备", format!("{led_devices} 台")))
        // 少于一台 LED 设备时说明为什么这个开关没有效果。
        .when(led_devices == 0, |this| {
            this.child(div().text_xs().child(
                "此功能需要至少一个支持 Razer Synapse 雷云且配有 LED 的设备。",
            ))
        })
        .into_any_element()
}