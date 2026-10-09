//! Original migration root `OD -> wg -> mg/pg/ct/yg/hg`.
//!
//! The DLL-backed scanner/import queue is not connected. Preview records and
//! callbacks stay inside this entity and never enter device/profile storage.
use gpui_kit::base::{
    Button as BaseButton, Checkbox as BaseCheckbox, CheckboxState, Progress, ProgressIndicator,
    ProgressTrack,
};
use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonRounded, ButtonVariants},
    select::{SelectEvent, SelectState},
    spinner::Spinner,
    tooltip::Tooltip,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n as i18n;
use razer_pages::features::Choice;
use razer_widgets::scroll::SourceScrollable as _;
use razer_widgets::surface;
use razer_widgets::theme::MainPageColors;
use razer_widgets::theme::MigrationColors;
use razer_widgets::theme::PaletteColors;
use razer_widgets::theme::ProfileAlertColors;
use std::collections::BTreeSet;

const SCENARIOS: &[(&str, &str)] = &[
    ("unknown", "扫描状态未读取"),
    ("records", "已检测到配置文件"),
    ("empty", "未找到备份文件"),
    ("scanning", "正在扫描"),
    ("preparing", "准备迁移（可取消）"),
    ("migrating", "正在迁移（不可取消）"),
    ("success", "迁移成功"),
    ("linked-game", "成功，但需重新关联游戏"),
    ("unused-macro", "成功，但需重新绑定宏"),
    ("warnings", "成功，但需重新关联游戏或宏"),
    ("failed", "部分成功，继续后显示失败列表"),
];
const BACKUP_PATH: &str = r"C:\ProgramData\Razer\Razer Synapse3 Data";

fn choices() -> Vec<Choice> {
    SCENARIOS
        .iter()
        .map(|(key, label)| Choice::new(*key, *label))
        .collect()
}

pub fn open_preview(window: &mut Window, cx: &mut App) {
    let page = cx.new(|cx| MigrationPage::new_preview(window, cx));
    window.open_dialog(cx, move |dialog, window, _| {
        dialog
            .title(i18n::t("PROFILE_MIGRATION"))
            .width(
                (window.rem_size() * 80.)
                    .min(window.viewport_size().width - window.rem_size() * 2.),
            )
            .child(page.clone())
    });
}

/// Current Dashboard module 96776: 46x38 toolbar slot, 40x40 icon container,
/// and an unscaled 24x24 SVG background. It is not the app picker's logo.
pub fn header_button(cx: &App) -> Button {
    Button::new("header-profile-migration")
        .ghost()
        .p_0()
        .border_0()
        .rounded(ButtonRounded::None)
        .w(surface::css(46.))
        .h(surface::css(38.))
        .flex_shrink_0()
        .accessibility_label(i18n::t("PROFILE_MIGRATION"))
        .tooltip(i18n::t("PROFILE_MIGRATION"))
        .custom(
            ButtonCustomVariant::new(cx)
                .color(cx.theme().transparent)
                .hover(cx.theme().secondary_hover)
                .active(cx.theme().secondary_hover),
        )
        .child(
            h_flex()
                .size(surface::css(40.))
                .flex_shrink_0()
                .justify_center()
                .child(img("synapse/header-profile-migration.svg").size(surface::css(24.))),
        )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Overlay {
    Scanning,
    Preparing,
    Migrating,
    Success {
        linked_game_warning: bool,
        unused_macro_warning: bool,
        failures: bool,
    },
    FailureDetails,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ItemKey {
    group: &'static str,
    guid: &'static str,
    owner: &'static str,
}

#[derive(Clone)]
struct MigrationItem {
    guid: &'static str,
    owner: &'static str,
    name: &'static str,
    damaged: bool,
    linked_game_failed: bool,
    unused: bool,
    used_macros: &'static [&'static str],
    missing_macros: &'static [&'static str],
}
impl MigrationItem {
    fn new(guid: &'static str, owner: &'static str, name: &'static str) -> Self {
        Self {
            guid,
            owner,
            name,
            damaged: false,
            linked_game_failed: false,
            unused: false,
            used_macros: &[],
            missing_macros: &[],
        }
    }
    fn key(&self, group: &'static str) -> ItemKey {
        ItemKey {
            group,
            guid: self.guid,
            owner: self.owner,
        }
    }
}

#[derive(Clone)]
struct MigrationGroup {
    id: &'static str,
    name: SharedString,
    image: &'static str,
    connected: bool,
    module: bool,
    migrated_date: Option<&'static str>,
    items: Vec<MigrationItem>,
}

fn preview_groups() -> Vec<MigrationGroup> {
    let mut keyboard =
        MigrationItem::new("keyboard-default", "sample-account", "默认配置 · 示例账号");
    keyboard.linked_game_failed = true;
    keyboard.used_macros = &["快速操作", "切换窗口"];
    keyboard.missing_macros = &["快速操作"];
    let mut unused_macro = MigrationItem::new("macro-spare", "sample-account", "备用宏");
    unused_macro.unused = true;
    let mut unused_effect = MigrationItem::new("chroma-spare", "sample-account", "备用灯光效果");
    unused_effect.unused = true;
    let mut damaged = MigrationItem::new("mouse-corrupt", "sample-account", "损坏的配置文件");
    damaged.damaged = true;
    vec![
        MigrationGroup {
            id: "device-653",
            name: "Razer BlackWidow V4 Pro".into(),
            image: "synapse/dashboard-653-0-1.png",
            connected: true,
            module: false,
            migrated_date: Some("2026/09/28"),
            // The source keys by BOTH GUID and account/guest owner.
            items: vec![
                keyboard,
                MigrationItem::new("keyboard-default", "guest", "默认配置 · 访客"),
            ],
        },
        MigrationGroup {
            id: "macro",
            name: i18n::t("MACROS").into(),
            image: "synapse/migration-macro.png",
            connected: true,
            module: true,
            // yg substitutes Date.now() for an existing module history date;
            // this is an explicit, fixed preview of that display branch.
            migrated_date: Some("2026/10/02"),
            items: vec![
                MigrationItem::new("macro-quick", "sample-account", "快速操作"),
                unused_macro,
            ],
        },
        MigrationGroup {
            id: "chroma-app",
            name: i18n::t("CHROMA EFFECTS").into(),
            image: "synapse/migration-chroma.png",
            connected: true,
            module: true,
            migrated_date: None,
            items: vec![
                MigrationItem::new("chroma-wave", "sample-account", "波浪"),
                unused_effect,
            ],
        },
        MigrationGroup {
            id: "device-182",
            name: "Razer DeathAdder V3 Pro".into(),
            image: "synapse/dashboard-182.png",
            connected: false,
            module: false,
            migrated_date: None,
            items: vec![
                MigrationItem::new("mouse-default", "guest", "默认配置"),
                damaged,
            ],
        },
    ]
}

#[derive(Default)]
struct MigrationSelection(BTreeSet<ItemKey>);
impl MigrationSelection {
    fn state(&self, group: &MigrationGroup) -> CheckboxState {
        let count = group
            .items
            .iter()
            .filter(|item| self.0.contains(&item.key(group.id)))
            .count();
        if count == 0 {
            CheckboxState::Unchecked
        } else if count == group.items.len() {
            CheckboxState::Checked
        } else {
            CheckboxState::Indeterminate
        }
    }
    fn group(&mut self, group: &MigrationGroup, checked: bool) {
        for item in &group.items {
            self.item(item.key(group.id), checked);
        }
    }
    fn item(&mut self, key: ItemKey, checked: bool) {
        if checked {
            self.0.insert(key);
        } else {
            self.0.remove(&key);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MigrationApp {
    Synapse,
    Chroma,
}

pub struct MigrationPage {
    app: MigrationApp,
    scenarios: Option<Entity<SelectState<Vec<Choice>>>>,
    scenario: String,
    groups: Vec<MigrationGroup>,
    selected: MigrationSelection,
    expanded: BTreeSet<&'static str>,
    collapsed_sections: BTreeSet<bool>,
    overlay: Option<Overlay>,
    overlay_focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    notice: String,
    _subscriptions: Vec<Subscription>,
}
impl MigrationPage {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self::new_for(MigrationApp::Synapse, cx)
    }
    pub fn new_for(app: MigrationApp, cx: &mut Context<Self>) -> Self {
        // An unread scanner result is not the source's confirmed empty result.
        // The standalone page exposes no test controls or fabricated records.
        Self {
            app,
            scenarios: None,
            scenario: "unknown".into(),
            groups: vec![],
            selected: MigrationSelection::default(),
            expanded: BTreeSet::new(),
            collapsed_sections: BTreeSet::new(),
            overlay: None,
            overlay_focus: cx.focus_handle(),
            return_focus: None,
            notice: String::new(),
            _subscriptions: vec![],
        }
    }
    fn new_preview(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let scenarios =
            cx.new(|cx| SelectState::new(choices(), Some(IndexPath::new(0)), window, cx));
        let subscription = cx.subscribe_in(
            &scenarios,
            window,
            |this: &mut Self, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(key)) = event {
                    this.choose(key, window, cx);
                }
            },
        );
        let mut page = Self::new(cx);
        page.scenarios = Some(scenarios);
        page._subscriptions.push(subscription);
        page
    }
    fn choose(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.close_overlay(window, cx);
        self.scenario = key.into();
        self.groups = if matches!(key, "unknown" | "empty" | "scanning") {
            vec![]
        } else {
            preview_groups()
        };
        self.selected.0.clear();
        self.expanded.clear();
        self.collapsed_sections.clear();
        self.notice.clear();
        // wg auto-selects newly encountered connected devices / installed modules.
        for group in self
            .groups
            .iter()
            .filter(|group| group.connected || key == "failed")
        {
            self.selected.group(group, true);
        }
        let overlay = match key {
            "scanning" => Some(Overlay::Scanning),
            "preparing" => Some(Overlay::Preparing),
            "migrating" => Some(Overlay::Migrating),
            "success" => Some(Overlay::Success {
                linked_game_warning: false,
                unused_macro_warning: false,
                failures: false,
            }),
            "linked-game" => Some(Overlay::Success {
                linked_game_warning: true,
                unused_macro_warning: false,
                failures: false,
            }),
            "unused-macro" => Some(Overlay::Success {
                linked_game_warning: false,
                unused_macro_warning: true,
                failures: false,
            }),
            "warnings" => Some(Overlay::Success {
                linked_game_warning: true,
                unused_macro_warning: true,
                failures: false,
            }),
            "failed" => Some(Overlay::Success {
                linked_game_warning: true,
                unused_macro_warning: true,
                failures: true,
            }),
            _ => None,
        };
        if let Some(overlay) = overlay {
            self.show_overlay(overlay, window, cx);
        }
        cx.notify();
    }
    fn show_overlay(&mut self, overlay: Overlay, window: &mut Window, cx: &mut Context<Self>) {
        if self.overlay.is_none() {
            self.return_focus = window.focused(cx);
        }
        self.overlay = Some(overlay);
        self.overlay_focus.focus(window, cx);
        cx.notify();
    }
    fn close_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.overlay = None;
        if let Some(focus) = self.return_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }
    fn continue_result(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // hg shows failure details only after dismissing the success summary.
        if matches!(self.overlay, Some(Overlay::Success { failures: true, .. })) {
            self.show_overlay(Overlay::FailureDetails, window, cx);
        } else {
            self.close_overlay(window, cx);
        }
    }
    fn banner(&self, cx: &mut Context<Self>) -> AnyElement {
        let selected = !self.selected.0.is_empty();
        v_flex()
            .items_center()
            .p(surface::css(20.))
            .mb(surface::css(20.))
            .rounded(surface::css(5.))
            .bg(MainPageColors.detail_surface())
            .text_center()
            .child(
                div()
                    .font_family("RazerF5")
                    .text_size(surface::css(26.))
                    .line_height(surface::css(31.))
                    .text_color(cx.theme().primary)
                    .mb(surface::css(10.))
                    .child(i18n::t("PROFILE_MIGRATION_HEADER_1").to_uppercase()),
            )
            .child(div().child(i18n::t("PROFILE_MIGRATION_DESC")))
            .when(selected, |view| {
                view.child(
                    h_flex()
                        .justify_center()
                        .flex_wrap()
                        .child(i18n::t("PROFILE_MIGRATION_DESC_2"))
                        .child(
                            Button::new("migration-backup-location")
                                .ghost()
                                .p_0()
                                .ml(surface::css(4.))
                                .h(surface::css(20.))
                                .border_0()
                                .text_size(surface::css(14.))
                                .tooltip(BACKUP_PATH)
                                .child(div().underline().child(i18n::t("LOCATION").to_lowercase()))
                                .child(img("synapse/external-link.svg").size(surface::css(16.)))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.notice = format!("界面预览 · 原版备份位置：{BACKUP_PATH}");
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(
                action("migration-start", i18n::t("MIGRATE_PROFILES"), true, cx)
                    .mt(surface::css(20.))
                    .disabled(!selected)
                    .on_click(cx.listener(|this, _, window, cx| {
                        if !this.selected.0.is_empty() {
                            this.show_overlay(Overlay::Preparing, window, cx);
                        }
                    })),
            )
            .into_any_element()
    }
    fn section(&self, connected: bool, cx: &mut Context<Self>) -> AnyElement {
        let open = !self.collapsed_sections.contains(&connected);
        let name = i18n::t(if connected {
            "CONNECTED_DEVICES_AND_MODULES"
        } else {
            "NON_CONNECTED_DEVICES"
        });
        v_flex()
            .mb(surface::css(20.))
            .child(
                Button::new(if connected {
                    "migration-connected"
                } else {
                    "migration-disconnected"
                })
                .ghost()
                .p_0()
                .h(surface::css(20.))
                .border_0()
                .text_size(surface::css(14.))
                .icon(if open {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .label(name.to_uppercase())
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !this.collapsed_sections.remove(&connected) {
                        this.collapsed_sections.insert(connected);
                    }
                    cx.notify();
                })),
            )
            .when(open, |view| {
                view.child(
                    h_flex()
                        .items_start()
                        .flex_wrap()
                        .gap(surface::css(20.))
                        .pt(surface::css(10.))
                        .children(
                            self.groups
                                .iter()
                                .filter(|g| g.connected == connected)
                                .map(|group| self.card(group, cx)),
                        ),
                )
            })
            .into_any_element()
    }
    fn card(&self, group: &MigrationGroup, cx: &mut Context<Self>) -> AnyElement {
        let id = group.id;
        let expanded = self.expanded.contains(id);
        let state = self.selected.state(group);
        let select_entity = cx.entity().downgrade();
        let checkbox = checkbox(
            format!("migration-all-{id}"),
            state,
            i18n::t("MIGRATE_ALL"),
            cx,
        )
        .on_change(move |state, _, _, cx| {
            let _ = select_entity.update(cx, |this, cx| {
                if let Some(group) = this.groups.iter().find(|g| g.id == id) {
                    this.selected.group(group, state == CheckboxState::Checked);
                }
                cx.notify();
            });
        });
        let title = group.name.clone();
        v_flex()
            .id(SharedString::from(format!("migration-card-{id}")))
            .w(surface::css(290.))
            .max_h(surface::css(440.))
            .flex_shrink_0()
            .px(surface::css(20.))
            .pt(surface::css(20.))
            .pb(surface::css(8.))
            .bg(cx.theme().popover)
            .rounded(surface::css(5.))
            .border_2()
            .border_color(cx.theme().transparent)
            .hover(|style| style.border_color(cx.theme().primary.opacity(0.3)))
            .child(
                div()
                    .id(SharedString::from(format!("migration-name-{id}")))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(14.))
                    .pb(surface::css(5.))
                    .text_color(cx.theme().primary)
                    .text_ellipsis()
                    .tooltip(move |window, cx| Tooltip::new(title.clone()).build(window, cx))
                    .child(group.name.to_uppercase()),
            )
            .child(
                h_flex()
                    .mb(surface::css(16.))
                    .gap(surface::css(4.))
                    .child(source_tooltip(
                        format!("migration-date-{id}"),
                        "synapse/migration-date.svg",
                        i18n::t("LAST_MIGRATED"),
                        16.,
                        cx,
                    ))
                    .child(
                        div()
                            .text_size(surface::css(12.))
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                group
                                    .migrated_date
                                    .map(SharedString::from)
                                    .unwrap_or_else(|| i18n::t("NEVER_MIGRATE").into()),
                            ),
                    ),
            )
            .child(
                img(group.image)
                    .w(surface::css(250.))
                    .h(surface::css(140.))
                    .object_fit(ObjectFit::Cover)
                    .mb(surface::css(20.))
                    .flex_shrink_0(),
            )
            .child(
                h_flex()
                    .gap(surface::css(10.))
                    .items_center()
                    .child(checkbox)
                    .child(
                        BaseButton::new(SharedString::from(format!("migration-expand-{id}")))
                            .accessibility_label(format!(
                                "{} {}",
                                if expanded { "收起" } else { "展开" },
                                group.name
                            ))
                            .flex()
                            .items_center()
                            .h(surface::css(20.))
                            .gap(surface::css(4.))
                            .text_color(if state == CheckboxState::Unchecked {
                                cx.theme().muted_foreground
                            } else {
                                cx.theme().foreground
                            })
                            .hover(|style| style.text_color(cx.theme().primary))
                            .child(i18n::t("MIGRATE_ALL"))
                            .child(
                                Icon::new(if expanded {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                })
                                .size(surface::css(14.)),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !this.expanded.remove(id) {
                                    this.expanded.insert(id);
                                }
                                cx.notify();
                            })),
                    ),
            )
            .when(expanded, |view| {
                view.child(
                    v_flex()
                        .id(SharedString::from(format!("migration-items-{id}")))
                        .max_h(surface::css(160.))
                        .w(surface::css(250.))
                        .pl(surface::css(28.))
                        .when(group.module, |items| items.mt(surface::css(10.)))
                        .scrollable_y()
                        .children(group.items.iter().map(|item| self.item(group, item, cx))),
                )
            })
            .into_any_element()
    }
    fn item(
        &self,
        group: &MigrationGroup,
        item: &MigrationItem,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = item.key(group.id);
        let selected = self.selected.0.contains(&key);
        let id = format!("{}-{}-{}", key.group, key.guid, key.owner);
        let target = cx.entity().downgrade();
        let check = checkbox(
            format!("migration-item-{id}"),
            if selected {
                CheckboxState::Checked
            } else {
                CheckboxState::Unchecked
            },
            item.name,
            cx,
        )
        .on_change(move |state, _, _, cx| {
            let _ = target.update(cx, |this, cx| {
                this.selected.item(key, state == CheckboxState::Checked);
                cx.notify();
            });
        });
        let label = BaseButton::new(SharedString::from(format!("migration-item-name-{id}")))
            .accessibility_label(item.name)
            .text_size(surface::css(14.))
            .h(surface::css(20.))
            .text_color(if selected {
                cx.theme().foreground
            } else {
                cx.theme().muted_foreground
            })
            .child(item.name)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selected.item(key, !this.selected.0.contains(&key));
                cx.notify();
            }));
        v_flex()
            .child(
                h_flex()
                    .items_start()
                    .gap(surface::css(10.))
                    .child(check)
                    .child(
                        v_flex()
                            .min_w_0()
                            .child(
                                h_flex()
                                    .child(label)
                                    .when(item.linked_game_failed, |row| {
                                        row.child(source_tooltip(
                                            format!("migration-game-{id}"),
                                            "synapse/migration-game-warning.svg",
                                            i18n::t("RELINK_A_LINKED_GAME_PROFILE"),
                                            20.,
                                            cx,
                                        ))
                                    })
                                    .when(item.damaged && !item.linked_game_failed, |row| {
                                        row.child(source_tooltip(
                                            format!("migration-damaged-{id}"),
                                            "synapse/migration-file-warning.svg",
                                            i18n::t("FILES_MIGHT_BE_CORRUPTED"),
                                            20.,
                                            cx,
                                        ))
                                    })
                                    .when(item.unused && group.module, |row| {
                                        row.child(source_tooltip(
                                            format!("migration-unused-{id}"),
                                            "synapse/migration-unused.svg",
                                            if group.id == "macro" {
                                                i18n::t("MACRO_NOT_IN_USED")
                                            } else {
                                                i18n::t("Chroma effect not in used")
                                            },
                                            20.,
                                            cx,
                                        ))
                                    })
                                    .when(!item.missing_macros.is_empty(), |row| {
                                        row.child(source_tooltip(
                                            format!("migration-missing-{id}"),
                                            "synapse/migration-macro-warning.svg",
                                            format!(
                                                "{}\n{}",
                                                i18n::t("UNUSED_MACROS_DESC"),
                                                item.missing_macros.join("\n")
                                            ),
                                            20.,
                                            cx,
                                        ))
                                    }),
                            )
                            .when(!item.used_macros.is_empty(), |column| {
                                column.child(
                                    v_flex()
                                        .border_l_1()
                                        .border_color(cx.theme().muted_foreground)
                                        .my(surface::css(5.))
                                        .children(item.used_macros.iter().enumerate().map(
                                            |(index, name)| {
                                                h_flex()
                                                    .my(surface::css(6.))
                                                    .text_color(if selected {
                                                        cx.theme().foreground
                                                    } else {
                                                        cx.theme().muted_foreground
                                                    })
                                                    .when(index == 0, |row| {
                                                        row.child(
                                                            img("synapse/migration-macro-icon.svg")
                                                                .size(surface::css(20.))
                                                                .mx(surface::css(10.))
                                                                .flex_shrink_0(),
                                                        )
                                                    })
                                                    .when(index != 0, |row| {
                                                        row.pl(surface::css(40.))
                                                    })
                                                    .child(*name)
                                            },
                                        )),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }
    fn empty(&self, cx: &App) -> AnyElement {
        v_flex()
            .w(surface::css(600.))
            .max_w_full()
            .px(surface::css(40.))
            .py(surface::css(30.))
            .bg(cx.theme().popover)
            .rounded(surface::css(5.))
            .child(
                div()
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .text_color(cx.theme().primary)
                    .mb(surface::css(20.))
                    .child(i18n::t("SYNAPSE_3_PROFILES_NOT_FOUND_HEADER").to_uppercase()),
            )
            .child(
                div()
                    .mb(surface::css(20.))
                    .child(i18n::t("SYNAPSE_3_PROFILES_NOT_FOUND_DESC")),
            )
            .child(v_flex().mb(surface::css(20.)).children((1..=4).map(|step| {
                div().child(i18n::t(&format!(
                    "SYNAPSE_3_PROFILES_NOT_FOUND_STEP_{step}"
                )))
            })))
            .into_any_element()
    }
    fn overlay(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(phase) = self.overlay else {
            return div().into_any_element();
        };
        let popup = v_flex()
            .id("migration-progress-popup")
            .occlude()
            .w((window.rem_size()
                * if phase == Overlay::Scanning {
                    18.75
                } else {
                    25.
                })
            .min(window.viewport_size().width - window.rem_size() * 2.))
            .max_h(window.viewport_size().height - window.rem_size() * 5.)
            .scrollable_y()
            .px(surface::css(20.))
            .py(surface::css(24.))
            .bg(cx.theme().popover)
            .border_1()
            .border_color(if phase == Overlay::FailureDetails {
                ProfileAlertColors::new().danger()
            } else {
                cx.theme().primary
            })
            .rounded(surface::css(3.))
            .text_size(surface::css(14.))
            .line_height(surface::css(20.))
            .text_color(cx.theme().foreground)
            .child(self.overlay_body(phase, cx));
        gpui_kit::base::Dialog::new(cx)
            .layer(1, true)
            .focus_handle(self.overlay_focus.clone())
            .close_on_backdrop_press(false)
            .on_ok(|_, _, _| false)
            // Escape dismisses this explicit preview; it is not a real cancellation.
            .on_close(cx.listener(|this, _, window, cx| this.close_overlay(window, cx)))
            .backdrop(
                div()
                    .absolute()
                    .inset_0()
                    .bg(cx.theme().title_bar.opacity(0.7)),
            )
            .popup(
                div()
                    .absolute()
                    .inset_0()
                    .w(window.viewport_size().width)
                    .h(window.viewport_size().height)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(popup)
                    .child(
                        div()
                            .absolute()
                            .top(surface::css(14.))
                            .left_0()
                            .w_full()
                            .text_center()
                            .text_size(surface::css(12.))
                            .text_color(PaletteColors.white())
                            .child("界面预览 · 示例状态；按 Esc 返回"),
                    ),
            )
            .into_any_element()
    }
    fn overlay_body(&self, phase: Overlay, cx: &mut Context<Self>) -> AnyElement {
        match phase {
            Overlay::Scanning => v_flex()
                .items_center()
                .child(
                    div()
                        .mb(surface::css(8.))
                        .child(format!("{}...", i18n::t("SCANNING_PROFILES"))),
                )
                .child(Spinner::new().with_size(cx.theme().font_size * (20. / 16.)))
                .into_any_element(),
            Overlay::Preparing | Overlay::Migrating => {
                let progress = if phase == Overlay::Preparing { 0. } else { 42. };
                v_flex()
                    .child(
                        div()
                            .mb(surface::css(8.))
                            .child(format!("{}...", i18n::t("MIGRATING_PROFILES"))),
                    )
                    .child(
                        Progress::new("migration-progress")
                            .value(progress)
                            .accessibility_label(i18n::t("MIGRATING_PROFILES"))
                            .child(
                                ProgressTrack::new()
                                    .w_full()
                                    .h(surface::css(5.))
                                    .rounded(surface::css(5.))
                                    .bg(cx.theme().primary.opacity(0.3))
                                    .child(
                                        ProgressIndicator::new()
                                            .h_full()
                                            .w(relative(progress / 100.))
                                            .rounded(surface::css(5.))
                                            .bg(cx.theme().primary),
                                    ),
                            ),
                    )
                    .child(
                        h_flex().justify_center().mt(surface::css(16.)).child(
                            action("migration-cancel", i18n::t("CANCEL"), false, cx)
                                .disabled(phase == Overlay::Migrating)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if this.overlay == Some(Overlay::Preparing) {
                                        this.notice = "界面预览 · 已取消准备迁移".into();
                                        this.close_overlay(window, cx);
                                    }
                                })),
                        ),
                    )
                    .into_any_element()
            }
            Overlay::Success {
                linked_game_warning,
                unused_macro_warning,
                ..
            } => v_flex()
                .items_center()
                .child(
                    img("synapse/migration-tick.svg")
                        .size(surface::css(48.))
                        .mb(surface::css(10.)),
                )
                .child(
                    div()
                        .text_center()
                        .mb(surface::css(16.))
                        .child(i18n::t("ALL_MIGRATED_PROFILES_SUCCESSFULLY")),
                )
                .when(linked_game_warning, |column| {
                    column.child(
                        div()
                            .text_center()
                            .mb(surface::css(16.))
                            .text_color(MainPageColors.tutorial_accent())
                            .child(i18n::t("RELINK_A_LINKED_GAME_PROFILE")),
                    )
                })
                .when(unused_macro_warning, |column| {
                    column.child(
                        div()
                            .text_center()
                            .mb(surface::css(16.))
                            .text_color(MainPageColors.tutorial_accent())
                            .child(i18n::t("UNUSED_MACROS_DESC")),
                    )
                })
                .child(
                    action(
                        "migration-continue",
                        i18n::t("BUTTON_TEXT_CONTINUE"),
                        true,
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.continue_result(window, cx))),
                )
                .into_any_element(),
            Overlay::FailureDetails => v_flex()
                .child(
                    div()
                        .text_center()
                        .text_size(surface::css(16.))
                        .text_color(ProfileAlertColors::new().danger())
                        .mb(surface::css(12.))
                        .child(i18n::t("ERROR")),
                )
                .child(
                    div()
                        .mb(surface::css(8.))
                        .child(i18n::t("ERROR_PROFILE_MIGRATION_DESC")),
                )
                // Lt shows product names and raw module IDs from failed groups.
                .child(
                    v_flex()
                        .pl(surface::css(20.))
                        .mb(surface::css(16.))
                        .child("• Razer DeathAdder V3 Pro")
                        .child("• macro"),
                )
                .child(h_flex().justify_center().child(
                    action("migration-error-ok", i18n::t("OK"), true, cx).on_click(
                        cx.listener(|this, _, window, cx| this.close_overlay(window, cx)),
                    ),
                ))
                .into_any_element(),
        }
    }
}

fn action(id: &'static str, label: impl Into<SharedString>, primary: bool, cx: &App) -> Button {
    let label: SharedString = label.into();
    Button::new(id)
        .label(label.to_uppercase())
        .min_w(surface::css(100.))
        .h(surface::css(28.))
        .px(surface::css(16.))
        .py_0()
        .rounded(cx.theme().font_size * (3. / 16.))
        .text_size(surface::css(12.))
        .line_height(surface::css(14.))
        .border_1()
        .border_color(PaletteColors.swatch_border())
        .custom(
            ButtonCustomVariant::new(cx)
                .color(if primary {
                    cx.theme().primary
                } else {
                    PaletteColors.secondary()
                })
                .foreground(if primary {
                    cx.theme().title_bar
                } else {
                    PaletteColors.white()
                })
                .hover(if primary {
                    cx.theme().primary.opacity(0.8)
                } else {
                    PaletteColors.secondary().opacity(0.8)
                }),
        )
}

fn checkbox(
    id: String,
    state: CheckboxState,
    label: impl Into<SharedString>,
    cx: &App,
) -> BaseCheckbox {
    BaseCheckbox::new(SharedString::from(id))
        .state(state)
        .accessibility_label(label)
        .size(surface::css(20.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(if state == CheckboxState::Checked {
            cx.theme().primary
        } else {
            MigrationColors::checkbox_border()
        })
        .rounded(surface::css(2.4))
        .hover(|style| style.border_color(cx.theme().primary))
        .focus_visible(|style| style.border_color(PaletteColors.white()))
        .when(state == CheckboxState::Checked, |check| {
            check.bg(cx.theme().primary).child(
                Icon::new(IconName::Check)
                    .size(surface::css(18.))
                    .text_color(cx.theme().popover),
            )
        })
        .when(state == CheckboxState::Indeterminate, |check| {
            check.child(
                div()
                    .w(surface::css(10.))
                    .h(surface::css(3.))
                    .bg(cx.theme().primary),
            )
        })
}

fn source_tooltip(
    id: String,
    asset: &'static str,
    text: impl Into<SharedString>,
    size: f32,
    cx: &App,
) -> BaseButton {
    let text = text.into();
    let border = PaletteColors.picker_border();
    let background = cx.theme().popover;
    BaseButton::new(SharedString::from(id))
        .accessibility_label(text.clone())
        .size(surface::css(size))
        .flex_shrink_0()
        .ml(surface::css(4.))
        .child(img(asset).size_full())
        .tooltip(move |window, cx| {
            let text = text.clone();
            Tooltip::element(move |_, _| {
                div()
                    .max_w(surface::css(320.))
                    .px(surface::css(10.))
                    .py(surface::css(8.))
                    .border_1()
                    .border_color(border)
                    .bg(background)
                    .text_size(surface::css(14.))
                    .child(text.clone())
            })
            .build(window, cx)
        })
}

impl Render for MigrationPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let preview = self.scenarios.is_some();
        v_flex()
            .id(match self.app {
                MigrationApp::Synapse => "synapse-profile-migration-root",
                MigrationApp::Chroma => "chroma-app-profile-migration-root",
            })
            .when(self.app == MigrationApp::Chroma && !preview, |view| {
                view.child(
                    h_flex()
                        .h(surface::css(40.))
                        .flex_shrink_0()
                        .justify_center()
                        .bg(razer_widgets::theme::ChromaSettingsColors::background())
                        .text_color(razer_widgets::theme::ChromaSettingsColors::secondary())
                        .child(i18n::t("PROFILE_MIGRATION").to_uppercase()),
                )
            })
            .when(!preview, |view| view.size_full())
            .when(preview, |view| view.gap(surface::css(12.)))
            .text_size(surface::css(14.))
            .line_height(surface::css(20.))
            .when_some(self.scenarios.as_ref(), |view, scenarios| {
                view.child(
                    h_flex()
                        .gap(surface::css(12.))
                        .flex_wrap()
                        .child(
                            div()
                                .text_color(if self.scenario != "unknown" {
                                    cx.theme().primary
                                } else {
                                    cx.theme().muted_foreground
                                })
                                .child(if self.scenario != "unknown" {
                                    "界面预览 · 示例数据"
                                } else {
                                    "迁移服务尚未接入，扫描状态未读取"
                                }),
                        )
                        .child(
                            surface::select(scenarios)
                                .items(choices())
                                .w(surface::css(300.))
                                .accessibility_label("配置迁移界面预览状态"),
                        ),
                )
            })
            .child(
                v_flex()
                    .id("migration-page-scroll")
                    .scrollable_y()
                    .when(preview, |view| {
                        view.max_h(
                            (window.viewport_size().height - window.rem_size() * 13.)
                                .max(window.rem_size() * 12.),
                        )
                    })
                    .when(!preview, |view| {
                        view.flex_1()
                            .min_h_0()
                            .px(surface::css(20.))
                            .pb(surface::css(20.))
                    })
                    .pt(surface::css(if preview { 10. } else { 20. }))
                    .bg(cx.theme().background)
                    .child(self.banner(cx))
                    .when(!self.groups.is_empty(), |column| {
                        column
                            .child(self.section(true, cx))
                            .child(self.section(false, cx))
                    })
                    .when(self.scenario == "empty", |column| {
                        column.child(h_flex().justify_center().child(self.empty(cx)))
                    })
                    .when(preview && self.scenario == "unknown", |column| {
                        column.child(
                            div()
                                .p(surface::css(30.))
                                .text_center()
                                .text_color(cx.theme().muted_foreground)
                                .child(
                                    "尚未扫描 Synapse 3 备份。可选择上方界面预览查看原版各状态。",
                                ),
                        )
                    })
                    .when(!self.notice.is_empty(), |column| {
                        column.child(
                            div()
                                .p(surface::css(12.))
                                .text_color(cx.theme().muted_foreground)
                                .child(self.notice.clone()),
                        )
                    }),
            )
            .child(self.overlay(window, cx))
    }
}
