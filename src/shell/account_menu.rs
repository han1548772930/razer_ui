//! Guest branch of rz-user-profile-menu's Root/N. Account commands require
//! the host session; the local exit command is routed back to AppShell.
use crate::{
    i18n,
    ui::{surface, theme::AccountMenuColors},
};
use gpui_kit::base::motion::{Easing, Presence, Transition};
use gpui_kit::base::{Button, PopoverState, Positioner};
use gpui_kit::component::{
    list::{List, ListDelegate, ListState},
    tooltip::Tooltip,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::time::Duration;

pub(super) struct AccountMenu {
    commands: Entity<ListState<AccountCommands>>,
    popup: Entity<PopoverState>,
    rendered: bool,
    shown: bool,
    was_open: bool,
    lifecycle_task: Option<Task<()>>,
    trigger_focus: FocusHandle,
    trigger_bounds: Bounds<Pixels>,
    _popup_observer: Subscription,
    _activation_observer: Subscription,
}

pub(super) struct ExitRequested;
impl EventEmitter<ExitRequested> for AccountMenu {}

impl AccountMenu {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let owner = cx.entity().downgrade();
        let popup = cx.new(|cx| PopoverState::new(false, cx));
        let observer = cx.observe_in(&popup, window, |this, popup, window, cx| {
            let open = popup.read(cx).is_open();
            if open == this.was_open {
                return;
            }
            this.was_open = open;
            this.lifecycle_task = None;
            if open {
                this.rendered = true;
            }
            if cx.reduce_motion() {
                this.shown = open;
                this.rendered = open;
                if open {
                    this.commands.focus_handle(cx).focus(window, cx);
                }
            } else {
                if !open {
                    this.shown = false;
                }
                // Root/m delays .show by 100ms and removes the hidden DOM
                // after 100ms. Its CSS transition itself lasts 200ms.
                this.lifecycle_task = Some(cx.spawn_in(window, async move |this, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    _ = this.update_in(cx, |this, window, cx| {
                        if open {
                            this.shown = true;
                        } else {
                            this.rendered = false;
                        }
                        if open && this.popup.focus_handle(cx).contains_focused(window, cx) {
                            this.commands.focus_handle(cx).focus(window, cx);
                        }
                        this.lifecycle_task = None;
                        this.commands.update(cx, |_, cx| cx.notify());
                        cx.notify();
                    });
                }));
            }
            this.commands.update(cx, |_, cx| cx.notify());
            cx.notify();
        });
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() {
                this.popup.update(cx, |popup, cx| popup.dismiss(window, cx));
            }
        });
        Self {
            commands: cx.new(|cx| {
                ListState::new(
                    AccountCommands {
                        owner,
                        popup: Some(popup.downgrade()),
                        cursor: None,
                    },
                    window,
                    cx,
                )
            }),
            popup,
            rendered: false,
            shown: false,
            was_open: false,
            lifecycle_task: None,
            trigger_focus: cx.focus_handle(),
            trigger_bounds: Bounds::default(),
            _popup_observer: observer,
            _activation_observer: activation,
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
        if !self
            .owner
            .upgrade()
            .is_some_and(|owner| owner.read(cx).shown)
            || !self
                .popup
                .as_ref()
                .and_then(|popup| popup.upgrade())
                .is_some_and(|popup| popup.read(cx).is_open())
        {
            return;
        }
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
        // Release List's borrow and dismiss the menu before requesting exit.
        window.defer(cx, move |window, cx| {
            if let Some(popup) = popup {
                _ = popup.update(cx, |popup, cx| popup.dismiss(window, cx));
            }
            _ = owner.update(cx, |_, cx| cx.emit(ExitRequested));
        });
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        let popup = self.popup.clone();
        window.defer(cx, move |window, cx| {
            if let Some(popup) = popup {
                _ = popup.update(cx, |popup, cx| popup.dismiss(window, cx));
            }
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
        let open = self.popup.read(cx).is_open();
        // Keep motion state only while the source DOM would be mounted.
        // Delayed .show is owned by the lifecycle task, so reversals sample
        // the previous channel with its original timing policy.
        let policy = |easing| Transition::new(Duration::from_millis(200)).easing(easing);
        let (opacity, movement) = if self.rendered {
            (
                Presence::new("account-opacity", self.shown)
                    .transition(policy(Easing::Linear))
                    .sample(window, cx)
                    .progress,
                Presence::new("account-position", self.shown)
                    .transition(policy(Easing::EaseOut))
                    .sample(window, cx)
                    .progress,
            )
        } else {
            (0., 0.)
        };
        let interactive = open && self.shown;
        let popup_focus = self.popup.focus_handle(cx);
        let popup = self.popup.clone();
        let trigger_bounds = self.trigger_bounds;
        let position =
            self.trigger_bounds.bottom_right() + point(px(0.), window.rem_size() * (4. / 16.));
        div().id("account-menu-popover").flex_shrink_0()
            .when(open && !self.shown, |root| root.on_mouse_down_out(cx.listener(|this, _, window, cx| {
                this.popup.update(cx, |popup, cx| popup.dismiss(window, cx));
            }))).child(
            Button::new("account-menu-trigger")
                .accessibility_label("账户菜单")
                .aria_description("当前使用本地访客工作区")
                .track_focus(&self.trigger_focus)
                .w(surface::css(46.))
                .h(surface::css(38.))
                .bg(if open { AccountMenuColors::trigger_active() } else { cx.theme().transparent })
                .hover(|button| button.bg(AccountMenuColors::trigger_active()))
                .focus_visible(|button| button.border_1().border_color(cx.theme().primary))
                .child(img("synapse/account-guest.svg").size(surface::css(20.)))
                .on_click(cx.listener(|this, _, window, cx| {
                    let open = this.popup.read(cx).is_open();
                    if !open { this.trigger_focus.focus(window, cx); }
                    this.popup.update(cx, |popup, cx| {
                        if open { popup.dismiss(window, cx); } else { popup.show(window, cx); }
                    });
                    if !open {
                        this.commands.update(cx, |list, cx| {
                            list.set_selected_index(Some(IndexPath::new(0).section(2)), window, cx);
                        });
                    }
                }))
                .on_prepaint({
                    let owner = cx.entity().downgrade();
                    move |bounds, _, cx| {
                        _ = owner.update(cx, |this, cx| {
                            if this.trigger_bounds != bounds {
                                this.trigger_bounds = bounds;
                                cx.notify();
                            }
                        });
                    }
                }))
            .when(open || self.rendered, |view| view.child(deferred(
                // Popup unconditionally occludes. Positioner supplies the same
                // anchoring/clamping while the hidden CSS phase passes input
                // through to the underlying page (pointer-events:none).
                Positioner::corner(Anchor::TopRight, position).margin(px(8.))
                    .when(interactive, |positioner| positioner.occlude()).child(
                div()
                    .id("account-menu")
                    .test_support()
                    .role(Role::Menu)
                    .aria_label("账户菜单")
                    .track_focus(&popup_focus)
                    .key_context("Popover")
                    .on_action(cx.listener(|this, _: &gpui_kit::base::actions::Cancel, window, cx| {
                        this.popup.update(cx, |popup, cx| popup.dismiss(window, cx));
                    }))
                    .relative()
                    .top(surface::css(-7. * (1. - movement)))
                    .opacity(opacity)
                    .w(surface::css(223.))
                    .p_0()
                    .px(surface::css(14.))
                    .py(surface::css(7.))
                    .border_1()
                    .border_color(AccountMenuColors::border())
                    .rounded(surface::css(5.))
                    .bg(AccountMenuColors::surface())
                    .when(open, |menu| menu.on_mouse_down_out(move |event, window, cx| {
                        if !trigger_bounds.contains(&event.position) {
                            popup.update(cx, |popup, cx| popup.dismiss(window, cx));
                        }
                    }))
                    .child(if interactive {
                        List::new(&self.commands).p_0().h(surface::css(116.)).into_any_element()
                    } else {
                        // Preserve appearance without List/Button hitboxes,
                        // keyboard handlers or tooltip timers during hiding.
                        v_flex().h(surface::css(116.)).children(AccountCommand::ALL.map(|command| {
                            v_flex().child(div()
                                .h(surface::css(26.)).pt(surface::css(5.)).pb(surface::css(4.))
                                .px(surface::css(18.)).text_size(surface::css(14.))
                                .line_height(surface::css(17.))
                                .text_color(cx.theme().foreground.opacity(if command.disabled_reason().is_some() { 0.3 } else { 1. }))
                                .child(command.label()))
                                .when(!matches!(command, AccountCommand::Exit), |row| row
                                    .child(div().h(surface::css(4.)))
                                    .child(div().h(surface::css(15.)).py(surface::css(7.))
                                        .child(div().h(surface::css(1.)).bg(AccountMenuColors::border()))))
                        })).into_any_element()
                    })
            )).with_priority(gpui_kit::base::POPUP_PRIORITY)))
    }
}

#[cfg(test)]
#[path = "account_menu_tests.rs"]
mod tests;
