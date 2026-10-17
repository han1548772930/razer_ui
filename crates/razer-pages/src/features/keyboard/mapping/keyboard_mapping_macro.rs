//! Current MapMacro 82508. PID 747 retains its own inline Sequence branch.
use super::*;
use crate::features::controls::Choice;
use gpui_kit::component::{
    IndexPath,
    select::{Select, SelectEvent, SelectState},
};
use razer_widgets::stepper::{Stepper, StepperEvent};

#[derive(Clone)]
pub struct KeyboardMappingMacroObservation {
    pub product_id: u32,
    pub profile_guid: String,
    pub macro_list: Vec<Value>,
    pub macro_available: bool,
    pub is_macro_installed: bool,
}
#[derive(Default)]
pub(super) struct State {
    observation: Option<KeyboardMappingMacroObservation>,
    requirements: Option<Result<Vec<razer_model::mapping_requirements::MacroRequirement>, String>>,
    selectors: [Option<Entity<SelectState<Vec<Choice>>>>; 2],
    selector_subscriptions: [Option<Subscription>; 2],
    playback: [Option<Entity<SelectState<Vec<Choice>>>>; 2],
    playback_subscriptions: [Option<Subscription>; 2],
    counts: [Option<Entity<Stepper>>; 2],
    subscriptions: [Option<Subscription>; 2],
}
impl State {
    pub(super) fn dismiss(&mut self) {
        self.selectors = Default::default();
        self.selector_subscriptions = Default::default();
        self.playback = Default::default();
        self.playback_subscriptions = Default::default();
        self.counts = Default::default();
        self.subscriptions = Default::default();
    }
}
pub(super) fn valid(workspace: &KeyboardProductWorkspace, slot: &Slot) -> bool {
    let Some(observation) = workspace.mapping_macro_observation() else {
        return false;
    };
    if !observation.macro_available {
        return false;
    }
    let group = &slot.staged["macroGroup"];
    let Some(selected) = observation
        .macro_list
        .iter()
        .find(|row| row["guid"] == group["guid"] && row["guid"].is_string())
    else {
        return false;
    };
    selected["name"]
        .as_str()
        .is_some_and(|name| !name.trim().is_empty())
        && workspace
            .mapping_macro_choices(selected)
            .iter()
            .any(|choice| choice["id"] == group["macroPlaybackOption"])
}

#[derive(Deserialize)]
struct SortSource {
    product_id: u32,
    chars: String,
}

fn source_name_cmp(pid: u32, left: &str, right: &str) -> std::cmp::Ordering {
    static DATA: OnceLock<Vec<SortSource>> = OnceLock::new();
    let alphabet = &DATA
        .get_or_init(|| read_keyboard_mapping_name_sort_catalog())
        .iter()
        .find(|row| row.product_id == pid)
        .expect("current keyboard macro helper")
        .chars;
    let left = left.to_lowercase();
    let right = right.to_lowercase();
    let left = left.encode_utf16().collect::<Vec<_>>();
    let right = right.encode_utf16().collect::<Vec<_>>();
    let rank = |name: &[u16], ix: usize| {
        name.get(ix).and_then(|unit| {
            alphabet
                .encode_utf16()
                .position(|candidate| candidate == *unit)
        })
    };
    let first = (rank(&left, 0), rank(&right, 0));
    let second = (rank(&left, 1), rank(&right, 1));
    if first.0 == first.1 {
        match second {
            (Some(left_rank), Some(right_rank)) if left_rank != right_rank => {
                right_rank.cmp(&left_rank)
            }
            _ => left.cmp(&right),
        }
    } else {
        match first {
            (Some(left_rank), Some(right_rank)) => left_rank.cmp(&right_rank),
            _ => left.cmp(&right),
        }
    }
}

impl KeyboardProductWorkspace {
    pub(super) fn mapping_macro_observation(&self) -> Option<&KeyboardMappingMacroObservation> {
        self.analog_gamepad
            .mapping
            .macros
            .observation
            .as_ref()
            .filter(|observation| observation.profile_guid == self.draft["guid"])
    }
    pub(super) fn mapping_macro_requirements(
        &self,
    ) -> anyhow::Result<&[razer_model::mapping_requirements::MacroRequirement]> {
        if self.mapping_macro_observation().is_none() {
            return Ok(&[]);
        }
        match &self.analog_gamepad.mapping.macros.requirements {
            Some(Ok(requirements)) => Ok(requirements),
            Some(Err(error)) => anyhow::bail!("{error}"),
            None => anyhow::bail!("Macro requirements have not been observed"),
        }
    }
    pub fn observe_mapping_macros(
        &mut self,
        observation: KeyboardMappingMacroObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if observation.product_id != self.spec.product_id
            || observation.profile_guid != self.draft["guid"]
        {
            return;
        }
        self.analog_gamepad.mapping.macros.requirements = Some(
            razer_model::mapping_requirements::MacroRequirement::from_source_catalog(
                &observation.macro_list,
            )
            .map_err(|error| format!("{error:#}")),
        );
        self.analog_gamepad.mapping.macros.observation = Some(observation);
        self.init_mapping_macro(0, window, cx);
        self.init_mapping_macro(1, window, cx);
        cx.notify();
    }
    pub(super) fn mapping_macro_choices(&self, selected: &Value) -> &[Value] {
        let source = source(self.spec.product_id);
        if selected["macroPlaybackOption"] == "sequence" {
            return &source.sequence_macro_choices;
        }
        if !source.macro_legacy_without_phase && selected["macroPlaybackOption"] == "phased" {
            return &source.phased_macro_choices;
        }
        let edit = self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .expect("mapping edit");
        let input = edit.button["inputID"].as_str().unwrap_or_default();
        let single_profile = self.spec.config["DeviceInfo"]["singleProfileDevice"] == true;
        let special = if source.macro_legacy_without_phase {
            &["DKM_SB_210", "DKM_SB_03"][..]
        } else {
            &["DKM_SB_210", "DKM_AI_PROMPT", "DKM_SB_03"][..]
        };
        if source.scroll_macro_inputs.iter().any(|id| id == input)
            || single_profile && special.contains(&input)
        {
            &source.scroll_macro_choices
        } else {
            &source.macro_choices
        }
    }
    fn stage_mapping_macro(&mut self, slot: usize, guid: Option<&str>, cx: &mut Context<Self>) {
        let selected = self
            .mapping_macro_observation()
            .and_then(|observation| {
                observation
                    .macro_list
                    .iter()
                    .filter(|row| guid.is_none() || row["guid"].as_str() == guid)
                    .min_by(|left, right| {
                        source_name_cmp(
                            self.spec.product_id,
                            &text(&left["name"]),
                            &text(&right["name"]),
                        )
                    })
            })
            .cloned();
        let Some(selected) = selected else { return };
        let option = self
            .mapping_macro_choices(&selected)
            .first()
            .map(|choice| choice["id"].clone())
            .unwrap_or(Value::Null);
        if let Some(edit) = &mut self.analog_gamepad.mapping.edit {
            edit.slots[slot].staged["macroGroup"] = json!({"name": selected["name"], "guid": selected["guid"], "macroPlaybackOption": option, "repeatCount": 2});
        }
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
    pub(super) fn init_mapping_macro(
        &mut self,
        slot: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(edit) = self.analog_gamepad.mapping.edit.as_ref() else {
            return;
        };
        if edit.slots[slot].category != "MACRO" {
            self.analog_gamepad.mapping.macros.selectors[slot] = None;
            self.analog_gamepad.mapping.macros.selector_subscriptions[slot] = None;
            self.analog_gamepad.mapping.macros.playback[slot] = None;
            self.analog_gamepad.mapping.macros.playback_subscriptions[slot] = None;
            self.analog_gamepad.mapping.macros.counts[slot] = None;
            self.analog_gamepad.mapping.macros.subscriptions[slot] = None;
            return;
        }
        if edit.slots[slot].staged["macroGroup"]["guid"].is_null() {
            self.stage_mapping_macro(slot, None, cx);
        }
        self.init_mapping_macro_selectors(slot, window, cx);
        let value = self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .and_then(|edit| edit.slots[slot].staged["macroGroup"]["repeatCount"].as_f64())
            .unwrap_or(2.);
        let count = cx.new(|cx| {
            Stepper::new(
                SharedString::from(format!("keyboard-map-macro-count-{slot}")),
                value,
                (1., 99., 1.),
                false,
                false,
                Some(2),
                window,
                cx,
            )
            .in_custom_keymapping()
            .live_update()
        });
        let subscription = cx.subscribe_in(
            &count,
            window,
            move |this, _, event: &StepperEvent, _, cx| {
                let Some(edit) = &mut this.analog_gamepad.mapping.edit else {
                    return;
                };
                if edit.slots[slot].category != "MACRO" {
                    return;
                }
                edit.slots[slot].staged["macroGroup"]["repeatCount"] = event
                    .draft
                    .as_ref()
                    .map(|draft| json!(draft))
                    .unwrap_or_else(|| json!(event.value as u32));
                cx.emit(KeyboardProductChanged);
                cx.notify();
            },
        );
        self.analog_gamepad.mapping.macros.counts[slot] = Some(count);
        self.analog_gamepad.mapping.macros.subscriptions[slot] = Some(subscription);
    }
    fn init_mapping_macro_selectors(
        &mut self,
        slot: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(edit) = &self.analog_gamepad.mapping.edit else {
            return;
        };
        let group = &edit.slots[slot].staged["macroGroup"];
        let mut rows = self
            .mapping_macro_observation()
            .map(|observation| observation.macro_list.iter().collect::<Vec<_>>())
            .unwrap_or_default();
        rows.sort_by(|left, right| {
            source_name_cmp(
                self.spec.product_id,
                &text(&left["name"]),
                &text(&right["name"]),
            )
        });
        let items = rows
            .iter()
            .filter_map(|row| Some(Choice::new(row["guid"].as_str()?, text(&row["name"]))))
            .collect::<Vec<_>>();
        let selected = items.iter().position(|item| group["guid"] == item.id());
        let macro_row = rows.iter().find(|row| row["guid"] == group["guid"]);
        let playback_rows = self
            .mapping_macro_choices(macro_row.copied().unwrap_or(&Value::Null))
            .iter()
            .filter(|choice| {
                !(source(self.spec.product_id).macro_legacy_without_phase
                    && edit.button["inputID"] == "DKM_SB_03"
                    && choice["id"] == "ContinuousHeld")
            })
            .collect::<Vec<_>>();
        let playback_selected = playback_rows
            .iter()
            .position(|row| row["id"] == group["macroPlaybackOption"]);
        let playback_items = playback_rows
            .iter()
            .map(|row| Choice::localized(text(&row["id"]), text(&row["content"])))
            .collect::<Vec<_>>();
        let selector = cx.new(|cx| {
            SelectState::new(items, selected.map(IndexPath::new), window, cx).searchable(false)
        });
        let subscription = cx.subscribe_in(&selector, window, move |this, _, event, window, cx| {
            let SelectEvent::Confirm(Some(guid)) = event else {
                return;
            };
            if !this
                .analog_gamepad
                .mapping
                .edit
                .as_ref()
                .is_some_and(|edit| edit.slots[slot].category == "MACRO")
            {
                return;
            }
            this.stage_mapping_macro(slot, Some(guid), cx);
            this.init_mapping_macro(slot, window, cx);
        });
        let playback = cx.new(|cx| {
            SelectState::new(
                playback_items,
                playback_selected.map(IndexPath::new),
                window,
                cx,
            )
            .searchable(false)
        });
        let playback_subscription =
            cx.subscribe_in(&playback, window, move |this, _, event, window, cx| {
                let SelectEvent::Confirm(Some(id)) = event else {
                    return;
                };
                let Some(edit) = &mut this.analog_gamepad.mapping.edit else {
                    return;
                };
                if edit.slots[slot].category != "MACRO" {
                    return;
                }
                edit.slots[slot].staged["macroGroup"]["macroPlaybackOption"] = json!(id);
                if id != "NTimes" {
                    edit.slots[slot].staged["macroGroup"]["repeatCount"] = json!(2);
                }
                this.init_mapping_macro(slot, window, cx);
                cx.emit(KeyboardProductChanged);
                cx.notify();
            });
        self.analog_gamepad.mapping.macros.selectors[slot] = Some(selector);
        self.analog_gamepad.mapping.macros.selector_subscriptions[slot] = Some(subscription);
        self.analog_gamepad.mapping.macros.playback[slot] = Some(playback);
        self.analog_gamepad.mapping.macros.playback_subscriptions[slot] =
            Some(playback_subscription);
    }
    pub(super) fn mapping_macro_element(&self, slot: usize, cx: &Context<Self>) -> AnyElement {
        let edit = self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .expect("mapping edit");
        let group = &edit.slots[slot].staged["macroGroup"];
        let observation = self.mapping_macro_observation();
        let available = observation.is_some_and(|observation| observation.macro_available);
        let mut panel = v_flex()
            .mx(surface::css(20.))
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_color(rgb(0xcccccc));
        if let Some(selector) = &self.analog_gamepad.mapping.macros.selectors[slot] {
            panel = panel.child(
                Select::new(selector)
                    .placeholder(" ")
                    .mb(surface::css(10.))
                    .w(surface::css(210.))
                    .h(surface::css(27.))
                    .menu_max_h(surface::css(180.)),
            );
        }
        panel = panel.child(
            div()
                .mb(surface::css(5.))
                .opacity(if available { 1. } else { 0.3 })
                .child(t("PLAYBACK_OPTION")),
        );
        if let Some(selector) = &self.analog_gamepad.mapping.macros.playback[slot] {
            panel = panel.child(
                Select::new(selector)
                    .placeholder(" ")
                    .mb(surface::css(10.))
                    .w(surface::css(210.))
                    .h(surface::css(27.))
                    .menu_max_h(surface::css(180.))
                    .disabled(!available),
            );
        }
        if group["macroPlaybackOption"] == "NTimes" {
            panel = panel
                .child(div().mb(surface::css(5.)).child(t("NUMBER_OF_TIMES")))
                .children(self.analog_gamepad.mapping.macros.counts[slot].clone());
        }
        if !available {
            panel = panel.child(div().mb(surface::css(20.)).child(t("CREATE_MACRO_MSG")));
        }
        if observation.is_some_and(|observation| observation.is_macro_installed) {
            panel = panel.child(
                BaseButton::new(SharedString::from(format!(
                    "keyboard-map-configure-macro-{slot}"
                )))
                .underline()
                .mb(surface::css(20.))
                .child(t("CONFIGURE_MACROS"))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.request_lighting_command(KeyboardLightingCommand::OpenMappingMacro, cx)
                })),
            );
        }
        panel.into_any_element()
    }
}

// Generated catalog reader: razer-pages/keyboard_mapping_name_sort_data.
fn read_keyboard_mapping_name_sort_catalog() -> Vec<SortSource> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<SortSource>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<SortSource> {
        let value = match r.tag()? {
            0 => SortSource {
                product_id: r.child(decode_2)?,
                chars: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_MAPPING_NAME_SORT_DATA,
        0,
        "b27a80ed127a26bea49361806e1e4c098b7e6e38a1122c7a18d6afdcdbf79443",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod keyboard_mapping_name_sort_data_catalog_tests {
    #[test]
    fn packaged_catalog_keyboard_mapping_name_sort_data_decodes() {
        let _ = super::read_keyboard_mapping_name_sort_catalog();
    }
}
// End generated catalog reader: razer-pages/keyboard_mapping_name_sort_data.
