//! Current product profile commands. The product-specific table is extracted
//! independently from each mounted profile bar and its menu consumer.
use super::*;
use crate::ui::theme::DropdownColors;
use gpui_kit::base::{Button as BaseButton, Popover, PopoverState, motion};
use gpui_kit::component::list::{ListDelegate, ListState};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum ProfileAction {
    Add,
    Import,
    LinkedGames,
    LinkGames,
    Share,
    Reshare,
    Rename,
    Duplicate,
    Export,
    Reset,
    Delete,
}

impl ProfileAction {
    fn id(self) -> &'static str {
        match self {
            Self::Add => "source-profile-add",
            Self::Import => "source-profile-import",
            Self::LinkedGames => "source-profile-linked-games",
            Self::LinkGames => "source-profile-link-games",
            Self::Share => "source-profile-share",
            Self::Reshare => "source-profile-reshare",
            Self::Rename => "source-profile-rename",
            Self::Duplicate => "source-profile-duplicate",
            Self::Export => "source-profile-export",
            Self::Reset => "source-profile-reset",
            Self::Delete => "source-profile-delete",
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum MenuItem {
    Separator,
    Action {
        command: ProfileAction,
        label_key: String,
        #[serde(default)]
        disabled_when: Vec<String>,
        #[serde(default)]
        hidden_when: Vec<String>,
    },
}

#[derive(Deserialize)]
struct RenameSpec {
    max_length: usize,
}

#[derive(Deserialize)]
pub(super) struct MenuSpec {
    product_id: u32,
    menu: Vec<MenuItem>,
    menu_width: f32,
    rename: RenameSpec,
    #[serde(default)]
    confirmations: Vec<Value>,
}

pub(super) fn spec(pid: u32) -> Option<&'static MenuSpec> {
    #[derive(Deserialize)]
    struct MenuData {
        products: Vec<MenuSpec>,
    }
    static DATA: OnceLock<MenuData> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../source_profile_menu_data.json"))
            .expect("audited current profile menus")
    })
    .products
    .iter()
    .find(|product| product.product_id == pid)
}

impl MenuSpec {
    pub(super) fn width(&self) -> f32 {
        self.menu_width
    }
    pub(super) fn name_limit(&self) -> usize {
        self.rename.max_length
    }
    pub(super) fn confirmation(&self, action: ProfileAction) -> Option<&Value> {
        let name = if action == ProfileAction::Delete {
            "delete"
        } else {
            "reset"
        };
        self.confirmations
            .iter()
            .find(|value| value["name"] == name)
    }
}

#[derive(Clone, PartialEq, Eq)]
struct MenuCommand {
    action: ProfileAction,
    label: String,
    disabled: bool,
}
impl MenuCommand {
    fn height(&self) -> f32 {
        if self.action == ProfileAction::LinkedGames && !self.disabled {
            47.
        } else {
            27.
        }
    }
}

fn linked_games(device: &Device) -> bool {
    device
        .active_profile_obj()
        .and_then(|profile| profile.settings.as_ref())
        .is_some_and(|settings| !settings.linked_games.is_empty())
}

// Only conditions with an actual local state are evaluated here. Service-only
// conditions are retained separately in the source receipt, not guessed from
// the fact that a local preview has no device service.
fn condition(rule: &str, workspace: &SourceProductWorkspace) -> Option<bool> {
    Some(match rule {
        "no_selected_profile" => workspace.device.active_profile_obj().is_none(),
        "single_profile" => workspace.device.profiles.len() <= 1,
        "profile_switch_disabled" => !workspace.profile_switch_enabled(),
        "linked_games_empty" => !linked_games(&workspace.device),
        "linked_games_present" => linked_games(&workspace.device),
        // Static unsupported entries are removed by the source table.
        "reset_unsupported" | "share_category_unsupported" | "share_unsupported" => true,
        _ => return None,
    })
}

fn sections(workspace: &SourceProductWorkspace) -> Vec<Vec<MenuCommand>> {
    let Some(spec) = spec(workspace.device.product_id) else {
        return vec![];
    };
    let mut result = vec![vec![]];
    for item in &spec.menu {
        match item {
            MenuItem::Separator => {
                if result.last().is_some_and(|section| !section.is_empty()) {
                    result.push(vec![]);
                }
            }
            MenuItem::Action {
                command,
                label_key,
                disabled_when,
                hidden_when,
            } => {
                if hidden_when
                    .iter()
                    .any(|rule| condition(rule, workspace) == Some(true))
                {
                    continue;
                }
                let disabled = !workspace.profile_switch_enabled()
                    || disabled_when
                        .iter()
                        .any(|rule| condition(rule, workspace).unwrap_or(false));
                result.last_mut().unwrap().push(MenuCommand {
                    action: *command,
                    label: crate::i18n::t(label_key),
                    disabled,
                });
            }
        }
    }
    result.retain(|section| !section.is_empty());
    result
}

pub(super) struct ProfileCommands {
    sections: Vec<Vec<MenuCommand>>,
    cursor: Option<IndexPath>,
    workspace: WeakEntity<SourceProductWorkspace>,
    pub(super) popup: Option<WeakEntity<PopoverState>>,
}

impl ProfileCommands {
    pub(super) fn new(workspace: WeakEntity<SourceProductWorkspace>) -> Self {
        Self {
            sections: vec![],
            cursor: None,
            workspace,
            popup: None,
        }
    }
    pub(super) fn refresh(&mut self, workspace: &SourceProductWorkspace) {
        let next = sections(workspace);
        if self.sections != next {
            self.sections = next;
            self.cursor = None;
        }
    }
    pub(super) fn height(&self) -> f32 {
        self.sections
            .iter()
            .map(|section| section.iter().map(MenuCommand::height).sum::<f32>() + 9.)
            .sum::<f32>()
            - 9.
    }
    fn width(&self, pid: u32, window: &Window) -> f32 {
        let minimum = spec(pid).map_or(155., MenuSpec::width);
        if pid == 3886 {
            return minimum;
        }
        let width = self
            .sections
            .iter()
            .flatten()
            .map(|command| surface::label_width(&command.label, 14., window) + 14.)
            .fold(minimum, f32::max);
        if pid == 241 { width } else { width.min(280.) }
    }
}

impl SourceProductWorkspace {
    pub(super) fn profile_more(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if spec(self.device.product_id).is_none() {
            return div().into_any_element();
        }
        self.profile_menu
            .update(cx, |list, _| list.delegate_mut().refresh(self));
        let list = self.profile_menu.clone();
        let height = list.read(cx).delegate().height();
        let width = list
            .read(cx)
            .delegate()
            .width(self.device.product_id, window);
        let confirmation = self.profile_confirmation.clone();
        let focus = if confirmation.is_some() {
            self.profile_confirm_focus.clone()
        } else {
            list.focus_handle(cx)
        };
        let confirm_focus = self.profile_confirm_focus.clone();
        let owner = cx.entity().downgrade();
        let open_owner = owner.clone();
        let pid = self.device.product_id;
        let disabled = !self.profile_switch_enabled();
        let more_state = window.use_keyed_state(
            (ElementId::from("source-profile-more"), "border"),
            cx,
            |_, _| surface::HoverBorderState::default(),
        );
        let border = surface::hover_border_color("source-profile-more", &more_state, window, cx);
        let trigger_state = more_state.clone();
        let top = confirmation
            .as_ref()
            .and_then(|confirm| spec(pid)?.confirmation(confirm.action)?["top"].as_f64())
            .unwrap_or(42.) as f32;
        Popover::new("source-profile-menu")
            .track_focus(&focus)
            .anchor(Anchor::TopLeft)
            .flex_shrink_0()
            .mr(surface::css(10.))
            .offset(if confirmation.is_some() {
                window.rem_size() * ((top - 26.) / 16.)
            } else {
                Pixels::ZERO
            })
            .trigger_with(move |_, _, _| {
                surface::hover_border_button(
                    "source-profile-more",
                    "synapse/profile-more.svg".into(),
                    "Profile actions",
                    border,
                    false,
                    26.,
                    trigger_state.clone(),
                )
                .disabled(disabled)
                .into_any_element()
            })
            .on_open_change(move |open, window, cx| {
                more_state.update(cx, |state, cx| {
                    state.open = *open;
                    cx.notify();
                });
                let _ = open_owner.update(cx, |this, cx| {
                    if !*open {
                        this.profile_confirmation = None;
                        this.profile_confirm_task = None;
                    } else {
                        this.profile_menu.update(cx, |list, cx| {
                            list.set_selected_index(Some(IndexPath::new(0)), window, cx)
                        });
                    }
                    cx.notify();
                });
            })
            .content(move |_, window, cx| {
                let popup = cx.entity().downgrade();
                list.update(cx, |state, _| {
                    state.delegate_mut().popup = Some(popup.clone())
                });
                if let Some(confirmation) = confirmation {
                    let opacity = motion::Presence::new("source-profile-confirm-opacity", true)
                        .transition(
                            motion::Transition::new(std::time::Duration::from_millis(300))
                                .easing(motion::Easing::Linear),
                        )
                        .sample(window, cx)
                        .progress;
                    return div()
                        .opacity(opacity)
                        .child(confirmation_element(
                            confirmation,
                            pid,
                            owner,
                            popup,
                            confirm_focus,
                            cx,
                        ))
                        .into_any_element();
                }
                let opacity = motion::Presence::new("source-profile-menu-opacity", true)
                    .transition(
                        motion::Transition::new(std::time::Duration::from_millis(100))
                            .easing(motion::Easing::Linear),
                    )
                    .sample(window, cx)
                    .progress;
                div()
                    .id("source-profile-actions")
                    .test_support()
                    .role(Role::Menu)
                    .w(surface::css(width))
                    .h(surface::css(height + 2.))
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(DropdownColors::new().surface())
                    .overflow_hidden()
                    .opacity(opacity)
                    .child(
                        gpui_kit::component::list::List::new(&list)
                            .p_0()
                            .h(surface::css(height + 9.)),
                    )
                    .into_any_element()
            })
            .into_any_element()
    }
}

fn confirmation_element(
    confirmation: super::profile_actions::ProfileConfirmation,
    pid: u32,
    owner: WeakEntity<SourceProductWorkspace>,
    popup: WeakEntity<PopoverState>,
    focus: FocusHandle,
    cx: &App,
) -> AnyElement {
    use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants};
    let definition = spec(pid).and_then(|spec| spec.confirmation(confirmation.action));
    let key = |field: &str, fallback: &str| {
        definition
            .and_then(|value| value[field].as_str())
            .unwrap_or(fallback)
            .to_owned()
    };
    let title = crate::i18n::t(&key("label_key", "DELETE_PROFILE_TITLE"));
    let description = crate::i18n::t(&key("description_key", "DELETE_PROFILE_MSG"));
    let action_label = crate::i18n::t(
        definition
            .and_then(|value| value["button_keys"][0].as_str())
            .unwrap_or("REMOVE"),
    );
    let width = definition
        .and_then(|value| value["width"].as_f64())
        .unwrap_or(300.) as f32;
    let colors = crate::ui::theme::ProfileAlertColors::new();
    let danger = if pid == 3886 {
        colors.headphone_danger()
    } else {
        colors.danger()
    };
    v_flex()
        .id("source-profile-confirmation")
        .test_support()
        .role(Role::Dialog)
        .aria_label(title.clone())
        .track_focus(&focus)
        .w(surface::css(width))
        .p(surface::css(20.))
        .rounded(surface::css(3.))
        .border_1()
        .border_color(danger)
        .bg(cx.theme().group_box)
        .text_size(surface::css(14.))
        .line_height(surface::css(17.))
        .shadow(vec![BoxShadow {
            color: cx.theme().title_bar.opacity(0.2),
            offset: point(Pixels::ZERO, cx.theme().font_size * (6. / 16.)),
            blur_radius: cx.theme().font_size * (10. / 16.),
            spread_radius: Pixels::ZERO,
            inset: false,
        }])
        .child(
            div()
                .text_center()
                .text_color(colors.danger())
                .mb(surface::css(10.))
                .child(title),
        )
        .child(div().text_center().mb(surface::css(10.)).child(description))
        .child(
            h_flex().justify_center().child(
                Button::new("source-profile-confirm")
                    .label(action_label)
                    .xsmall()
                    .h(surface::css(27.))
                    .min_w(surface::css(90.))
                    .px(surface::css(5.))
                    .py(surface::css(4.))
                    .text_size(surface::css(12.))
                    .line_height(surface::css(14.))
                    .rounded(cx.theme().font_size * (3. / 16.))
                    .border_1()
                    .border_color(cx.theme().title_bar.opacity(0.3))
                    .custom(
                        ButtonCustomVariant::new(cx)
                            .color(danger)
                            .foreground(cx.theme().primary_foreground)
                            .hover(danger.opacity(0.8))
                            .active(danger.opacity(0.6)),
                    )
                    .on_click(move |_, window, cx| {
                        let _ = popup.update(cx, |popup, cx| popup.dismiss(window, cx));
                        let _ = owner.update(cx, |owner, cx| {
                            owner.confirm_profile_action(confirmation.clone(), window, cx)
                        });
                    }),
            ),
        )
        .into_any_element()
}

impl ListDelegate for ProfileCommands {
    type Item = CommandRow;

    fn sections_count(&self, _: &App) -> usize {
        self.sections.len()
    }
    fn items_count(&self, section: usize, _: &App) -> usize {
        self.sections[section].len()
    }
    fn set_selected_index(
        &mut self,
        index: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
        self.cursor = index;
    }
    fn render_item(
        &mut self,
        index: IndexPath,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Option<CommandRow> {
        Some(CommandRow {
            command: self.sections.get(index.section)?.get(index.row)?.clone(),
            selected: false,
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
            .and_then(|index| self.sections.get(index.section)?.get(index.row))
            .cloned()
        else {
            return;
        };
        if command.disabled {
            return;
        }
        let workspace = self.workspace.clone();
        let popup = self.popup.clone();
        window.defer(cx, move |window, cx| {
            let Some(workspace) = workspace.upgrade() else {
                return;
            };
            // Check live state after deferral and before dismissing the menu.
            if !sections(workspace.read(cx))
                .iter()
                .flatten()
                .any(|item| item.action == command.action && !item.disabled)
            {
                return;
            }
            if !matches!(command.action, ProfileAction::Delete | ProfileAction::Reset) {
                if let Some(popup) = &popup {
                    let _ = popup.update(cx, |popup, cx| popup.dismiss(window, cx));
                }
            }
            workspace.update(cx, |workspace, cx| {
                workspace.run_profile_action(command.action, window, cx)
            });
        });
    }
}

#[derive(IntoElement)]
pub(super) struct CommandRow {
    command: MenuCommand,
    selected: bool,
}
impl Selectable for CommandRow {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
    fn is_selected(&self) -> bool {
        self.selected
    }
}
impl RenderOnce for CommandRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = DropdownColors::new();
        let id = self.command.action.id();
        let hover = window.use_keyed_state((ElementId::from(id), "hover"), cx, |_, _| false);
        let highlighted = !self.command.disabled && (self.selected || *hover.read(cx));
        let background = motion::transition(
            (id, "background"),
            if highlighted {
                colors.hover()
            } else {
                rgba(0).into()
            },
            motion::Transition::new(std::time::Duration::from_millis(300))
                .easing(motion::Easing::Ease),
            window,
            cx,
        );
        BaseButton::new(self.command.action.id())
            .disabled(self.command.disabled)
            .focusable(false)
            .tab_stop(false)
            .role(Role::MenuItem)
            .accessibility_label(self.command.label.clone())
            .w_full()
            .h(surface::css(self.command.height()))
            .px(surface::css(6.))
            .py(surface::css(5.))
            .justify_start()
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_color(cx.theme().foreground)
            .when(self.command.disabled, |row| row.opacity(0.3))
            .bg(background)
            .on_hover(move |value, _, cx| {
                hover.update(cx, |state, cx| {
                    *state = *value;
                    cx.notify();
                })
            })
            .child(
                v_flex()
                    .items_start()
                    .child(self.command.label)
                    // The source reserves an icon strip below LINKED_GAMES. Local
                    // executable associations have no service-provided game icons.
                    .when(
                        self.command.action == ProfileAction::LinkedGames && !self.command.disabled,
                        |column| column.child(div().h(surface::css(20.))),
                    ),
            )
    }
}
