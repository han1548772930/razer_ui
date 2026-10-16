//! Current MapAILauncher 96867. Engine IDs are observed, never fabricated.
use super::*;
use crate::features::controls::Choice;
use gpui_kit::component::radio::Radio;
use gpui_kit::component::{
    IndexPath,
    select::{Select, SelectEvent, SelectState},
};

#[derive(Clone)]
pub struct KeyboardMappingAiObservation {
    product_id: u32,
    profile_guid: String,
    engines: Vec<Value>,
}
impl KeyboardMappingAiObservation {
    pub fn new(product_id: u32, profile_guid: impl Into<String>, engines: Vec<Value>) -> Self {
        Self {
            product_id,
            profile_guid: profile_guid.into(),
            engines,
        }
    }
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn profile_guid(&self) -> &str {
        &self.profile_guid
    }
}
#[derive(Default)]
pub(super) struct State {
    observation: Option<KeyboardMappingAiObservation>,
    selectors: [Option<Entity<SelectState<Vec<Choice>>>>; 2],
    subscriptions: [Option<Subscription>; 2],
    composer: [String; 2],
    chat: [Value; 2],
}
impl State {
    pub(super) fn dismiss(&mut self) {
        self.selectors = Default::default();
        self.subscriptions = Default::default();
        self.composer = Default::default();
        self.chat = Default::default();
    }
}
#[derive(Deserialize)]
struct Artwork {
    product_id: u32,
    composer_asset: Option<String>,
    services_asset: Option<String>,
}
fn artwork(pid: u32) -> &'static Artwork {
    static DATA: OnceLock<Vec<Artwork>> = OnceLock::new();
    DATA.get_or_init(|| read_keyboard_mapping_ai_catalog())
        .iter()
        .find(|row| row.product_id == pid)
        .expect("source-mounted AI mapping")
}
pub(super) fn valid(workspace: &KeyboardProductWorkspace, slot: &Slot) -> bool {
    let Some(observation) = workspace
        .analog_gamepad
        .mapping
        .ai
        .observation
        .as_ref()
        .filter(|observation| {
            observation.product_id == workspace.spec.product_id
                && workspace.draft["guid"] == observation.profile_guid
        })
    else {
        return false;
    };
    let group = &slot.staged["aiLauncherGroup"];
    match group["assignment"].as_str() {
        Some("OPEN_PROMPT_COMPOSER") => matches!(group["param"].as_str(), Some("chat" | "image")),
        Some("OPEN_AI_CHAT") => observation
            .engines
            .iter()
            .any(|engine| engine["id"] == group["param"]),
        _ => false,
    }
}
impl KeyboardProductWorkspace {
    pub fn observe_mapping_ai(
        &mut self,
        observation: KeyboardMappingAiObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if observation.product_id != self.spec.product_id
            || self.draft["guid"] != observation.profile_guid
        {
            return false;
        }
        self.analog_gamepad.mapping.ai.observation = Some(observation);
        for slot in 0..2 {
            self.init_mapping_ai(slot, window, cx)
        }
        cx.notify();
        true
    }
    pub(super) fn init_mapping_ai(
        &mut self,
        slot: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.analog_gamepad.mapping.ai.selectors[slot] = None;
        self.analog_gamepad.mapping.ai.subscriptions[slot] = None;
        let Some(edit) = self.analog_gamepad.mapping.edit.as_ref() else {
            return;
        };
        if edit.slots[slot].category != "AI_LAUNCHER" {
            return;
        }
        let engines = self
            .analog_gamepad
            .mapping
            .ai
            .observation
            .as_ref()
            .filter(|observation| {
                observation.product_id == self.spec.product_id
                    && self.draft["guid"] == observation.profile_guid
            })
            .map(|observation| observation.engines.clone())
            .unwrap_or_default();
        let selected = engines
            .iter()
            .position(|engine| engine["id"] == edit.slots[slot].staged["aiLauncherGroup"]["param"])
            .unwrap_or(0);
        let group = &edit.slots[slot].staged["aiLauncherGroup"];
        self.analog_gamepad.mapping.ai.composer[slot] =
            if group["assignment"] == "OPEN_PROMPT_COMPOSER" {
                group["param"].as_str().unwrap_or("chat").into()
            } else {
                "chat".into()
            };
        self.analog_gamepad.mapping.ai.chat[slot] = engines
            .get(selected)
            .map(|engine| engine["id"].clone())
            .unwrap_or(json!(0));
        let choices = engines
            .iter()
            .map(|engine| Choice::new(text(&engine["id"]), text(&engine["name"])))
            .collect::<Vec<_>>();
        let selector = cx.new(|cx| {
            SelectState::new(
                choices,
                (!engines.is_empty()).then_some(IndexPath::new(selected)),
                window,
                cx,
            )
            .searchable(false)
        });
        let subscription = cx.subscribe_in(&selector, window, move |this, _, event, _, cx| {
            let SelectEvent::Confirm(Some(id)) = event else {
                return;
            };
            let Some(observation) = this.analog_gamepad.mapping.ai.observation.as_ref() else {
                return;
            };
            let Some(engine) = observation
                .engines
                .iter()
                .find(|engine| text(&engine["id"]) == *id)
            else {
                return;
            };
            let id = engine["id"].clone();
            if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                if edit.slots[slot].category != "AI_LAUNCHER"
                    || edit.slots[slot].staged["aiLauncherGroup"]["assignment"] != "OPEN_AI_CHAT"
                {
                    return;
                }
                edit.slots[slot].staged["aiLauncherGroup"]["param"] = id.clone();
            }
            this.analog_gamepad.mapping.ai.chat[slot] = id;
            cx.emit(KeyboardProductChanged);
            cx.notify();
        });
        self.analog_gamepad.mapping.ai.selectors[slot] = Some(selector);
        self.analog_gamepad.mapping.ai.subscriptions[slot] = Some(subscription);
    }
    fn change_mapping_ai_launcher(&mut self, slot: usize, launcher: &str, cx: &mut Context<Self>) {
        let state = &self.analog_gamepad.mapping.ai;
        let composer = if state.composer[slot].is_empty() {
            "chat"
        } else {
            &state.composer[slot]
        };
        let param = if launcher == "OPEN_PROMPT_COMPOSER" {
            json!(composer)
        } else {
            state.chat[slot].clone()
        };
        if let Some(edit) = &mut self.analog_gamepad.mapping.edit {
            let group = &mut edit.slots[slot].staged["aiLauncherGroup"];
            group["assignment"] = json!(launcher);
            group["param"] = param;
        }
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
    pub(super) fn mapping_ai_element(&self, slot: usize, cx: &Context<Self>) -> AnyElement {
        let edit = self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .expect("mapping edit");
        let group = &edit.slots[slot].staged["aiLauncherGroup"];
        let composer = group["assignment"] == "OPEN_PROMPT_COMPOSER";
        let mut panel = v_flex()
            .mx(surface::css(20.))
            .mb(surface::css(20.))
            .gap(surface::css(10.))
            .child(t("LAUNCH_AI_OVERLAY"));
        for (launcher, tip) in [
            ("OPEN_PROMPT_COMPOSER", "AI_LAUNCHER_DES4"),
            ("OPEN_AI_CHAT", "AI_CHAT_TIP"),
        ] {
            panel = panel.child(
                h_flex()
                    .child(
                        Radio::new(SharedString::from(format!(
                            "keyboard-ai-radio-{slot}-{launcher}"
                        )))
                        .label(t(launcher))
                        .checked(group["assignment"] == launcher)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.change_mapping_ai_launcher(slot, launcher, cx)
                        })),
                    )
                    .child(surface::help_control(
                        SharedString::from(format!("keyboard-ai-tip-{slot}-{launcher}")),
                        t(tip),
                    )),
            );
            if launcher == "OPEN_PROMPT_COMPOSER" {
                let mut row = h_flex().ml(surface::css(30.));
                for (label, value) in [("TEXT", "chat"), ("IMAGE", "image")] {
                    row = row.child(
                        Button::new(SharedString::from(format!(
                            "keyboard-ai-composer-{slot}-{value}"
                        )))
                        .label(t(label))
                        .outline()
                        .selected(group["param"] == value)
                        .disabled(!composer)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                if edit.slots[slot].staged["aiLauncherGroup"]["assignment"]
                                    != "OPEN_PROMPT_COMPOSER"
                                {
                                    return;
                                }
                                edit.slots[slot].staged["aiLauncherGroup"]["param"] = json!(value);
                            }
                            this.analog_gamepad.mapping.ai.composer[slot] = value.into();
                            cx.emit(KeyboardProductChanged);
                            cx.notify();
                        })),
                    );
                }
                panel = panel.child(row);
            }
        }
        if let Some(selector) = &self.analog_gamepad.mapping.ai.selectors[slot] {
            panel = panel.child(
                div()
                    .ml(surface::css(30.))
                    .w(surface::css(180.))
                    .child(Select::new(selector).disabled(composer)),
            );
        }
        let artwork = artwork(self.spec.product_id);
        let image = if composer {
            &artwork.composer_asset
        } else {
            &artwork.services_asset
        };
        if let Some(image) = image {
            panel = panel.child(
                img(SharedString::from(image.clone()))
                    .w(surface::css(210.))
                    .h(surface::css(120.))
                    .my(surface::css(20.)),
            )
        }
        panel.into_any_element()
    }
}

// Generated catalog reader: razer-pages/keyboard_mapping_ai_data.
fn read_keyboard_mapping_ai_catalog() -> Vec<Artwork> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Artwork>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Artwork> {
        let value = match r.tag()? {
            0 => Artwork {
                product_id: r.child(decode_2)?,
                composer_asset: r.child(decode_3)?,
                services_asset: r.child(decode_3)?,
            },
            1 => Artwork {
                product_id: r.child(decode_2)?,
                composer_asset: None,
                services_asset: None,
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
    fn decode_3(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<String>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_4)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_MAPPING_AI_DATA,
        0,
        "200c2009cb7227b55f5a712e856ed2b62f049ce7ecaeb0a8b7ff9032cf954caa",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod keyboard_mapping_ai_data_catalog_tests {
    #[test]
    fn packaged_catalog_keyboard_mapping_ai_data_decodes() {
        let _ = super::read_keyboard_mapping_ai_catalog();
    }
}
// End generated catalog reader: razer-pages/keyboard_mapping_ai_data.
