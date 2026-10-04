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

#[path = "linked_games.rs"]
mod linked_games;
#[path = "onboard_memory.rs"]
mod onboard_memory;
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
        let settings = ProfileSettings::for_product(device.product_id);
        Profile {
            source_settings: None,
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
        let settings = ProfileSettings::for_product(device.product_id);
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

    pub(super) fn profile_toolbar(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // 653 zh.renderProfileBar enables OBM outside BLE. The other audited
        // product roots explicitly disable it; no slot contents are inferred.
        let has_obm = self.pid() == 653 && !self.device.use_ble;
        let list = self.profile_menu.clone();
        let pid = self.pid();
        let confirmation = self.profile_confirmation.clone();
        let focus = if confirmation.is_some() {
            self.profile_confirm_focus.clone()
        } else {
            list.focus_handle(cx)
        };
        let confirmation_focus = self.profile_confirm_focus.clone();
        let workspace = cx.entity().downgrade();
        let obm_workspace = workspace.clone();
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
        // `.profile-bar .dots3` 也带 `.hover-border`：边框 `#222` → 悬停 `#5d5d5d`
        // → 打开 `#44d62c`，并且 `transition:border-color .2s`。
        let more_state = window.use_keyed_state(
            (ElementId::from("profile-more"), "hover-border-state"),
            cx,
            |_, _| surface::HoverBorderState::default(),
        );
        let more_border = surface::hover_border_color("profile-more", &more_state, window, cx);
        let trigger_state = more_state.clone();
        h_flex()
            .id("profile-bar")
            .test_support()
            // 源码 `.profile-bar{height:26px}`（`.nav-tabs .profile-bar` 只覆盖
            // color/justify-content/margin/width，高度仍是 26）。
            .h(surface::css(26.))
            .w_full()
            .min_w_0()
            .max_w(surface::css(if has_obm {
                surface::PROFILE_BAR_WIDTH_OBM
            } else {
                surface::PROFILE_BAR_WIDTH
            }))
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
                        // The alert is nested in the relative more-menu wrapper.
                        // 777's direct-child top:100px rule does not match it;
                        // the effective top is 52px (42px on 182/653).
                        cx.theme().font_size * (if pid == 777 { 26. } else { 16. } / 16.)
                    } else {
                        Pixels::ZERO
                    })
                    .trigger_with(move |_open, _, _| {
                        surface::hover_border_button(
                            "profile-more",
                            "synapse/profile-more.svg".into(),
                            "配置文件选项",
                            more_border,
                            false,
                            surface::NAV_MORE_WIDTH,
                            trigger_state.clone(),
                        )
                        .tooltip(|window, cx| Tooltip::new("配置文件选项").build(window, cx))
                        .mr(surface::css(10.))
                        .into_any_element()
                    })
                    .on_open_change({
                        let more_state = more_state.clone();
                        move |open, window, cx| {
                            more_state.update(cx, |state, cx| {
                                state.open = *open;
                                cx.notify();
                            });
                            if *open {
                                opened_list.update(cx, |list, cx| {
                                    list.set_selected_index(Some(IndexPath::new(0)), window, cx)
                                });
                            }
                            if !*open {
                                _ = open_workspace.update(cx, |workspace, cx| {
                                    workspace.profile_confirmation = None;
                                    cx.notify();
                                });
                            }
                        }
                    })
                    .content(move |_, window, cx| {
                        let popup = cx.entity().downgrade();
                        if let Some(confirmation) = confirmation {
                            // Source bP keeps the reset/delete nodes mounted
                            // and toggles .show: opacity .3s linear; hiding
                            // immediately sets visibility:hidden.
                            let opacity = gpui_kit::base::motion::Presence::new(
                                "profile-confirm-opacity",
                                true,
                            )
                            .transition(
                                gpui_kit::base::motion::Transition::new(
                                    std::time::Duration::from_millis(300),
                                )
                                .easing(gpui_kit::base::motion::Easing::Linear),
                            )
                            .sample(window, cx)
                            .progress;
                            return div()
                                .opacity(opacity)
                                .child(profile_confirmation(
                                    confirmation,
                                    pid,
                                    popup,
                                    workspace,
                                    confirmation_focus,
                                    cx,
                                ))
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
                bar.child(onboard_memory::OnboardMemoryControl::new(obm_workspace))
            })
            .into_any_element()
    }
}

fn profile_confirmation(
    confirmation: ProfileConfirmation,
    pid: u32,
    popup: WeakEntity<PopoverState>,
    workspace: WeakEntity<DeviceWorkspace>,
    focus: FocusHandle,
    cx: &App,
) -> impl IntoElement {
    let colors = ProfileAlertColors::new();
    // 777 overrides the border and button, but renders del-title-normal,
    // whose #fd4949 title survives the later del-title-only override.
    let danger = if pid == 777 {
        colors.headphone_danger()
    } else {
        colors.danger()
    };
    let button_foreground = if pid == 777 {
        cx.theme().button_foreground
    } else {
        cx.theme().primary_foreground
    };
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
        .child(div().text_center().text_color(colors.danger()).mb(surface::css(10.))
            .when(confirmation.reset, |title| title.font_weight(FontWeight::BOLD)).child(title))
        .child(div().text_center().mb(surface::css(10.)).child(if confirmation.reset {
            "当你重置配置文件时，所有按键绑定和已配置的设置都将丢失并重置为默认值。你可以重置配置文件或选择仅重置所有按键绑定。"
        } else {
            "你将要删除此配置文件。此配置文件中的所有绑定将会被删除。"
        }))
        .child(h_flex().justify_center().gap(surface::css(10.)).children(actions.into_iter().enumerate().map(|(index, (id, label, next))| {
            let popup = popup.clone();
            let workspace = workspace.clone();
            Button::new(id).label(label).xsmall().h(surface::css(27.)).min_w(surface::css(90.))
                .when(confirmation.reset, |button| button.flex_1())
                .when(index == 0, |button| button.track_focus(&focus))
                .px(surface::css(if pid == 777 { 0. } else { 5. })).py(surface::css(4.)).text_size(surface::css(12.))
                .line_height(surface::css(if pid == 777 { 17. } else { 14. }))
                .rounded(cx.theme().font_size * (3. / 16.)).border_1().border_color(cx.theme().title_bar.opacity(0.3))
                .custom(ButtonCustomVariant::new(cx).color(danger).foreground(button_foreground)
                    .hover(danger.opacity(0.8)).active(danger.opacity(0.6)))
                .on_click(move |_, window, cx| {
                    _ = popup.update(cx, |popup, cx| popup.dismiss(window, cx));
                    _ = workspace.update(cx, |workspace, cx| workspace.continue_with(next.clone(), window, cx));
                })
        })))
}

/// ImportExportModal's original shell. Base owns focus and dismissal; keeping
/// the presentation here avoids Component Dialog's inset title and fixed shadow.
/// The local linked-program editor shares this shell; the original linked-games
/// command opened the separate /profiles application, whose UI is not yet adapted.
pub(super) struct ProfileDialog {
    title: String,
    content: AnyView,
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    owner: WeakEntity<DeviceWorkspace>,
}

fn dismiss_profile_dialog(owner: &WeakEntity<DeviceWorkspace>, window: &mut Window, cx: &mut App) {
    let dialog = owner
        .update(cx, |workspace, cx| {
            let dialog = workspace.profile_dialog.take();
            // 弹层关闭后导航行恢复常态（源码 `showLinkedGames` 复位）。
            workspace.linked_games_open = false;
            cx.notify();
            dialog
        })
        .ok()
        .flatten();
    if let Some(dialog) = dialog {
        if let Some(focus) = dialog.read(cx).return_focus.clone() {
            window.focus(&focus, cx);
        }
    }
}

fn profile_dialog_footer(cx: &App) -> Div {
    h_flex()
        .w_full()
        .flex_shrink_0()
        .justify_center()
        .gap(surface::css(10.))
        .py(surface::css(16.))
        .px(surface::css(20.))
        .border_t_1()
        .border_color(cx.theme().input)
        .rounded_b(surface::css(4.))
        .bg(cx.theme().sidebar)
}

/// 原版对话框按钮就是 `.thx-btn`（profiles 主样式）：
/// `background-color:#44d62c;border-radius:3px;color:#000;padding:.5rem 1.5rem;
///  text-align:center;text-transform:uppercase;transition:opacity .3s`，
/// `:hover{opacity:.8}`、`:active{opacity:.6}`、
/// `.disabled,.disabled:hover{cursor:default;opacity:.3}`；
/// `.test{background-color:#707070;border:1px solid #0000004d;color:#fff}`；
/// 删除确认里的 `div.thx-btn{background-color:#fd4949;border:1px solid #0000004d;
///  color:#111;font-size:12px;height:27px;line-height:14px;min-width:90px;
///  padding:4px 5px}`。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ThxKind {
    /// `.thx-btn`：绿底黑字。
    Primary,
    /// `.thx-btn.test`：灰底白字。
    Test,
}

/// 删除/重置确认弹层里的红按钮不走这里：它由 `ProfileAlertColors` 提供
/// `.profile-del div.thx-btn{background-color:#fd4949;color:#111}` 的底色，
/// 并且 777 还有自己的覆盖（见 `profile_confirmation`）。

fn profile_dialog_button(
    id: &'static str,
    label: impl Into<SharedString>,
    kind: ThxKind,
    cx: &App,
) -> Button {
    let (background, foreground) = match kind {
        ThxKind::Primary => (gpui_kit::rgb(0x44d62c), gpui_kit::rgb(0x000000)),
        ThxKind::Test => (gpui_kit::rgb(0x707070), gpui_kit::rgb(0xffffff)),
    };
    Button::new(id)
        .label(label.into().to_uppercase())
        .xsmall()
        .h(surface::css(27.))
        .min_w(surface::css(90.))
        .px(surface::css(5.))
        .py(surface::css(4.))
        .text_size(surface::css(12.))
        .line_height(surface::css(14.))
        .rounded(cx.theme().font_size * (3. / 16.))
        .border_1()
        // `border:1px solid #0000004d`
        .border_color(gpui_kit::rgba(0x0000004d))
        .bg(background)
        .text_color(foreground)
        // `.thx-btn:hover{opacity:.8}`
        .hover(|button| button.opacity(0.8))
}

impl DeviceWorkspace {
    pub fn dismiss_profile_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(dialog) = self.profile_dialog.take() {
            if let Some(focus) = dialog.read(cx).return_focus.clone() {
                window.focus(&focus, cx);
            }
            cx.notify();
        }
    }

    fn show_profile_dialog(
        &mut self,
        title: String,
        content: AnyView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let return_focus = window.focused(cx);
        let owner = cx.entity().downgrade();
        let dialog = cx.new(|cx| ProfileDialog {
            title,
            content,
            focus: cx.focus_handle(),
            return_focus,
            owner,
        });
        let focus = dialog.read(cx).focus.clone();
        window.focus(&focus, cx);
        self.profile_dialog = Some(dialog);
        cx.notify();
    }
}

impl Render for ProfileDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let margin = window.rem_size();
        let width =
            (window.rem_size() * (602. / 16.)).min((viewport.width - margin * 2.).max(px(0.)));
        let height =
            (window.rem_size() * (481. / 16.)).min((viewport.height - margin * 2.).max(px(0.)));
        let top =
            (window.rem_size() * (104. / 16.)).min((viewport.height - height - margin).max(margin));
        let close_owner = self.owner.clone();
        let button_owner = self.owner.clone();
        gpui_kit::base::Dialog::new(cx)
            .focus_handle(self.focus.clone())
            // ImportExportModal's backdrop has no dismissal handler.
            .close_on_backdrop_press(false)
            .on_ok(|_, _, _| false)
            .on_close(move |_, window, cx| dismiss_profile_dialog(&close_owner, window, cx))
            .backdrop(
                div()
                    .absolute()
                    .size_full()
                    .bg(cx.theme().title_bar.opacity(0.7)),
            )
            .popup(
                v_flex()
                    .id("profile-dialog-frame")
                    .test_support()
                    .aria_label(self.title.clone())
                    .absolute()
                    .top(top)
                    .left((viewport.width - width) / 2.)
                    .w(width)
                    .h(height)
                    .border_1()
                    .border_color(cx.theme().input)
                    .rounded(surface::css(5.))
                    .bg(cx.theme().group_box)
                    .occlude()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .child(
                        h_flex()
                            .relative()
                            .w_full()
                            .h(surface::css(36.))
                            .flex_shrink_0()
                            .justify_center()
                            .rounded_t(surface::css(4.))
                            .bg(cx.theme().sidebar)
                            .border_b_1()
                            .border_color(cx.theme().input)
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                div()
                                    .mx(surface::css(36.))
                                    .truncate()
                                    .child(self.title.clone()),
                            )
                            .child(
                                BaseButton::new("profile-dialog-close")
                                    .absolute()
                                    .right(surface::css(5.))
                                    .top_0()
                                    .w(surface::css(20.))
                                    .h(surface::css(36.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .accessibility_label("关闭配置文件窗口")
                                    .hover(|button| button.bg(DropdownColors::new().hover()))
                                    .child(img("synapse/mapping-close.svg").size(surface::css(20.)))
                                    .on_click(move |_, window, cx| {
                                        dismiss_profile_dialog(&button_owner, window, cx)
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_h_0()
                            .w_full()
                            .child(div().absolute().inset_0().child(self.content.clone())),
                    ),
            )
    }
}

#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;
