//! Current Macro business IIFE and 58190/21700/1519; current contract:
//! docs/products/shared/macros.md. Local drafts remain separate from observed
//! recorder events and device metadata. CSS px scale with the shared rem unit.
use gpui_kit::base::{
    TestSupportExt as _,
    motion::{self, Easing, Presence, Transition},
};
use gpui_kit::component::{
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n as i18n;
use razer_widgets::keyboard_navigation::Button as BaseButton;
use razer_widgets::scroll::SourceScrollable as _;
use razer_widgets::source_input;
use razer_widgets::surface::css;
use razer_widgets::tutorial::TutorialIndicator;
use std::time::Duration;

#[path = "macro_page/bindings.rs"]
mod bindings;
#[path = "macro_page/body.rs"]
mod body;
#[path = "macro_page/chrome.rs"]
mod chrome;
#[path = "macro_page/editors.rs"]
mod editors;
#[path = "macro_page/keyboard.rs"]
mod keyboard;
#[path = "macro_page/keyboard_windows.rs"]
mod keyboard_windows;
#[path = "macro_page/launch.rs"]
mod launch;
#[path = "macro_page/library.rs"]
mod library;
#[path = "macro_page/nested.rs"]
mod nested;
#[path = "macro_page/nested_overlay.rs"]
mod nested_overlay;
#[path = "macro_page/palette.rs"]
mod palette;
#[path = "macro_page/phased.rs"]
mod phased;
#[path = "macro_page/record_help.rs"]
mod record_help;
#[path = "macro_page/record_options.rs"]
mod record_options;
#[path = "macro_page/record_shortcut.rs"]
mod record_shortcut;
#[path = "macro_page/recording.rs"]
mod recording;
#[path = "macro_page/tooltip.rs"]
mod tooltip;
use razer_model::macro_library::recording as recording_decode;
use razer_model::macro_library::{ExecutionSnapshot, MacroType};
use razer_service::mapping::recorder as recording_actor;
#[path = "macro_page/row_actions.rs"]
mod row_actions;
#[path = "macro_page/row_controls.rs"]
mod row_controls;
#[path = "macro_page/row_drag.rs"]
mod row_drag;
#[path = "macro_page/row_view.rs"]
mod row_view;
#[path = "macro_page/selection.rs"]
mod selection;
#[path = "macro_page/state.rs"]
mod state;
#[cfg(test)]
#[path = "macro_page/tests.rs"]
mod tests;
#[path = "macro_page/text.rs"]
mod text;
#[path = "macro_page/text_emoji.rs"]
mod text_emoji;
#[path = "macro_page/text_overlay.rs"]
mod text_overlay;
#[path = "macro_page/transfer.rs"]
mod transfer;
#[path = "macro_page/tree.rs"]
mod tree;
#[path = "macro_page/unsaved.rs"]
mod unsaved;
use state::{ActionItem, ActionKind, Entry, EntryKind, Sort, Tutorial, value_is_translation_key};

use row_drag::{ActionDrag, ActionDragPreview};

/// These are the source WindowStorage calls, independent of busy/recording UI.
pub use razer_service::mapping::recorder::StorageChange as MacroRecorderStorageChanged;
impl EventEmitter<MacroRecorderStorageChanged> for MacroPage {}

/// The mapping engine acknowledged the source recorder's enable operation.
#[derive(Clone, Copy)]
pub struct MacroGlobalShortcutsRestored;
impl EventEmitter<MacroGlobalShortcutsRestored> for MacroPage {}

fn tr(key: &str) -> String {
    i18n::t(&format!("MACRO_SOURCE.{key}")).to_string()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MacroTab {
    MyMacros,
    KeyBinds,
    Help,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DelayBound {
    Min,
    Max,
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

pub struct MacroPage {
    library: Entity<razer_pages::features::macro_library::MacroLibrary>,
    library_snapshot: razer_pages::features::macro_library::MacroLibraryFile,
    tab: MacroTab,
    focus: FocusHandle,
    history: Vec<MacroTab>,
    history_index: usize,
    entries: Vec<Entry>,
    next_id: u64,
    current: Option<u64>,
    actions_for: Option<u64>,
    actions: Vec<ActionItem>,
    selected_actions: Vec<usize>,
    hovered_action: Option<usize>,
    saved_actions_for: Option<u64>,
    saved_actions: Vec<ActionItem>,
    actions_dirty: bool,
    execution: Option<ExecutionSnapshot>,
    undo: Vec<Vec<ActionItem>>,
    redo: Vec<Vec<ActionItem>>,
    inactive_drafts: std::collections::HashMap<u64, unsaved::ActionDraft>,
    suspended_action: Option<unsaved::PendingAction>,
    unsaved_focus: FocusHandle,
    unsaved_return_focus: Option<FocusHandle>,
    tree_selection: Option<u64>,
    tutorial: Tutorial,
    selector_open: bool,
    more_open: bool,
    sort_open: bool,
    tree_menu: Option<u64>,
    tree_scrolling: bool,
    transfer_busy: bool,
    transfer_task: Option<transfer::PendingTransfer>,
    tree_hover: Option<u64>,
    sort: Sort,
    search: Entity<InputState>,
    name: Entity<InputState>,
    action_editor: Entity<InputState>,
    text_editor: Entity<TextareaState>,
    text_ui: text::TextUi,
    launch_ui: launch::LaunchUi,
    keyboard_ui: keyboard::KeyboardUi,
    record_ui: record_options::RecordUi,
    recording: recording::RecordingUi,
    phased_ui: phased::PhasedUi,
    delay_min_editor: Entity<InputState>,
    delay_max_editor: Entity<InputState>,
    randomized_open: Option<usize>,
    editing_action: Option<usize>,
    numeric_generation: u64,
    action_edit_tasks: Vec<Task<()>>,
    launch_open: Option<usize>,
    choice_action: Option<usize>,
    source_viewport: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>,
    rename: Option<u64>,
    rename_in_tree: bool,
    deletion: Option<u64>,
    locale: String,
    search_focused: bool,
    devices: Vec<Entity<razer_pages::features::ProductWorkspace>>,
    device_subscriptions: Vec<Subscription>,
    local_bindings: Vec<bindings::LocalBinding>,
    binding_menu: Option<String>,
    binding_dialog: Option<Entity<bindings::BindingDialog>>,
    binding_subscription: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
}

impl MacroPage {
    pub fn new(
        library: Entity<razer_pages::features::macro_library::MacroLibrary>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let file = library.read(cx).snapshot();
        let saved_actions = file
            .entries
            .iter()
            .find(|entry| Some(entry.id) == file.current)
            .map(|entry| entry.actions.clone())
            .unwrap_or_default();
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(tr("TEXT_MACRO_SEARCH")));
        let name = cx.new(|cx| source_input::input_state(32, window, cx));
        let action_editor = cx.new(|cx| InputState::new(window, cx));
        let text_editor = cx.new(|cx| {
            TextareaState::new(window, cx).placeholder(tr("TEXT_TEXT_FUNCTION_PLACEHOLDER"))
        });
        let text_search =
            cx.new(|cx| InputState::new(window, cx).placeholder(tr("TEXT_SEARCH_EMOJI")));
        let delay_min_editor = cx.new(|cx| {
            InputState::new(window, cx).validate(|value, _| editors::valid_randomized_draft(value))
        });
        let delay_max_editor = cx.new(|cx| {
            InputState::new(window, cx).validate(|value, _| editors::valid_randomized_draft(value))
        });
        let keyboard_focus = cx.focus_handle();
        let mut record_ui = record_options::RecordUi::new(window, cx);
        record_ui.shortcut.selected = file.record_shortcut.clone();
        let mut record_subscriptions = [
            (&record_ui.fixed, record_options::NumberField::Fixed),
            (&record_ui.min, record_options::NumberField::Min),
            (&record_ui.max, record_options::NumberField::Max),
        ]
        .into_iter()
        .map(|(input, field)| {
            cx.subscribe_in(input, window, move |this, _, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.record_number_changed(field, window, cx);
                }
                if matches!(event, InputEvent::Blur) {
                    this.record_number_blurred(field, window, cx);
                }
                if matches!(event, InputEvent::Focus | InputEvent::Blur) {
                    cx.notify();
                }
            })
        })
        .collect::<Vec<_>>();
        for (index, focus) in record_ui.help_focus.iter().enumerate() {
            record_subscriptions.push(cx.on_focus(focus, window, move |this, _, cx| {
                this.record_ui.set_help(index, true);
                cx.notify();
            }));
            record_subscriptions.push(cx.on_blur(focus, window, move |this, _, cx| {
                this.record_ui.set_help(index, false);
                cx.notify();
            }));
        }
        let subscriptions = vec![
            cx.subscribe(&record_ui.start, |this, _, event, cx| {
                if let select::SelectEvent::Confirm(Some(value)) = event {
                    this.set_record_start_delay(value, cx);
                }
            }),
            cx.subscribe_in(
                &library,
                window,
                |this,
                 _,
                 _: &razer_pages::features::macro_library::MacroLibraryChanged,
                 window,
                 cx| {
                    this.receive_library(window, cx);
                },
            ),
            cx.subscribe_in(
                &library,
                window,
                |this,
                 _,
                 _: &razer_pages::features::macro_library::MacroLibraryProfileImported,
                 window,
                 cx| {
                    this.receive_library(window, cx);
                },
            ),
            cx.on_blur(&keyboard_focus, window, |this, _, cx| {
                this.finish_keyboard_editor();
                cx.notify();
            }),
            cx.subscribe_in(&delay_min_editor, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.update_randomized_range(DelayBound::Min, window, cx);
                } else if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    this.finish_randomized_range(matches!(event, InputEvent::Blur), window, cx);
                }
            }),
            cx.subscribe_in(&delay_max_editor, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.update_randomized_range(DelayBound::Max, window, cx);
                } else if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    this.finish_randomized_range(matches!(event, InputEvent::Blur), window, cx);
                }
            }),
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
            cx.subscribe_in(&action_editor, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.action_input_changed(window, cx);
                    cx.notify();
                }
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. })
                    && !this
                        .editing_action
                        .and_then(|index| this.actions().get(index))
                        .is_some_and(|item| item.kind == ActionKind::Text)
                {
                    this.finish_action_edit(window, cx);
                }
            }),
            cx.subscribe_in(&text_editor, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.text_input_changed(window, cx);
                }
            }),
            cx.subscribe(&text_search, |this, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    this.text_search_changed(cx);
                }
                if matches!(event, InputEvent::Focus | InputEvent::Blur) {
                    cx.notify();
                }
            }),
        ];
        let mut page = Self {
            library,
            library_snapshot: file.clone(),
            tab: MacroTab::MyMacros,
            focus: cx.focus_handle(),
            history: vec![MacroTab::MyMacros],
            history_index: 0,
            entries: file.entries,
            next_id: file.next_id,
            current: file.current,
            actions_for: file.current,
            actions: saved_actions.clone(),
            selected_actions: Vec::new(),
            hovered_action: None,
            saved_actions_for: file.current,
            saved_actions,
            actions_dirty: false,
            execution: None,
            undo: Vec::new(),
            redo: Vec::new(),
            inactive_drafts: Default::default(),
            suspended_action: None,
            unsaved_focus: cx.focus_handle(),
            unsaved_return_focus: None,
            tree_selection: file.current,
            tutorial: file.tutorial,
            selector_open: false,
            more_open: false,
            sort_open: false,
            tree_menu: None,
            tree_scrolling: false,
            transfer_busy: false,
            transfer_task: None,
            tree_hover: None,
            sort: Sort::Ascending,
            search,
            name,
            action_editor,
            text_editor,
            text_ui: text::TextUi::new(text_search),
            launch_ui: Default::default(),
            keyboard_ui: keyboard::KeyboardUi::new(keyboard_focus),
            record_ui,
            recording: Default::default(),
            phased_ui: Default::default(),
            delay_min_editor,
            delay_max_editor,
            randomized_open: None,
            editing_action: None,
            numeric_generation: 0,
            action_edit_tasks: Vec::new(),
            launch_open: None,
            choice_action: None,
            source_viewport: Default::default(),
            rename: None,
            rename_in_tree: false,
            deletion: None,
            locale: i18n::locale(),
            search_focused: false,
            devices: Vec::new(),
            device_subscriptions: Vec::new(),
            local_bindings: Vec::new(),
            binding_menu: None,
            binding_dialog: None,
            binding_subscription: None,
            _subscriptions: subscriptions
                .into_iter()
                .chain(record_subscriptions)
                .collect(),
        };
        page.adopt_record_preferences(
            page.library_snapshot.recording_preferences.clone(),
            window,
            cx,
        );
        page.load_current_execution();
        page.sync_phased_mount(cx);
        page.refresh_translated_actions(window, cx);
        page.activate_record_shortcut(window, cx);
        page
    }
    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_locale(window, cx);
        self.receive_library(window, cx);
        self.activate_record_shortcut(window, cx);
        self.focus.focus(window, cx);
        cx.notify();
    }
    pub fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = i18n::locale();
        if self.locale == locale {
            return;
        }
        self.locale = locale;
        self.refresh_translated_actions(window, cx);
        self.update_search_placeholder(window, cx);
        self.text_editor.update(cx, |input, cx| {
            input.set_placeholder(tr("TEXT_TEXT_FUNCTION_PLACEHOLDER"), window, cx)
        });
        self.text_ui.search.update(cx, |input, cx| {
            input.set_placeholder(tr("TEXT_SEARCH_EMOJI"), window, cx)
        });
        if let Some(dialog) = &self.binding_dialog {
            dialog.update(cx, |dialog, cx| dialog.refresh_locale(window, cx));
        }
        cx.notify();
    }
    fn load_current_actions(&mut self) {
        self.record_ui.close();
        self.clear_action_editors();
        self.actions_for = self.current;
        self.actions = self
            .entries
            .iter()
            .find(|entry| Some(entry.id) == self.current)
            .map(|entry| entry.actions.clone())
            .unwrap_or_default();
        self.saved_actions_for = self.current;
        self.saved_actions = self.actions.clone();
        self.actions_dirty = false;
        self.load_current_execution();
        self.undo.clear();
        self.redo.clear();
        self.selected_actions.clear();
        if let Some(draft) = self.current.and_then(|id| self.inactive_drafts.remove(&id)) {
            self.actions = draft.actions;
            self.undo = draft.undo;
            self.redo = draft.redo;
            self.selected_actions = draft.selected;
            self.actions_dirty = draft.actions_dirty;
            self.execution = draft.execution;
        }
    }
    pub fn has_previous_page(&self) -> bool {
        self.history_index > 0
    }
    pub fn has_next_page(&self) -> bool {
        self.history_index + 1 < self.history.len()
    }
    pub fn step_history(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.recording_busy() {
            return;
        }
        if forward && self.has_next_page() {
            self.history_index += 1;
        } else if !forward && self.has_previous_page() {
            self.history_index -= 1;
        } else {
            return;
        }
        self.tab = self.history[self.history_index];
        self.finish_pending_edits(window, cx);
        self.sync_phased_mount(cx);
        self.dismiss_transient_ui(window, cx);
    }
    pub fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.request_action(unsaved::PendingAction::Refresh, window, cx);
    }
    fn dismiss_transient_ui(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.record_ui.close();
        self.selector_open = false;
        self.more_open = false;
        self.sort_open = false;
        self.tree_menu = None;
        self.deletion = None;
        if let Some(dialog) = self.binding_dialog.take() {
            dialog.update(cx, |dialog, cx| dialog.close(window, cx));
        }
        self.binding_subscription = None;
        self.binding_menu = None;
        self.rename = None;
        self.rename_in_tree = false;
        self.clear_action_editors();
        self.search
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.focus.focus(window, cx);
        cx.notify();
    }
    fn set_tab(&mut self, tab: MacroTab, window: &mut Window, cx: &mut Context<Self>) {
        if self.recording_busy() || self.tab == tab {
            return;
        }
        self.tab = tab;
        self.history.truncate(self.history_index + 1);
        self.history.push(tab);
        self.history_index = self.history.len() - 1;
        self.finish_pending_edits(window, cx);
        self.sync_phased_mount(cx);
        self.dismiss_transient_ui(window, cx);
    }
    fn macro_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.kind == EntryKind::Macro)
            .count()
    }
    /// Mn disables the entire profile bar during recording/settings, Help and
    /// the two active tutorial steps. Initial creation remains available.
    fn profile_actions_blocked(&self) -> bool {
        self.recording_busy()
            || self.record_ui.open
            || self.tab == MacroTab::Help
            || matches!(self.tutorial, Tutorial::Record | Tutorial::Add)
            || self.suspended_action.is_some()
    }
    fn actions(&self) -> &[ActionItem] {
        if self.actions_for == self.current {
            if self.recording_busy() {
                &self.recording.preview
            } else {
                &self.actions
            }
        } else {
            &[]
        }
    }

    fn clear_action_editors(&mut self) {
        self.record_ui.close();
        self.finish_keyboard_editor();
        self.cancel_action_editors();
    }
    fn cancel_action_editors(&mut self) {
        self.action_edit_tasks.clear();
        self.numeric_generation = self.numeric_generation.wrapping_add(1);
        self.hovered_action = None;
        self.cancel_keyboard_editor();
        self.editing_action = None;
        self.text_ui.close_emoji();
        self.cancel_launch_editor();
        self.randomized_open = None;
        self.choice_action = None;
    }
    pub fn add_action(&mut self, kind: &str, cx: &mut Context<Self>) {
        let index = self
            .selected_actions
            .iter()
            .copied()
            .max()
            .map_or(self.actions().len(), |index| index + 1);
        self.add_action_at(kind, index, cx);
    }
    pub fn can_undo(&self) -> bool {
        !self.recording_busy() && !self.record_ui.open && !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.recording_busy() && !self.record_ui.open && !self.redo.is_empty()
    }
    pub fn can_save(&self) -> bool {
        !self.recording_busy()
            && self.current.is_some()
            && self.actions_for == self.current
            && (self.saved_actions_for != self.current
                || self.actions != self.saved_actions
                || self.actions_dirty)
    }
    fn can_save_with_pending(&self, cx: &App) -> bool {
        if self.recording_busy() {
            return false;
        }
        self.can_save()
            || self.keyboard_pending()
            || self
                .editing_action
                .and_then(|index| self.actions().get(index))
                .is_some_and(|item| {
                    item.kind != ActionKind::Text
                        && editors::committed_value(item.kind, &self.action_editor.read(cx).value())
                            != item.value
                })
            || self
                .randomized_open
                .and_then(|index| self.actions().get(index))
                .is_some_and(|item| {
                    item.number_min != self.delay_min_editor.read(cx).value().as_ref()
                        || item.number_max != self.delay_max_editor.read(cx).value().as_ref()
                })
    }

    pub fn begin_action_edit(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.recording_busy() || self.record_ui.open {
            return;
        }
        if self
            .actions()
            .get(index)
            .is_some_and(|item| item.kind == ActionKind::Keyboard)
        {
            self.open_keyboard_editor(index, window, cx);
            return;
        }
        self.finish_keyboard_editor();
        if self
            .actions()
            .get(index)
            .is_some_and(|item| item.kind == ActionKind::Text)
        {
            self.open_text_editor(index, window, cx);
            return;
        }
        if self.editing_action.is_some() {
            self.finish_action_edit(window, cx);
        }
        self.finish_randomized_range(false, window, cx);
        self.cancel_launch_editor();
        let Some(item) = self.actions().get(index).cloned() else {
            return;
        };
        let kind = item.kind;
        if matches!(
            kind,
            ActionKind::Macro | ActionKind::Launch | ActionKind::Mouse
        ) {
            return;
        }
        let value = item.value;
        self.action_edit_tasks.clear();
        self.numeric_generation = self.numeric_generation.wrapping_add(1);
        self.editing_action = Some(index);
        self.action_editor.update(cx, |input, cx| {
            input.set_value(value, window, cx);
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        cx.notify();
    }

    fn finish_action_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.action_edit_tasks.clear();
        self.numeric_generation = self.numeric_generation.wrapping_add(1);
        let Some(index) = self.editing_action.take() else {
            return;
        };
        if index >= self.actions.len() || self.actions_for != self.current {
            return;
        }
        let kind = self.actions[index].kind;
        if kind == ActionKind::Text {
            // dn's outside/close path discards its modal draft. Only its own
            // Save dispatches updateMacroItem; an outer Save must not commit it.
            self.text_ui.close_emoji();
            cx.notify();
            return;
        }
        let value = editors::committed_value(kind, &self.action_editor.read(cx).value());
        let current = &self.actions[index].value;
        if value == *current
            && !(kind == ActionKind::Delay && self.actions[index].state == "randomized")
        {
            cx.notify();
            return;
        }
        self.checkpoint_actions();
        self.actions[index].value = value;
        if kind == ActionKind::Delay {
            self.actions[index].state = "fixed".into();
        }
        self.sync_loop_value(index);
        self.redo.clear();
        self.rebuild_action_execution();
        window.blur(cx);
        cx.notify();
    }

    pub fn begin_delay_bound_edit(
        &mut self,
        index: usize,
        bound: DelayBound,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(item) = self.actions.get(index).cloned() else {
            return;
        };
        if item.kind != ActionKind::Delay
            || item.state != "randomized"
            || self.record_delay() != 2
            || self.recording_busy()
            || self.record_ui.open
        {
            return;
        }
        if self.editing_action.is_some() {
            self.finish_action_edit(window, cx);
        }
        if self.randomized_open != Some(index) {
            self.finish_randomized_range(false, window, cx);
        }
        self.cancel_launch_editor();
        self.randomized_open = Some(index);
        self.delay_min_editor.update(cx, |input, cx| {
            input.set_value(item.number_min, window, cx);
        });
        self.delay_max_editor.update(cx, |input, cx| {
            input.set_value(item.number_max, window, cx);
        });
        let editor = if bound == DelayBound::Min {
            &self.delay_min_editor
        } else {
            &self.delay_max_editor
        };
        editor.update(cx, |input, cx| {
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        cx.notify();
    }

    fn update_randomized_range(
        &mut self,
        bound: DelayBound,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.randomized_open.is_none() || self.record_delay() != 2 {
            return;
        }
        let raw_min = self.delay_min_editor.read(cx).value().to_string();
        let raw_max = self.delay_max_editor.read(cx).value().to_string();
        let (min, max) = editors::normalize_randomized_range(&raw_min, &raw_max, bound);
        if min != raw_min {
            self.delay_min_editor
                .update(cx, |input, cx| input.set_value(min, window, cx));
        }
        if max != raw_max {
            self.delay_max_editor
                .update(cx, |input, cx| input.set_value(max, window, cx));
        }
        cx.notify();
    }

    fn finish_randomized_range(
        &mut self,
        only_if_blurred: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if only_if_blurred {
            let focused = window.focused(cx);
            if focused == Some(self.delay_min_editor.read(cx).focus_handle(cx))
                || focused == Some(self.delay_max_editor.read(cx).focus_handle(cx))
            {
                return;
            }
        }
        let Some(index) = self.randomized_open.take() else {
            return;
        };
        let min = self.delay_min_editor.read(cx).value().to_string();
        let max = self.delay_max_editor.read(cx).value().to_string();
        let Some(item) = self.actions.get(index) else {
            return;
        };
        if item.kind != ActionKind::Delay
            || item.state != "randomized"
            || self.record_delay() != 2
            || self.actions_for != self.current
        {
            return;
        }
        if item.number_min != min || item.number_max != max {
            self.checkpoint_actions();
            self.actions[index].number_min = min;
            self.actions[index].number_max = max;
            self.redo.clear();
            self.rebuild_action_execution();
        }
        cx.notify();
    }

    pub fn add_action_at(&mut self, kind: &str, index: usize, cx: &mut Context<Self>) {
        self.add_action_at_phase(kind, index, self.active_phase(), cx);
    }

    fn add_action_at_phase(
        &mut self,
        kind: &str,
        index: usize,
        phase: Option<u8>,
        cx: &mut Context<Self>,
    ) {
        let Some(current) = self.current else { return };
        if self.tutorial != Tutorial::Complete || self.recording_busy() || self.record_ui.open {
            return;
        }
        let Some(kind) = ActionKind::from_palette(kind) else {
            return;
        };
        if self.actions_for != Some(current) {
            self.actions_for = Some(current);
            self.actions.clear();
            self.saved_actions_for = Some(current);
            self.saved_actions.clear();
            self.undo.clear();
            self.redo.clear();
        }
        self.finish_keyboard_editor();
        let Some(items) = self.new_action_items(kind) else {
            return;
        };
        let mut items = items;
        if self.current_macro_type() == razer_pages::features::macro_library::MacroType::Phased {
            for item in &mut items {
                item.phase = phase;
            }
        }
        self.checkpoint_actions();
        let index = index.min(self.actions.len());
        for selected in &mut self.selected_actions {
            if *selected >= index {
                *selected += items.len();
            }
        }
        self.actions.splice(index..index, items);
        if self.current_macro_type() == razer_pages::features::macro_library::MacroType::Phased {
            self.normalize_phased_rows();
        }
        self.clear_action_editors();
        self.redo.clear();
        self.rebuild_action_execution();
        cx.notify();
    }
    pub fn save_actions(&mut self, cx: &mut Context<Self>) -> bool {
        self.finish_keyboard_editor();
        if !self.can_save() {
            return true;
        }
        let Some(entry) = self
            .entries
            .iter()
            .find(|entry| Some(entry.id) == self.current)
        else {
            return false;
        };
        let execution = match self.library_snapshot.execution_for_actions(
            entry,
            &self.actions,
            self.execution.as_ref(),
            self.recording_movement_setting(),
        ) {
            Ok(execution) => execution,
            Err(error) => {
                self.recording.error = Some(error);
                cx.notify();
                return false;
            }
        };
        self.saved_actions_for = self.current;
        self.saved_actions = self.actions.clone();
        self.actions_dirty = false;
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| Some(entry.id) == self.current)
        {
            entry.actions = self.saved_actions.clone();
            entry.execution = Some(execution.clone());
        }
        self.execution = Some(execution);
        self.publish_library(cx);
        self.undo.clear();
        self.redo.clear();
        self.selected_actions.clear();
        cx.notify();
        true
    }
    pub fn undo_action(&mut self, cx: &mut Context<Self>) {
        if !self.can_undo() {
            return;
        }
        self.finish_keyboard_editor();
        if let Some(previous) = self.undo.pop() {
            self.invalidate_execution();
            self.redo.push(self.actions.clone());
            self.actions = previous;
            self.rebuild_action_execution();
            self.clear_action_editors();
            self.selected_actions.clear();
            cx.notify();
        }
    }
    pub fn redo_action(&mut self, cx: &mut Context<Self>) {
        if !self.can_redo() {
            return;
        }
        self.finish_keyboard_editor();
        if let Some(next) = self.redo.pop() {
            self.checkpoint_actions();
            self.actions = next;
            self.rebuild_action_execution();
            self.clear_action_editors();
            self.selected_actions.clear();
            cx.notify();
        }
    }
    pub fn toggle_action_selection(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.recording_busy() || self.record_ui.open || self.tutorial != Tutorial::Complete {
            return;
        }
        if index >= self.actions().len() {
            return;
        }
        if let Some(position) = self
            .selected_actions
            .iter()
            .position(|&selected| selected == index)
        {
            self.selected_actions.remove(position);
        } else {
            self.selected_actions.push(index);
        }
        cx.notify();
    }

    pub fn delete_selected_actions(&mut self, cx: &mut Context<Self>) {
        if self.recording_busy() || self.record_ui.open || self.tutorial != Tutorial::Complete {
            return;
        }
        self.finish_keyboard_editor();
        if self.selected_actions.is_empty() || self.actions_for != self.current {
            return;
        }
        self.checkpoint_actions();
        let selected: std::collections::HashSet<_> = self.selected_actions.drain(..).collect();
        self.actions = self
            .actions
            .iter()
            .enumerate()
            .filter_map(|(index, action)| (!selected.contains(&index)).then_some(action.clone()))
            .collect();
        self.clear_action_editors();
        self.redo.clear();
        self.rebuild_action_execution();
        cx.notify();
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

fn format_delay(value: f64) -> String {
    let bits = value.clamp(0., 99_999.999).to_bits();
    let shift = 1075 - ((bits >> 52) & 0x7ff) as u32;
    if shift >= 128 {
        return "0".into();
    }
    // St.w uses Number(value.toFixed(3)): round the exact binary value,
    // choosing the larger decimal integer on a tie. Multiplying f64 first
    // can change which side of the tie an entered value lies on.
    let scaled = u128::from((bits & ((1 << 52) - 1)) | (1 << 52)) * 1000;
    let divisor = 1u128 << shift;
    let rounded = (scaled + divisor / 2) / divisor;
    (rounded as f64 / 1000.).to_string()
}

impl Render for MacroPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.tab {
            MacroTab::MyMacros => self.editor(window, cx),
            MacroTab::KeyBinds => self.key_binds(window, cx),
            MacroTab::Help => self.help(cx),
        };
        v_flex()
            .id("macro-window")
            .test_support()
            .on_prepaint({
                let viewport = self.source_viewport.clone();
                move |bounds, _, _| viewport.set(bounds)
            })
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
                    if this.suspended_action.is_some() {
                        this.cancel_suspended_action(window, cx);
                    } else {
                        this.finish_pending_edits(window, cx);
                        this.dismiss_transient_ui(window, cx);
                    }
                    cx.stop_propagation();
                }
            }))
            .child(self.navigation(window, cx))
            .when(self.transfer_busy, |root| {
                root.child(
                    div()
                        .id("macro-transfer-status")
                        .test_support()
                        .role(Role::Status)
                        .aria_label("Macro file transfer in progress"),
                )
            })
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
            .when_some(self.binding_dialog.clone(), |root, dialog| {
                root.child(dialog)
            })
            .when(self.suspended_action.is_some(), |root| {
                root.child(self.unsaved_confirmation(window, cx))
            })
    }
}
