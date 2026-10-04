use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ActionKind {
    Delay,
    Keyboard,
    Mouse,
    Macro,
    Launch,
    Command,
    Text,
    Loop,
}

/// One locally editable event row. The current Synapse editor renders a
/// compact action-specific value beside every event (delay seconds, key,
/// mouse button, text, command, and so on). The value is intentionally kept
/// as display text here: no device recorder or mapping service is available
/// in this shell, while the edit/undo/save interaction remains reviewable.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct ActionItem {
    pub(super) kind: ActionKind,
    pub(super) value: String,
    /// The second launch target (`Content1`) stays separate from the program
    /// path (`Content0`) when switching the Program/Website radio selection.
    pub(super) secondary_value: String,
    /// Delay randomization keeps the two source `Number.min`/`Number.max`
    /// values separate from the fixed delay value. These remain local draft
    /// fields because profile delay settings are service-owned in Synapse.
    pub(super) number_min: String,
    pub(super) number_max: String,
    /// Source keeps button/loop state beside the primary value (`State` or
    /// `LoopEvent.State`). It is local metadata here because no recorder or
    /// mapping service is connected to this shell.
    pub(super) state: String,
}

impl ActionItem {
    pub(super) fn new(kind: ActionKind) -> Self {
        let value = match kind {
            ActionKind::Delay => "0.000".to_string(),
            ActionKind::Keyboard
            | ActionKind::Mouse
            | ActionKind::Macro
            | ActionKind::Launch
            | ActionKind::Command => String::new(),
            ActionKind::Text => String::new(),
            ActionKind::Loop => "1".to_string(),
        };
        let state = match kind {
            ActionKind::Loop => "start".to_string(),
            ActionKind::Launch => "program".to_string(),
            ActionKind::Delay => "fixed".to_string(),
            _ => String::new(),
        };
        Self {
            kind,
            value,
            secondary_value: String::new(),
            number_min: if kind == ActionKind::Delay {
                "0.000".to_string()
            } else {
                String::new()
            },
            number_max: if kind == ActionKind::Delay {
                "0.000".to_string()
            } else {
                String::new()
            },
            state,
        }
    }
}

impl ActionKind {
    pub(super) fn value_is_translation_key(self, value: &str) -> bool {
        match self {
            // These are source placeholders. Once a user enters text, command
            // content, a path, or a selected macro, it must render verbatim.
            Self::Text | Self::Command | Self::Launch => false,
            Self::Delay => false,
            Self::Keyboard => value == "TEXT_NO_KEY_SET",
            Self::Mouse => editors::MOUSE_ACTION_KEYS.contains(&value),
            Self::Macro => value == "TEXT_SELECT_A_MACRO",
            Self::Loop => false,
        }
    }
}

impl ActionKind {
    pub(super) fn from_palette(kind: &str) -> Option<Self> {
        Some(match kind {
            "delay" => Self::Delay,
            "keyboard" => Self::Keyboard,
            "mouse" => Self::Mouse,
            "macro" => Self::Macro,
            "launch" => Self::Launch,
            "command" => Self::Command,
            "text" => Self::Text,
            "loop" => Self::Loop,
            _ => return None,
        })
    }
    pub(super) fn icon(self) -> &'static str {
        match self {
            Self::Delay => "delay",
            Self::Keyboard => "keyboard",
            Self::Mouse => "mouse",
            Self::Macro => "macro",
            Self::Launch => "launch",
            Self::Command => "command",
            Self::Text => "text",
            Self::Loop => "loop",
        }
    }
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Delay => "TEXT_ADD_MENU_DELAY",
            Self::Keyboard => "TEXT_ADD_MENU_KEYBOARD",
            Self::Mouse => "TEXT_ADD_MENU_MOUSE_FUNCTION",
            Self::Macro => "TEXT_ADD_MENU_MACRO",
            Self::Launch => "TEXT_ADD_MENU_LAUNCH",
            Self::Command => "TEXT_ADD_MENU_RUN_COMMAND",
            Self::Text => "TEXT_ADD_MENU_TEXT_FUNCTION",
            Self::Loop => "TEXT_ADD_MENU_LOOP",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum EntryKind {
    Macro,
    Folder,
}

#[derive(Clone)]
pub(super) struct Entry {
    pub id: u64,
    pub name: String,
    pub kind: EntryKind,
    pub parent: Option<u64>,
    pub open: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Tutorial {
    Initial,
    Record,
    Add,
    Complete,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Sort {
    Ascending,
    Descending,
    Latest,
    Oldest,
}
impl Sort {
    pub(super) const ALL: [Self; 4] = [
        Self::Ascending,
        Self::Descending,
        Self::Latest,
        Self::Oldest,
    ];
    pub(super) fn key(self) -> &'static str {
        match self {
            Self::Ascending => "TEXT_MACRO_ORDER_ASCENDING",
            Self::Descending => "TEXT_MACRO_ORDER_DESCENDING",
            Self::Latest => "TEXT_MACRO_ORDER_LATEST",
            Self::Oldest => "TEXT_MACRO_ORDER_ORDER",
        }
    }
}

// 15030.eh/u sorts only un's root array. The second UTF-16 unit comparison
// really has the opposite direction to the first; do not replace it with a
// normal alphabetical or locale-aware comparator.
fn source_name_order(a: &str, b: &str) -> std::cmp::Ordering {
    let a = a.to_lowercase();
    let b = b.to_lowercase();
    let lexical = || a.encode_utf16().cmp(b.encode_utf16());
    if a.is_empty() || b.is_empty() {
        return lexical();
    }
    fn rank(value: Option<u16>) -> i32 {
        value.map_or(-1, |c| {
            i32::from(c)
                + if "!@#$%^&*()_+[]{}|;:,.<>?/\\`~"
                    .encode_utf16()
                    .any(|p| p == c)
                {
                    0
                } else {
                    100
                }
        })
    }
    let mut aa = a.encode_utf16();
    let mut bb = b.encode_utf16();
    let (a0, a1, b0, b1) = (
        rank(aa.next()),
        rank(aa.next()),
        rank(bb.next()),
        rank(bb.next()),
    );
    if a0 == b0 {
        if a1 == b1 || a1 == -1 || b1 == -1 {
            lexical()
        } else {
            b1.cmp(&a1)
        }
    } else if a0 == -1 || b0 == -1 {
        lexical()
    } else {
        a0.cmp(&b0)
    }
}

impl MacroPage {
    fn unique_name(&self, kind: EntryKind) -> String {
        if kind == EntryKind::Folder {
            // Ys coerces folder names after replacing the first "Folder ",
            // scans 1..511, then falls back to the translated base name.
            let occupied: Vec<_> = self
                .entries
                .iter()
                .filter(|e| e.kind == kind)
                .filter_map(|entry| {
                    let name = entry.name.replacen("Folder ", "", 1);
                    let name = name.trim();
                    if let Some(hex) = name.strip_prefix("0x").or_else(|| name.strip_prefix("0X")) {
                        u32::from_str_radix(hex, 16).ok().map(f64::from)
                    } else if let Some(binary) =
                        name.strip_prefix("0b").or_else(|| name.strip_prefix("0B"))
                    {
                        u32::from_str_radix(binary, 2).ok().map(f64::from)
                    } else if let Some(octal) =
                        name.strip_prefix("0o").or_else(|| name.strip_prefix("0O"))
                    {
                        u32::from_str_radix(octal, 8).ok().map(f64::from)
                    } else {
                        name.parse::<f64>().ok()
                    }
                })
                .collect();
            return (1..512)
                .find(|n| !occupied.contains(&f64::from(*n)))
                .map(|n| format!("Folder {n}"))
                .unwrap_or_else(|| tr("TEXT_ADD_FOLDER"));
        }
        let prefix = "Macro";
        (1..)
            .map(|n| format!("{prefix} {n}"))
            .find(|name| {
                !self
                    .entries
                    .iter()
                    .any(|e| e.kind == kind && e.name == *name)
            })
            .unwrap()
    }
    pub(super) fn create_entry(
        &mut self,
        kind: EntryKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 25572.OM and Zs[h.$g] both concat to root profiles, not selected folder.
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(Entry {
            id,
            name: self.unique_name(kind),
            kind,
            parent: None,
            open: false,
        });
        if kind == EntryKind::Macro {
            self.clear_action_editors();
            self.current = Some(id);
            self.actions_for = Some(id);
            self.actions.clear();
            self.saved_actions_for = Some(id);
            self.saved_actions.clear();
            self.undo.clear();
            self.redo.clear();
            self.tree_selection = Some(id);
            self.selector_open = false;
            if self.tutorial == Tutorial::Initial {
                self.tutorial = Tutorial::Record;
            }
            self.set_tab(MacroTab::MyMacros, window, cx);
        }
        self.more_open = false;
        self.tree_menu = None;
        cx.notify();
    }
    pub(super) fn select_entry(&mut self, id: u64, cx: &mut Context<Self>) {
        self.binding_menu = None;
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            self.tree_selection = Some(id);
            if entry.kind == EntryKind::Folder {
                entry.open = !entry.open;
            } else if self.current != Some(id) {
                self.editing_action = None;
                self.launch_open = None;
                self.launch_is_website = false;
                self.randomized_open = None;
                self.choice_action = None;
                self.current = Some(id);
                self.actions_for = None;
                self.actions.clear();
                self.saved_actions_for = None;
                self.saved_actions.clear();
                self.undo.clear();
                self.redo.clear();
                self.selector_open = false;
                self.tree_menu = None;
            }
        }
        cx.notify();
    }
    pub(super) fn start_rename(
        &mut self,
        id: u64,
        in_tree: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(entry) = self.entries.iter().find(|e| e.id == id) {
            let name = entry.name.clone();
            self.rename = Some(id);
            self.rename_in_tree = in_tree;
            self.more_open = false;
            self.tree_menu = None;
            self.name.update(cx, |input, cx| {
                input.set_value(name, window, cx);
                input.focus(window, cx);
                input.select_all(window, cx);
            });
            cx.notify();
        }
    }
    pub(super) fn cancel_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.rename = None;
        self.rename_in_tree = false;
        self.focus.focus(window, cx);
        cx.notify();
    }
    pub(super) fn finish_rename(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.rename.take() else {
            return;
        };
        let in_tree = std::mem::take(&mut self.rename_in_tree);
        let parent = self
            .entries
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| e.parent);
        let name = self.name.read(cx).value().trim().to_string();
        // Header kt checks all macros. Tree kt checks siblings, then an's
        // callback also rejects every existing macro name (including elsewhere).
        let valid = !name.is_empty()
            && name.encode_utf16().count() <= 32
            && !self.entries.iter().any(|e| {
                e.id != id
                    && e.name == name
                    && (e.kind == EntryKind::Macro || in_tree && e.parent == parent)
            });
        if valid {
            if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
                entry.name = name;
            }
        }
        cx.notify();
    }
    pub(super) fn duplicate_current(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.current {
            self.duplicate_entry(id, true, false, cx);
        }
    }
    pub(super) fn duplicate_entry(
        &mut self,
        id: u64,
        select: bool,
        from_tree: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(original) = self.entries.iter().find(|e| e.id == id).cloned() else {
            return;
        };
        // 25572.iV: clone recursively; separate global file/folder name sets;
        // new identities/dates for every item; insert into the original parent.
        let query = if from_tree {
            self.search.read(cx).value().trim().to_lowercase()
        } else {
            String::new()
        };
        let all = self.ancestor_matches(id, &query);
        let mut originals = Vec::new();
        self.collect_projected(&original, &query, all, &mut originals);
        let mut ids = std::collections::HashMap::new();
        for mut entry in originals {
            let old = entry.id;
            entry.id = self.next_id;
            self.next_id += 1;
            ids.insert(old, entry.id);
            entry.parent = if old == id {
                original.parent
            } else {
                entry.parent.and_then(|p| ids.get(&p).copied())
            };
            let base = duplicate_base(&entry.name);
            entry.name = (1..)
                .map(|n| format!("{base} ({n})"))
                .find(|name| {
                    !self
                        .entries
                        .iter()
                        .any(|e| e.kind == entry.kind && e.name == *name)
                })
                .unwrap();
            // an's expanded state is local. Only search's projected expanded
            // flag can be present in the copied data passed to iV.
            entry.open = from_tree && !query.is_empty() && entry.kind == EntryKind::Folder;
            self.entries.push(entry);
        }
        if select && original.kind == EntryKind::Macro {
            self.current = ids.get(&id).copied();
            self.tree_selection = self.current;
        }
        self.more_open = false;
        self.tree_menu = None;
        cx.notify();
    }
    pub(super) fn move_entry(&mut self, id: u64, destination: u64, cx: &mut Context<Self>) {
        if id == destination {
            return;
        }
        let Some(target) = self.entries.iter().find(|e| e.id == destination) else {
            return;
        };
        let parent = if target.kind == EntryKind::Folder {
            Some(destination)
        } else {
            target.parent
        };
        // an rejects itself; Zs[h.uZ] rejects descendants before removing the
        // source. $s appends into folders, or to a file destination's siblings.
        let mut ancestor = Some(destination);
        while let Some(p) = ancestor {
            if p == id {
                return;
            }
            ancestor = self
                .entries
                .iter()
                .find(|e| e.id == p)
                .and_then(|e| e.parent);
        }
        let Some(index) = self.entries.iter().position(|e| e.id == id) else {
            return;
        };
        let mut entry = self.entries.remove(index);
        entry.parent = parent;
        self.entries.push(entry);
        self.tree_menu = None;
        cx.notify();
    }
    pub(super) fn request_delete(&mut self, id: u64, cx: &mut Context<Self>) {
        self.deletion = Some(id);
        self.more_open = false;
        self.tree_menu = None;
        self.selector_open = false;
        cx.notify();
    }
    pub(super) fn delete_entry(&mut self, id: u64, cx: &mut Context<Self>) {
        // Mn's page redirect checks the deleted item's own guid against the
        // sole macro. A folder containing that macro does not meet that test.
        let redirect = self.macro_count() == 1
            && self
                .entries
                .iter()
                .any(|e| e.id == id && e.kind == EntryKind::Macro);
        let mut deleted = vec![id];
        loop {
            let children: Vec<_> = self
                .entries
                .iter()
                .filter(|e| {
                    e.parent.is_some_and(|p| deleted.contains(&p)) && !deleted.contains(&e.id)
                })
                .map(|e| e.id)
                .collect();
            if children.is_empty() {
                break;
            }
            deleted.extend(children);
        }
        self.entries.retain(|e| !deleted.contains(&e.id));
        self.forget_bindings(&deleted);
        if redirect && self.tab != MacroTab::MyMacros {
            self.tab = MacroTab::MyMacros;
            self.history.truncate(self.history_index + 1);
            self.history.push(self.tab);
            self.history_index = self.history.len() - 1;
        }
        if self.current.is_some_and(|id| deleted.contains(&id)) {
            self.clear_action_editors();
            // zs chooses macroList[0], whose creation order is unchanged by a
            // profile-tree move. Entry IDs retain that same insertion order.
            self.current = self
                .entries
                .iter()
                .filter(|e| e.kind == EntryKind::Macro)
                .min_by_key(|e| e.id)
                .map(|e| e.id);
            self.actions_for = None;
            self.actions.clear();
            self.saved_actions_for = None;
            self.saved_actions.clear();
            self.undo.clear();
            self.redo.clear();
        }
        self.tree_selection = self.current;
        self.deletion = None;
        self.more_open = false;
        self.tree_menu = None;
        cx.notify();
    }
    fn matches_query(&self, entry: &Entry, query: &str) -> bool {
        entry.name.to_lowercase().contains(query)
            || entry.kind == EntryKind::Folder
                && self
                    .entries
                    .iter()
                    .filter(|e| e.parent == Some(entry.id))
                    .any(|e| self.matches_query(e, query))
    }
    fn ancestor_matches(&self, id: u64, query: &str) -> bool {
        let mut id = Some(id);
        while let Some(current) = id {
            let Some(entry) = self.entries.iter().find(|e| e.id == current) else {
                break;
            };
            if entry.name.to_lowercase().contains(query) {
                return true;
            }
            id = entry.parent;
        }
        false
    }
    fn collect_projected(&self, entry: &Entry, query: &str, all: bool, result: &mut Vec<Entry>) {
        result.push(entry.clone());
        let all = all || entry.name.to_lowercase().contains(query);
        for child in self
            .entries
            .iter()
            .filter(|e| e.parent == Some(entry.id) && (all || self.matches_query(e, query)))
        {
            self.collect_projected(child, query, all, result);
        }
    }
    pub(super) fn visible_entries(&self, cx: &App) -> Vec<(Entry, usize)> {
        let query = self.search.read(cx).value().trim().to_lowercase();
        fn append(
            page: &MacroPage,
            parent: Option<u64>,
            depth: usize,
            query: &str,
            all: bool,
            result: &mut Vec<(Entry, usize)>,
        ) {
            let mut entries: Vec<_> = page
                .entries
                .iter()
                .filter(|e| e.parent == parent && (all || page.matches_query(e, query)))
                .collect();
            if parent.is_none() {
                entries.sort_by(|a, b| match page.sort {
                    Sort::Ascending => source_name_order(&a.name, &b.name),
                    Sort::Descending => source_name_order(&b.name, &a.name),
                    Sort::Latest => b.id.cmp(&a.id),
                    Sort::Oldest => a.id.cmp(&b.id),
                });
            }
            for entry in entries {
                let mut display = entry.clone();
                if !query.is_empty() && display.kind == EntryKind::Folder {
                    display.open = true;
                }
                let open = display.open;
                result.push((display, depth));
                if entry.kind == EntryKind::Folder && open {
                    append(
                        page,
                        Some(entry.id),
                        depth + 1,
                        query,
                        all || entry.name.to_lowercase().contains(query),
                        result,
                    );
                }
            }
        }
        let mut result = Vec::new();
        append(self, None, 0, &query, false, &mut result);
        result
    }
}

fn duplicate_base(name: &str) -> String {
    let stripped = if name.ends_with(')') {
        name.rfind('(')
            .and_then(|at| {
                let digits = &name[at + 1..name.len() - 1];
                (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
                    .then_some(&name[..at])
            })
            .unwrap_or(name)
    } else {
        name
    };
    let stripped = stripped.trim();
    if stripped.is_empty() {
        name.to_string()
    } else {
        stripped.to_string()
    }
}
