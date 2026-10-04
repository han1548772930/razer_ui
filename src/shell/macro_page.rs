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
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::time::Duration;

mod body;
mod chrome;
mod editors;
mod state;
mod tree;
use state::{ActionItem, ActionKind, Entry, EntryKind, Sort, Tutorial};

/// Payload carried while reordering event rows in the current macro editor.
/// The page id prevents a drag that started in another MacroPage instance from
/// mutating this editor when independent windows are open.
#[derive(Clone)]
pub(super) struct ActionDrag {
    pub(super) page: EntityId,
    pub(super) index: usize,
    pub(super) palette_kind: Option<&'static str>,
}

/// The web editor uses the browser's drag image for functional items. GPUI
/// still requires a preview entity for the drag lifetime; keeping it empty
/// preserves the source's unobtrusive row drag while the insertion highlight
/// is painted by the target row.
pub(super) struct ActionDragPreview;
impl Render for ActionDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

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
pub(super) enum DelayBound {
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

pub(super) struct MacroPage {
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
    saved_actions_for: Option<u64>,
    saved_actions: Vec<ActionItem>,
    undo: Vec<Vec<ActionItem>>,
    redo: Vec<Vec<ActionItem>>,
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
    action_editor: Entity<InputState>,
    text_editor: Entity<TextareaState>,
    launch_program: Entity<InputState>,
    launch_website: Entity<InputState>,
    launch_is_website: bool,
    delay_min_editor: Entity<InputState>,
    delay_max_editor: Entity<InputState>,
    randomized_open: Option<usize>,
    editing_action: Option<usize>,
    launch_open: Option<usize>,
    choice_action: Option<usize>,
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
        let action_editor = cx.new(|cx| InputState::new(window, cx));
        let text_editor = cx.new(|cx| {
            TextareaState::new(window, cx).placeholder(tr("TEXT_TEXT_FUNCTION_PLACEHOLDER"))
        });
        let launch_program = cx.new(|cx| InputState::new(window, cx));
        let launch_website = cx.new(|cx| InputState::new(window, cx));
        let delay_min_editor = cx.new(|cx| {
            InputState::new(window, cx).validate(|value, _| editors::valid_randomized_draft(value))
        });
        let delay_max_editor = cx.new(|cx| {
            InputState::new(window, cx).validate(|value, _| editors::valid_randomized_draft(value))
        });
        let subscriptions = vec![
            cx.observe(&launch_program, |_, _, cx| cx.notify()),
            cx.observe(&launch_website, |_, _, cx| cx.notify()),
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
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. })
                    && !this
                        .editing_action
                        .and_then(|index| this.actions().get(index))
                        .is_some_and(|item| item.kind == ActionKind::Text)
                {
                    this.finish_action_edit(window, cx);
                }
            }),
            cx.subscribe_in(&text_editor, window, |_, editor, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    let value = editor.read(cx).value().to_string();
                    if value.encode_utf16().count() > 250 {
                        editor.update(cx, |input, cx| {
                            input.set_value(truncate_utf16(&value, 250), window, cx);
                        });
                    }
                    cx.notify();
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
            actions_for: None,
            actions: Vec::new(),
            selected_actions: Vec::new(),
            saved_actions_for: None,
            saved_actions: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
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
            action_editor,
            text_editor,
            launch_program,
            launch_website,
            launch_is_website: false,
            delay_min_editor,
            delay_max_editor,
            randomized_open: None,
            editing_action: None,
            launch_open: None,
            choice_action: None,
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
        self.clear_action_editors();
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
    fn actions(&self) -> &[ActionItem] {
        if self.actions_for == self.current {
            &self.actions
        } else {
            &[]
        }
    }

    fn clear_action_editors(&mut self) {
        self.editing_action = None;
        self.launch_open = None;
        self.launch_is_website = false;
        self.randomized_open = None;
        self.choice_action = None;
    }
    pub(super) fn add_action(&mut self, kind: &str, cx: &mut Context<Self>) {
        let Some(current) = self.current else { return };
        if self.tutorial != Tutorial::Complete {
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
        self.undo.push(self.actions.clone());
        self.actions.push(ActionItem::new(kind));
        self.clear_action_editors();
        self.selected_actions.clear();
        self.redo.clear();
        cx.notify();
    }
    pub(super) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub(super) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub(super) fn can_save(&self) -> bool {
        self.actions_for == self.current
            && (self.saved_actions_for != self.current || self.actions != self.saved_actions)
    }

    pub(super) fn begin_action_edit(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editing_action.is_some() {
            self.finish_action_edit(window, cx);
        }
        self.finish_randomized_range(false, window, cx);
        self.launch_open = None;
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
        self.editing_action = Some(index);
        if kind == ActionKind::Text {
            self.text_editor.update(cx, |input, cx| {
                input.set_value(value, window, cx);
                input.focus(window, cx);
                input.select_all(window, cx);
            });
        } else {
            self.action_editor.update(cx, |input, cx| {
                input.set_value(value, window, cx);
                input.focus(window, cx);
                input.select_all(window, cx);
            });
        }
        cx.notify();
    }

    fn finish_action_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.editing_action.take() else {
            return;
        };
        if index >= self.actions.len() || self.actions_for != self.current {
            return;
        }
        let kind = self.actions[index].kind;
        let raw = if kind == ActionKind::Text {
            truncate_utf16(&self.text_editor.read(cx).value(), 250)
        } else {
            self.action_editor.read(cx).value().trim().to_string()
        };
        let value = match kind {
            ActionKind::Delay => raw
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .map(|value| format!("{:.3}", value.clamp(0.0, 99_999.999)))
                .unwrap_or_else(|| self.actions[index].value.clone()),
            ActionKind::Loop => raw
                .parse::<u32>()
                .ok()
                .map(|value| value.clamp(1, 99_999).to_string())
                .unwrap_or_else(|| self.actions[index].value.clone()),
            _ => raw,
        };
        let current = &self.actions[index].value;
        if value == *current {
            cx.notify();
            return;
        }
        self.undo.push(self.actions.clone());
        self.actions[index].value = value;
        self.redo.clear();
        window.blur(cx);
        cx.notify();
    }

    pub(super) fn open_launch_editor(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editing_action.is_some() {
            self.finish_action_edit(window, cx);
        }
        self.finish_randomized_range(false, window, cx);
        let Some(item) = self.actions.get(index).cloned() else {
            return;
        };
        if item.kind != ActionKind::Launch {
            return;
        }
        self.launch_open = Some(index);
        self.launch_is_website = item.state == "website";
        self.launch_program.update(cx, |input, cx| {
            input.set_value(item.value, window, cx);
        });
        self.launch_website.update(cx, |input, cx| {
            input.set_value(item.secondary_value, window, cx);
        });
        cx.notify();
    }

    pub(super) fn save_launch_editor(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.launch_open.take() else {
            return;
        };
        let Some(item) = self.actions.get(index) else {
            return;
        };
        if item.kind != ActionKind::Launch || self.actions_for != self.current {
            return;
        }
        let program = self.launch_program.read(cx).value().to_string();
        let website = self.launch_website.read(cx).value().to_string();
        let state = if self.launch_is_website {
            "website"
        } else {
            "program"
        };
        if item.value != program || item.secondary_value != website || item.state != state {
            self.undo.push(self.actions.clone());
            self.actions[index].value = program;
            self.actions[index].secondary_value = website;
            self.actions[index].state = state.to_string();
            self.redo.clear();
        }
        cx.notify();
    }

    pub(super) fn toggle_delay_randomized(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(item) = self.actions.get(index).cloned() else {
            return;
        };
        if item.kind != ActionKind::Delay {
            return;
        }
        self.undo.push(self.actions.clone());
        self.clear_action_editors();
        if item.state == "randomized" {
            self.actions[index].state = "fixed".to_string();
        } else {
            let fixed = parse_delay(&item.value).clamp(0.0, 5.0);
            let max = (fixed + 1.0).min(5.0);
            self.actions[index].state = "randomized".to_string();
            self.actions[index].number_min = fixed.to_string();
            self.actions[index].number_max = max.max(fixed).to_string();
        }
        self.redo.clear();
        cx.notify();
    }

    pub(super) fn begin_delay_bound_edit(
        &mut self,
        index: usize,
        bound: DelayBound,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(item) = self.actions.get(index).cloned() else {
            return;
        };
        if item.kind != ActionKind::Delay || item.state != "randomized" {
            return;
        }
        if self.editing_action.is_some() {
            self.finish_action_edit(window, cx);
        }
        if self.randomized_open != Some(index) {
            self.finish_randomized_range(false, window, cx);
        }
        self.launch_open = None;
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
        if self.randomized_open.is_none() {
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
        if item.kind != ActionKind::Delay || self.actions_for != self.current {
            return;
        }
        if item.number_min != min || item.number_max != max {
            self.undo.push(self.actions.clone());
            self.actions[index].number_min = min;
            self.actions[index].number_max = max;
            self.redo.clear();
        }
        cx.notify();
    }

    pub(super) fn step_loop(&mut self, index: usize, direction: i8, cx: &mut Context<Self>) {
        let Some(item) = self.actions.get(index).cloned() else {
            return;
        };
        if item.kind != ActionKind::Loop {
            return;
        }
        let value = item.value.parse::<i64>().unwrap_or(1);
        let next = (value + i64::from(direction)).clamp(1, 99_999).to_string();
        if next == item.value {
            return;
        }
        self.undo.push(self.actions.clone());
        self.actions[index].value = next;
        self.redo.clear();
        cx.notify();
    }

    pub(super) fn set_action_state(&mut self, index: usize, state: String, cx: &mut Context<Self>) {
        if index >= self.actions.len() || self.actions[index].state == state {
            return;
        }
        self.undo.push(self.actions.clone());
        self.actions[index].state = state;
        self.redo.clear();
        cx.notify();
    }

    pub(super) fn choose_action_value(
        &mut self,
        index: usize,
        value: &str,
        cx: &mut Context<Self>,
    ) {
        if index >= self.actions.len() || self.actions[index].value == value {
            self.choice_action = None;
            return;
        }
        self.undo.push(self.actions.clone());
        self.actions[index].value = value.to_string();
        self.redo.clear();
        self.choice_action = None;
        cx.notify();
    }

    pub(super) fn toggle_loop_state(&mut self, index: usize, cx: &mut Context<Self>) {
        let next = self
            .actions
            .get(index)
            .filter(|item| item.kind == ActionKind::Loop)
            .map(|item| if item.state == "end" { "start" } else { "end" });
        if let Some(next) = next {
            self.set_action_state(index, next.to_string(), cx);
        }
    }

    pub(super) fn add_action_at(&mut self, kind: &str, index: usize, cx: &mut Context<Self>) {
        let Some(current) = self.current else { return };
        if self.tutorial != Tutorial::Complete {
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
        self.undo.push(self.actions.clone());
        self.actions
            .insert(index.min(self.actions.len()), ActionItem::new(kind));
        self.clear_action_editors();
        self.selected_actions.clear();
        self.redo.clear();
        cx.notify();
    }
    pub(super) fn save_actions(&mut self, cx: &mut Context<Self>) {
        if !self.can_save() {
            return;
        }
        self.saved_actions_for = self.current;
        self.saved_actions = self.actions.clone();
        self.undo.clear();
        self.redo.clear();
        self.selected_actions.clear();
        cx.notify();
    }
    pub(super) fn undo_action(&mut self, cx: &mut Context<Self>) {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(self.actions.clone());
            self.actions = previous;
            self.clear_action_editors();
            self.selected_actions.clear();
            cx.notify();
        }
    }
    pub(super) fn redo_action(&mut self, cx: &mut Context<Self>) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(self.actions.clone());
            self.actions = next;
            self.clear_action_editors();
            self.selected_actions.clear();
            cx.notify();
        }
    }
    pub(super) fn toggle_action_selection(&mut self, index: usize, cx: &mut Context<Self>) {
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

    pub(super) fn delete_selected_actions(&mut self, cx: &mut Context<Self>) {
        if self.selected_actions.is_empty() || self.actions_for != self.current {
            return;
        }
        self.undo.push(self.actions.clone());
        let selected: std::collections::HashSet<_> = self.selected_actions.drain(..).collect();
        self.actions = self
            .actions
            .iter()
            .enumerate()
            .filter_map(|(index, action)| (!selected.contains(&index)).then_some(action.clone()))
            .collect();
        self.clear_action_editors();
        self.redo.clear();
        cx.notify();
    }

    /// Move an event row to the insertion position indicated by a drop target.
    /// The source editor inserts before the target row; removing an earlier
    /// source first therefore shifts the destination one slot to the left.
    pub(super) fn move_action(&mut self, source: usize, target: usize, cx: &mut Context<Self>) {
        if self.actions_for != self.current
            || source >= self.actions.len()
            || target > self.actions.len()
            || source == target
        {
            return;
        }
        self.undo.push(self.actions.clone());
        let action = self.actions.remove(source);
        let destination = if source < target { target - 1 } else { target };
        self.actions.insert(destination, action);
        self.selected_actions.clear();
        self.clear_action_editors();
        self.redo.clear();
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

fn truncate_utf16(value: &str, max_units: usize) -> String {
    let mut units = 0;
    let mut end = 0;
    for (index, character) in value.char_indices() {
        let next = character.len_utf16();
        if units + next > max_units {
            break;
        }
        units += next;
        end = index + character.len_utf8();
    }
    value[..end].to_string()
}

fn parse_delay(value: &str) -> f64 {
    value
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .unwrap_or(0.0)
}

fn format_delay(value: f64) -> String {
    format!("{:.3}", value.clamp(0.0, 99_999.999))
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
