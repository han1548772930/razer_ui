//! Current Macro business IIFE and 58190/21700/1519; see the scoped review in
//! docs/re/macro-current-source-review.md. Only explicitly created local data is
//! represented here. Recording, service devices and capability flags are not
//! fabricated. CSS coordinates below are CSS px, scaled by the shared rem unit.
use crate::{
    i18n,
    ui::{scroll::SourceScrollable as _, surface::css, tutorial::TutorialIndicator},
};
use gpui_kit::base::{
    Button as BaseButton,
    motion::{self, Easing, Presence, Transition},
};
use gpui_kit::component::{
    input::{Input, InputEvent, InputState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::time::Duration;

mod body;
mod chrome;
mod state;
mod tree;
use state::{Entry, EntryKind, Sort, Tutorial};

fn tr(key: &str) -> String {
    i18n::t(&format!("MACRO_SOURCE.{key}")).to_string()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MacroTab {
    MyMacros,
    KeyBinds,
    Help,
}
impl MacroTab {
    fn key(self) -> &'static str {
        match self {
            Self::MyMacros => "TEXT_NAV_TAB_MY_MACROS",
            Self::KeyBinds => "TEXT_NAV_TAB_KEY_BINDS",
            Self::Help => "HELP",
        }
    }
    fn id(self) -> &'static str {
        match self {
            Self::MyMacros => "macro-my-macros",
            Self::KeyBinds => "macro-key-binds",
            Self::Help => "macro-help",
        }
    }
}

pub(super) struct MacroPage {
    tab: MacroTab,
    focus: FocusHandle,
    history: Vec<MacroTab>,
    history_index: usize,
    entries: Vec<Entry>,
    next_id: u64,
    current: Option<u64>,
    tree_selection: Option<u64>,
    tutorial: Tutorial,
    selector_open: bool,
    more_open: bool,
    sort_open: bool,
    tree_menu: Option<u64>,
    tree_scrolling: bool,
    tree_hover: Option<u64>,
    sort: Sort,
    search: Entity<InputState>,
    name: Entity<InputState>,
    rename: Option<u64>,
    rename_in_tree: bool,
    deletion: Option<u64>,
    locale: String,
    search_focused: bool,
    _subscriptions: Vec<Subscription>,
}

impl MacroPage {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(tr("TEXT_MACRO_SEARCH")));
        let name = cx.new(|cx| {
            InputState::new(window, cx).validate(|value, _| value.encode_utf16().count() <= 32)
        });
        let subscriptions = vec![
            cx.observe(&search, |_, _, cx| cx.notify()),
            cx.subscribe_in(&search, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::Focus | InputEvent::Blur) {
                    this.search_focused = matches!(event, InputEvent::Focus);
                    this.update_search_placeholder(window, cx);
                }
            }),
            cx.subscribe_in(&name, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    this.finish_rename(cx);
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.focus.focus(window, cx);
                    }
                }
            }),
        ];
        Self {
            tab: MacroTab::MyMacros,
            focus: cx.focus_handle(),
            history: vec![MacroTab::MyMacros],
            history_index: 0,
            entries: Vec::new(),
            next_id: 1,
            current: None,
            tree_selection: None,
            tutorial: Tutorial::Initial,
            selector_open: false,
            more_open: false,
            sort_open: false,
            tree_menu: None,
            tree_scrolling: false,
            tree_hover: None,
            sort: Sort::Ascending,
            search,
            name,
            rename: None,
            rename_in_tree: false,
            deletion: None,
            locale: i18n::locale(),
            search_focused: false,
            _subscriptions: subscriptions,
        }
    }
    pub(super) fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
        cx.notify();
    }
    pub(super) fn has_previous_page(&self) -> bool {
        self.history_index > 0
    }
    pub(super) fn has_next_page(&self) -> bool {
        self.history_index + 1 < self.history.len()
    }
    pub(super) fn step_history(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if forward && self.has_next_page() {
            self.history_index += 1;
        } else if !forward && self.has_previous_page() {
            self.history_index -= 1;
        } else {
            return;
        }
        self.tab = self.history[self.history_index];
        self.refresh(window, cx);
    }
    pub(super) fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Local entries currently have no unsaved event edits to discard.
        self.selector_open = false;
        self.more_open = false;
        self.sort_open = false;
        self.tree_menu = None;
        self.deletion = None;
        self.rename = None;
        self.rename_in_tree = false;
        self.search
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.focus.focus(window, cx);
        cx.notify();
    }
    fn set_tab(&mut self, tab: MacroTab, window: &mut Window, cx: &mut Context<Self>) {
        if self.tab == tab {
            return;
        }
        self.tab = tab;
        self.history.truncate(self.history_index + 1);
        self.history.push(tab);
        self.history_index = self.history.len() - 1;
        self.refresh(window, cx);
    }
    fn macro_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.kind == EntryKind::Macro)
            .count()
    }
    fn selected_name(&self) -> String {
        self.entries
            .iter()
            .find(|e| Some(e.id) == self.current)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| tr("TEXT_PROFILE_BAR_DROPDOWN"))
    }
    fn update_search_placeholder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let placeholder = if self.search_focused {
            String::new()
        } else {
            tr("TEXT_MACRO_SEARCH")
        };
        self.search.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx)
        });
        cx.notify();
    }
}

impl Render for MacroPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.locale != i18n::locale() {
            self.locale = i18n::locale();
            self.update_search_placeholder(window, cx);
        }
        let body = match self.tab {
            MacroTab::MyMacros => self.editor(window, cx),
            MacroTab::KeyBinds => self.key_binds(cx),
            MacroTab::Help => self.help(cx),
        };
        v_flex()
            .id("macro-window")
            .size_full()
            .relative()
            .bg(rgb(0x222222))
            .font_family("Roboto")
            .text_color(rgb(0xcccccc))
            .text_size(css(14.))
            .line_height(css(17.))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    this.refresh(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(self.navigation(window, cx))
            .child(
                div()
                    .id("macro-body-scroll")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .scrollable_both()
                    .child(body),
            )
            .when(self.deletion.is_some(), |root| {
                root.child(self.delete_confirmation(window, cx))
            })
    }
}
