//! PID70 MapLighting.596cfe94 factory4278. DeviceInfo has no
//! isAdvancedMapLighting: this mounted form chooses one local Chroma profile.
use super::super::Choice;
use super::*;
use razer_widgets::keyboard_navigation::Button as BaseButton;
use gpui_kit::component::{
    IndexPath,
    select::{SelectEvent, SelectState},
};
use razer_widgets::theme::MouseCustomizeColors;

#[derive(Default)]
pub(super) struct State {
    catalog: Vec<Value>,
    selected: usize,
    selected_id: Option<Value>,
    available: bool,
    select: Option<Entity<SelectState<Vec<Choice>>>>,
}
impl State {
    fn choices(&self) -> Vec<Choice> {
        if self.catalog.is_empty() {
            return vec![Choice::new("0", "")];
        }
        self.catalog
            .iter()
            .enumerate()
            .map(|(index, profile)| {
                Choice::new(
                    index.to_string(),
                    profile["name"].as_str().unwrap_or_default(),
                )
            })
            .collect()
    }
}
impl MouseProductWorkspace {
    pub(super) fn initialize_mouse_mapping_lighting_70(
        &mut self,
        button: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let observed = self.lighting_observation();
        let catalog = observed["chromaProfiles"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let active = catalog
            .iter()
            .position(|profile| profile["id"] == observed["selectedChromaProfileGuid"]);
        let mut state = State {
            selected: active.unwrap_or(0),
            selected_id: active.map(|index| catalog[index]["id"].clone()),
            available: catalog
                .first()
                .is_some_and(|row| row["name"].as_str().is_some_and(|name| !name.is_empty())),
            catalog,
            ..Default::default()
        };
        if button["assignment"] == "SWITCH_LIGHTING" {
            if let Some(index) = state
                .catalog
                .iter()
                .position(|profile| profile["id"] == button["assignmentGroup"]["ID"])
            {
                state.selected = index;
                state.selected_id = Some(state.catalog[index]["id"].clone());
            }
        }
        let select = cx.new(|cx| {
            SelectState::new(
                state.choices(),
                Some(IndexPath::new(state.selected)),
                window,
                cx,
            )
        });
        self.mapping_current_state.subscriptions.push(cx.subscribe(
            &select,
            |this, _, event, cx| {
                let SelectEvent::Confirm(Some(index)) = event else {
                    return;
                };
                let Ok(index) = index.parse::<usize>() else {
                    return;
                };
                let state = &mut this.mapping_current_state.lighting;
                let Some(profile) = state.catalog.get(index) else {
                    return;
                };
                state.selected = index;
                state.selected_id = Some(profile["id"].clone());
                this.mapping_current_state.can_save = state.available;
                this.mapping_current_state.changed = true;
                cx.notify();
            },
        ));
        state.select = Some(select);
        self.mapping_current_state.can_save =
            state.available && button["assignment"] != "SWITCH_LIGHTING";
        self.mapping_current_state.changed = false;
        self.mapping_current_state.basic_requires_synapse = true;
        self.mapping_current_state.lighting = state;
    }
    pub(super) fn reconcile_mouse_mapping_lighting_70(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 70 || self.mapping_current_state.function != "SWITCH_LIGHTING" {
            return;
        }
        let observation = self.lighting_observation();
        let catalog = observation["chromaProfiles"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let selected_guid = observation["selectedChromaProfileGuid"].clone();
        let state = &mut self.mapping_current_state.lighting;
        if catalog == state.catalog {
            return;
        }
        if catalog.is_empty() {
            // The source empty update leaves lightpacAvailable unchanged.
            state.catalog = catalog;
            state.selected = 0;
        } else {
            state.selected = catalog
                .iter()
                .position(|row| Some(&row["id"]) == state.selected_id.as_ref())
                .or_else(|| catalog.iter().position(|row| row["id"] == selected_guid))
                .unwrap_or(0);
            state.selected_id = Some(catalog[state.selected]["id"].clone());
            state.available = catalog[0]["name"]
                .as_str()
                .is_some_and(|name| !name.is_empty());
            state.catalog = catalog;
            self.mapping_current_state.can_save = state.available;
            self.mapping_current_state.changed = false;
        }
        if let Some(select) = &state.select {
            let choices = state.choices();
            let selected = state.selected;
            select.update(cx, |select, cx| {
                select.set_items(choices, window, cx);
                select.set_selected_index(Some(IndexPath::new(selected)), window, cx);
            });
        }
    }
    pub(super) fn mouse_mapping_lighting_70_payload(&self, button: &Value) -> Option<Vec<Value>> {
        let profile = self
            .mapping_current_state
            .lighting
            .catalog
            .get(self.mapping_current_state.lighting.selected)?;
        Some(vec![
            json!({"inputID":button["inputID"],"inputType":button["inputType"],"isHyperShift":self.hypershift,"outputType":"lightPacGroup","lightPacGroup":{"ID":profile["id"],"Name":profile["name"]}}),
        ])
    }
    pub(super) fn mouse_mapping_lighting_70_body(&self, cx: &Context<Self>) -> AnyElement {
        let state = &self.mapping_current_state.lighting;
        let installed = self.lighting_observation()["isChromaInstalled"] == true
            || self.lighting_observation()["installedModules"]
                .as_array()
                .is_some_and(|rows| rows.iter().any(|row| row == "chroma-app"));
        let mut body = v_flex()
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_color(MouseCustomizeColors::text());
        if installed {
            if let Some(select) = &state.select {
                body = body.child(
                    razer_widgets::synapse_select::select(select)
                        .items(state.choices())
                        .w_full(),
                );
            }
            if !state.available {
                body = body.child(
                    div()
                        .mb(surface::css(20.))
                        .child(t("CONFIGURE_CHROMA_STUDIO_MSG")),
                );
            }
            body = body.child(
                BaseButton::new("mouse70-mapping-open-chroma")
                    .p_0()
                    .mb(surface::css(20.))
                    .justify_start()
                    .child(div().underline().child(t("CONFIGURE_CHROMA_STUDIO")))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.emit_lighting("openChromaStudio", json!({}), cx)
                    })),
            );
        }
        if !installed {
            body = body.child(
                div()
                    .mb(surface::css(20.))
                    .child(t("REQUIRE_CHROMA_APP_TOOLTIP")),
            );
        }
        // The source absent-module branch opens the remote module installer;
        // Internet installation surfaces are excluded from this local product.
        body.into_any_element()
    }
}
