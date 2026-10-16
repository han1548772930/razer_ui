//! Retained current MapMouse/Multimedia/Windows/Backlight action dropdowns.
use super::*;
use crate::features::controls::Choice;
use gpui_kit::component::{
    IndexPath,
    select::{Select, SelectEvent, SelectState},
};

#[derive(Default)]
pub(super) struct State {
    selectors: [Option<Entity<SelectState<Vec<Choice>>>>; 2],
    subscriptions: [Option<Subscription>; 2],
}
impl State {
    pub(super) fn dismiss(&mut self) {
        self.selectors = Default::default();
        self.subscriptions = Default::default();
    }
}
pub(super) fn valid(workspace: &KeyboardProductWorkspace, slot: &Slot) -> bool {
    source(workspace.spec.product_id)
        .options
        .get(&slot.category)
        .is_some_and(|options| {
            workspace
                .mapping_option_choices(&slot.category)
                .iter()
                .any(|choice| choice["id"] == slot.staged[&options.group][&options.field])
        })
}
impl KeyboardProductWorkspace {
    fn mapping_option_choices(&self, name: &str) -> Vec<&'static Value> {
        let source = source(self.spec.product_id);
        let Some(options) = source.options.get(name) else {
            return Vec::new();
        };
        let button = self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .map(|edit| &edit.button);
        let disable_turbo = button.is_some_and(|button| {
            source
                .no_turbo
                .iter()
                .any(|id| button["buttonKey"] == id.as_str())
                || button["inputType"] == "ControllerInput"
                || button["disableTurboInAssignment"]
                    .as_array()
                    .is_some_and(|rows| rows.iter().any(|row| row == name))
        });
        let extended =
            self.spec.config["DeviceInfo"]["extendSupportMappings"]["mouseGroup"].as_array();
        options
            .choices
            .iter()
            .filter(|choice| {
                name != "MOUSE_FUNCTION"
                    || !disable_turbo
                    || !extended.is_some_and(|rows| rows.iter().any(|row| row == &choice["id"]))
            })
            .collect()
    }
    pub(super) fn init_mapping_options(
        &mut self,
        slot: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(edit) = &self.analog_gamepad.mapping.edit else {
            return;
        };
        let name = edit.slots[slot].category.clone();
        let Some(options) = source(self.spec.product_id).options.get(&name) else {
            self.analog_gamepad.mapping.options.selectors[slot] = None;
            self.analog_gamepad.mapping.options.subscriptions[slot] = None;
            return;
        };
        let choices = self.mapping_option_choices(&name);
        let selected = choices.iter().position(|choice| {
            choice["id"] == edit.slots[slot].staged[&options.group][&options.field]
        });
        let choices = choices
            .iter()
            .map(|choice| Choice::localized(text(&choice["id"]), text(&choice["content"])))
            .collect::<Vec<_>>();
        let selector = cx.new(|cx| {
            SelectState::new(choices, selected.map(IndexPath::new), window, cx).searchable(false)
        });
        let group = options.group.clone();
        let field = options.field.clone();
        let subscription = cx.subscribe_in(&selector, window, move |this, _, event, window, cx| {
            let SelectEvent::Confirm(Some(id)) = event else {
                return;
            };
            let Some(edit) = &mut this.analog_gamepad.mapping.edit else {
                return;
            };
            if edit.slots[slot].category != name {
                return;
            }
            let old = edit.slots[slot].staged[&group].clone();
            let mut payload = json!({"outputType":group});
            payload[&group] = json!({});
            payload[&group][&field] = json!(id);
            if let Some(turbo) = old.get("turboMode") {
                payload[&group]["turboMode"] = turbo.clone();
            }
            if name == "MULTIMEDIA" && matches!(id.as_str(), "VolumeUp" | "VolumeDown") {
                payload[&group]["useTurbo"] = json!(true);
            }
            two_tap::replace_slot(&mut edit.slots[slot], payload);
            this.sync_mapping_turbo(slot, window, cx);
            cx.emit(KeyboardProductChanged);
            cx.notify();
        });
        self.analog_gamepad.mapping.options.selectors[slot] = Some(selector);
        self.analog_gamepad.mapping.options.subscriptions[slot] = Some(subscription);
    }
    pub(super) fn mapping_options_element(&self, slot: usize) -> Option<AnyElement> {
        self.analog_gamepad.mapping.options.selectors[slot]
            .as_ref()
            .map(|selector| {
                div()
                    .mx(surface::css(20.))
                    .mb(surface::css(20.))
                    .child(Select::new(selector).w_full())
                    .into_any_element()
            })
    }
}
