//! Local profile commands. The source bundles share naming, inline rename and
//! delete rules; only the workspace store is written, never the device service.
use super::*;
use crate::ui::theme::{DropdownColors, ProfileAlertColors};
use gpui_kit::base::{Button as BaseButton, Popover, PopoverState};
use gpui_kit::component::{
    button::ButtonCustomVariant,
    input::{Escape, Input},
    list::{List, ListDelegate, ListState},
    tooltip::Tooltip,
};
use gpui_kit::prelude::FluentBuilder as _;

#[path = "linked_games.rs"]
mod linked_games;
#[path = "profile_transfer.rs"]
mod profile_transfer;

#[derive(Clone)]
struct ProfileTarget {
    workspace: WeakEntity<DeviceWorkspace>,
    device: String,
    profile: String,
}
impl ProfileTarget {
    fn new(workspace: &DeviceWorkspace, cx: &Context<DeviceWorkspace>) -> Self {
        Self {
            workspace: cx.entity().downgrade(),
            device: workspace.identity(),
            profile: workspace.device.active_profile.clone(),
        }
    }
    fn current(&self, cx: &App) -> Option<Entity<DeviceWorkspace>> {
        let workspace = self.workspace.upgrade()?;
        let value = workspace.read(cx);
        (value.identity() == self.device && value.device.active_profile == self.profile)
            .then_some(workspace)
    }
}

fn default_profile_name(profiles: &[Profile], computer_name: &str) -> String {
    let base = if computer_name.is_empty() {
        "Default".to_string()
    } else {
        format!("{computer_name}-Default")
    };
    if !profiles.iter().any(|profile| profile.name == base) {
        return base;
    }
    (1..)
        .map(|index| format!("{base} {index}"))
        .find(|name| !profiles.iter().any(|profile| profile.name == *name))
        .expect("an unused profile name")
}

fn duplicate_profile_name(profiles: &[Profile], name: &str) -> String {
    // Source helper d9 removes an existing trailing numeric copy suffix first.
    let base = name
        .strip_suffix(')')
        .and_then(|name| name.rsplit_once(" ("))
        .filter(|(_, suffix)| !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit()))
        .map(|(base, _)| base.trim_end())
        .unwrap_or(name);
    if !profiles.iter().any(|profile| profile.name == base) {
        return base.to_string();
    }
    (1..)
        .map(|index| format!("{base} ({index})"))
        .find(|name| !profiles.iter().any(|profile| profile.name == *name))
        .expect("an unused duplicate name")
}

fn next_profile_id(device: &Device, saved: &Device) -> String {
    // Reserve saved IDs too: deleting an unsaved copy then creating another
    // must not accidentally reuse the identity to which Discard returns.
    (1..)
        .map(|index| format!("local-profile-{index}"))
        .find(|id| {
            !device
                .profiles
                .iter()
                .chain(&saved.profiles)
                .any(|p| p.id == *id || p.guid == *id)
        })
        .expect("an unused local identity")
}

pub(super) fn create_local_profile(device: &mut Device, saved: &Device, duplicate: bool) {
    let id = next_profile_id(device, saved);
    let mut profile = if duplicate {
        let Some(mut profile) = device
            .profiles
            .iter()
            .find(|p| p.id == device.active_profile)
            .cloned()
        else {
            return;
        };
        profile.name = duplicate_profile_name(&device.profiles, &profile.name);
        profile
    } else {
        let mut settings = ProfileSettings::default();
        settings.normalize(device.product_id);
        Profile {
            id: String::new(),
            guid: String::new(),
            name: default_profile_name(
                &device.profiles,
                &std::env::var("COMPUTERNAME").unwrap_or_default(),
            ),
            dpi_stages: None,
            settings: Some(settings),
        }
    };
    profile.id = id.clone();
    profile.guid = id.clone();
    device.profiles.push(profile);
    device.active_profile = id;
}

fn rename_local_profile(device: &mut Device, id: &str, name: &str) -> bool {
    let name = name.trim();
    if name.is_empty()
        || name.encode_utf16().count() > 32
        || device
            .profiles
            .iter()
            .any(|profile| profile.name == name && profile.id != id)
    {
        return false;
    }
    let Some(profile) = device.profiles.iter_mut().find(|profile| profile.id == id) else {
        return false;
    };
    if profile.name == name {
        return false;
    }
    profile.name = name.to_string();
    true
}

pub(super) fn delete_local_profile(device: &mut Device, id: &str) -> bool {
    if device.profiles.len() <= 1 {
        return false;
    }
    let Some(index) = device.profiles.iter().position(|profile| profile.id == id) else {
        return false;
    };
    device.profiles.remove(index);
    if device.active_profile == id {
        // Source reducer chooses the first remaining profile for 182/653/777.
        device.active_profile = device.profiles[0].id.clone();
    }
    true
}

pub(super) fn reset_local_profile(device: &mut Device, id: &str, bindings_only: bool) -> bool {
    let Some(profile) = device.profiles.iter_mut().find(|profile| profile.id == id) else {
        return false;
    };
    if bindings_only {
        if let Some(settings) = profile.settings.as_mut() {
            settings.bindings.clear();
            settings.hypershift_bindings.clear();
        }
    } else {
        let mut settings = ProfileSettings::default();
        settings.normalize(device.product_id);
        profile.settings = Some(settings);
        profile.dpi_stages = None;
    }
    true
}

#[derive(Clone, Copy)]
enum ProfileCommand {
    Add,
    Import,
    LinkedGames,
    Rename,
    Duplicate,
    Export,
    Reset,
    Delete,
}

#[derive(Clone)]
pub(super) struct ProfileConfirmation {
    id: String,
    name: String,
    reset: bool,
}
impl ProfileCommand {
    fn id(self) -> &'static str {
        match self {
            Self::Add => "profile-add",
            Self::Import => "profile-import",
            Self::LinkedGames => "profile-linked-games",
            Self::Rename => "profile-rename",
            Self::Duplicate => "profile-duplicate",
            Self::Export => "profile-export",
            Self::Reset => "profile-reset",
            Self::Delete => "profile-delete",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Add => "添加",
            Self::Import => "导入",
            Self::LinkedGames => "关联游戏",
            Self::Rename => "重命名",
            Self::Duplicate => "复制",
            Self::Export => "导出",
            Self::Reset => "重置",
            Self::Delete => "删除",
        }
    }
    fn disabled_reason(self, device: &Device) -> Option<&'static str> {
        match self {
            Self::Delete if device.profiles.len() <= 1 => Some("至少保留一个配置文件"),
            _ => None,
        }
    }
}

pub(super) struct ProfileCommands {
    sections: Vec<Vec<ProfileCommand>>,
    cursor: Option<IndexPath>,
    workspace: WeakEntity<DeviceWorkspace>,
    popup: Option<WeakEntity<PopoverState>>,
}

pub(super) fn command_list(
    pid: u32,
    workspace: WeakEntity<DeviceWorkspace>,
    window: &mut Window,
    cx: &mut Context<DeviceWorkspace>,
) -> Entity<ListState<ProfileCommands>> {
    use ProfileCommand::*;
    let mut last = Vec::new();
    if [182, 653, crate::demo::DEMO_PRODUCT_ID].contains(&pid) {
        last.push(Reset);
    }
    last.push(Delete);
    cx.new(|cx| {
        ListState::new(
            ProfileCommands {
                sections: vec![
                    vec![Add, Import],
                    vec![LinkedGames],
                    vec![Rename, Duplicate, Export],
                    last,
                ],
                cursor: None,
                workspace,
                popup: None,
            },
            window,
            cx,
        )
    })
}

impl ListDelegate for ProfileCommands {
    type Item = ProfileCommandRow;

    fn sections_count(&self, _: &App) -> usize {
        self.sections.len()
    }
    fn items_count(&self, section: usize, _: &App) -> usize {
        self.sections[section].len()
    }
    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
        self.cursor = ix;
    }
    fn render_item(
        &mut self,
        ix: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let command = *self.sections.get(ix.section)?.get(ix.row)?;
        let workspace = self.workspace.upgrade()?;
        Some(ProfileCommandRow {
            command,
            highlighted: false,
            disabled_reason: command.disabled_reason(&workspace.read(cx).device),
        })
    }
    fn render_section_footer(
        &mut self,
        _: usize,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<impl IntoElement> {
        Some(
            div()
                .h(surface::css(9.))
                .py(surface::css(4.))
                .px(surface::css(6.))
                .child(div().h(surface::css(1.)).bg(cx.theme().border)),
        )
    }
    fn confirm(&mut self, _: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        let Some(command) = self
            .cursor
            .and_then(|ix| self.sections.get(ix.section)?.get(ix.row))
            .copied()
        else {
            return;
        };
        let Some(workspace) = self.workspace.upgrade() else {
            return;
        };
        if command
            .disabled_reason(&workspace.read(cx).device)
            .is_some()
        {
            return;
        }
        let popup = self.popup.clone();
        // Dismiss first, returning focus before a command opens its own editor.
        // Deferral releases List's mutable borrow before the workspace updates.
        window.defer(cx, move |window, cx| {
            if !matches!(command, ProfileCommand::Delete | ProfileCommand::Reset) {
                if let Some(popup) = popup {
                    _ = popup.update(cx, |popup, cx| popup.dismiss(window, cx));
                }
            }
            workspace.update(cx, |workspace, cx| {
                workspace.run_profile_command(command, window, cx)
            });
        });
    }
}

#[derive(IntoElement)]
pub(super) struct ProfileCommandRow {
    command: ProfileCommand,
    highlighted: bool,
    disabled_reason: Option<&'static str>,
}
impl Selectable for ProfileCommandRow {
    fn selected(mut self, selected: bool) -> Self {
        self.highlighted = selected;
        self
    }
    fn is_selected(&self) -> bool {
        self.highlighted
    }
}
impl RenderOnce for ProfileCommandRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = DropdownColors::new();
        BaseButton::new(self.command.id())
            .disabled(self.disabled_reason.is_some())
            .focusable(false)
            .tab_stop(false)
            .role(Role::MenuItem)
            .accessibility_label(self.command.label())
            .justify_start()
            .w_full()
            .h(surface::css(27.))
            .px(surface::css(6.))
            .py(surface::css(5.))
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_color(cx.theme().foreground)
            .when(self.disabled_reason.is_none(), |row| {
                row.when(self.highlighted, |row| row.bg(colors.hover()))
                    .hover(|row| row.bg(colors.hover()))
            })
            .when_some(self.disabled_reason, |row, reason| {
                row.text_color(cx.theme().foreground.opacity(0.3))
                    .aria_description(reason)
                    .tooltip(move |window, cx| Tooltip::new(reason).build(window, cx))
            })
            .child(self.command.label())
    }
}

impl DeviceWorkspace {
    pub(super) fn refresh_profile_choices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let choices = self
            .device
            .profiles
            .iter()
            .map(|p| Choice::new(&p.id, p.name.clone()))
            .collect();
        self.controls
            .profile
            .update(cx, |state, cx| state.set_items(choices, window, cx));
    }

    pub(super) fn install_profile_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.subscribe_in(
            &self.profile_name,
            window,
            |this, _, event, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    this.finish_profile_rename(window, cx);
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.controls
                            .profile
                            .update(cx, |state, cx| state.focus(window, cx));
                    }
                }
            },
        ));
    }

    fn begin_profile_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(profile) = self.device.active_profile_obj() else {
            return;
        };
        let name = profile.name.clone();
        self.profile_rename = Some(profile.id.clone());
        self.profile_name.update(cx, |input, cx| {
            input.set_value(name, window, cx);
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        cx.notify();
    }

    pub(super) fn finish_profile_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.profile_rename.take() else {
            return;
        };
        let name = self.profile_name.read(cx).value();
        if rename_local_profile(&mut self.device, &id, name.as_ref()) {
            self.refresh_profile_choices(window, cx);
            self.sync_controls(window, cx);
            self.changed(cx);
        }
        cx.notify();
    }

    fn run_profile_command(
        &mut self,
        command: ProfileCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if command.disabled_reason(&self.device).is_some() {
            return;
        }
        match command {
            ProfileCommand::Add => self.add_profile(window, cx),
            ProfileCommand::Duplicate => self.continue_with(Continue::DuplicateProfile, window, cx),
            ProfileCommand::Rename => self.begin_profile_rename(window, cx),
            ProfileCommand::Delete | ProfileCommand::Reset => {
                if let Some(profile) = self.device.active_profile_obj() {
                    self.profile_confirmation = Some(ProfileConfirmation {
                        id: profile.id.clone(),
                        name: profile.name.clone(),
                        reset: matches!(command, ProfileCommand::Reset),
                    });
                    self.profile_confirm_focus.focus(window, cx);
                    cx.notify();
                }
            }
            ProfileCommand::Import => self.open_profile_import(window, cx),
            ProfileCommand::Export => self.continue_with(Continue::ExportProfile, window, cx),
            ProfileCommand::LinkedGames => self.open_linked_games(window, cx),
        }
    }

    pub(super) fn profile_toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        // 653 zh.renderProfileBar enables OBM outside BLE. The other audited
        // product roots explicitly disable it; no slot contents are inferred.
        let has_obm = self.pid() == 653 && !self.device.use_ble;
        let list = self.profile_menu.clone();
        let confirmation = self.profile_confirmation.clone();
        let focus = if confirmation.is_some() {
            self.profile_confirm_focus.clone()
        } else {
            list.focus_handle(cx)
        };
        let confirmation_focus = self.profile_confirm_focus.clone();
        let workspace = cx.entity().downgrade();
        let open_workspace = workspace.clone();
        let menu_height = self
            .profile_menu
            .read(cx)
            .delegate()
            .sections
            .iter()
            .map(|section| section.len() as f32 * 27. + 9.)
            .sum::<f32>();
        let opened_list = list.clone();
        h_flex()
            .id("profile-bar")
            .test_support()
            .h(surface::css(27.))
            .w_full()
            .min_w_0()
            .max_w(surface::css(if has_obm { 354. } else { 322. }))
            .child(
                div()
                    .size(surface::css(26.))
                    .flex_shrink_0()
                    .ml(surface::css(10.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(img("synapse/profile.svg").size(surface::css(20.))),
            )
            .child(
                div()
                    .w(surface::css(250.))
                    .min_w_0()
                    .px(surface::css(10.))
                    .child(if self.profile_rename.is_some() {
                        div()
                            .id("profile-name-editor")
                            .test_support()
                            .w_full()
                            .min_w_0()
                            .on_action(cx.listener(|this, _: &Escape, window, cx| {
                                this.profile_rename = None;
                                this.controls
                                    .profile
                                    .update(cx, |state, cx| state.focus(window, cx));
                                cx.notify();
                            }))
                            .child(
                                Input::new(&self.profile_name)
                                    .id("profile-name-input")
                                    .aria_label("配置文件名称")
                                    .h(surface::css(27.))
                                    .w_full()
                                    .min_w_0()
                                    .rounded_none()
                                    .focus_bordered(false)
                                    .border_1()
                                    .border_color(cx.theme().primary)
                                    .bg(cx.theme().group_box)
                                    .px(surface::css(5.))
                                    .py(surface::css(5.))
                                    .text_size(surface::css(14.))
                                    .line_height(surface::css(17.)),
                            )
                            .into_any_element()
                    } else {
                        surface::select(&self.controls.profile)
                            .items(
                                self.device
                                    .profiles
                                    .iter()
                                    .map(|p| Choice::new(&p.id, p.name.clone()))
                                    .collect(),
                            )
                            .id("profile-select")
                            .accessibility_label("配置文件")
                            .w_full()
                            .into_any_element()
                    }),
            )
            .child(
                Popover::new("profile-menu-popover")
                    .track_focus(&focus)
                    .flex_shrink_0()
                    .mr(surface::css(10.))
                    .offset(if confirmation.is_some() {
                        cx.theme().font_size * (if self.pid() == 777 { 74. } else { 16. } / 16.)
                    } else {
                        Pixels::ZERO
                    })
                    .trigger_with(|open, _, cx| {
                        BaseButton::new("profile-more")
                            .accessibility_label("配置文件选项")
                            .tooltip(|window, cx| Tooltip::new("配置文件选项").build(window, cx))
                            .size(surface::css(26.))
                            .p_0()
                            .rounded_none()
                            .border_1()
                            .border_color(if open {
                                cx.theme().primary
                            } else {
                                cx.theme().background
                            })
                            .bg(cx.theme().transparent)
                            .when(!open, |button| {
                                button.hover(|button| button.border_color(cx.theme().border))
                            })
                            .active(|button| button.border_color(cx.theme().primary))
                            .child(img("synapse/profile-more.svg").size(surface::css(20.)))
                            .into_any_element()
                    })
                    .on_open_change(move |open, window, cx| {
                        if *open {
                            opened_list.update(cx, |list, cx| {
                                list.set_selected_index(Some(IndexPath::new(0)), window, cx)
                            });
                        }
                        if !open {
                            _ = open_workspace.update(cx, |workspace, cx| {
                                workspace.profile_confirmation = None;
                                cx.notify();
                            });
                        }
                    })
                    .content(move |_, _, cx| {
                        let popup = cx.entity().downgrade();
                        if let Some(confirmation) = confirmation {
                            return profile_confirmation(
                                confirmation,
                                popup,
                                workspace,
                                confirmation_focus,
                                cx,
                            )
                            .into_any_element();
                        }
                        list.update(cx, |list, _| list.delegate_mut().popup = Some(popup));
                        // List sections share footer geometry. Clip its last separator,
                        // which the original menu omits, without altering keyboard rows.
                        div()
                            .id("profile-actions")
                            .test_support()
                            .role(Role::Menu)
                            .w(surface::css(155.))
                            .h(surface::css(menu_height - 9. + 2.))
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded_none()
                            .bg(DropdownColors::new().surface())
                            .overflow_hidden()
                            .child(List::new(&list).p_0().h(surface::css(menu_height)))
                            .into_any_element()
                    }),
            )
            .when(has_obm, |bar| {
                bar.child(
                    Popover::new("profile-obm-popover")
                        .flex_shrink_0()
                        .trigger_with(|open, _, cx| {
                            BaseButton::new("profile-obm")
                                .accessibility_label("板载内存")
                                .size(surface::css(26.))
                                .border_1()
                                .border_color(if open {
                                    cx.theme().primary
                                } else {
                                    cx.theme().transparent
                                })
                                .hover(|button| button.border_color(cx.theme().border))
                                .tooltip(|window, cx| Tooltip::new("板载内存").build(window, cx))
                                .child(img("synapse/profile-obm.svg").size(surface::css(20.)))
                                .into_any_element()
                        })
                        .content(|_, _, cx| {
                            v_flex()
                                .id("profile-obm-content")
                                .test_support()
                                .w(surface::css(300.))
                                .p(surface::css(20.))
                                .gap(surface::css(14.))
                                .bg(cx.theme().popover)
                                .border_1()
                                .border_color(cx.theme().border)
                                .text_size(surface::css(14.))
                                .child(
                                    div()
                                        .font_weight(FontWeight::BOLD)
                                        .child(crate::i18n::t_or("ON_BOARD_MEMORY", "板载内存")),
                                )
                                .child(crate::i18n::t_or(
                                    "ON_BOARD_MEMORY_CONTROLLER_TOOLTIP",
                                    "使用板载内存直接将配置文件存储在 Razer 雷蛇设备上。",
                                ))
                                .child(surface::note(
                                    "当前配置保存在本机。设备的板载配置尚未读取。",
                                    cx,
                                ))
                                .children((1..=4).map(|slot| {
                                    h_flex()
                                        .justify_between()
                                        .gap(surface::css(16.))
                                        .py(surface::css(8.))
                                        .border_b_1()
                                        .border_color(cx.theme().border)
                                        .child(format!("板载槽位 {slot}"))
                                        .child(surface::note("未读取", cx))
                                }))
                                .child(
                                    Button::new("profile-obm-write")
                                        .label("写入设备")
                                        .disabled(true)
                                        .tooltip("读取设备的板载配置后才能写入"),
                                )
                                .into_any_element()
                        }),
                )
            })
            .into_any_element()
    }
}

fn profile_confirmation(
    confirmation: ProfileConfirmation,
    popup: WeakEntity<PopoverState>,
    workspace: WeakEntity<DeviceWorkspace>,
    focus: FocusHandle,
    cx: &App,
) -> impl IntoElement {
    let danger = ProfileAlertColors::new().danger();
    let actions = if confirmation.reset {
        vec![
            (
                "profile-reset-bindings",
                "重置按键绑定",
                Continue::ResetProfile {
                    id: confirmation.id.clone(),
                    bindings_only: true,
                },
            ),
            (
                "profile-reset-all",
                "重置配置文件",
                Continue::ResetProfile {
                    id: confirmation.id.clone(),
                    bindings_only: false,
                },
            ),
        ]
    } else {
        vec![(
            "profile-delete-confirm",
            "删除",
            Continue::DeleteProfile(confirmation.id.clone()),
        )]
    };
    let title = if confirmation.reset {
        "重置配置文件"
    } else {
        "删除配置文件"
    };
    v_flex().id("profile-confirmation").test_support().role(Role::Dialog)
        .aria_label(format!("{title}：{}", confirmation.name))
        .w(surface::css(300.)).p(surface::css(20.))
        .rounded(cx.theme().font_size * (3. / 16.)).border_1().border_color(danger)
        .bg(cx.theme().group_box).text_size(surface::css(14.)).line_height(surface::css(17.))
        .shadow(vec![BoxShadow {
            color: cx.theme().title_bar.opacity(0.2),
            offset: point(Pixels::ZERO, cx.theme().font_size * (6. / 16.)),
            blur_radius: cx.theme().font_size * (10. / 16.),
            spread_radius: Pixels::ZERO,
            inset: false,
        }])
        .child(div().text_center().text_color(danger).mb(surface::css(10.))
            .when(confirmation.reset, |title| title.font_weight(FontWeight::BOLD)).child(title))
        .child(div().text_center().mb(surface::css(10.)).child(if confirmation.reset {
            "当你重置配置文件时，所有按键绑定和已配置的设置都将丢失并重置为默认值。你可以重置配置文件或选择仅重置所有按键绑定。"
        } else {
            "你将要删除此配置文件。此配置文件中的所有绑定将会被删除。"
        }))
        .child(h_flex().justify_center().gap(surface::css(10.)).children(actions.into_iter().enumerate().map(|(index, (id, label, next))| {
            let popup = popup.clone();
            let workspace = workspace.clone();
            Button::new(id).label(label).h(surface::css(27.)).min_w(surface::css(90.))
                .when(confirmation.reset, |button| button.flex_1())
                .when(index == 0, |button| button.track_focus(&focus))
                .px(surface::css(5.)).py(surface::css(4.)).text_size(surface::css(12.))
                .rounded_none().border_1().border_color(cx.theme().title_bar.opacity(0.3))
                .custom(ButtonCustomVariant::new(cx).color(danger).foreground(cx.theme().primary_foreground)
                    .hover(danger.opacity(0.8)).active(danger.opacity(0.6)))
                .on_click(move |_, window, cx| {
                    _ = popup.update(cx, |popup, cx| popup.dismiss(window, cx));
                    _ = workspace.update(cx, |workspace, cx| workspace.continue_with(next.clone(), window, cx));
                })
        })))
}

#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;
