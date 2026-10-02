//! Guest branch of rz-user-profile-menu's Root/N. Account commands require
//! the host session; the local exit command is routed back to AppShell.
use crate::{
    i18n,
    ui::{surface, theme::AccountMenuColors},
};
use gpui_kit::base::{Button, Popover, PopoverState};
use gpui_kit::component::{
    list::{List, ListDelegate, ListState},
    tooltip::Tooltip,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};

pub(super) struct AccountMenu {
    commands: Entity<ListState<AccountCommands>>,
}

pub(super) struct ExitRequested;
impl EventEmitter<ExitRequested> for AccountMenu {}

impl AccountMenu {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let owner = cx.entity().downgrade();
        Self {
            commands: cx.new(|cx| {
                ListState::new(
                    AccountCommands {
                        owner,
                        popup: None,
                        cursor: None,
                    },
                    window,
                    cx,
                )
            }),
        }
    }
}

#[derive(Clone, Copy)]
enum AccountCommand {
    LogIn,
    Feedback,
    Exit,
}
impl AccountCommand {
    const ALL: [Self; 3] = [Self::LogIn, Self::Feedback, Self::Exit];
    fn id(self) -> &'static str {
        match self {
            Self::LogIn => "account-log-in",
            Self::Feedback => "account-feedback",
            Self::Exit => "account-exit",
        }
    }
    fn label(self) -> String {
        i18n::t(match self {
            Self::LogIn => "TEXT_LOG_IN",
            Self::Feedback => "TEXT_FEEDBACK",
            Self::Exit => "TEXT_EXIT",
        })
    }
    fn disabled_reason(self) -> Option<&'static str> {
        match self {
            // Source Log In calls logOut to reopen the host's login window.
            Self::LogIn => Some("账户登录服务尚未连接。"),
            Self::Feedback => Some("反馈窗口尚未连接。"),
            Self::Exit => None,
        }
    }
}

struct AccountCommands {
    owner: WeakEntity<AccountMenu>,
    popup: Option<WeakEntity<PopoverState>>,
    cursor: Option<IndexPath>,
}
impl ListDelegate for AccountCommands {
    type Item = AccountCommandRow;
    fn sections_count(&self, _: &App) -> usize {
        AccountCommand::ALL.len()
    }
    fn items_count(&self, _: usize, _: &App) -> usize {
        1
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
        _: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        Some(AccountCommandRow {
            command: *AccountCommand::ALL.get(ix.section)?,
            selected: false,
        })
    }
    fn render_section_footer(
        &mut self,
        section: usize,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Option<impl IntoElement> {
        (section + 1 < AccountCommand::ALL.len()).then(|| {
            div()
                .h(surface::css(15.))
                .py(surface::css(7.))
                .child(div().h(surface::css(1.)).bg(AccountMenuColors::border()))
        })
    }
    fn confirm(&mut self, _: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        let Some(command) = self
            .cursor
            .and_then(|ix| AccountCommand::ALL.get(ix.section))
            .copied()
        else {
            return;
        };
        if command.disabled_reason().is_some() {
            return;
        }
        let popup = self.popup.clone();
        let owner = self.owner.clone();
        // Release List's borrow and restore the toolbar focus before opening
        // the shell's unsaved-changes dialog.
        window.defer(cx, move |window, cx| {
            if let Some(popup) = popup {
                _ = popup.update(cx, |popup, cx| popup.dismiss(window, cx));
            }
            _ = owner.update(cx, |_, cx| cx.emit(ExitRequested));
        });
    }
}

#[derive(IntoElement)]
struct AccountCommandRow {
    command: AccountCommand,
    selected: bool,
}
impl Selectable for AccountCommandRow {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
    fn is_selected(&self) -> bool {
        self.selected
    }
}
impl RenderOnce for AccountCommandRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let reason = self.command.disabled_reason();
        div()
            .when(!matches!(self.command, AccountCommand::Exit), |row| {
                row.pb(surface::css(4.))
            })
            .child(
                Button::new(self.command.id())
                    .role(Role::MenuItem)
                    .accessibility_label(self.command.label())
                    .focusable(false)
                    .tab_stop(false)
                    .disabled(reason.is_some())
                    .w_full()
                    .h(surface::css(26.))
                    .justify_start()
                    .pt(surface::css(5.))
                    .pb(surface::css(4.))
                    .px(surface::css(18.))
                    .rounded(surface::css(13.))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .text_color(cx.theme().foreground)
                    .when(reason.is_none(), |row| {
                        row.when(self.selected, |row| row.bg(AccountMenuColors::hover()))
                            .hover(|row| row.bg(AccountMenuColors::hover()))
                    })
                    .when_some(reason, |row, reason| {
                        row.text_color(cx.theme().foreground.opacity(0.3))
                            .aria_description(reason)
                            .tooltip(move |window, cx| Tooltip::new(reason).build(window, cx))
                    })
                    .child(self.command.label()),
            )
    }
}

impl Render for AccountMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let list = self.commands.clone();
        let opened_list = list.clone();
        let focus = list.focus_handle(cx);
        Popover::new("account-menu-popover")
            .anchor(Anchor::TopRight)
            .offset(window.rem_size() * (4. / 16.))
            .track_focus(&focus)
            .flex_shrink_0()
            .trigger_with(|open, _, cx| {
                Button::new("account-menu-trigger")
                    .accessibility_label("账户菜单")
                    .aria_description("当前使用本地访客工作区")
                    .w(surface::css(46.))
                    .h(surface::css(38.))
                    .bg(if open {
                        AccountMenuColors::trigger_active()
                    } else {
                        cx.theme().transparent
                    })
                    .hover(|button| button.bg(AccountMenuColors::trigger_active()))
                    .focus_visible(|button| button.border_1().border_color(cx.theme().primary))
                    .child(img("synapse/account-guest.svg").size(surface::css(20.)))
                    .into_any_element()
            })
            .on_open_change(move |open, window, cx| {
                if *open {
                    opened_list.update(cx, |list, cx| {
                        list.set_selected_index(Some(IndexPath::new(0).section(2)), window, cx)
                    });
                }
            })
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                list.update(cx, |list, _| list.delegate_mut().popup = Some(popup));
                div()
                    .id("account-menu")
                    .role(Role::Menu)
                    .aria_label("账户菜单")
                    .w(surface::css(223.))
                    .p_0()
                    .px(surface::css(14.))
                    .py(surface::css(7.))
                    .border_1()
                    .border_color(AccountMenuColors::border())
                    .rounded(surface::css(5.))
                    .bg(AccountMenuColors::surface())
                    .child(List::new(&list).p_0().h(surface::css(116.)))
            })
    }
}
