//! Retained current Keymap, profile and Chroma target dropdowns.
use super::*;
use crate::features::controls::Choice;
use gpui_kit::component::{
    IndexPath,
    select::{Select, SelectEvent, SelectState},
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Kind {
    Keymap,
    Profile,
    ProfileChroma,
    Lighting,
}
#[derive(Default)]
pub(super) struct State {
    selectors: BTreeMap<(usize, Kind), Entity<SelectState<Vec<Choice>>>>,
    subscriptions: BTreeMap<(usize, Kind), Subscription>,
}
impl State {
    pub(super) fn dismiss(&mut self) {
        self.selectors.clear();
        self.subscriptions.clear();
    }
}
impl KeyboardProductWorkspace {
    pub(in crate::features) fn init_mapping_targets(
        &mut self,
        slot: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(edit) = &self.analog_gamepad.mapping.edit else {
            return;
        };
        let staged = edit.slots[slot].staged.clone();
        for kind in [
            Kind::Keymap,
            Kind::Profile,
            Kind::ProfileChroma,
            Kind::Lighting,
        ] {
            let (rows, selected): (Vec<(String, String)>, &Value) = match kind {
                Kind::Keymap => (
                    self.mapping_keymaps()
                        .into_iter()
                        .filter(|row| row["guid"] != self.draft["activeKeymapId"])
                        .filter_map(|row| Some((row["guid"].as_str()?.into(), text(&row["name"]))))
                        .collect(),
                    &staged["keymapGroup"]["guid"],
                ),
                Kind::Profile => (
                    self.mapping_profile_rows()
                        .iter()
                        .filter(|row| row["guid"] != self.draft["guid"])
                        .filter_map(|row| Some((row["guid"].as_str()?.into(), text(&row["name"]))))
                        .collect(),
                    &staged["profileNavigationGroup"]["guid"],
                ),
                Kind::ProfileChroma | Kind::Lighting => (
                    self.lighting_observation
                        .as_ref()
                        .into_iter()
                        .flat_map(|observation| &observation.chroma_profiles)
                        .filter_map(|row| Some((row["id"].as_str()?.into(), text(&row["name"]))))
                        .collect(),
                    if kind == Kind::Lighting {
                        &staged["lightPacGroup"]["ID"]
                    } else {
                        &staged["profileNavigationGroup"]["lightpacId"]
                    },
                ),
            };
            let selected = rows.iter().position(|(id, _)| selected == id.as_str());
            let choices = rows
                .iter()
                .map(|(id, name)| Choice::new(id.clone(), name.clone()))
                .collect::<Vec<_>>();
            if let Some(selector) = self
                .analog_gamepad
                .mapping
                .targets
                .selectors
                .get(&(slot, kind))
            {
                selector.update(cx, |selector, cx| {
                    selector.set_items(choices, window, cx);
                    selector.set_selected_index(selected.map(IndexPath::new), window, cx);
                });
                continue;
            }
            let selector = cx.new(|cx| {
                SelectState::new(choices, selected.map(IndexPath::new), window, cx)
                    .searchable(false)
            });
            let subscription =
                cx.subscribe_in(&selector, window, move |this, _, event, window, cx| {
                    let SelectEvent::Confirm(Some(id)) = event else {
                        return;
                    };
                    if this.analog_gamepad.mapping.edit.is_none() {
                        return;
                    }
                    match kind {
                        Kind::Keymap => this.stage_specific_mapping_keymap(slot, Some(id), cx),
                        Kind::Profile | Kind::ProfileChroma => {
                            if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                let field = if kind == Kind::Profile {
                                    "guid"
                                } else {
                                    "lightpacId"
                                };
                                edit.slots[slot].staged["profileNavigationGroup"][field] =
                                    json!(id);
                            }
                            cx.emit(KeyboardProductChanged);
                        }
                        Kind::Lighting => {
                            let name = this
                                .lighting_observation
                                .as_ref()
                                .and_then(|observation| {
                                    observation
                                        .chroma_profiles
                                        .iter()
                                        .find(|row| row["id"] == id.as_str())
                                })
                                .map(|row| text(&row["name"]));
                            if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                edit.slots[slot].staged["lightPacGroup"]["ID"] = json!(id);
                                edit.slots[slot].staged["lightPacGroup"]["Name"] =
                                    json!(name.unwrap_or_default());
                            }
                            cx.emit(KeyboardProductChanged);
                        }
                    }
                    this.init_mapping_targets(slot, window, cx);
                    cx.notify();
                });
            self.analog_gamepad
                .mapping
                .targets
                .selectors
                .insert((slot, kind), selector);
            self.analog_gamepad
                .mapping
                .targets
                .subscriptions
                .insert((slot, kind), subscription);
        }
    }
    pub(super) fn mapping_target_element(
        &self,
        slot: usize,
        kind: Kind,
        disabled: bool,
    ) -> Option<AnyElement> {
        self.analog_gamepad
            .mapping
            .targets
            .selectors
            .get(&(slot, kind))
            .map(|selector| {
                Select::new(selector)
                    .placeholder("")
                    .ml(surface::css(if kind == Kind::Lighting { 0. } else { 30. }))
                    .w(surface::css(if kind == Kind::Lighting {
                        210.
                    } else {
                        180.
                    }))
                    .h(surface::css(27.))
                    .menu_max_h(surface::css(180.))
                    .disabled(disabled)
                    .into_any_element()
            })
    }
}
