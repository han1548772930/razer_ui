//! Current MapHyper, MapDisable, MapMedia and MapWindows lazy factories.
//! Policies follow each mounted factory's imports, including ID converter CONFIG.
use super::mapping_current::Action;
use super::*;
use gpui_kit::component::IndexPath;
use gpui_kit::component::select::{SelectEvent, SelectState};
use razer_widgets::theme::MouseMappingColors;

#[derive(Deserialize)]
struct BasicAction {
    content: String,
    payload: String,
}
#[derive(Deserialize)]
struct Policy {
    product_id: u32,
    hyper_labels: Vec<String>,
    hyper_group: String,
    disable_group: String,
    media: Vec<BasicAction>,
    media_required: Vec<String>,
    windows: Vec<BasicAction>,
    primary_restore: bool,
    suppress_hyper_secondary: bool,
}
fn policy(product_id: u32) -> Option<&'static Policy> {
    static POLICIES: OnceLock<Vec<Policy>> = OnceLock::new();
    POLICIES
        .get_or_init(|| read_mouse_mapping_basic_current_catalog())
        .iter()
        .find(|policy| policy.product_id == product_id)
}
impl MouseProductWorkspace {
    pub(super) fn apply_current_mouse_basic_search_option(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(policy) = policy(self.spec.product_id) else {
            return;
        };
        let state = &mut self.mapping_current_state;
        if !matches!(state.function.as_str(), "MULTIMEDIA" | "WINDOWS_SHORTCUT") {
            return;
        }
        state.selected = index;
        state.can_save = true;
        state.changed = true;
        state.basic_requires_synapse = state.function == "WINDOWS_SHORTCUT"
            || state
                .choices
                .get(index)
                .is_some_and(|a| policy.media_required.contains(&a.content));
        if let Some(select) = &state.select {
            select.update(cx, |select, cx| {
                select.set_selected_index(Some(IndexPath::new(index)), window, cx)
            });
        }
    }
    pub(super) fn current_mouse_restore_primary(
        &self,
        mappings: &mut Vec<Value>,
        button: &Value,
        proposed: &Value,
    ) {
        if !policy(self.spec.product_id).is_some_and(|p| p.primary_restore)
            || self.hypershift
            || button["assignment"] != "MOUSE_FUNCTION"
            || button["assignmentGroup"]["mouseAssignment"] != "Click"
            || proposed["outputType"] == "mouseGroup"
                && proposed["mouseGroup"]["mouseAssignment"] == "Click"
        {
            return;
        }
        // Preserve the source's legacy mouseInput comparison exactly. Neither
        // this helper nor rq translates it to the current inputID field.
        let clicks = mappings
            .iter()
            .filter(|m| {
                m["outputType"] == "mouseGroup"
                    && m["mouseGroup"]["mouseAssignment"] == "Click"
                    && m["mouseInput"] != "LeftClick"
            })
            .count();
        if clicks > 1 {
            return;
        }
        if let Some(index) = mappings.iter().position(|m| {
            m["mouseInput"] == "LeftClick"
                && m.get("mouseGroup").is_some_and(|g| !g.is_null())
                && m["mouseGroup"]["mouseAssignment"] != "Click"
        }) {
            mappings.remove(index);
        }
    }
    pub(super) fn initialize_current_mouse_basic(
        &mut self,
        button: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(policy) = policy(self.spec.product_id) else {
            return;
        };
        let state = &mut self.mapping_current_state;
        let actions = match state.function.as_str() {
            "MULTIMEDIA" => &policy.media,
            "WINDOWS_SHORTCUT" => &policy.windows,
            _ => return,
        };
        state.choices = actions
            .iter()
            .map(|a| Action {
                id: a.payload.clone(),
                content: a.content.clone(),
            })
            .collect();
        // assignDefaultValues tests the projected `${assignment}Group` before
        // restoring by content. A failed findIndex remains no selection.
        state.selected = if button.get("assignmentGroup").is_some()
            && !button["assignmentGroup"].is_null()
            && button["assignment"] == state.function
        {
            state
                .choices
                .iter()
                .position(|a| button["assignmentValue"] == a.content)
                .unwrap_or(usize::MAX)
        } else {
            0
        };
        state.basic_requires_synapse = state.function == "WINDOWS_SHORTCUT"
            || state
                .choices
                .get(state.selected)
                .is_some_and(|a| policy.media_required.contains(&a.content));
        let selected = (state.selected != usize::MAX).then_some(IndexPath::new(state.selected));
        let select = cx.new(|cx| SelectState::new(state.choice_items(), selected, window, cx));
        state.subscriptions.push(
            cx.subscribe_in(&select, window, move |this, _, event, _, cx| {
                let SelectEvent::Confirm(Some(id)) = event else {
                    return;
                };
                let state = &mut this.mapping_current_state;
                let Some(selected) = state.choices.iter().position(|a| &a.id == id) else {
                    return;
                };
                state.selected = selected;
                state.changed = true;
                state.can_save = true;
                state.basic_requires_synapse = state.function == "WINDOWS_SHORTCUT"
                    || policy
                        .media_required
                        .contains(&state.choices[selected].content);
                cx.notify();
            }),
        );
        state.select = Some(select);
        self.current_mouse_basic_mount_save_guard(button);
    }

    pub(super) fn current_mouse_basic_mount_save_guard(&mut self, button: &Value) {
        let state = &mut self.mapping_current_state;
        // MapMedia.assignDefaultValues calls enableSave(false,false,true) only
        // when the original assignment has a group and its selected content
        // requires Synapse. It can override the parent's category canSave.
        if state.function == "MULTIMEDIA"
            && button.get("assignmentGroup").is_some()
            && !button["assignmentGroup"].is_null()
            && state.basic_requires_synapse
        {
            state.can_save = false;
            state.changed = false;
        }
    }

    pub(super) fn current_mouse_basic_body(&self) -> Option<AnyElement> {
        let policy = policy(self.spec.product_id)?;
        let state = &self.mapping_current_state;
        match state.function.as_str() {
            "RAZER_HYPERSHIFT" => Some(
                v_flex()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .text_color(MouseMappingColors::hypershift_description())
                    .mb(surface::css(20.))
                    .child(t(&policy.hyper_labels[0]))
                    .child(div().h(surface::css(17.)))
                    .child(t(&policy.hyper_labels[1]))
                    .into_any_element(),
            ),
            "DISABLE" => Some(div().into_any_element()),
            "MULTIMEDIA" | "WINDOWS_SHORTCUT" => Some(
                div()
                    .w_full()
                    .mb(surface::css(if state.function == "WINDOWS_SHORTCUT" {
                        20.
                    } else {
                        10.
                    }))
                    .child(
                        razer_widgets::synapse_select::select(state.select.as_ref()?)
                            .items(state.choice_items())
                            .accessibility_label(t(&state.function))
                            .w_full(),
                    )
                    .into_any_element(),
            ),
            _ => None,
        }
    }

    /// Exact child rq/getMappingData outputs. The enclosing editor handles
    /// local flat profile packing; this result is never a device write receipt.
    pub(super) fn current_mouse_basic_payloads(&self, button: &Value) -> Option<Vec<Value>> {
        let policy = policy(self.spec.product_id)?;
        let state = &self.mapping_current_state;
        let base = |group: &str, hyper: bool| json!({"inputID":button["inputID"],"inputType":button["inputType"],"isHyperShift":hyper,"outputType":group});
        match state.function.as_str() {
            "RAZER_HYPERSHIFT" => {
                let mut payloads = vec![base(&policy.hyper_group, self.hypershift)];
                if !self.hypershift
                    && !(policy.suppress_hyper_secondary && button["buttonKey"] == "Hypershift")
                {
                    payloads.push(base(&policy.hyper_group, true));
                }
                Some(payloads)
            }
            "DISABLE" => Some(vec![base(&policy.disable_group, self.hypershift)]),
            "MULTIMEDIA" => {
                let action = state.choices.get(state.selected)?;
                let mut payload = base("multimediaGroup", self.hypershift);
                payload["multimediaGroup"] = json!({"multimediaAssignment":action.id});
                if matches!(action.content.as_str(), "VOLUME_DOWN" | "VOLUME_UP") {
                    payload["multimediaGroup"]["useTurbo"] = json!(true);
                }
                Some(vec![payload])
            }
            "WINDOWS_SHORTCUT" => {
                let action = state.choices.get(state.selected)?;
                let mut payload = base("win8ShortcutsGroup", self.hypershift);
                payload["win8ShortcutsGroup"] = json!({"windowsShortcutAssignment":action.id});
                Some(vec![payload])
            }
            _ => None,
        }
    }
}

// Generated catalog reader: razer-pages/mouse_mapping_basic_current_data.
fn read_mouse_mapping_basic_current_catalog() -> Vec<Policy> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Policy>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Policy> {
        let value = match r.tag()? {
            0 => Policy {
                product_id: r.child(decode_2)?,
                hyper_labels: r.child(decode_3)?,
                hyper_group: r.child(decode_4)?,
                disable_group: r.child(decode_4)?,
                media: r.child(decode_5)?,
                media_required: r.child(decode_3)?,
                windows: r.child(decode_5)?,
                primary_restore: r.child(decode_7)?,
                suppress_hyper_secondary: r.child(decode_7)?,
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
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_4)?,
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
    fn decode_5(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<BasicAction>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_6)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<BasicAction> {
        let value = match r.tag()? {
            0 => BasicAction {
                content: r.child(decode_4)?,
                payload: r.child(decode_4)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_MOUSE_MAPPING_BASIC_CURRENT_DATA,
        0,
        "9ba875b7720f11bab70d545bf7745b1801490b863e3732272f10783ad1910f01",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
#[test]
fn packaged_catalog_mouse_mapping_basic_current_data_decodes() {
    let _ = read_mouse_mapping_basic_current_catalog();
}
// End generated catalog reader: razer-pages/mouse_mapping_basic_current_data.
