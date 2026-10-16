//! PID70 MapSwitchProfile.6bdbef50, factory8361. The profile catalog is
//! supplied by the local owner; Chroma choices come from its actual reducer.
use super::super::Choice;
use super::*;
use gpui_kit::component::{
    IndexPath,
    radio::Radio,
    select::{SelectEvent, SelectState},
};

const OPTIONS: &[(&str, &str)] = &[
    ("NextProfile", "NEXT_PROFILE"),
    ("PreviousProfile", "PREVIOUS_PROFILE"),
    ("CycleUp", "CYCLE_UP_PROFILE"),
    ("CycleDown", "CYCLE_DOWN_PROFILE"),
    ("Specific", "SPECIFIC_PROFILE"),
];

#[derive(Default)]
pub(super) struct State {
    profiles: Vec<Value>,
    selected: usize,
    option: usize,
    switch_lighting: bool,
    chroma: Vec<Value>,
    selected_chroma: usize,
    profile_select: Option<Entity<SelectState<Vec<Choice>>>>,
    chroma_select: Option<Entity<SelectState<Vec<Choice>>>>,
}

impl MouseProductWorkspace {
    pub fn observe_local_mapping_profiles(
        &mut self,
        profiles: Vec<Value>,
        active: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let reducer = json!({
            "profiles": profiles, "selectedProfileGuid": active
        });
        if self.mouse_customize_observation["profileReducer"] == reducer {
            return;
        }
        self.mouse_customize_observation["profileReducer"] = reducer;
        if self.spec.product_id == 70 && self.mapping_current_state.function == "SWITCH_PROFILE" {
            if let Some(button) = self
                .mapping_input
                .as_deref()
                .and_then(|input| self.current_mapping_source_button(input))
            {
                self.initialize_mouse_profile_70(&button, window, cx);
            }
        }
        cx.notify();
    }

    pub(super) fn initialize_mouse_profile_70(
        &mut self,
        button: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let reducer = &self.mouse_customize_observation["profileReducer"];
        let profiles = reducer["profiles"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|profile| profile["guid"] != reducer["selectedProfileGuid"])
            .cloned()
            .collect::<Vec<_>>();
        let observed = self.lighting_observation();
        let chroma = observed["chromaProfiles"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let group = &button["assignmentGroup"];
        let mapped = button["assignment"] == "SWITCH_PROFILE";
        let option = if mapped && group.get("Name").is_some() {
            4
        } else if mapped {
            OPTIONS
                .iter()
                .position(|(id, _)| group["profileNavigationAssignment"] == *id)
                .unwrap_or(usize::MAX)
        } else {
            0
        };
        let selected = if mapped && group.get("Name").is_some() {
            profiles
                .iter()
                .position(|profile| profile["guid"] == group["ProfileId"])
                .unwrap_or(usize::MAX)
        } else {
            0
        };
        // Source render selects the existing guid independently from the saved
        // selectedProfile index until changeProfile sets userSelectingProfile.
        let displayed = profiles
            .iter()
            .position(|profile| profile["guid"] == group["guid"])
            .unwrap_or(0);
        let existing_chroma = mapped
            .then(|| {
                chroma
                    .iter()
                    .position(|profile| profile["id"] == group["lightpacId"])
            })
            .flatten();
        let selected_chroma = existing_chroma
            .or_else(|| {
                chroma
                    .iter()
                    .position(|profile| profile["id"] == observed["selectedChromaProfileGuid"])
            })
            .unwrap_or(usize::MAX);
        let choices = profiles
            .iter()
            .filter_map(|profile| {
                Some(Choice::new(
                    profile["guid"].as_str()?,
                    profile["name"].as_str()?,
                ))
            })
            .collect::<Vec<_>>();
        let chroma_choices = chroma
            .iter()
            .filter_map(|profile| {
                Some(Choice::new(
                    profile["id"].as_str()?,
                    profile["name"].as_str()?,
                ))
            })
            .collect::<Vec<_>>();
        let profile_select = cx.new(|cx| {
            SelectState::new(
                choices,
                (!profiles.is_empty()).then_some(IndexPath::new(displayed)),
                window,
                cx,
            )
        });
        let chroma_select = cx.new(|cx| {
            SelectState::new(
                chroma_choices,
                (selected_chroma < chroma.len()).then_some(IndexPath::new(selected_chroma)),
                window,
                cx,
            )
        });
        let profile_subscription =
            cx.subscribe_in(&profile_select, window, |this, _, event, _, cx| {
                if let SelectEvent::Confirm(Some(id)) = event {
                    let state = &mut this.mapping_current_state.profile;
                    if let Some(index) = state
                        .profiles
                        .iter()
                        .position(|profile| profile["guid"] == id.as_str())
                    {
                        state.selected = index;
                        this.mapping_current_state.changed = true;
                        this.mapping_current_state.can_save = true;
                        cx.notify();
                    }
                }
            });
        let chroma_subscription =
            cx.subscribe_in(&chroma_select, window, |this, _, event, _, cx| {
                if let SelectEvent::Confirm(Some(id)) = event {
                    let state = &mut this.mapping_current_state.profile;
                    if let Some(index) = state
                        .chroma
                        .iter()
                        .position(|profile| profile["id"] == id.as_str())
                    {
                        state.selected_chroma = index;
                        this.mapping_current_state.changed = true;
                        this.mapping_current_state.can_save = true;
                        cx.notify();
                    }
                }
            });
        let state = &mut self.mapping_current_state;
        state.can_save = !profiles.is_empty();
        state.changed = !profiles.is_empty();
        state.basic_requires_synapse = true;
        state.profile = State {
            profiles,
            selected,
            option,
            switch_lighting: existing_chroma.is_some(),
            chroma,
            selected_chroma,
            profile_select: Some(profile_select),
            chroma_select: Some(chroma_select),
        };
        state
            .subscriptions
            .extend([profile_subscription, chroma_subscription]);
        self.mapping_190_state.missing_observation = None;
    }

    pub(super) fn mouse_profile_70_payload(&self, button: &Value) -> Option<Vec<Value>> {
        let state = &self.mapping_current_state.profile;
        let mut group = json!({"profileNavigationAssignment":OPTIONS.get(state.option)?.0});
        if state.option == 4 {
            group["guid"] = state.profiles.get(state.selected)?["guid"].clone();
            if state.switch_lighting
                && let Some(profile) = state.chroma.get(state.selected_chroma)
            {
                group["lightpacId"] = profile["id"].clone();
            }
        }
        Some(vec![
            json!({"inputID":button["inputID"],"inputType":button["inputType"],
            "isHyperShift":self.hypershift,"outputType":"profileNavigationGroup","profileNavigationGroup":group}),
        ])
    }

    pub(super) fn mouse_profile_70_body(&self, cx: &Context<Self>) -> AnyElement {
        let state = &self.mapping_current_state.profile;
        let disabled = state.profiles.is_empty();
        let mut body = v_flex().opacity(if disabled { 0.3 } else { 1. });
        for (index, (id, label)) in OPTIONS.iter().enumerate() {
            body = body.child(
                Radio::new(SharedString::from(format!("mouse70-profile-{id}")))
                    .label(t(label))
                    .checked(state.option == index)
                    .disabled(disabled)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let state = &mut this.mapping_current_state.profile;
                        state.option = index;
                        state.selected = 0;
                        if let Some(select) = &state.profile_select {
                            select.update(cx, |select, cx| {
                                select.set_selected_index(Some(IndexPath::new(0)), window, cx)
                            });
                        }
                        this.mapping_current_state.changed = true;
                        this.mapping_current_state.can_save = true;
                        cx.notify();
                    })),
            );
        }
        if state.option == 4 {
            let lighting_disabled = self.lighting_observation()["isBasicLightingDevice"] == true
                || state.chroma.is_empty();
            let mut specific = v_flex().ml(surface::css(30.)).w(surface::css(180.));
            if let Some(select) = &state.profile_select {
                specific = specific.child(
                    razer_widgets::synapse_select::select(select)
                        .items(
                            state
                                .profiles
                                .iter()
                                .filter_map(|profile| {
                                    Some(Choice::new(
                                        profile["guid"].as_str()?,
                                        profile["name"].as_str()?,
                                    ))
                                })
                                .collect(),
                        )
                        .disabled(disabled)
                        .w_full(),
                );
            }
            specific = specific.child(
                Checkbox::new("mouse70-profile-lighting")
                    .w(surface::css(170.))
                    .mb(surface::css(10.))
                    .label(t("SWITCH_LIGHTING_PROFILE"))
                    .checked(state.switch_lighting)
                    .disabled(disabled || lighting_disabled)
                    .on_click(cx.listener(|this, on, _, cx| {
                        this.mapping_current_state.profile.switch_lighting = *on;
                        this.mapping_current_state.changed = true;
                        this.mapping_current_state.can_save = true;
                        cx.notify();
                    })),
            );
            if state.switch_lighting
                && !lighting_disabled
                && let Some(select) = &state.chroma_select
            {
                specific = specific.child(
                    razer_widgets::synapse_select::select(select)
                        .items(
                            state
                                .chroma
                                .iter()
                                .filter_map(|profile| {
                                    Some(Choice::new(
                                        profile["id"].as_str()?,
                                        profile["name"].as_str()?,
                                    ))
                                })
                                .collect(),
                        )
                        .w_full(),
                );
            }
            body = body.child(specific);
        }
        body.into_any_element()
    }
}
