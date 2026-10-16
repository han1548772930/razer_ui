//! Mounted MapSwitchKeymap (32989), using current product helper choices.
use super::*;
use gpui_kit::component::radio::Radio;

fn assignment_id(id: &str, slot: usize) -> String {
    if slot == 0 {
        return id.into();
    }
    match id {
        "nextKeymap" => "nextKeyMapSecondary",
        "previousKeymap" => "previousKeyMapSecondary",
        "cycleUpKeymap" => "cycleUpKeyMapSecondary",
        "cycleDownKeymap" => "cycleDownKeyMapSecondary",
        "specificKeymap" => "specificKeymapMapSecondary",
        _ => id,
    }
    .into()
}

impl KeyboardProductWorkspace {
    pub(super) fn mapping_keymaps(&self) -> Vec<&Value> {
        let mut keymaps = self.draft["keymaps"]
            .as_array()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        keymaps.sort_by(|left, right| text(&left["slot"]).cmp(&text(&right["slot"])));
        keymaps
    }

    pub(super) fn stage_specific_mapping_keymap(
        &mut self,
        slot: usize,
        guid: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let current = self.draft["activeKeymapId"].as_str();
        let selected = self
            .mapping_keymaps()
            .into_iter()
            .filter(|keymap| keymap["guid"].as_str() != current)
            .find(|keymap| guid.is_none() || keymap["guid"].as_str() == guid)
            .cloned();
        let Some(selected) = selected else { return };
        if let Some(edit) = &mut self.analog_gamepad.mapping.edit {
            let clutch = edit.slots[slot].staged["keymapGroup"]["isClutch"] == true;
            let mut group = json!({
                "guid": selected["guid"], "name": selected["name"],
                "type": assignment_id("specificKeymap", slot), "isClutch": clutch,
            });
            if clutch {
                group["previousKeymapGuid"] = self.draft["activeKeymapId"].clone();
            }
            edit.slots[slot].staged["keymapGroup"] = group;
        }
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }

    pub(super) fn mapping_keymap_element(&self, slot: usize, cx: &Context<Self>) -> AnyElement {
        let edit = self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .expect("mapping edit");
        let group = &edit.slots[slot].staged["keymapGroup"];
        let keymaps = self.mapping_keymaps();
        let current = self.draft["activeKeymapId"].as_str();
        let first = keymaps.first().and_then(|keymap| keymap["guid"].as_str());
        let last = keymaps.last().and_then(|keymap| keymap["guid"].as_str());
        let single = keymaps.len() < 2;
        let specific = group["type"] == assignment_id("specificKeymap", slot);
        let mut panel = v_flex().mx(surface::css(20.)).gap(surface::css(10.));
        for choice in &source(self.spec.product_id).keymap_choices {
            let base_id = choice["id"].as_str().unwrap_or_default();
            let id = assignment_id(base_id, slot);
            let label = text(&choice["content"]);
            let disabled = single
                || match base_id {
                    "nextKeymap" => current == last,
                    "previousKeymap" => current == first,
                    _ => false,
                };
            let selected = group["type"] == id;
            panel = panel.child(
                Radio::new(SharedString::from(format!(
                    "keyboard-map-keymap-{slot}-{id}"
                )))
                .label(t(&label))
                .checked(selected)
                .disabled(disabled)
                .on_click(cx.listener(move |this, _, window, cx| {
                    if id == assignment_id("specificKeymap", slot) {
                        this.stage_specific_mapping_keymap(slot, None, cx);
                        this.init_mapping_targets(slot, window, cx);
                        return;
                    }
                    if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                        edit.slots[slot].staged["keymapGroup"] =
                            json!({"type": id, "isClutch": false});
                    }
                    cx.emit(KeyboardProductChanged);
                    cx.notify();
                })),
            );
        }
        if specific {
            panel =
                panel.children(self.mapping_target_element(slot, targets::Kind::Keymap, single));
            panel = panel.child(
                Checkbox::new(SharedString::from(format!(
                    "keyboard-map-keymap-clutch-{slot}"
                )))
                .label(t("SWITCH_BACK_TO_PREVIOUS_KEYMAP_DESCRIPTION"))
                .checked(group["isClutch"] == true)
                .disabled(single)
                .on_click(cx.listener(move |this, enabled: &bool, _, cx| {
                    if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                        let group = &mut edit.slots[slot].staged["keymapGroup"];
                        group["isClutch"] = json!(*enabled);
                        if *enabled {
                            group["previousKeymapGuid"] = this.draft["activeKeymapId"].clone();
                        } else if let Some(group) = group.as_object_mut() {
                            group.remove("previousKeymapGuid");
                        }
                    }
                    cx.emit(KeyboardProductChanged);
                    cx.notify();
                })),
            );
        }
        panel.into_any_element()
    }
}
