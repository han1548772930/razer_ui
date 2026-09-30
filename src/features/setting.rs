//! 设置页（`TAB_SETTING`）。
//!
//! # 布局
//!
//! 这是主前端应用设置，不是设备产品页：`.main-setting` 横向分为约 180px
//! 固定左侧导航和独立滚动的右侧 `.setting-content`。
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
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::shell::AppShell;
use crate::features::KEYBOARD_LAYOUTS;
use crate::ui::widgets::{
    btn, card, card_title, select_row, stepper_row, toggle_button, widget_slot, EmptyState,
    SettingRow,
};

/// 渲染设置页。
pub fn render(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    let device_data = app.current().map(|device| {
        (
            device.display_name(),
            device.active_profile_obj().map(|profile| profile.name.clone()),
            device.active_profile.clone(),
            device.is_single_profile,
            device.profiles
                .iter()
                .map(|profile| {
                    (
                        profile.name.clone(),
                        profile.id.clone(),
                        profile.guid.clone(),
                        profile.guid == device.active_profile,
                    )
                })
                .collect::<Vec<_>>(),
            device.features.keyboard.is_some(),
            device.features.keyboard.as_ref().map(|keyboard| keyboard.layout.clone()),
        )
    });
    let selected = selected_setting(app);
    let device_content = if let Some((device_name, active, active_guid, single, profiles, keyboard, keyboard_layout)) = device_data {
        let profile_count = profiles.len();
        v_flex()
            .gap_3()
            .child(
                card()
                    .child(card_title("配置文件切换"))
                    .child(SettingRow::new(
                        "当前配置",
                        active.clone().unwrap_or_else(|| "（无）".to_string()),
                    ))
                    .child(SettingRow::new("配置文件数", format!("{profile_count}")))
                    .child(SettingRow::new(
                        "单配置设备",
                        if single { "是" } else { "否" },
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
            )
            .child(
                card()
                    .child(card_title("配置文件"))
                    .children(profiles.into_iter().map(
                        |(name, id, profile_guid, is_active)| {
                            let profile_id = profile_guid.clone();
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
                                    btn(
                                        format!("setting-profile-{profile_id}"),
                                        if is_active { "使用中" } else { "切换" },
                                    )
                                    .disabled(is_active)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let index = this.current().and_then(|device| {
                                            device
                                                .profiles
                                                .iter()
                                                .position(|profile| profile.guid == profile_id)
                                        });
                                        if let Some(index) = index {
                                            this.select_profile(index, cx);
                                        }
                                    })),
                                )
                                .into_any_element()
                        },
                    ))
                    .child(SettingRow::new(
                        "生效方式",
                        "按 profile 的 guid 切换，而不是数组顺序",
                    )),
            )
            .child(app_profile_card(app, cx))
            .child(global_brightness_card(app, cx))
            .child(
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
                    .when(!keyboard, |this| {
                        this.child(div().text_sm().child("该设备不是键盘，没有键盘布局设置。"))
                    })
                    .when(keyboard, |this| {
                        this.child(div().font_bold().text_sm().child(format!(
                            "可选布局（{}）",
                            KEYBOARD_LAYOUTS.len()
                        )))
                        .children(KEYBOARD_LAYOUTS.iter().map(|(code, label)| {
                            SettingRow::new(*code, *label)
                        }))
                    }),
            )
            .child(
                card()
                    .child(card_title("配置文件迁移"))
                    .child(div().text_sm().child(
                        "PROFILE_MIGRATION：将旧版雷云配置迁移到当前版本。",
                    ))
                    .child(SettingRow::new("当前设备", device_name))
                    .child(SettingRow::new("当前 guid", active_guid))
                    .child(btn("setting-profile-migrate", "迁移配置").disabled(true))
                    .child(div().text_xs().child(
                        "迁移服务尚未接入，按钮保持禁用，不伪装成已完成操作。",
                    )),
            )
            .into_any_element()
    } else {
        EmptyState::new("设备服务未返回设备。应用设置仍可查看，但设备配置、配置文件和键盘布局暂不可用。")
            .into_any_element()
    };

    v_flex()
        .size_full()
        .min_w(px(600.))
        .child(
            h_flex()
                .size_full()
                .child(setting_nav(selected, cx))
                .child(
                    v_flex()
                        .flex_grow(1.)
                        .min_w(px(420.))
                        .h_full()
                        .gap_3()
                        .p(px(24.))
                        .overflow_y_scrollbar()
                        .child(div().text_xl().font_bold().child("设置"))
                        .child(div().text_sm().text_color(cx.theme().muted_foreground).child(
                            "应用级设置与设备配置分开管理；右侧内容不会改变顶栏导航结构。",
                        ))
                        .child(setting_content(selected, app, cx, device_content)),
                ),
        )
        .into_any_element()
}

fn setting_nav(selected: &'static str, cx: &mut Context<AppShell>) -> AnyElement {
    let active_background = cx.theme().primary;
    let active_text = cx.theme().background;
    let inactive_hover = cx.theme().secondary_hover;
    let groups = [
        ("general", "常规"),
        ("account", "账户"),
        ("devices", "设备与模块"),
        ("profiles", "配置文件"),
        ("notifications", "通知"),
        ("startup", "启动行为"),
        ("updates", "更新"),
        ("logs", "日志"),
        ("about", "关于"),
    ];

    v_flex()
        .flex_shrink_0()
        .w(px(180.))
        .min_w(px(180.))
        .h_full()
        .gap_1()
        .p(px(16.))
        .bg(cx.theme().secondary)
        .child(div().text_lg().font_bold().mb(px(12.)).child("应用设置"))
        .children(groups.into_iter().map(|(key, label)| {
            let active = selected == key;
            let setting_key = key.to_string();
            div()
                .id(ElementId::Name(format!("setting-nav-{key}").into()))
                .w_full()
                .h(px(30.))
                .px(px(10.))
                .mb(px(2.))
                .rounded(px(5.))
                .items_center()
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.set_open_select(
                        Some(format!("__setting_section:{setting_key}").into()),
                        cx,
                    );
                }))
                .when(active, |this| this.bg(active_background).text_color(active_text))
                .when(!active, |this| {
                    this.hover(move |style| style.bg(inactive_hover))
                })
                .child(label)
                .into_any_element()
        }))
        .into_any_element()
}

fn selected_setting(app: &AppShell) -> &'static str {
    match app.open_select.as_deref() {
        Some("__setting_section:account") => "account",
        Some("__setting_section:devices") => "devices",
        Some("__setting_section:profiles") => "profiles",
        Some("__setting_section:notifications") => "notifications",
        Some("__setting_section:startup") => "startup",
        Some("__setting_section:updates") => "updates",
        Some("__setting_section:logs") => "logs",
        Some("__setting_section:about") => "about",
        _ => "general",
    }
}

fn setting_content(
    selected: &'static str,
    app: &AppShell,
    cx: &mut Context<AppShell>,
    device_content: AnyElement,
) -> AnyElement {
    match selected {
        "account" => simple_settings_card("账户", "账户登录服务尚未接入。当前没有可确认的账户状态。"),
        "devices" => simple_settings_card("设备与模块", "设备模块由当前设备能力清单提供；未连接的模块不会显示为可写。"),
        "profiles" => device_content,
        "notifications" => simple_settings_card("通知", "通知服务尚未接入，不显示虚假的推送状态。"),
        "startup" => simple_settings_card("启动行为", "启动项服务尚未接入。"),
        "updates" => simple_settings_card("更新", "更新检查服务尚未接入。"),
        "logs" => simple_settings_card("日志", "日志查看器尚未接入。"),
        "about" => simple_settings_card("关于", "版本与许可信息由构建元数据提供。"),
        _ => app_settings_card(app, cx),
    }
}

fn simple_settings_card(title: &'static str, message: &'static str) -> AnyElement {
    card()
        .child(card_title(title))
        .child(div().text_sm().child(message))
        .child(div().text_xs().child("当前状态为只读，服务接入后才会显示可操作控件。"))
        .into_any_element()
}

fn account_settings_card() -> AnyElement {
    card()
        .child(card_title("账户"))
        .child(SettingRow::new("登录状态", "未连接账户"))
        .child(SettingRow::new("云同步", "不可用"))
        .child(div().text_xs().child(
            "账户服务未接入；不会显示登录、同步或下载按钮来伪装可用功能。",
        ))
        .into_any_element()
}

fn device_settings_card(app: &AppShell) -> AnyElement {
    card()
        .child(card_title("设备与模块"))
        .child(SettingRow::new("已发现设备", format!("{} 台", app.devices.len())))
        .child(SettingRow::new(
            "当前设备",
            app.current()
                .map(|device| device.display_name())
                .unwrap_or_else(|| "无".to_string()),
        ))
        .child(SettingRow::new("设备服务", "本地快照/配置可用"))
        .child(div().text_xs().child(
            "设备安装、卸载和模块下载由设备服务负责；当前页面只展示本地已确认状态。",
        ))
        .into_any_element()
}

fn notification_settings_card() -> AnyElement {
    card()
        .child(card_title("通知"))
        .child(SettingRow::new("设备状态通知", "服务未接入"))
        .child(SettingRow::new("更新通知", "服务未接入"))
        .child(SettingRow::new("错误通知", "由页面状态直接显示"))
        .child(div().text_xs().child(
            "通知服务接入后，开关和通知级别才会在此处出现。",
        ))
        .into_any_element()
}

fn startup_settings_card() -> AnyElement {
    card()
        .child(card_title("启动行为"))
        .child(SettingRow::new("随系统启动", "未配置"))
        .child(SettingRow::new("启动后恢复页面", "未配置"))
        .child(div().text_xs().child(
            "启动项写入需要应用服务确认；当前不提供无效的开关控件。",
        ))
        .into_any_element()
}

fn update_settings_card() -> AnyElement {
    card()
        .child(card_title("更新"))
        .child(SettingRow::new("当前版本", env!("CARGO_PKG_VERSION")))
        .child(SettingRow::new("更新检查", "尚未执行"))
        .child(div().text_xs().child(
            "更新检查服务未接入；页面不会把本地版本号误报为已更新。",
        ))
        .into_any_element()
}

fn log_settings_card() -> AnyElement {
    card()
        .child(card_title("日志"))
        .child(SettingRow::new("日志状态", "由应用进程写入 debug.log"))
        .child(SettingRow::new("导出日志", "服务未接入"))
        .child(div().text_xs().child(
            "日志路径和导出动作需要壳层接口；当前只显示已知日志状态。",
        ))
        .into_any_element()
}

fn about_settings_card() -> AnyElement {
    card()
        .child(card_title("关于"))
        .child(SettingRow::new("应用", "Razer UI"))
        .child(SettingRow::new("版本", env!("CARGO_PKG_VERSION")))
        .child(SettingRow::new("界面基线", "Razer Synapse 主前端"))
        .into_any_element()
}

fn app_settings_card(app: &AppShell, cx: &mut Context<AppShell>) -> AnyElement {
    card()
        .child(card_title("常规"))
        .child(SettingRow::new("语言", "简体中文"))
        .child(SettingRow::new("账户", "未连接账户"))
        .child(SettingRow::new("启动行为", "未配置自动启动"))
        .child(SettingRow::new(
            "通知",
            if app.last_saved.is_some() { "有最近操作反馈" } else { "无新的通知" },
        ))
        .child(div().text_xs().text_color(cx.theme().muted_foreground).child(
            "语言、账户、启动和通知服务需要对应的应用服务；当前仅显示真实可确认的本地状态。",
        ))
        .child(
            h_flex()
                .gap_2()
                .child(btn("setting-check-updates", "检查更新").disabled(true))
                .child(btn("setting-open-logs", "打开日志").disabled(true)),
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

    let games: Vec<(String, bool, String, String)> = app
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
            (
                entry.game.clone(),
                entry.chroma,
                target,
                entry.profile_guid.clone(),
            )
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
        .children(games.into_iter().map(|(label, chroma, target, current_guid)| {
            // 标题、按钮提示、对话框都要用这个名字，各自持有一份。
            let dialog_label = label.clone();
            let status_label = label.clone();
            let game_identity = label.clone();
            let profile_game_identity = game_identity.clone();
            let chroma_game_identity = game_identity.clone();
            let profile_identity = current_guid.clone();
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
                            btn(
                                ElementId::Name(
                                    format!("setting-game-profile-{}", game_identity).into(),
                                ),
                                if guids.is_empty() {
                                    "该设备无配置文件".to_string()
                                } else {
                                    format!("关联配置：{target}")
                                },
                            )
                                .disabled(guids.is_empty())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    // 空 guid 表示「默认」，然后依次走各配置文件。
                                    let current = this
                                        .linked_games
                                        .iter()
                                        .find(|entry| entry.game == profile_game_identity)
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
                                    if let Some(index) =
                                        linked_game_index(this, &profile_game_identity)
                                    {
                                        this.set_linked_game_profile(index, next, cx);
                                        this.last_saved =
                                            Some(format!("「{status_label}」已关联到 {label}"));
                                        cx.notify();
                                    }
                                })),
                        )
                        .child(
                            btn(
                                ElementId::Name(
                                    format!("setting-game-chroma-{}", game_identity).into(),
                                ),
                                if chroma { "幻彩：开" } else { "幻彩：关" },
                            )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(index) =
                                        linked_game_index(this, &chroma_game_identity)
                                    {
                                        this.toggle_linked_game_chroma(index, cx);
                                    }
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
                                        if let Some(index) = linked_game_index(this, &dialog_label) {
                                            this.remove_linked_game(index, cx);
                                        }
                                    });
                                    true
                                })
                                .trigger(btn(
                                    ElementId::Name(
                                        format!("setting-game-remove-{}", profile_identity).into(),
                                    ),
                                    "删除",
                                ))
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

fn linked_game_index(app: &AppShell, game_identity: &str) -> Option<usize> {
    app.linked_games
        .iter()
        .position(|entry| entry.game == game_identity)
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
