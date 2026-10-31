//! Each current MapDefault's imported localization and DEFAULTPROFILE policy.
//! A missing `dpiStages` property is a source mismatch, never filled from drafts.
use super::*;
use razer_widgets::keyboard_navigation::Button as BaseButton;
use razer_widgets::theme::{MouseCustomizeColors, MouseMappingColors};

#[derive(Deserialize)]
struct Policy {
    product_id: u32,
    labels: BTreeMap<String, String>,
    supports_boss_key: bool,
    stage_keys: Vec<String>,
    stage_values: Option<Vec<Value>>,
    configure_key: String,
    target: String,
    ai_labels: Vec<String>,
}

fn policy(product_id: u32) -> Option<&'static Policy> {
    static POLICIES: OnceLock<Vec<Policy>> = OnceLock::new();
    POLICIES
        .get_or_init(|| read_mouse_mapping_default_current_catalog())
        .iter()
        .find(|policy| policy.product_id == product_id)
}

impl MouseProductWorkspace {
    pub(super) fn current_mouse_default_body(
        &self,
        button: &Value,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(policy) = policy(self.spec.product_id) else {
            return div().into_any_element();
        };
        // qt passes activeButton.defaultValue || "" to MapDefault.keyName.
        let name = button["defaultValue"].as_str().unwrap_or("");
        let normalized = name
            .replace([' ', '/', '\t', '\r', '\n'], "_")
            .to_uppercase();
        let label = if policy.supports_boss_key && normalized == "RAZER_HYPERSHIFT" {
            name.to_owned()
        } else if let Some(key) = policy.labels.get(&normalized) {
            t(key)
        } else {
            name.to_owned()
        };
        let mut body = v_flex().w(surface::css(210.)).font_family("Roboto").child(
            div()
                .w_full()
                .border_1()
                .border_color(MouseCustomizeColors::line())
                .p(surface::css(5.))
                .mb(surface::css(20.))
                .text_size(surface::css(14.))
                .line_height(surface::css(17.))
                .text_color(MouseCustomizeColors::text())
                .text_center()
                .overflow_hidden()
                .child(label),
        );
        if button["buttonKey"] == "CycleUpSensitivityStages" {
            // The original uses the imported default profile, not the live
            // profile. Older CONFIGs omit the lowercase field it reads.
            if let Some(values) = &policy.stage_values {
                let target = policy.target.clone();
                body = body.child(
                    v_flex()
                        .h(surface::css(140.))
                        .mb(surface::css(20.))
                        .justify_between()
                        .text_size(surface::css(14.))
                        .line_height(surface::css(17.))
                        .children(policy.stage_keys.iter().zip(values).map(|(key, value)| {
                            div()
                                .text_color(MouseMappingColors::description())
                                .child(format!(
                                    "{} - {}",
                                    t(key),
                                    value.as_f64().map(|v| v.to_string()).unwrap_or_default()
                                ))
                        }))
                        .child(
                            BaseButton::new("mouse-current-default-configure-sensitivity")
                                .mt(surface::css(20.))
                                .mb(surface::css(10.))
                                .p_0()
                                .text_color(MouseCustomizeColors::text())
                                .cursor_pointer()
                                .child(div().underline().child(t(&policy.configure_key)))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.set_page(&target, window, cx);
                                })),
                        ),
                );
            }
        }
        if button["buttonKey"] == "RazerAI" {
            let mut description = v_flex()
                .text_color(MouseMappingColors::description())
                .text_size(surface::css(14.))
                .line_height(surface::css(17.));
            let has_description =
                button
                    .get("buttonDescription")
                    .is_some_and(|value| match value {
                        Value::Null => false,
                        Value::Bool(value) => *value,
                        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.),
                        Value::String(value) => !value.is_empty(),
                        _ => true,
                    });
            if has_description {
                description = description.child(
                    h_flex()
                        .mb(surface::css(15.))
                        .gap(surface::css(6.))
                        .items_start()
                        .child(
                            img("synapse/mouse-default-ai-long-press.svg")
                                .size(surface::css(18.))
                                .flex_shrink_0(),
                        )
                        .child(t(&policy.ai_labels[0])),
                );
            }
            description = description
                .child(div().mb(surface::css(15.)).child(t(&policy.ai_labels[1])))
                .child(div().child(t(&policy.ai_labels[2])));
            body = body
                .child(description)
                .child(
                    img("synapse/keyboard-mapping-ai/ai-launcher.5b68ac10.svg")
                        .w(surface::css(210.))
                        .h(surface::css(120.))
                        .my(surface::css(20.))
                        .object_fit(ObjectFit::Contain),
                )
                .children(self.current_mouse_synapse_notice());
        }
        body.into_any_element()
    }
}

// Generated catalog reader: razer-pages/mouse_mapping_default_current_data.
fn read_mouse_mapping_default_current_catalog() -> Vec<Policy> {
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
                labels: r.child(decode_3)?,
                supports_boss_key: r.child(decode_5)?,
                stage_keys: r.child(decode_6)?,
                stage_values: None,
                configure_key: r.child(decode_4)?,
                target: r.child(decode_4)?,
                ai_labels: r.child(decode_6)?,
            },
            1 => Policy {
                product_id: r.child(decode_2)?,
                labels: r.child(decode_3)?,
                supports_boss_key: r.child(decode_5)?,
                stage_keys: r.child(decode_6)?,
                stage_values: r.child(decode_7)?,
                configure_key: r.child(decode_4)?,
                target: r.child(decode_4)?,
                ai_labels: r.child(decode_6)?,
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
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_4, decode_4)?,
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
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_4)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<std::vec::Vec<serde_json::Value>>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_8)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<serde_json::Value>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_9)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_MOUSE_MAPPING_DEFAULT_CURRENT_DATA,
        0,
        "49f23c255f25a737c70d527f09c34837df9892dde21cbd68aa65b0b6919d89a7",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod mouse_mapping_default_current_data_catalog_tests {
    #[test]
    fn packaged_catalog_mouse_mapping_default_current_data_decodes() {
        let _ = super::read_mouse_mapping_default_current_catalog();
    }
}
// End generated catalog reader: razer-pages/mouse_mapping_default_current_data.
