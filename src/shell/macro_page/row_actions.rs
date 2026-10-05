//! Current Macro 25572 O/R/C/P, 15030.cf and main HH/Xg/T9 reducers.
//! The native list excludes the source's actionBar sentinel at index zero.
use super::*;
use crate::features::macro_library::{KeyboardEvent, MacroType, MouseEvent};

pub(super) fn pair_id(item: &ActionItem) -> Option<u64> {
    match item.kind {
        ActionKind::Keyboard => item.keyboard.as_ref()?.pair_id,
        ActionKind::Mouse => item.mouse.as_ref()?.pair_id,
        ActionKind::Loop => item.loop_pair_id,
        _ => None,
    }
}

pub(super) fn event_state(item: &ActionItem) -> Option<u8> {
    match item.kind {
        ActionKind::Keyboard => item.keyboard.as_ref()?.state,
        ActionKind::Mouse => item.mouse.as_ref()?.state,
        ActionKind::Loop => match item.state.as_str() {
            "start" => Some(0),
            "end" => Some(1),
            _ => None,
        },
        _ => None,
    }
}

/// 15030.cf uses keyboard parity, unlike 25572.M's literal 0/1 highlight.
pub(super) fn counterpart(actions: &[ActionItem], index: usize) -> Option<usize> {
    let item = actions.get(index)?;
    let id = pair_id(item)?;
    let state = event_state(item)?;
    let opposite = if item.kind == ActionKind::Keyboard {
        state ^ 1
    } else if state == 1 {
        0
    } else {
        1
    };
    actions.iter().position(|other| {
        other.kind == item.kind
            && pair_id(other) == Some(id)
            && event_state(other) == Some(opposite)
    })
}

impl MacroPage {
    pub(super) fn next_event_pair_id(&self) -> Option<u64> {
        self.actions
            .iter()
            .chain(&self.saved_actions)
            .chain(self.undo.iter().flatten())
            .chain(self.redo.iter().flatten())
            .chain(self.entries.iter().flat_map(|entry| &entry.actions))
            .chain(self.inactive_drafts.values().flat_map(|draft| {
                draft
                    .actions
                    .iter()
                    .chain(draft.undo.iter().flatten())
                    .chain(draft.redo.iter().flatten())
            }))
            .filter_map(pair_id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
    }

    pub(super) fn new_action_items(&self, kind: ActionKind) -> Option<Vec<ActionItem>> {
        if self.record_ui.open
            || kind == ActionKind::Delay && self.current_macro_type() != MacroType::Standard
        {
            return None;
        }
        if !matches!(
            kind,
            ActionKind::Keyboard | ActionKind::Mouse | ActionKind::Loop
        ) {
            return Some(vec![ActionItem::new(kind)]);
        }
        let id = self.next_event_pair_id()?;
        let make = |state| {
            let mut item = ActionItem::new(kind);
            match kind {
                ActionKind::Keyboard => {
                    item.keyboard = Some(KeyboardEvent {
                        pair_id: Some(id),
                        makecode: None,
                        state,
                        flag: state,
                        key_type: None,
                    })
                }
                ActionKind::Mouse => {
                    item.mouse = Some(MouseEvent {
                        pair_id: Some(id),
                        button: None,
                        state,
                    })
                }
                ActionKind::Loop => {
                    item.loop_pair_id = Some(id);
                    item.state = if state == Some(1) { "end" } else { "start" }.into();
                }
                _ => unreachable!(),
            }
            item
        };
        Some(
            if self.current_macro_type() == MacroType::Sequence && kind != ActionKind::Loop {
                vec![make(None)]
            } else {
                vec![make(Some(0)), make(Some(1))]
            },
        )
    }

    pub(super) fn duplicate_action(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.record_ui.open
            || self.tutorial != Tutorial::Complete
            || self.actions_for != self.current
        {
            return;
        }
        let Some(mut item) = self.actions().get(index).cloned() else {
            return;
        };
        // R always produces a fresh down/up pair for a keyboard copy, including
        // a null-state Sequence row. This is not O's macro-type creation rule.
        let copies = match item.kind {
            ActionKind::Keyboard => {
                let Some(id) = self.next_event_pair_id() else {
                    return;
                };
                let key = item.keyboard.get_or_insert(KeyboardEvent {
                    pair_id: None,
                    makecode: None,
                    state: None,
                    flag: None,
                    key_type: None,
                });
                let down = key.state.unwrap_or(0) & !1;
                key.pair_id = Some(id);
                key.state = Some(down);
                key.flag = Some(down);
                let mut up = item.clone();
                let key = up.keyboard.as_mut().unwrap();
                key.state = Some(down + 1);
                key.flag = Some(down + 1);
                vec![item, up]
            }
            ActionKind::Mouse => {
                let Some(id) = self.next_event_pair_id() else {
                    return;
                };
                let mouse = item.mouse.get_or_insert(MouseEvent {
                    pair_id: None,
                    button: None,
                    state: None,
                });
                mouse.pair_id = Some(id);
                if mouse.button.is_some_and(|button| (6..=9).contains(&button)) {
                    vec![item]
                } else {
                    mouse.state = Some(0);
                    let mut up = item.clone();
                    up.mouse.as_mut().unwrap().state = Some(1);
                    vec![item, up]
                }
            }
            ActionKind::Loop => {
                let Some(id) = self.next_event_pair_id() else {
                    return;
                };
                item.loop_pair_id = Some(id);
                item.state = "start".into();
                let mut end = item.clone();
                end.state = "end".into();
                vec![item, end]
            }
            _ => vec![item],
        };
        self.undo.push(self.actions.clone());
        for selected in &mut self.selected_actions {
            if *selected > index {
                *selected += copies.len();
            }
        }
        self.actions.splice(index + 1..index + 1, copies);
        self.clear_action_editors();
        self.redo.clear();
        cx.notify();
    }

    pub(super) fn delete_action(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.record_ui.open
            || self.tutorial != Tutorial::Complete
            || self.actions_for != self.current
        {
            return;
        }
        let Some(item) = self.actions().get(index) else {
            return;
        };
        // Xg deletes both ends only for Loop. Key/mouse deletion may leave a
        // lone event, which the source continues to allow editing and saving.
        let paired = (item.kind == ActionKind::Loop)
            .then(|| counterpart(&self.actions, index))
            .flatten();
        let order: Vec<_> = (0..self.actions.len())
            .filter(|&i| i != index && Some(i) != paired)
            .collect();
        self.commit_row_order(order, cx);
    }

    fn commit_row_order(&mut self, order: Vec<usize>, cx: &mut Context<Self>) {
        if order.iter().copied().eq(0..self.actions.len()) {
            return;
        }
        let actions = order.iter().map(|&i| self.actions[i].clone()).collect();
        self.selected_actions = order
            .iter()
            .enumerate()
            .filter_map(|(next, old)| self.selected_actions.contains(old).then_some(next))
            .collect();
        self.undo
            .push(std::mem::replace(&mut self.actions, actions));
        self.redo.clear();
        self.clear_action_editors();
        cx.notify();
    }

    pub(super) fn drop_actions(
        &mut self,
        drag: &ActionDrag,
        target: usize,
        cx: &mut Context<Self>,
    ) {
        if self.record_ui.open
            || self.tutorial != Tutorial::Complete
            || drag.page != cx.entity_id()
            || drag.document != self.current
            || self.actions_for != self.current
            || self.actions() != drag.baseline.as_slice()
            || !drag.allows(target)
        {
            return;
        }
        if let Some(kind) = drag.palette_kind {
            self.add_action_at(kind, target, cx);
            return;
        }
        // T9 keeps the target object's identity during removal and inserts
        // after it. Dropping on a selected member of the moved group is a no-op.
        if target > 0 && drag.indices.contains(&(target - 1)) {
            return;
        }
        let mut order: Vec<_> = (0..self.actions.len())
            .filter(|i| !drag.indices.contains(i))
            .collect();
        let position = order.iter().filter(|&&i| i < target).count();
        order.splice(position..position, drag.indices.iter().copied());
        // T9 repairs crossing Loop boundaries after a move. Keep identities
        // in the permutation so duplicate-looking rows cannot be confused.
        for &moved in &drag.indices {
            if self.actions[moved].kind != ActionKind::Loop {
                continue;
            }
            let Some(paired) = counterpart(&self.actions, moved) else {
                continue;
            };
            let start = order.iter().position(|&i| i == moved).unwrap();
            let end = order.iter().position(|&i| i == paired).unwrap();
            let id = pair_id(&self.actions[moved]);
            let mut crossings = std::collections::HashMap::new();
            for (position, &i) in order
                .iter()
                .enumerate()
                .take(start.max(end) + 1)
                .skip(start.min(end))
            {
                let other = &self.actions[i];
                if other.kind == ActionKind::Loop
                    && pair_id(other) != id
                    && let Some(other_id) = pair_id(other)
                    && crossings.remove(&other_id).is_none()
                {
                    crossings.insert(other_id, position);
                }
            }
            if !crossings.is_empty() {
                let destination = if end > start {
                    *crossings.values().min().unwrap()
                } else {
                    *crossings.values().max().unwrap()
                };
                let paired = order.remove(end);
                order.insert(destination, paired);
            }
        }
        self.commit_row_order(order, cx);
    }

    pub(super) fn sync_loop_value(&mut self, index: usize) {
        if self.actions[index].kind == ActionKind::Loop
            && let Some(paired) = counterpart(&self.actions, index)
        {
            self.actions[paired].value = self.actions[index].value.clone();
        }
    }

    pub(super) fn choose_mouse_action(&mut self, index: usize, button: u8, cx: &mut Context<Self>) {
        if self.record_ui.open || self.actions_for != self.current {
            return;
        }
        let Some(old) = self
            .actions
            .get(index)
            .filter(|item| item.kind == ActionKind::Mouse)
            .cloned()
        else {
            return;
        };
        let Some(key) = editors::MOUSE_ACTION_KEYS.get(button as usize) else {
            return;
        };
        if old
            .mouse
            .as_ref()
            .is_some_and(|mouse| mouse.button == Some(button))
        {
            self.choice_action = None;
            cx.notify();
            return;
        }
        let pair =
            if old.mouse.as_ref().is_some_and(|mouse| {
                mouse.button.is_some_and(|v| v >= 6) && mouse.pair_id.is_none()
            }) && button < 6
            {
                let Some(id) = self.next_event_pair_id() else {
                    return;
                };
                Some(id)
            } else {
                old.mouse.as_ref().and_then(|mouse| mouse.pair_id)
            };
        self.undo.push(self.actions.clone());
        // Legacy display-only mouse rows stay unpaired; choosing an option is
        // an explicit local edit, not evidence that a matching row was recorded.
        let mouse = self.actions[index].mouse.get_or_insert(MouseEvent {
            pair_id: None,
            button: None,
            state: None,
        });
        let previous = mouse.button.unwrap_or(0);
        mouse.pair_id = pair;
        mouse.button = Some(button);
        self.actions[index].value = (*key).into();
        let paired = counterpart(&self.actions, index);
        if previous < 6 {
            if button >= 6 {
                self.actions[index].mouse.as_mut().unwrap().state = None;
                if let Some(paired) = paired {
                    self.actions.remove(paired);
                    self.selected_actions = self
                        .selected_actions
                        .iter()
                        .filter_map(|&i| {
                            (i != paired).then_some(if i > paired { i - 1 } else { i })
                        })
                        .collect();
                }
            } else if let Some(paired) = paired {
                self.actions[paired].mouse.as_mut().unwrap().button = Some(button);
                self.actions[paired].value = (*key).into();
            }
        } else if button < 6 {
            self.actions[index].mouse.as_mut().unwrap().state = Some(1);
            let mut down = self.actions[index].clone();
            down.mouse.as_mut().unwrap().state = Some(0);
            self.actions.insert(index, down);
            // C clones the selected state along with the source row.
            let selected = self.selected_actions.contains(&index);
            for i in &mut self.selected_actions {
                if *i >= index {
                    *i += 1;
                }
            }
            if selected {
                self.selected_actions.push(index);
            }
        }
        self.clear_action_editors();
        self.redo.clear();
        cx.notify();
    }
}
