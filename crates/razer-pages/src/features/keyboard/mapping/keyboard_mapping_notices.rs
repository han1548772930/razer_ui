//! Current renderFunction prerequisite notices and 3466.Xq macro predicate.
use super::*;
#[derive(Deserialize)]
struct Source {
    product_id: u32,
    require_categories: Vec<String>,
    always_inputs: Vec<String>,
    software_outputs: Vec<String>,
    macro_event_types: Vec<Value>,
    macro_mouse_buttons: Vec<Value>,
    synapse_margin_bottom: f32,
}
fn source(pid: u32) -> &'static Source {
    static DATA: OnceLock<Vec<Source>> = OnceLock::new();
    DATA.get_or_init(|| read_keyboard_mapping_notices_catalog())
        .iter()
        .find(|row| row.product_id == pid)
        .expect("audited current keyboard prerequisite caller")
}
impl KeyboardProductWorkspace {
    pub(super) fn mapping_macro_requires_synapse(&self, guid: &Value) -> bool {
        if self.spec.config["DeviceInfo"]["OBMSpecs"]["supportedMappings"]["macroGroup"] != true {
            return true;
        }
        let Some(row) = self.mapping_macro_observation().and_then(|observation| {
            observation
                .macro_list
                .iter()
                .find(|row| row["guid"] == *guid)
        }) else {
            return false;
        };
        let engine = &row["appEngine"];
        if engine.is_null() {
            return false;
        }
        if matches!(
            engine["mouseMoveType"].as_str(),
            Some("absolute" | "foreground")
        ) {
            return true;
        }
        let metadata = source(self.spec.product_id);
        row["macroEvents"].as_array().is_some_and(|events| {
            events.iter().any(|event| {
                metadata.macro_event_types.contains(&event["Type"])
                    || metadata
                        .macro_mouse_buttons
                        .contains(&event["MouseEvent"]["MouseButton"])
            })
        })
    }
    fn mapping_requires_synapse(&self, slot: usize) -> bool {
        let Some(edit) = &self.analog_gamepad.mapping.edit else {
            return false;
        };
        let current = &edit.slots[0].category;
        if current == "AUDIO_FUNCTION"
            || current == "CONTROLLER"
                && !self.spec.config["DeviceInfo"]["controllerAssignmentsWithoutSynapse"].is_null()
        {
            return false;
        }
        let metadata = source(self.spec.product_id);
        let staged = &edit.slots[slot];
        let requires = if staged.category == "KEYBOARD_FUNCTION" {
            slot == 1
        } else if let Some(options) = super::source(self.spec.product_id)
            .options
            .get(&staged.category)
        {
            let id = &staged.staged[&options.group][&options.field];
            let choice = options.choices.iter().find(|choice| choice["id"] == *id);
            let excluded = self.spec.config["SYNAPSE_NOT_REQUIRED_FUNCTIONS"].as_array();
            let hardware = excluded.is_some_and(|rows| {
                rows.iter()
                    .any(|row| row == id || choice.is_some_and(|choice| row == &choice["content"]))
            });
            !hardware || !staged.staged[&options.group]["turboMode"].is_null()
        } else if staged.category == "MACRO" {
            if let Some(row) = self.mapping_macro_observation().and_then(|observation| {
                observation
                    .macro_list
                    .iter()
                    .find(|row| row["guid"] == staged.staged["macroGroup"]["guid"])
            }) {
                let events = row["macroEvents"].as_array();
                let engine = &row["appEngine"];
                let native_supported =
                    self.spec.config["DeviceInfo"]["OBMSpecs"]["supportedMappings"]["macroGroup"]
                        .as_bool()
                        == Some(true);
                let software_events = !engine.is_null()
                    && (matches!(
                        engine["mouseMoveType"].as_str(),
                        Some("absolute" | "foreground")
                    ) || events.is_some_and(|events| {
                        events.iter().any(|event| {
                            metadata.macro_event_types.contains(&event["Type"])
                                || metadata
                                    .macro_mouse_buttons
                                    .contains(&event["MouseEvent"]["MouseButton"])
                        })
                    }));
                let playback = self.mapping_macro_choices(row).iter().find(|choice| {
                    choice["id"] == staged.staged["macroGroup"]["macroPlaybackOption"]
                });
                !native_supported
                    || software_events
                    || edit.button["isRequireSynapse"] == true
                    || playback.is_some_and(|choice| {
                        metadata
                            .software_outputs
                            .iter()
                            .any(|content| choice["content"] == content.as_str())
                    })
            } else {
                edit.button["isRequireSynapse"].as_bool().unwrap_or(true)
            }
        } else {
            metadata.require_categories.contains(&staged.category)
        };
        current == "APP_SPECIFIC"
            || (current != "DEFAULT" || edit.button["buttonKey"] == "RazerAI")
                && (self.spec.config["DeviceInfo"]["isOBMDevice"] != true
                    || requires
                    || metadata
                        .always_inputs
                        .iter()
                        .any(|input| edit.button["buttonKey"] == input.as_str()))
    }
    pub(super) fn mapping_chroma_install_element(
        &self,
        slot: usize,
        cx: &Context<Self>,
    ) -> AnyElement {
        v_flex()
            .child(prerequisite_notice(
                slot,
                "chroma.png",
                "REQUIRE_CHROMA_APP",
                "REQUIRE_CHROMA_APP_TOOLTIP",
                0.,
                0.,
            ))
            .child(
                v_flex()
                    .p(surface::css(10.))
                    .pb(surface::css(15.))
                    .bg(rgb(0x222222))
                    .rounded(surface::css(4.))
                    .font_family("Roboto")
                    .text_size(surface::css(14.))
                    .text_color(rgb(0xcccccc))
                    .child(t("REQUIRE_CHROMA_STUDIO_TOOLTIP"))
                    .child(
                        Button::new(SharedString::from(format!(
                            "keyboard-map-install-chroma-{slot}"
                        )))
                        .label(t("INSTALL_CHROMA_STUDIO"))
                        .primary()
                        .mt(surface::css(10.))
                        .mx(surface::css(7.))
                        .h_auto()
                        .w_full()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.request_lighting_command(
                                KeyboardLightingCommand::InstallChroma,
                                cx,
                            )
                        })),
                    ),
            )
            .into_any_element()
    }
    pub(super) fn mapping_prerequisite_element(
        &self,
        slot: usize,
        cx: &Context<Self>,
    ) -> AnyElement {
        let edit = self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .expect("mounted mapping prerequisite");
        let chroma = edit.slots[0].category == "SWITCH_LIGHTING"
            && self
                .lighting_observation
                .as_ref()
                .is_some_and(|observation| observation.chroma_installed_module);
        let mut notices = v_flex().mx(surface::css(20.));
        for (show, icon, label, tooltip) in [
            (
                chroma,
                "chroma.png",
                "REQUIRE_CHROMA_APP",
                "REQUIRE_CHROMA_APP_TOOLTIP",
            ),
            (
                self.mapping_requires_synapse(slot),
                "synapse.svg",
                "REQUIRES_RAZER_SYNAPSE",
                "REQUIRES_RAZER_SYNAPSE_TOOLTIP",
            ),
        ] {
            if !show {
                continue;
            }
            notices = notices.child(prerequisite_notice(
                slot,
                icon,
                label,
                tooltip,
                10.,
                if chroma {
                    10.
                } else {
                    source(self.spec.product_id).synapse_margin_bottom
                },
            ));
        }
        let _ = cx;
        notices.into_any_element()
    }
}

fn prerequisite_notice(
    slot: usize,
    icon: &str,
    label: &str,
    tooltip: &'static str,
    top: f32,
    bottom: f32,
) -> AnyElement {
    let text = t(label);
    let text = if label == "REQUIRES_RAZER_SYNAPSE" {
        text.strip_suffix("Razer Synapse")
            .map(|prefix| format!("{prefix}\nRazer Synapse"))
            .unwrap_or(text)
    } else {
        text
    };
    h_flex()
        .w(surface::css(210.))
        .h(surface::css(44.))
        .mt(surface::css(top))
        .mb(surface::css(bottom))
        .border_1()
        .border_color(rgb(0x707070))
        .rounded(surface::css(3.))
        .items_center()
        .font_family("Roboto")
        .child(
            img(SharedString::from(format!(
                "synapse/keyboard-mapping-notices/{icon}"
            )))
            .size(surface::css(24.))
            .ml(surface::css(10.))
            .flex_shrink_0(),
        )
        .child(
            div()
                .w(surface::css(142.))
                .mx(surface::css(5.))
                .flex_shrink_0()
                .text_size(surface::css(14.))
                .line_height(surface::css(14.))
                .text_color(rgb(0x707070))
                .child(text),
        )
        .child(
            BaseButton::new(SharedString::from(format!(
                "keyboard-map-prerequisite-help-{slot}-{icon}"
            )))
            .size(surface::css(14.))
            .mr(surface::css(10.))
            .rounded_full()
            .bg(rgb(0x4a4a4a))
            .hover(|help| help.bg(rgba(0xffffff4d)))
            .child(img("synapse/keyboard-mapping-notices/help.svg").size_full())
            .tooltip(move |window, cx| {
                gpui_kit::component::tooltip::Tooltip::new(t(tooltip)).build(window, cx)
            }),
        )
        .into_any_element()
}

// Generated catalog reader: razer-pages/keyboard_mapping_notices_data.
fn read_keyboard_mapping_notices_catalog() -> Vec<Source> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Source>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Source> {
        let value = match r.tag()? {
            0 => Source {
                product_id: r.child(decode_2)?,
                require_categories: r.child(decode_3)?,
                always_inputs: r.child(decode_3)?,
                software_outputs: r.child(decode_3)?,
                macro_event_types: r.child(decode_5)?,
                macro_mouse_buttons: r.child(decode_5)?,
                synapse_margin_bottom: r.child(decode_7)?,
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
    ) -> anyhow::Result<std::vec::Vec<serde_json::Value>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_6)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f32> {
        let value = match r.tag()? {
            0 => r.f32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_MAPPING_NOTICES_DATA,
        0,
        "0b88cac3133eb55492759da6443c58619692c87ff9a7b6d06b9d539d4bc497eb",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod keyboard_mapping_notices_data_catalog_tests {
    #[test]
    fn packaged_catalog_keyboard_mapping_notices_data_decodes() {
        let _ = super::read_keyboard_mapping_notices_catalog();
    }
}
// End generated catalog reader: razer-pages/keyboard_mapping_notices_data.
