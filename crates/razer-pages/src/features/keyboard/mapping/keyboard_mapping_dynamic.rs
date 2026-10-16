//! Mounted MapDynamicKeyStroke: four actions, trigger graph and raw actuation.
//! Receipts contain each current product's complete source and converter bytes.
use super::*;
use crate::features::controls::Choice;
use gpui_kit::base::{Slider as BaseSlider, SliderThumb, SliderTrack};
use gpui_kit::component::{
    IndexPath,
    select::{Select, SelectEvent, SelectState},
};

use std::collections::BTreeSet;
use std::{cell::Cell, rc::Rc};

const ACTIONS: [&str; 4] = ["key1", "key2", "key3", "key4"];
const TRIGGERS: [&str; 4] = [
    "actuationDownTopState",
    "actuationDownBottomState",
    "actuationUpBottomState",
    "actuationUpTopState",
];

#[derive(Deserialize)]
struct Metadata {
    product_id: u32,
    unit: f32,
    defaults: BTreeMap<String, f64>,
    headers: Vec<Header>,
    body_labels: BTreeMap<String, String>,
}
#[derive(Deserialize)]
struct Header {
    id: String,
    name: String,
    #[serde(rename = "indicationText")]
    indication_text: String,
    min: f32,
    max: f32,
    icon: String,
}

#[cfg(test)]
mod metadata_tests {
    use super::all_metadata;

    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn all_current_dynamic_mapping_metadata_loads() {
        // Source header descriptors contain actuation values and bounds.
        // startX belongs to the live action-trigger drag, not these descriptors.
        let rows = all_metadata();
        assert!(!rows.is_empty());
        for row in rows {
            assert_eq!(row.headers.len(), 4, "product {}", row.product_id);
            for header in &row.headers {
                assert!(row.defaults[&header.id].is_finite());
                assert!(header.min < header.max);
            }
        }
    }
}

fn all_metadata() -> &'static [Metadata] {
    static DATA: OnceLock<Vec<Metadata>> = OnceLock::new();
    DATA.get_or_init(|| crate::data::keyboard_mapping_dynamic_data!())
}

fn metadata(pid: u32) -> Option<&'static Metadata> {
    all_metadata().iter().find(|row| row.product_id == pid)
}
fn empty_action() -> Value {
    json!({"keyboardGroup":{"key":"","keyGroup":0,"modifiers":[]},"triggers":TRIGGERS.into_iter().map(|id|(id.to_owned(),json!("NONE"))).collect::<serde_json::Map<_,_>>()})
}
#[derive(Clone, Default, Deserialize, Serialize)]
pub(super) struct Draft {
    initialized: bool,
    group: Value,
    original_group: Value,
    pub(super) changed: bool,
}
#[derive(Clone, Default)]
struct Graph {
    dots: BTreeSet<usize>,
    lines: BTreeSet<(usize, usize)>,
}
impl Graph {
    fn from_triggers(value: &Value) -> Self {
        let mut graph = Self::default();
        let mut start = None;
        for (index, field) in TRIGGERS.into_iter().enumerate() {
            match value[field].as_str() {
                Some("CONTINUOUS") => {
                    if start.is_none() {
                        start = Some(index)
                    }
                }
                Some("ONCE" | "NONE") => {
                    if value[field] == "ONCE" {
                        graph.dots.insert(index);
                    }
                    if let Some(first) = start.take() {
                        graph.lines.insert((first, index));
                    }
                }
                _ => {}
            }
        }
        // Original S.forEach does not flush a trailing CONTINUOUS run.
        graph
    }
    fn triggers(&self) -> Value {
        json!(
            TRIGGERS
                .into_iter()
                .enumerate()
                .map(|(index, id)| {
                    let state = if self.dots.contains(&index) {
                        "ONCE"
                    } else if self
                        .lines
                        .iter()
                        .any(|(start, end)| *start <= index && index < *end)
                    {
                        "CONTINUOUS"
                    } else {
                        "NONE"
                    };
                    (id.to_owned(), json!(state))
                })
                .collect::<serde_json::Map<_, _>>()
        )
    }
}
struct Drag {
    action: usize,
    start: usize,
    start_x: f32,
    position: Pixels,
    width: f32,
    max: f32,
}
#[derive(Default)]
pub(super) struct State {
    groups: [[Option<Entity<SelectState<Vec<Choice>>>>; 4]; 2],
    targets: [[Option<Entity<SelectState<Vec<Choice>>>>; 4]; 2],
    subscriptions: [Vec<Subscription>; 2],
    sliders: [[Option<Entity<SliderState>>; 4]; 2],
    slider_subscriptions: [Vec<Subscription>; 2],
    graphs: [[Graph; 4]; 2],
    drag: [Option<Drag>; 2],
    adjustment: [Option<usize>; 2],
    syncing: bool,
    bounds: [[Rc<Cell<Bounds<Pixels>>>; 4]; 2],
    header_bounds: [[Rc<Cell<Bounds<Pixels>>>; 4]; 2],
    adjustment_bounds: [Rc<Cell<Bounds<Pixels>>>; 2],
    help_bounds: [Rc<Cell<Bounds<Pixels>>>; 2],
    help_tip_bounds: [Rc<Cell<Bounds<Pixels>>>; 2],
    help_hover: [bool; 2],
    reset_bounds: [Rc<Cell<Bounds<Pixels>>>; 2],
    reset_tip_bounds: [Rc<Cell<Bounds<Pixels>>>; 2],
    reset_hover: [bool; 2],
}
impl State {
    pub(super) fn dismiss(&mut self) {
        *self = Self::default();
    }
}
pub(super) fn recording_group(
    workspace: &KeyboardProductWorkspace,
    target: usize,
) -> Option<&Value> {
    let slot = target.checked_sub(2)? / 4;
    let action = (target - 2) % 4;
    let edit = workspace.analog_gamepad.mapping.edit.as_ref()?;
    let row = edit.slots.get(slot)?;
    (row.category == "DYNAMIC_KEY_STROKE")
        .then(|| &row.dynamic.group[ACTIONS[action]]["keyboardGroup"])
}
pub(super) fn valid(workspace: &KeyboardProductWorkspace, slot: &Slot) -> bool {
    if metadata(workspace.spec.product_id).is_none() || !slot.dynamic.changed {
        return false;
    }
    ready(&slot.dynamic.group)
}
fn ready(group: &Value) -> bool {
    let mut any = false;
    for action in ACTIONS {
        let row = &group[action];
        let key = row["keyboardGroup"]["key"]
            .as_str()
            .is_some_and(|key| !key.is_empty());
        let default = empty_action();
        let trigger = row
            .get("triggers")
            .filter(|value| !value.is_null())
            .unwrap_or(&default["triggers"])
            != &default["triggers"];
        if key != trigger {
            return false;
        }
        any |= key && trigger;
    }
    any
}
pub(super) fn staged(slot: &Slot) -> Option<Value> {
    if slot.category != "DYNAMIC_KEY_STROKE" {
        return None;
    }
    let mut group = slot.dynamic.group.clone();
    for action in ACTIONS {
        if group[action]["keyboardGroup"]["key"] == ""
            && group[action]["triggers"] == empty_action()["triggers"]
        {
            group.as_object_mut()?.remove(action);
        }
    }
    Some(json!({"outputType":"dynamicKeyStrokeGroup","dynamicKeyStrokeGroup":group}))
}
pub(super) fn heading_tip(pid: u32) -> Option<&'static str> {
    metadata(pid)?
        .body_labels
        .get("heading_tip")
        .map(String::as_str)
}
fn candidates(pid: u32, group: usize) -> Vec<&'static Value> {
    let rows = source(pid);
    let kind = rows
        .key_groups
        .get(group)
        .and_then(|row| row["content"].as_str())
        .unwrap_or_default();
    rows.key_targets
        .iter()
        .filter(|row| row["type"] == kind)
        .collect()
}
fn choice_group(pid: u32, group: usize, index: usize) -> Option<Value> {
    let rows = source(pid);
    let mut key = *candidates(pid, group).get(index)?;
    let mut modifiers = vec![];
    if let Some(modifier) = key["modifier"].as_str() {
        if let Some(row) = rows
            .key_targets
            .iter()
            .find(|row| row["keyCode"] == modifier)
        {
            modifiers.push(row["inputID"].clone());
        }
        if let Some(row) = rows
            .key_targets
            .iter()
            .find(|row| row["keyCode"] == key["keyCode"])
        {
            key = row;
        }
    }
    Some(
        json!({"key":key["inputID"].as_str().unwrap_or_default(),"keyGroup":group,"modifiers":modifiers}),
    )
}
impl KeyboardProductWorkspace {
    pub(super) fn mapping_dynamic_heading_help(
        &self,
        slot: usize,
        cx: &Context<Self>,
    ) -> AnyElement {
        let state = &self.analog_gamepad.mapping.dynamic;
        let bounds = state.help_bounds[slot].clone();
        let tip = heading_tip(self.spec.product_id).unwrap_or_default();
        div()
            .relative()
            .child(
                BaseButton::new(("keyboard-dynamic-heading-help", slot))
                    .size(surface::css(14.))
                    .rounded_full()
                    .bg(rgb(0x6c6c6c))
                    .hover(|help| help.bg(rgba(0xffffff4d)))
                    .child(img("synapse/keyboard-mapping-notices/help.svg").size_full())
                    .on_prepaint(move |rect, _, _| bounds.set(rect))
                    .on_hover(cx.listener(move |this, hover: &bool, _, cx| {
                        this.analog_gamepad.mapping.dynamic.help_hover[slot] = *hover;
                        cx.notify();
                    })),
            )
            .child(
                deferred(SourceTip {
                    label: t(tip),
                    trigger: state.help_bounds[slot].clone(),
                    bounds: state.help_tip_bounds[slot].clone(),
                    open: state.help_hover[slot],
                    reset: false,
                    opacity_scale: 1.,
                    reset_action: None,
                    identity: (
                        ElementId::from(("keyboard-dynamic-heading-tip", cx.entity_id())),
                        format!("slot-{slot}"),
                    )
                        .into(),
                })
                .with_priority(if state.help_hover[slot] { 9999 } else { 200 }),
            )
            .into_any_element()
    }
    pub(super) fn init_mapping_dynamic(
        &mut self,
        slot: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(edit) = &mut self.analog_gamepad.mapping.edit else {
            return;
        };
        if edit.slots[slot].category != "DYNAMIC_KEY_STROKE" {
            self.analog_gamepad.mapping.dynamic.groups[slot] = Default::default();
            self.analog_gamepad.mapping.dynamic.targets[slot] = Default::default();
            self.analog_gamepad.mapping.dynamic.subscriptions[slot].clear();
            self.analog_gamepad.mapping.dynamic.slider_subscriptions[slot].clear();
            self.analog_gamepad.mapping.dynamic.sliders[slot] = Default::default();
            self.analog_gamepad.mapping.dynamic.drag[slot] = None;
            self.analog_gamepad.mapping.dynamic.adjustment[slot] = None;
            self.analog_gamepad.mapping.dynamic.help_hover[slot] = false;
            self.analog_gamepad.mapping.dynamic.reset_hover[slot] = false;
            if (0..4).any(|action| self.mapping_keyboard_recording_active(2 + slot * 4 + action)) {
                self.analog_gamepad.mapping.keyboard.stop();
            }
            return;
        }
        let Some(m) = metadata(self.spec.product_id) else {
            return;
        };
        let draft = &mut edit.slots[slot].dynamic;
        if !draft.initialized {
            let existing = &edit.slots[slot].original["dynamicKeyStrokeGroup"];
            let mut group = if existing.is_object() {
                json!(
                    m.headers
                        .iter()
                        .map(|h| (h.id.clone(), existing[&h.id].clone()))
                        .collect::<serde_json::Map<_, _>>()
                )
            } else {
                json!(m.defaults)
            };
            for action in ACTIONS {
                group[action] = existing.get(action).cloned().unwrap_or_else(empty_action);
            }
            edit.slots[slot].dynamic = Draft {
                initialized: true,
                original_group: group.clone(),
                group,
                changed: false,
            };
        }
        self.analog_gamepad.mapping.dynamic.subscriptions[slot].clear();
        for action in 0..4 {
            let group = &self.analog_gamepad.mapping.edit.as_ref().unwrap().slots[slot]
                .dynamic
                .group[ACTIONS[action]]["keyboardGroup"];
            let kind = group["keyGroup"].as_u64().unwrap_or(0) as usize;
            let groups = source(self.spec.product_id)
                .key_groups
                .iter()
                .map(|r| Choice::localized(text(&r["id"]), text(&r["label"])))
                .collect::<Vec<_>>();
            let choices = candidates(self.spec.product_id, kind);
            let selected = choices.iter().position(|r| r["inputID"] == group["key"]);
            let selected =
                if kind == 6 && group["modifiers"].as_array().is_some_and(|r| !r.is_empty()) {
                    let key = source(self.spec.product_id)
                        .key_targets
                        .iter()
                        .find(|r| r["inputID"] == group["key"]);
                    key.and_then(|key| {
                        choices
                            .iter()
                            .enumerate()
                            .filter(|(_, r)| r["keyCode"] == key["keyCode"])
                            .nth(1)
                            .map(|(i, _)| i)
                    })
                    .or(selected)
                } else {
                    selected
                };
            let targets = choices
                .iter()
                .enumerate()
                .map(|(index, r)| Choice::new(index.to_string(), text(&r["name"])))
                .collect::<Vec<_>>();
            let groups = cx.new(|cx| {
                SelectState::new(groups, Some(IndexPath::new(kind)), window, cx).searchable(false)
            });
            let targets = cx.new(|cx| {
                SelectState::new(targets, selected.map(IndexPath::new), window, cx)
                    .searchable(false)
            });
            let group_sub = cx.subscribe_in(&groups, window, move |this, _, event, window, cx| {
                let SelectEvent::Confirm(Some(value)) = event else {
                    return;
                };
                let Ok(kind) = value.parse::<usize>() else {
                    return;
                };
                if kind >= source(this.spec.product_id).key_groups.len() {
                    return;
                }
                this.analog_gamepad.mapping.keyboard.stop();
                let key = if kind == 0 {
                    Value::String(String::new())
                } else {
                    candidates(this.spec.product_id, kind)
                        .first()
                        .map(|r| r["inputID"].clone())
                        .unwrap_or(Value::Null)
                };
                this.update_mapping_dynamic_action(
                    slot,
                    action,
                    Some(json!({"key":key,"keyGroup":kind,"modifiers":[]})),
                    None,
                    cx,
                );
                this.init_mapping_dynamic(slot, window, cx);
            });
            let target_sub = cx.subscribe_in(&targets, window, move |this, _, event, _, cx| {
                let SelectEvent::Confirm(Some(value)) = event else {
                    return;
                };
                let Ok(index) = value.parse::<usize>() else {
                    return;
                };
                let Some(edit) = &this.analog_gamepad.mapping.edit else {
                    return;
                };
                let kind =
                    edit.slots[slot].dynamic.group[ACTIONS[action]]["keyboardGroup"]["keyGroup"]
                        .as_u64()
                        .unwrap_or(0) as usize;
                if let Some(group) = choice_group(this.spec.product_id, kind, index) {
                    this.update_mapping_dynamic_action(slot, action, Some(group), None, cx);
                }
            });
            let state = &mut self.analog_gamepad.mapping.dynamic;
            state.groups[slot][action] = Some(groups);
            state.targets[slot][action] = Some(targets);
            state.subscriptions[slot].extend([group_sub, target_sub]);
            state.graphs[slot][action] = Graph::from_triggers(
                &self.analog_gamepad.mapping.edit.as_ref().unwrap().slots[slot]
                    .dynamic
                    .group[ACTIONS[action]]["triggers"],
            );
            self.init_mapping_keyboard_recording(2 + slot * 4 + action, cx);
        }
        self.init_mapping_dynamic_sliders(slot, cx);
    }
    fn update_mapping_dynamic_action(
        &mut self,
        slot: usize,
        action: usize,
        keyboard: Option<Value>,
        triggers: Option<Value>,
        cx: &mut Context<Self>,
    ) {
        let Some(edit) = &mut self.analog_gamepad.mapping.edit else {
            return;
        };
        if edit.slots[slot].category != "DYNAMIC_KEY_STROKE" {
            return;
        }
        let draft = &mut edit.slots[slot].dynamic;
        if let Some(group) = keyboard {
            draft.group[ACTIONS[action]]["keyboardGroup"] = group;
        }
        if let Some(triggers) = triggers {
            draft.group[ACTIONS[action]]["triggers"] = triggers;
        }
        if !draft.group[ACTIONS[action]]["keyboardGroup"]["key"]
            .as_str()
            .is_some_and(|k| !k.is_empty())
        {
            draft.group[ACTIONS[action]]["triggers"] = empty_action()["triggers"].clone();
            self.analog_gamepad.mapping.dynamic.graphs[slot][action] = Graph::default();
            self.analog_gamepad.mapping.dynamic.drag[slot] = None;
        } else {
            self.analog_gamepad.mapping.dynamic.graphs[slot][action] =
                Graph::from_triggers(&draft.group[ACTIONS[action]]["triggers"]);
        }
        draft.changed = ready(&draft.group) && draft.group != draft.original_group;
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
    pub(super) fn change_mapping_dynamic_recording(
        &mut self,
        target: usize,
        key: String,
        modifiers: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        if recording_group(self, target).is_none() {
            return;
        }
        let slot = (target - 2) / 4;
        let action = (target - 2) % 4;
        self.update_mapping_dynamic_action(
            slot,
            action,
            Some(json!({"key":key,"modifiers":modifiers,"keyGroup":0})),
            None,
            cx,
        );
    }
    fn dynamic_bounds(&self, slot: usize, index: usize) -> Option<(f32, f32, f32)> {
        let m = metadata(self.spec.product_id)?;
        let h = m.headers.get(index)?;
        let group = &self.analog_gamepad.mapping.edit.as_ref()?.slots[slot]
            .dynamic
            .group;
        let value = group[&h.id].as_f64()? as f32;
        let partner = match index {
            0 => 1,
            1 => 0,
            2 => 3,
            _ => 2,
        };
        let other = group[&m.headers[partner].id].as_f64()? as f32;
        let other = (other / m.unit).round() / 10.;
        let (min, max) = if index == 0 || index == 3 {
            (h.min, other - 0.1)
        } else {
            (other + 0.1, h.max)
        };
        Some((min, max, (value / m.unit).round() / 10.))
    }
    fn init_mapping_dynamic_sliders(&mut self, slot: usize, cx: &mut Context<Self>) {
        self.analog_gamepad.mapping.dynamic.syncing = true;
        for index in 0..4 {
            let Some((min, max, value)) = self.dynamic_bounds(slot, index) else {
                continue;
            };
            if let Some(slider) = self.analog_gamepad.mapping.dynamic.sliders[slot][index].clone() {
                slider.update(cx, |state, cx| {
                    *state = std::mem::replace(state, SliderState::new())
                        .min(min)
                        .max(max)
                        .default_value(value);
                    cx.notify();
                });
                continue;
            }
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(min)
                    .max(max)
                    .step(0.1)
                    .default_value(value)
            });
            let subscription = cx.subscribe(&slider, move |this, _, event: &SliderEvent, cx| {
                if this.analog_gamepad.mapping.dynamic.syncing {
                    return;
                }
                if let SliderEvent::Release(value) = event {
                    let Some(m) = metadata(this.spec.product_id) else {
                        return;
                    };
                    this.change_mapping_dynamic_actuation(
                        slot,
                        index,
                        value.start() * 10. * m.unit,
                        cx,
                    );
                } else {
                    // Original onChange updates internal state while pressed;
                    // absent callOnChangeOnEveryStep, only window mouseup submits.
                    cx.notify();
                }
            });
            self.analog_gamepad.mapping.dynamic.sliders[slot][index] = Some(slider);
            self.analog_gamepad.mapping.dynamic.slider_subscriptions[slot].push(subscription);
        }
        self.analog_gamepad.mapping.dynamic.syncing = false;
    }
    fn change_mapping_dynamic_actuation(
        &mut self,
        slot: usize,
        index: usize,
        value: f32,
        cx: &mut Context<Self>,
    ) {
        let Some(m) = metadata(self.spec.product_id) else {
            return;
        };
        let Some(edit) = &mut self.analog_gamepad.mapping.edit else {
            return;
        };
        if edit.slots[slot].category != "DYNAMIC_KEY_STROKE" {
            return;
        }
        let draft = &mut edit.slots[slot].dynamic;
        for index in if index == 0 || index == 3 { [0, 3] } else { [1, 2] } {
            draft.group[&m.headers[index].id] = json!(value.round() as u32);
        }
        draft.changed = ready(&draft.group) && draft.group != draft.original_group;
        self.init_mapping_dynamic_sliders(slot, cx);
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
    fn begin_mapping_dynamic_trigger(
        &mut self,
        slot: usize,
        action: usize,
        index: usize,
        position: Pixels,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if !self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .is_some_and(|edit| {
                edit.slots[slot].dynamic.group[ACTIONS[action]]["keyboardGroup"]["key"]
                    .as_str()
                    .is_some_and(|key| !key.is_empty())
            })
        {
            return;
        }
        let graph = &mut self.analog_gamepad.mapping.dynamic.graphs[slot][action];
        if graph.dots.remove(&index) {
            let triggers = graph.triggers();
            self.update_mapping_dynamic_action(slot, action, None, Some(triggers), cx);
            return;
        }
        let next = graph
            .dots
            .iter()
            .copied()
            .chain(graph.lines.iter().map(|(start, _)| *start))
            .filter(|i| *i >= index)
            .min()
            .unwrap_or(3);
        self.analog_gamepad.mapping.dynamic.drag[slot] = Some(Drag {
            action,
            start: index,
            position,
            width: 18.,
            max: 74. * (next - index) as f32 + 18.,
            start_x: (f32::from(
                self.analog_gamepad.mapping.dynamic.bounds[slot][action]
                    .get()
                    .size
                    .width,
            ) / f32::from(surface::css(1.).to_pixels(window.rem_size()))
                - 18.)
                / 3.
                * index as f32,
        });
        cx.notify();
    }
    fn move_mapping_dynamic_trigger(
        &mut self,
        slot: usize,
        position: Pixels,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(drag) = &mut self.analog_gamepad.mapping.dynamic.drag[slot] {
            drag.width = (f32::from(position - drag.position)
                / f32::from(surface::css(1.).to_pixels(window.rem_size())))
            .clamp(18., drag.max);
            cx.notify();
        }
    }
    fn finish_mapping_dynamic_trigger(
        &mut self,
        slot: usize,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.analog_gamepad.mapping.dynamic.drag[slot].take() else {
            return;
        };
        let width = f32::from(
            self.analog_gamepad.mapping.dynamic.bounds[slot][drag.action]
                .get()
                .size
                .width,
        ) / f32::from(surface::css(1.).to_pixels(window.rem_size()));
        if width <= 0. {
            return;
        }
        let steps = (drag.width / (width / 3.)).floor() as usize;
        let graph = &mut self.analog_gamepad.mapping.dynamic.graphs[slot][drag.action];
        if steps >= 1 {
            graph.lines.insert((drag.start, drag.start + steps));
        } else {
            graph.dots.insert(drag.start);
        }
        let triggers = graph.triggers();
        self.update_mapping_dynamic_action(slot, drag.action, None, Some(triggers), cx);
    }
    fn dismiss_mapping_dynamic_adjustment(
        &mut self,
        slot: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let state = &mut self.analog_gamepad.mapping.dynamic;
        let Some(index) = state.adjustment[slot] else {
            return;
        };
        // The source reset tip is inside p.current; document mousedown must
        // not dismiss the adjustment before its bubbled reset click runs.
        if state.reset_hover[slot] && state.reset_tip_bounds[slot].get().contains(&position) {
            return;
        }
        if !state.header_bounds[slot][index].get().contains(&position)
            && !state.adjustment_bounds[slot].get().contains(&position)
        {
            state.adjustment[slot] = None;
            state.reset_hover[slot] = false;
            cx.notify();
        }
    }
    pub(super) fn mapping_dynamic_element(&self, slot: usize, cx: &Context<Self>) -> AnyElement {
        let Some(m) = metadata(self.spec.product_id) else {
            return div().into_any_element();
        };
        let edit = self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .expect("mounted Dynamic Key Stroke");
        let state = &self.analog_gamepad.mapping.dynamic;
        let mut headers = h_flex()
            .ml(surface::css(220.))
            .justify_between()
            .mb(surface::css(-20.));
        for (index, h) in m.headers.iter().enumerate() {
            let bounds = state.header_bounds[slot][index].clone();
            let value = self
                .dynamic_bounds(slot, index)
                .map(|(_, _, value)| format!("{value:.1} mm"))
                .unwrap_or_default();
            let header = v_flex()
                .relative()
                .on_prepaint(move |rect, _, _| bounds.set(rect))
                .items_center()
                .gap(surface::css(2.))
                .child(
                    div()
                        .id(SharedString::from(format!(
                            "keyboard-dynamic-header-{slot}-{index}"
                        )))
                        .text_color(rgb(0xcccccc))
                        .underline()
                        .cursor_pointer()
                        .hover(|d| d.text_color(rgb(0x44d62c)))
                        .child(value)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.analog_gamepad.mapping.dynamic.reset_hover[slot] = false;
                            let current = &mut this.analog_gamepad.mapping.dynamic.adjustment[slot];
                            *current = if *current == Some(index) { None } else { Some(index) };
                            cx.notify();
                        })),
                )
                .child(div().text_size(surface::css(12.)).child(t(&h.name)))
                .child(
                    img(SharedString::from(format!(
                        "synapse/keyboard-mapping-dynamic/{}",
                        h.icon
                    )))
                    .size(surface::css(12.)),
                );
            headers = headers.child(header);
        }
        let mut body = v_flex()
            .relative()
            .mx(surface::css(20.))
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_color(rgb(0x999999))
            .child(
                div().mb(surface::css(20.)).child(t(m
                    .body_labels
                    .get("VwB")
                    .map(String::as_str)
                    .unwrap_or("DYNAMIC_KEY_STROKE_DESC"))),
            )
            .child(headers);
        for action in 0..4 {
            let group = &edit.slots[slot].dynamic.group[ACTIONS[action]]["keyboardGroup"];
            let empty = !group["key"].as_str().is_some_and(|key| !key.is_empty());
            let mut fields = v_flex().w(surface::css(210.)).gap(surface::css(5.));
            if let Some(groups) = &state.groups[slot][action] {
                fields = fields.child(
                    Select::new(groups)
                        .w_full()
                        .h(surface::css(27.))
                        .menu_max_h(surface::css(180.))
                        .placeholder(""),
                );
            }
            if group["keyGroup"] == 0 {
                fields = fields
                    .child(self.mapping_keyboard_recording_element(2 + slot * 4 + action, cx));
            } else if let Some(targets) = &state.targets[slot][action] {
                fields = fields.child(
                    Select::new(targets)
                        .w_full()
                        .h(surface::css(27.))
                        .menu_max_h(surface::css(180.))
                        .placeholder(""),
                );
            }
            let graph = &state.graphs[slot][action];
            let bounds = state.bounds[slot][action].clone();
            let mut triggers = h_flex()
                .relative()
                .flex_1()
                .justify_between()
                .h(surface::css(18.))
                .mx(surface::css(25.))
                .on_prepaint(move |rect, _, _| bounds.set(rect));
            for index in 0..4 {
                let node = div()
                    .id(SharedString::from(format!(
                        "keyboard-dynamic-trigger-{slot}-{action}-{index}"
                    )))
                    .relative()
                    .size(surface::css(18.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .when(action < 3, |node| {
                        node.child(
                            div()
                                .absolute()
                                .top(surface::css(15.66))
                                .left(surface::css(8.5))
                                .w(surface::css(1.))
                                .h(surface::css(86.))
                                .bg(rgb(0x707070)),
                        )
                    })
                    .child(
                        img("synapse/keyboard-mapping-dynamic/add.svg")
                            .size(surface::css(12.))
                            .opacity(if empty { 0.3 } else { 1. }),
                    )
                    .when(graph.dots.contains(&index), |node| {
                        node.child(
                            div()
                                .absolute()
                                .size_full()
                                .rounded_full()
                                .bg(rgb(0x44d62c)),
                        )
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            this.begin_mapping_dynamic_trigger(
                                slot,
                                action,
                                index,
                                event.position.x,
                                window,
                                cx,
                            );
                        }),
                    );
                triggers = triggers.child(node);
            }
            for &(start, end) in &graph.lines {
                triggers = triggers.child(
                    div()
                        .id(SharedString::from(format!(
                            "keyboard-dynamic-line-{slot}-{action}-{start}-{end}"
                        )))
                        .absolute()
                        .left(surface::css(74. * start as f32))
                        .top_0()
                        .w(surface::css(74. * (end - start) as f32))
                        .h(surface::css(18.))
                        .rounded(surface::css(10.))
                        .bg(rgb(0x44d62c))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let graph =
                                &mut this.analog_gamepad.mapping.dynamic.graphs[slot][action];
                            graph.lines.remove(&(start, end));
                            let triggers = graph.triggers();
                            this.update_mapping_dynamic_action(
                                slot,
                                action,
                                None,
                                Some(triggers),
                                cx,
                            );
                        })),
                );
            }
            if let Some(drag) = state.drag[slot]
                .as_ref()
                .filter(|drag| drag.action == action)
            {
                triggers = triggers.child(
                    div()
                        .absolute()
                        .left(surface::css(drag.start_x))
                        .top_0()
                        .w(surface::css(drag.width))
                        .h(surface::css(18.))
                        .rounded(surface::css(10.))
                        .bg(rgb(0x44d62c)),
                );
            }
            body = body.child(
                v_flex()
                    .mt(surface::css(20.))
                    .child(
                        div()
                            .mb(surface::css(10.))
                            .child(t(&format!("ACTION_{}", action + 1))),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap(surface::css(10.))
                            .child(fields)
                            .child(triggers),
                    ),
            );
        }
        if let Some(index) = state.adjustment[slot] {
            let h = &m.headers[index];
            let reset = edit.slots[slot].dynamic.group[&h.id].as_f64() != Some(m.defaults[&h.id]);
            let reset_bounds = state.reset_bounds[slot].clone();
            let mut adjustment = v_flex()
                .id(SharedString::from(format!(
                    "keyboard-dynamic-adjustment-{slot}"
                )))
                .w(surface::css(370.))
                .p(surface::css(20.))
                .bg(rgb(0x000000))
                .border_1()
                .border_color(rgb(0x666666))
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .mb(surface::css(10.))
                                .child(t(&h.indication_text).to_uppercase()),
                        )
                        .child(
                            BaseButton::new(("keyboard-dynamic-reset", slot))
                                .size(surface::css(20.))
                                .mb(surface::css(10.))
                                .opacity(if reset { 1. } else { 0.3 })
                                .when(!reset, |button| {
                                    button.cursor(CursorStyle::OperationNotAllowed)
                                })
                                .child(
                                    img("synapse/keyboard-mapping-dynamic/refresh.svg").size_full(),
                                )
                                .on_prepaint(move |rect, _, _| reset_bounds.set(rect))
                                .on_hover(cx.listener(move |this, hover: &bool, window, cx| {
                                    // The source tooltip is a DOM descendant of the reset
                                    // icon: entering it must not count as leaving the icon.
                                    let tip = this.analog_gamepad.mapping.dynamic.reset_tip_bounds
                                        [slot]
                                        .get();
                                    this.analog_gamepad.mapping.dynamic.reset_hover[slot] =
                                        *hover || tip.contains(&window.mouse_position());
                                    cx.notify();
                                }))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(m) = metadata(this.spec.product_id) {
                                        this.change_mapping_dynamic_actuation(
                                            slot,
                                            index,
                                            m.defaults[&m.headers[index].id] as f32,
                                            cx,
                                        );
                                    }
                                })),
                        ),
                );
            if let Some(slider) = &state.sliders[slot][index] {
                adjustment = adjustment.child(DynamicSlider {
                    state: slider.clone(),
                });
            }
            adjustment = adjustment.child(
                deferred(SourceTip {
                    label: t("RESET_TO_DEFAULT"),
                    trigger: state.reset_bounds[slot].clone(),
                    bounds: state.reset_tip_bounds[slot].clone(),
                    open: state.reset_hover[slot],
                    reset: true,
                    opacity_scale: if reset { 1. } else { 0.3 },
                    reset_action: Some((cx.entity().downgrade(), slot, index)),
                    identity: (
                        ElementId::from(("keyboard-dynamic-reset-tip", cx.entity_id())),
                        format!("slot-{slot}"),
                    )
                        .into(),
                })
                .with_priority(1001),
            );
            body = body.child(
                deferred(Adjustment {
                    content: adjustment.into_any_element(),
                    header: state.header_bounds[slot][index].clone(),
                    bounds: state.adjustment_bounds[slot].clone(),
                })
                .with_priority(1000),
            );
        }
        body.on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                this.dismiss_mapping_dynamic_adjustment(slot, event.position, cx)
            }),
        )
        .on_mouse_down_out(cx.listener(move |this, event: &MouseDownEvent, _, cx| {
            if event.button == MouseButton::Left {
                this.dismiss_mapping_dynamic_adjustment(slot, event.position, cx);
            }
        }))
        .on_mouse_move(
            cx.listener(move |this, event: &MouseMoveEvent, window, cx| {
                this.move_mapping_dynamic_trigger(slot, event.position.x, window, cx)
            }),
        )
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(move |this, _, window, cx| {
                this.finish_mapping_dynamic_trigger(slot, window, cx)
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(move |this, _, window, cx| {
                this.finish_mapping_dynamic_trigger(slot, window, cx);
            }),
        )
        .into_any_element()
    }
}

#[derive(IntoElement)]
struct Adjustment {
    content: AnyElement,
    header: Rc<Cell<Bounds<Pixels>>>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
}

#[derive(IntoElement)]
struct DynamicSlider {
    state: Entity<SliderState>,
}
impl RenderOnce for DynamicSlider {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let percentage = state.percentage().end;
        let value = state.value().start();
        let min = state.min_value();
        let max = state.max_value();
        BaseSlider::new(&self.state)
            .relative()
            .w_full()
            .h(surface::css(64.))
            .mt(surface::css(5.))
            .child(
                div()
                    .absolute()
                    .bottom(surface::css(25.))
                    .w_full()
                    .h(surface::css(6.))
                    .rounded(surface::css(3.))
                    .bg(rgba(0x44d62c4d)),
            )
            .child(
                SliderTrack::new(&self.state)
                    .absolute()
                    .left(surface::css(8.))
                    .right(surface::css(8.))
                    .bottom(surface::css(20.))
                    .h(surface::css(16.))
                    .child(
                        div()
                            .absolute()
                            .left(relative(percentage))
                            .ml(surface::css(-40.))
                            .top(surface::css(-28.))
                            .w(surface::css(80.))
                            .px(surface::css(8.))
                            .py(surface::css(4.))
                            .rounded(surface::css(3.))
                            .bg(rgb(0x00ff00))
                            .text_color(rgb(0x212121))
                            .text_size(surface::css(12.))
                            .line_height(surface::css(14.))
                            .child(
                                h_flex()
                                    .items_center()
                                    .child(
                                        svg()
                                            .path(
                                                "synapse/keyboard-mapping-dynamic/upload-black.svg",
                                            )
                                            .text_color(rgb(0x222222))
                                            .size(surface::css(15.))
                                            .mr(surface::css(5.))
                                            .with_transformation(Transformation::rotate(radians(
                                                std::f32::consts::PI,
                                            ))),
                                    )
                                    .child(format!("{value:.1} mm")),
                            ),
                    )
                    .child(
                        SliderThumb::new(&self.state)
                            .absolute()
                            .left(relative(percentage))
                            .ml(surface::css(-8.))
                            .top_0()
                            .size(surface::css(16.))
                            .rounded(surface::css(8.))
                            .bg(rgb(0x44d62c))
                            .hover(|thumb| thumb.border_2().border_color(rgb(0x44d62c)))
                            .active(|thumb| thumb.border_2().border_color(rgb(0x44d62c))),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .child(format!("{min} mm")),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .right_0()
                    .child(format!("{max} mm")),
            )
    }
}

#[derive(IntoElement)]
struct SourceTip {
    label: String,
    trigger: Rc<Cell<Bounds<Pixels>>>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    open: bool,
    reset: bool,
    opacity_scale: f32,
    reset_action: Option<(WeakEntity<KeyboardProductWorkspace>, usize, usize)>,
    identity: ElementId,
}
impl RenderOnce for SourceTip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let trigger = self.trigger.get();
        let mut measurement = dynamic_tip_surface(self.reset)
            .child(self.label.clone())
            .into_any_element();
        let measured = measurement.layout_as_root(
            size(AvailableSpace::MaxContent, AvailableSpace::MinContent),
            window,
            cx,
        );
        // 61050 copies the hidden DOM node's clientWidth/clientHeight,
        // excluding the one-pixel borders, into the portal border-box size.
        let tip_size = if self.reset {
            measured
        } else {
            measured.map(|length| (length - px(2.)).max(px(1.)))
        };
        let viewport = window.viewport_size();
        let mut left = trigger.right();
        let mut top = trigger.bottom();
        if self.reset {
            left = trigger.right() + surface::css(4.).to_pixels(window.rem_size()) - tip_size.width;
        } else {
            if left + tip_size.width > viewport.width {
                left = trigger.left() - tip_size.width;
            }
            if top + tip_size.height > viewport.height {
                top = trigger.bottom() - tip_size.height;
            }
            let position_policy =
                gpui_kit::base::motion::Transition::new(std::time::Duration::from_millis(100));
            left = gpui_kit::base::motion::transition(
                (self.identity.clone(), "left"),
                left,
                position_policy.clone(),
                window,
                cx,
            );
            top = gpui_kit::base::motion::transition(
                (self.identity.clone(), "top"),
                top,
                position_policy,
                window,
                cx,
            );
        }
        let opacity = gpui_kit::base::motion::transition(
            (self.identity.clone(), "opacity"),
            if self.open { 1. } else { 0. },
            gpui_kit::base::motion::Transition::new(std::time::Duration::from_millis(300))
                .easing(gpui_kit::base::motion::Easing::Linear),
            window,
            cx,
        );
        let visible = if self.reset {
            self.open
        } else {
            gpui_kit::base::motion::Presence::new((self.identity.clone(), "visibility"), self.open)
                .transition(gpui_kit::base::motion::Transition::new(
                    std::time::Duration::from_millis(200),
                ))
                .sample(window, cx)
                .should_render()
        };
        let text = if self.reset || self.open {
            self.label
        } else {
            String::new()
        };
        let reset_action = self.reset_action;
        DynamicTipPosition {
            origin: point(left, top),
            child: dynamic_tip_surface(self.reset)
                .id((self.identity, "surface"))
                .opacity(opacity * self.opacity_scale)
                .when(!visible, |element| element.invisible())
                .when(!self.reset, |element| element.w(tip_size.width))
                .when(!self.open && !self.reset, |element| {
                    element.h(tip_size.height)
                })
                .when_some(reset_action, |element, (owner, slot, index)| {
                    let hover_owner = owner.clone();
                    element
                        .on_hover(move |hover, window, cx| {
                            let _ = hover_owner.update(cx, |this, cx| {
                                this.analog_gamepad.mapping.dynamic.reset_hover[slot] = *hover
                                    || this.analog_gamepad.mapping.dynamic.reset_bounds[slot]
                                        .get()
                                        .contains(&window.mouse_position());
                                cx.notify();
                            });
                        })
                        .on_click(move |_, _, cx| {
                            let _ = owner.update(cx, |this, cx| {
                                if let Some(m) = metadata(this.spec.product_id) {
                                    this.change_mapping_dynamic_actuation(
                                        slot,
                                        index,
                                        m.defaults[&m.headers[index].id] as f32,
                                        cx,
                                    );
                                }
                            });
                        })
                })
                .on_prepaint(move |rect, window, _| {
                    if self.bounds.replace(rect).size != rect.size {
                        window.request_animation_frame();
                    }
                })
                .child(text)
                .into_any_element(),
        }
    }
}

fn dynamic_tip_surface(reset: bool) -> Div {
    div()
        .px(surface::css(10.))
        .py(surface::css(8.))
        .bg(rgb(0x000000))
        .border_1()
        .border_color(rgb(0x5d5d5d))
        .font_family("Roboto")
        .text_size(surface::css(14.))
        .line_height(surface::css(16.))
        .text_color(rgb(0xcccccc))
        .when(reset, |tip| tip.whitespace_nowrap())
        .when(!reset, |tip| tip.w(surface::css(210.)))
}

// Source reset-tip is not viewport-clamped, and 61050 only applies its own
// independent x/y fallback. GPUI Anchored's additional fit would change both.
struct DynamicTipPosition {
    origin: Point<Pixels>,
    child: AnyElement,
}
impl IntoElement for DynamicTipPosition {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for DynamicTipPosition {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let child = self.child.request_layout(window, cx);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    display: Display::Flex,
                    ..Style::default()
                },
                [child],
                cx,
            ),
            (),
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_element_offset(self.origin - bounds.origin, |window| {
            self.child.prepaint(window, cx)
        });
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}
impl RenderOnce for Adjustment {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let header = self.header.get();
        let popup = self.bounds.get();
        let viewport = window.viewport_size();
        let unit = f32::from(surface::css(1.).to_pixels(window.rem_size()));
        let width = if popup.size.width > px(0.) {
            f32::from(popup.size.width)
        } else {
            370. * unit
        };
        let height = f32::from(popup.size.height);
        let left = (f32::from(header.left()) + (f32::from(header.size.width) - width) / 2.)
            .min(f32::from(viewport.width) - width - 16. * unit)
            .max(16. * unit);
        let top = (f32::from(header.bottom()) - 22. * unit)
            .min(f32::from(viewport.height) - height - 16. * unit)
            .max(16. * unit);
        anchored()
            .anchor(Anchor::TopLeft)
            .position(point(
                px((left / unit).round() * unit),
                px((top / unit).round() * unit),
            ))
            .child(
                div()
                    .on_prepaint(move |rect, window, _| {
                        if self.bounds.replace(rect).size != rect.size {
                            window.request_animation_frame();
                        }
                    })
                    .child(self.content),
            )
    }
}
